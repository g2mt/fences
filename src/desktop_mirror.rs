use tracing::info;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

const PW_RENDERFULLCONTENT: PRINT_WINDOW_FLAGS = PRINT_WINDOW_FLAGS(0x00000002);

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
            let bounds = crate::app::App::get().screen_bounds();
            let width = bounds.width.load(std::sync::atomic::Ordering::Relaxed);
            let height = bounds.height.load(std::sync::atomic::Ordering::Relaxed);

            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen_dc));
            let mem_bmp = CreateCompatibleBitmap(screen_dc, width, height);
            SelectObject(mem_dc, mem_bmp.into());
            ReleaseDC(None, screen_dc);

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
        unsafe {
            if !self.bitmap.is_invalid() {
                DeleteObject(self.bitmap.into());
            }
            if !self.hdc.is_invalid() {
                DeleteDC(self.hdc);
            }
        }
    }
}
