use tracing::info;
use windows_sys::core::{w, BOOL};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct DesktopMirror {
    hdc: HDC,
    mem_bmp: HBITMAP,
    width: i32,
    height: i32,
}
unsafe impl Send for DesktopMirror {}
unsafe impl Sync for DesktopMirror {}

unsafe fn get_desktop_window() -> HWND {
    let mut desktop_hwnd = FindWindowW(w!("Progman"), std::ptr::null());

    let mut workerw: HWND = std::ptr::null_mut();
    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: isize) -> BOOL {
        let p = lparam as *mut HWND;
        let defview = FindWindowExW(
            hwnd,
            std::ptr::null_mut(),
            w!("SHELLDLL_DefView"),
            std::ptr::null(),
        );
        if defview != std::ptr::null_mut() {
            *p = hwnd;
            return 0;
        }
        1
    }

    EnumWindows(Some(enum_windows_proc), &mut workerw as *mut _ as isize);

    if workerw != std::ptr::null_mut() {
        desktop_hwnd = workerw;
    }

    desktop_hwnd
}

impl DesktopMirror {
    pub fn new() -> Self {
        unsafe {
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let desktop_dc = GetDC(std::ptr::null_mut());
            let mem_dc = CreateCompatibleDC(desktop_dc);
            let mem_bmp = CreateCompatibleBitmap(desktop_dc, width, height);
            SelectObject(mem_dc, mem_bmp as HGDIOBJ);
            ReleaseDC(std::ptr::null_mut(), desktop_dc);

            Self {
                hdc: mem_dc,
                mem_bmp,
                width,
                height,
            }
        }
    }

    pub fn update(&self) {
        info!("updating DesktopMirror");
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);

            let desktop_hwnd = get_desktop_window();
            let desktop_dc = GetDC(desktop_hwnd);

            // Map the virtual screen coordinates to the desktop window's client coordinates
            let mut pt = POINT { x: left, y: top };
            ScreenToClient(desktop_hwnd, &mut pt);

            BitBlt(
                self.hdc,
                0,
                0,
                self.width,
                self.height,
                desktop_dc,
                pt.x,
                pt.y,
                SRCCOPY,
            );

            ReleaseDC(desktop_hwnd, desktop_dc);
        }
    }

    pub fn hdc(&self) -> HDC {
        self.hdc
    }
}

impl Drop for DesktopMirror {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.mem_bmp as HGDIOBJ);
            DeleteDC(self.hdc);
        }
    }
}
