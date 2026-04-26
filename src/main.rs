use anyhow::Result;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod desktop_cover;
mod fence;
mod window;

use crate::desktop_cover::DesktopCover;

fn main() -> Result<()> {
    unsafe {
        let _desktop_cover = DesktopCover::new()?;
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
