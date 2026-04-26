use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct WinHandle(pub HWND);
unsafe impl Send for WinHandle {}

pub trait Window: Send {
    fn handle(&self) -> WinHandle;
    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}
