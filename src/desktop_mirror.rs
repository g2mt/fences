use std::ptr::{null, null_mut};

use tracing::info;
use windows_sys::core::w;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const PW_RENDERFULLCONTENT: u32 = 0x00000002;

#[allow(dead_code)]
pub struct DesktopMirror {
    hdc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
}
unsafe impl Send for DesktopMirror {}
unsafe impl Sync for DesktopMirror {}

impl DesktopMirror {
    pub fn new() -> Self {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);

            let screen_dc = GetDC(null_mut());
            let mem_dc = CreateCompatibleDC(screen_dc);
            let mem_bmp = CreateCompatibleBitmap(screen_dc, width, height);
            SelectObject(mem_dc, mem_bmp as HGDIOBJ);
            ReleaseDC(null_mut(), screen_dc);

            let mirror = Self {
                hdc: mem_dc,
                bitmap: mem_bmp,
                width,
                height,
            };

            mirror.update();

            mirror
        }
    }

    pub fn update(&self) {
        info!("updating DesktopMirror");
        unsafe {
            // Progman hosts the desktop. On systems with an active wallpaper slideshow/
            // Windows 10+, a WorkerW window behind the icons may hold the wallpaper,
            // but Progman + PW_RENDERFULLCONTENT still renders wallpaper + icons fine.
            let desktop_hwnd = FindWindowW(w!("Progman"), null());
            if !desktop_hwnd.is_null() {
                PrintWindow(desktop_hwnd, self.hdc, PW_RENDERFULLCONTENT);
            }
        }
    }

    pub fn hdc(&self) -> HDC {
        self.hdc
    }
}

impl Drop for DesktopMirror {
    fn drop(&mut self) {
        unsafe {
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap as HGDIOBJ);
            }
            if !self.hdc.is_null() {
                DeleteDC(self.hdc);
            }
        }
    }
}
