use windows_sys::Win32::Foundation::HWND;

#[derive(Copy, Clone)]
pub(crate) struct HWNDWrapper(pub HWND);
unsafe impl Send for HWNDWrapper {}
