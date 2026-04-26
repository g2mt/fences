useanyhow::Result;
use std::collections::BTreeMap;
use std::sync::Mutex;
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod app;
mod window;
mod desktop_cover;

use crate::app::APP;
use crate::desktop_cover::DesktopCover;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg != WM_NCCREATE {
        let app = APP.get().unwrap().try_lock().expect("can only lock after initialization");
        if let Some(window) = app.windows.get(&window::WinHandle(hwnd)) {
            return window.lock().unwrap().wndproc(msg, wparam, lparam);
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn main() -> Result<()> {
    APP.get_or_init(|| Mutex::new(app::App { windows: BTreeMap::new() }));
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());
        let class_name = w!("BottomWindowClass");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name,
        };
        if RegisterClassW(&wc) == 0 {
            return Err(anyhow!("RegisterClassW failed"));
        }
        let desktop_cover = DesktopCover::new(h_instance, class_name)?;
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
