use std::sync::atomic::Ordering;

use windows::core::w;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;

const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(0x00000002);

#[derive(Default)]
pub struct DesktopMirror {
    hdc: HDC,
    bitmap: HBITMAP,
    width: i32,
    height: i32,
}
unsafe impl Send for DesktopMirror {}
unsafe impl Sync for DesktopMirror {}

impl DesktopMirror {
    fn reset(&mut self) {
        unsafe {
            if !self.bitmap.is_invalid() {
                DeleteObject(self.bitmap.into());
            }
            self.bitmap = Default::default();
            if !self.hdc.is_invalid() {
                DeleteDC(self.hdc);
            }
            self.hdc = Default::default();
        }
    }

    pub fn update(&mut self) {
        let bounds = App::get().screen_bounds();
        let width = bounds.width.load(Ordering::Relaxed);
        let height = bounds.height.load(Ordering::Relaxed);

        if width != self.width || height != self.height {
            self.reset();
            unsafe {
                let screen_dc = GetDC(None);
                let hdc = CreateCompatibleDC(Some(screen_dc));
                let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
                SelectObject(hdc, bitmap.into());
                ReleaseDC(None, screen_dc);
                self.bitmap = bitmap;
                self.hdc = hdc;
            }
        }

        unsafe {
            // Progman hosts the desktop. On systems with an active wallpaper slideshow/
            // Windows 10+, a WorkerW window behind the icons may hold the wallpaper,
            // but Progman + PW_RENDERFULLCONTENT still renders wallpaper + icons fine.
            let desktop_hwnd = FindWindowW(w!("Progman"), None);
            if let Ok(desktop_hwnd) = desktop_hwnd {
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
        self.reset();
    }
}
