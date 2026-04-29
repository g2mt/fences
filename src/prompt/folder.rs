use tracing::info;
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::*;

use crate::app::App;
use crate::utils::HWNDWrapper;
use crate::window::Window;

pub fn browse_for_folder_sync() -> Option<String> {}

pub fn browse_for_folder<F>(f: F)
where
    F: FnOnce(Option<String>, HWND) + Send + 'static,
{
    std::thread::spawn(move || {
        let result = browse_for_folder_sync();
        f(result, HWND(std::ptr::null_mut()));
    });
}
