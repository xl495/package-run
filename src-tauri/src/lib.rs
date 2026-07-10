pub mod ports;
pub mod projects;
pub mod runner;
pub mod settings;
pub mod update;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

/// Bring the main management window to front (Herd "Open …" behavior).
fn show_main_window<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn toggle_main_window<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show_main_window(app);
    }
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Kept for API compatibility; main window is a normal window now (no popover pin).
#[tauri::command]
fn set_window_pinned(_pinned: bool) {}

#[tauri::command]
fn set_running_badge(app: tauri::AppHandle, count: usize) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tooltip = if count == 0 {
            "Package Run".to_string()
        } else {
            format!("Package Run — {count} running")
        };
        tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;

        #[cfg(any(target_os = "macos", target_os = "linux"))]
        tray.set_title((count > 0).then(|| count.to_string()))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn build_tray_menu<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    let open = MenuItem::with_id(app, "open", "Open Package Run", true, None::<&str>)?;
    let add = MenuItem::with_id(app, "add_project", "Add Project…", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let updates = MenuItem::with_id(app, "updates", "Check for Updates…", true, None::<&str>)?;
    let releases = MenuItem::with_id(app, "releases", "Open Releases Page", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Package Run", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    Menu::with_items(
        app,
        &[
            &open,
            &sep1,
            &add,
            &settings,
            &sep2,
            &updates,
            &releases,
            &sep3,
            &quit,
        ],
    )
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
            update::check_app_update,
            set_running_badge,
            set_window_pinned,
            quit_app,
        ])
        .setup(|app| {
            // Stay in the menu bar / tray; no permanent Dock icon on macOS.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let menu = build_tray_menu(app.handle())?;
            let icon = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))?;

            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("Package Run")
                .menu(&menu)
                // Herd-like: left click opens the native menu (right click does too).
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "open" => show_main_window(app),
                        "add_project" => {
                            show_main_window(app);
                            let _ = app.emit("tray-add-project", ());
                        }
                        "settings" => {
                            show_main_window(app);
                            let _ = app.emit("open-settings", ());
                        }
                        "updates" => {
                            show_main_window(app);
                            let _ = app.emit("check-updates", ());
                        }
                        "releases" => {
                            let _ = app.opener().open_url(update::RELEASES_PAGE, None::<&str>);
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    // Double-click left toggles the main window for power users.
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // Global shortcut: show / focus the main window.
            {
                use tauri_plugin_global_shortcut::ShortcutState;
                app.handle().plugin(
                    tauri_plugin_global_shortcut::Builder::new()
                        .with_handler(|app, _shortcut, event| {
                            if event.state == ShortcutState::Pressed {
                                show_main_window(app);
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
            // Close → hide to tray (app keeps running).
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
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
