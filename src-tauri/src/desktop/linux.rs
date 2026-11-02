//! Linux (X11): a window with the `_NET_WM_WINDOW_TYPE_DESKTOP` hint sits behind
//! everything and is skipped by taskbars. Wayland compositors generally ignore
//! this hint; proper Wayland support needs wlr-layer-shell (see ROADMAP).

use super::{PauseRules, Rect};
use gtk::prelude::*;
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

pub fn attach(window: &WebviewWindow, rect: Rect) -> Result<String, String> {
    window
        .set_position(PhysicalPosition::new(rect.x, rect.y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(PhysicalSize::new(rect.width, rect.height))
        .map_err(|e| e.to_string())?;

    // GTK must be touched from the main thread, and the hint must be set before mapping.
    let w = window.clone();
    window
        .run_on_main_thread(move || {
            if let Ok(gtk_window) = w.gtk_window() {
                gtk_window.set_type_hint(gtk::gdk::WindowTypeHint::Desktop);
                gtk_window.set_keep_below(true);
                gtk_window.set_skip_taskbar_hint(true);
                gtk_window.set_skip_pager_hint(true);
                gtk_window.set_accept_focus(false);
                gtk_window.stick();
            }
            let _ = w.show();
        })
        .map_err(|e| e.to_string())?;
    Ok("strategy: X11 _NET_WM_WINDOW_TYPE_DESKTOP".into())
}

pub fn should_auto_pause(_rules: PauseRules) -> bool {
    // TODO: detect fullscreen windows via _NET_ACTIVE_WINDOW / _NET_WM_STATE_FULLSCREEN.
    false
}

pub fn restore() {}
