use std::sync::Mutex;

use anyhow::Result;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod app;
mod desktop_cover;
mod window;

use crate::app::APP;
use crate::desktop_cover::DesktopCover;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // try to unlock to prevent reentrancy from wndproc
    if let Ok(app) = APP.get().unwrap().try_lock() {
        if let Some(window) = app.window(window::WinHandle(hwnd)) {
            return window.lock().unwrap().wndproc(msg, wparam, lparam);
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn main() -> Result<()> {
    APP.get_or_init(|| Mutex::new(app::App::new()));
    unsafe {
        let desktop_cover = DesktopCover::new(Some(wndproc))?;
        {
            let mut app = APP.get().unwrap().lock().unwrap();
            app.add_window(desktop_cover);
        }
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
