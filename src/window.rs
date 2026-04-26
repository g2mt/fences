use windows_sys::Win32::Foundation::*;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct WinHandle(pub HWND);
unsafe impl Send for WinHandle {}

pub trait Window: Send {
    fn handle(&self) -> WinHandle;
    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}
