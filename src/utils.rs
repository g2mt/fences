use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Copy, Clone)]
pub(crate) struct HWNDWrapper(pub HWND);
unsafe impl Send for HWNDWrapper {}
unsafe impl Sync for HWNDWrapper {}

pub fn create_button(
    text: &'static str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    hwndparent: HWND,
    hmenu: Option<HMENU>,
    hinstance: HINSTANCE,
) -> HWND {
    let text_u16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(text_u16.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            0,
            0,
            0,
            0,
            Some(hwndparent),
            hmenu,
            Some(hinstance.into()),
            None,
        )
        .unwrap()
    }
}
