use tracing::info;
use windows_sys::core::{w, BOOL};
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct DesktopMirror {
    hdc: HDC,
}
unsafe impl Send for DesktopMirror {}
unsafe impl Sync for DesktopMirror {}

impl DesktopMirror {
    pub fn new() -> Self {}

    pub fn update(&self) {
        info!("updating DesktopMirror");
        unsafe {}
    }

    pub fn hdc(&self) -> HDC {
        self.hdc
    }
}

impl Drop for DesktopMirror {
    fn drop(&mut self) {
        unsafe {}
    }
}
