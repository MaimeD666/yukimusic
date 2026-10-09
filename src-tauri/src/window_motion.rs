//! Prevent the intermediate restore/maximize animation during a player mode change.
//! Geometry still restores normally; this affects only this window's DWM transition.
use std::ffi::c_void;

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        window: *mut c_void,
        attribute: u32,
        value: *const c_void,
        size: u32,
    ) -> i32;
}

pub struct QuietTransition {
    window: *mut c_void,
    applied: bool,
}

impl QuietTransition {
    pub fn new(window: &tauri::WebviewWindow) -> tauri::Result<Self> {
        let window = window.hwnd()?.0;
        let disabled: i32 = 1;
        // DWMWA_TRANSITIONS_FORCEDISABLED, with a Win32 BOOL payload.
        // https://learn.microsoft.com/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute
        let applied =
            unsafe { DwmSetWindowAttribute(window, 3, (&disabled as *const i32).cast(), 4) >= 0 };
        Ok(Self { window, applied })
    }
}

impl Drop for QuietTransition {
    fn drop(&mut self) {
        if self.applied {
            let disabled: i32 = 0;
            // Restore standard window animations after the mode transition, also on error.
            unsafe {
                DwmSetWindowAttribute(self.window, 3, (&disabled as *const i32).cast(), 4);
            }
        }
    }
}
