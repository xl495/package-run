pub mod ports;
pub mod projects;
pub mod runner;
pub mod settings;

#[cfg(target_os = "macos")]
use std::sync::atomic::Ordering;
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
#[cfg(target_os = "macos")]
use tauri_plugin_positioner::{Position, WindowExt};

fn toggle_popover(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        // Only macOS anchors the window under the menu bar icon; on
        // Windows/Linux it's a regular window that keeps its position.
        #[cfg(target_os = "macos")]
        let _ = window.move_window(Position::TrayBottomCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn set_window_pinned(state: tauri::State<'_, projects::UiState>, pinned: bool) {
    state
        .window_pinned
        .store(pinned, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
fn set_running_badge(app: tauri::AppHandle, count: usize) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tooltip = if count == 0 {
            "Package Run".to_string()
        } else {
            format!("Package Run - {count} running")
        };
        tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        tray.set_title((count > 0).then(|| count.to_string()))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(runner::RunnerState::default())
        .manage(projects::UiState::default())
        .invoke_handler(tauri::generate_handler![
            projects::list_projects,
            projects::pick_and_add_project,
            projects::remove_project,
            projects::open_in_editor,
            projects::open_in_terminal,
            projects::reveal_in_finder,
            runner::start_script,
            runner::stop_script,
            runner::running_tasks,
            runner::get_log_file,
            projects::toggle_pin,
            projects::set_project_order,
            projects::set_script_order,
            projects::set_script_autostart,
            projects::set_package_manager,
            projects::set_launch_config,
            projects::installed_package_managers,
            ports::port_info,
            ports::kill_port,
            settings::get_shortcut,
            settings::set_shortcut,
            set_running_badge,
            set_window_pinned,
            quit_app,
        ])
        .setup(|app| {
            // Menu bar app: no Dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                // Template rendering is a macOS concept; on Windows the raw
                // icon is used as-is.
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("Package Run")
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popover(tray.app_handle());
                    }
                })
                .build(app)?;

            // Global shortcut plugin: no default binding; the user records
            // their own in settings and we register it dynamically.
            {
                use tauri_plugin_global_shortcut::ShortcutState;
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(|app, _shortcut, event| {
                            if event.state == ShortcutState::Pressed {
                                toggle_popover(app);
                            }
                        })
                        .build(),
                )?;
                if let Some(sc) = settings::load(app.handle()).shortcut {
                    use tauri_plugin_global_shortcut::GlobalShortcutExt;
                    if let Err(e) = app.global_shortcut().register(sc.as_str()) {
                        eprintln!("global shortcut registration failed: {e}");
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                // Hide instead of closing so the app keeps living in the tray.
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Click outside the popover -> hide it (Herd-like behavior),
                // unless a native dialog is currently open. Popover-style
                // auto-hide only applies to the macOS menu bar mode.
                #[cfg(target_os = "macos")]
                WindowEvent::Focused(false) => {
                    let ui = window.app_handle().state::<projects::UiState>();
                    if !ui.dialog_open.load(Ordering::SeqCst)
                        && !ui.window_pinned.load(Ordering::SeqCst)
                    {
                        let _ = window.hide();
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                runner::shutdown_all(&app.state::<runner::RunnerState>());
            }
        });
}
