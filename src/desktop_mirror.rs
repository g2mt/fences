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
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let desktop_dc = GetDC(std::ptr::null_mut());
            BitBlt(
                self.hdc,
                0,
                0,
                self.width,
                self.height,
                desktop_dc,
                left,
                top,
                SRCCOPY,
            );
            ReleaseDC(std::ptr::null_mut(), desktop_dc);
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
