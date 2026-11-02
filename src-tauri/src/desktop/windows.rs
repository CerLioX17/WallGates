//! Windows: render behind the desktop icons using the WorkerW technique.
//!
//! Sending the undocumented message 0x052C to `Progman` makes Explorer create a
//! `WorkerW` window that sits between the wallpaper and the icon layer
//! (`SHELLDLL_DefView`). Parenting our window to it puts it behind the icons.
//!
//! Explorer's window tree differs by version:
//! * Windows 10 / 11 before 24H2: the target `WorkerW` is a top-level window,
//!   the sibling right after the window that hosts `SHELLDLL_DefView`.
//! * Windows 11 24H2+: `SHELLDLL_DefView` and the `WorkerW` are both children of
//!   `Progman`; we parent to that child `WorkerW`. Confirmed working on 26H2
//!   (Progman children: ["SHELLDLL_DefView", "WorkerW"], 4K single monitor).
//! If neither is found we parent to `Progman` and insert ourselves below
//! `SHELLDLL_DefView` in the z-order.

use super::{PauseRules, Rect};
use std::ffi::c_void;
use tauri::WebviewWindow;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, ScreenToClient, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::WindowsAndMessaging::*;

const SPAWN_WORKERW: u32 = 0x052C;

unsafe fn find_child(parent: Option<HWND>, after: Option<HWND>, class: PCWSTR) -> Option<HWND> {
    FindWindowExW(parent, after, class, PCWSTR::null())
        .ok()
        .filter(|h| !h.0.is_null())
}

unsafe extern "system" fn enum_proc(top: HWND, lparam: LPARAM) -> BOOL {
    // Pre-24H2 layout: the window that contains SHELLDLL_DefView is followed by our WorkerW.
    if find_child(Some(top), None, w!("SHELLDLL_DefView")).is_some() {
        if let Some(worker) = find_child(None, Some(top), w!("WorkerW")) {
            *(lparam.0 as *mut HWND) = worker;
            return BOOL(0); // stop enumerating
        }
    }
    BOOL(1)
}

/// Returns (parent, insert_after, strategy name) for the wallpaper window.
unsafe fn find_target() -> Result<(HWND, Option<HWND>, &'static str), String> {
    let progman = FindWindowW(w!("Progman"), PCWSTR::null())
        .map_err(|e| format!("Progman not found: {e}"))?;

    let mut result = 0usize;
    for (wp, lp) in [(0xD, 0x1), (0, 0)] {
        SendMessageTimeoutW(
            progman,
            SPAWN_WORKERW,
            WPARAM(wp),
            LPARAM(lp),
            SMTO_NORMAL,
            1000,
            Some(&mut result),
        );
    }

    // Windows 10 / 11 (pre-24H2).
    let mut worker = HWND(std::ptr::null_mut());
    let _ = EnumWindows(Some(enum_proc), LPARAM(&mut worker as *mut HWND as isize));
    if !worker.0.is_null() {
        return Ok((worker, None, "top-level WorkerW (Windows 10 / 11 pre-24H2)"));
    }

    // Windows 11 24H2+.
    if let Some(worker) = find_child(Some(progman), None, w!("WorkerW")) {
        return Ok((worker, None, "WorkerW child of Progman (Windows 11 24H2+)"));
    }

    // Last resort: inside Progman, below the icon layer.
    let defview = find_child(Some(progman), None, w!("SHELLDLL_DefView"));
    Ok((progman, defview, "Progman below SHELLDLL_DefView (fallback)"))
}

unsafe fn child_classes(parent: HWND) -> Vec<String> {
    let mut out = Vec::new();
    let mut after: Option<HWND> = None;
    while let Some(child) = find_child(Some(parent), after, PCWSTR::null()) {
        out.push(class_name(child));
        after = Some(child);
        if out.len() > 32 {
            break;
        }
    }
    out
}

/// Explorer's desktop window tree, for the diagnostics log.
unsafe fn describe_desktop() -> String {
    let Ok(progman) = FindWindowW(w!("Progman"), PCWSTR::null()) else {
        return "Progman: not found".into();
    };
    let mut lines = vec![format!("Progman children: {:?}", child_classes(progman))];
    let mut after: Option<HWND> = None;
    let mut n = 0;
    while let Some(worker) = find_child(None, after, w!("WorkerW")) {
        lines.push(format!("top-level WorkerW #{n} children: {:?}", child_classes(worker)));
        after = Some(worker);
        n += 1;
        if n > 8 {
            break;
        }
    }
    lines.join("; ")
}

pub fn attach(window: &WebviewWindow, rect: Rect) -> Result<String, String> {
    let raw = window.hwnd().map_err(|e| e.to_string())?;
    let hwnd = HWND(raw.0 as *mut c_void);

    unsafe {
        let (parent, insert_after, strategy) = find_target()?;

        // Turn the popup into a non-activating child window.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let style = (style & !(WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0))
            | WS_CHILD.0
            | WS_VISIBLE.0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let ex = (ex & !WS_EX_APPWINDOW.0) | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex as isize);

        SetParent(hwnd, Some(parent)).map_err(|e| format!("SetParent failed: {e}"))?;

        // Child coordinates are relative to the parent's client area, which
        // starts at the virtual-screen origin (can be negative with multiple monitors).
        let mut origin = POINT { x: rect.x, y: rect.y };
        let _ = ScreenToClient(parent, &mut origin);

        let mut flags = SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED;
        if insert_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        SetWindowPos(
            hwnd,
            insert_after,
            origin.x,
            origin.y,
            rect.width as i32,
            rect.height as i32,
            flags,
        )
        .map_err(|e| format!("SetWindowPos failed: {e}"))?;

        Ok(format!(
            "strategy: {strategy}; parent class: {}; position {},{} size {}x{}; {}",
            class_name(parent),
            origin.x,
            origin.y,
            rect.width,
            rect.height,
            describe_desktop()
        ))
    }
}

unsafe fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let len = GetClassNameW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len.max(0) as usize])
}

pub fn should_auto_pause(rules: PauseRules) -> bool {
    unsafe {
        if rules.battery {
            let mut status = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut status).is_ok() && status.ACLineStatus == 0 {
                return true;
            }
        }
        if !rules.fullscreen && !rules.maximized {
            return false;
        }

        let fg = GetForegroundWindow();
        if fg.0.is_null() || IsIconic(fg).as_bool() {
            return false;
        }
        // Desktop, taskbar and our own windows never pause the wallpaper.
        let class = class_name(fg);
        if matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == std::process::id() {
            return false;
        }

        if rules.maximized && IsZoomed(fg).as_bool() {
            return true;
        }
        if rules.fullscreen {
            let mut win = RECT::default();
            if GetWindowRect(fg, &mut win).is_err() {
                return false;
            }
            let monitor = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let m = info.rcMonitor;
                return win.left <= m.left
                    && win.top <= m.top
                    && win.right >= m.right
                    && win.bottom >= m.bottom;
            }
        }
        false
    }
}

/// Re-apply the user's static Windows wallpaper so the last rendered frame of
/// our window does not stay stuck on the desktop after we quit.
pub fn restore() {
    unsafe {
        let mut buf = [0u16; 1024];
        if SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            let _ = SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                Some(buf.as_mut_ptr() as *mut c_void),
                SPIF_UPDATEINIFILE,
            );
        }
    }
}
