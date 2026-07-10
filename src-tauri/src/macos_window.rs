//! Align macOS traffic lights with our custom overlay titlebar.
//!
//! Tauri's `trafficLightPosition.y` only grows the system titlebar container;
//! it does **not** vertically center the buttons. We reposition the buttons
//! inside their real superview so they share the midline with "Package Run".

use tauri::{Manager, Runtime, WebviewWindow};

/// Logical height of the in-app `.titlebar` (must match CSS).
pub const TITLEBAR_HEIGHT: f64 = 52.0;
/// Left inset for the close button.
pub const TRAFFIC_LIGHT_X: f64 = 16.0;

/// Vertically center traffic lights inside a titlebar of [`TITLEBAR_HEIGHT`].
pub fn align_traffic_lights<R: Runtime>(window: &WebviewWindow<R>) {
    use objc2::msg_send;
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use objc2_foundation::NSRect;

    unsafe {
        let Ok(ns_ptr) = window.ns_window() else {
            return;
        };
        let ns_window: &NSWindow = &*ns_ptr.cast();

        let Some(close) = ns_window.standardWindowButton(NSWindowButton::CloseButton) else {
            return;
        };
        let Some(miniaturize) =
            ns_window.standardWindowButton(NSWindowButton::MiniaturizeButton)
        else {
            return;
        };
        let Some(zoom) = ns_window.standardWindowButton(NSWindowButton::ZoomButton) else {
            return;
        };

        // Hierarchy: frame view → titlebar container → button container → buttons
        let Some(btn_super) = close.superview() else {
            return;
        };
        let Some(title_bar) = btn_super.superview() else {
            return;
        };
        let Some(frame_view) = title_bar.superview() else {
            return;
        };

        let close_frame = close.frame();
        let mini_frame = miniaturize.frame();
        let space = mini_frame.origin.x - close_frame.origin.x;
        let btn_h = close_frame.size.height;

        // 1) Titlebar container: full width, fixed height, pinned to top of frame view.
        //    Cocoa origins are bottom-left of the superview.
        let fv: NSRect = msg_send![&*frame_view, frame];
        let mut tb = title_bar.frame();
        tb.origin.x = 0.0;
        tb.origin.y = fv.size.height - TITLEBAR_HEIGHT;
        tb.size.width = fv.size.width;
        tb.size.height = TITLEBAR_HEIGHT;
        let _: () = msg_send![&*title_bar, setFrame: tb];

        // 2) Button container fills the titlebar (buttons' frames are relative to this).
        let mut bs = btn_super.frame();
        bs.origin.x = 0.0;
        bs.origin.y = 0.0;
        bs.size.width = TITLEBAR_HEIGHT.max(tb.size.width); // keep wide enough
        // Use full titlebar size so vertical centering is against 52px.
        bs.size.width = tb.size.width;
        bs.size.height = TITLEBAR_HEIGHT;
        let _: () = msg_send![&*btn_super, setFrame: bs];

        // 3) Center each traffic light vertically in the 52px button container.
        let btn_y = ((TITLEBAR_HEIGHT - btn_h) / 2.0).max(0.0);
        for (i, btn) in [&close, &miniaturize, &zoom].into_iter().enumerate() {
            let mut rect = btn.frame();
            rect.origin.x = TRAFFIC_LIGHT_X + (i as f64 * space);
            rect.origin.y = btn_y;
            let _: () = msg_send![&**btn, setFrame: rect];
        }
    }
}

/// Call after the main window exists; re-apply after first layout / theme pass.
pub fn setup_main_window<R: Runtime>(app: &tauri::AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    align_traffic_lights(&window);

    // Layout often settles a few frames later (scale factor, theme, first paint).
    for delay_ms in [16_u64, 80, 200, 500] {
        let win = window.clone();
        let handle = win.app_handle().clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            let _ = handle.run_on_main_thread(move || {
                align_traffic_lights(&win);
            });
        });
    }
}
