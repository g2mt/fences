use std::sync::Mutex;

use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct DesktopMirror {
    inner: Mutex<DesktopMirrorInner>,
}
unsafe impl Send for DesktopMirror {}
unsafe impl Sync for DesktopMirror {}

struct DesktopMirrorInner {
    hdc: HDC,
    mem_bmp: HBITMAP,
    width: i32,
    height: i32,
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
                inner: Mutex::new(DesktopMirrorInner {
                    hdc: mem_dc,
                    mem_bmp,
                    width,
                    height,
                }),
            }
        }
    }

    pub fn update(&self) {
        let inner = self.inner.lock().unwrap();
        unsafe {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let desktop_dc = GetDC(std::ptr::null_mut());
            BitBlt(
                inner.hdc,
                0,
                0,
                inner.width,
                inner.height,
                desktop_dc,
                left,
                top,
                SRCCOPY,
            );
            ReleaseDC(std::ptr::null_mut(), desktop_dc);
        }
    }

    pub fn hdc(&self) -> HDC {
        self.inner.lock().unwrap().hdc
    }
}

impl Drop for DesktopMirror {
    fn drop(&mut self) {
        let inner = self.inner.lock().unwrap();
        unsafe {
            DeleteObject(inner.mem_bmp as HGDIOBJ);
            DeleteDC(inner.hdc);
        }
    }
}
