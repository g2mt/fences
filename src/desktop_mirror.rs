use std::ptr::{null, null_mut};
use std::sync::Mutex;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct DesktopMirror {
    inner: Mutex<DesktopMirrorInner>,
}

struct DesktopMirrorInner {
    mem_dc: HDC,
    mem_bmp: HBITMAP,
    width: i32,
    height: i32,
}

impl DesktopMirror {
    pub fn new() -> Self {
        unsafe {
            let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let desktop_dc = GetDC(null_mut());
            let mem_dc = CreateCompatibleDC(desktop_dc);
            let mem_bmp = CreateCompatibleBitmap(desktop_dc, width, height);
            SelectObject(mem_dc, mem_bmp as HGDIOBJ);
            ReleaseDC(null_mut(), desktop_dc);

            Self {
                inner: Mutex::new(DesktopMirrorInner {
                    mem_dc,
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
            let desktop_dc = GetDC(null_mut());
            BitBlt(
                inner.mem_dc,
                0,
                0,
                inner.width,
                inner.height,
                desktop_dc,
                left,
                top,
                SRCCOPY,
            );
            ReleaseDC(null_mut(), desktop_dc);
        }
    }

    pub fn get_dc(&self) -> HDC {
        self.inner.lock().unwrap().mem_dc
    }
}

impl Drop for DesktopMirror {
    fn drop(&mut self) {
        let inner = self.inner.lock().unwrap();
        unsafe {
            DeleteObject(inner.mem_bmp as HGDIOBJ);
            DeleteDC(inner.mem_dc);
        }
    }
}
