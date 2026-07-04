use windows_sys::Win32::Foundation::HWND;

#[derive(Copy, Clone)]
pub struct HWNDWrapper(pub HWND);
unsafe impl Send for HWNDWrapper {}
unsafe impl Sync for HWNDWrapper {}
