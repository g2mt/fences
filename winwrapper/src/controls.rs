use std::cell::LazyCell;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::{CreateFontIndirectW, HFONT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

thread_local! {
    static CONTROL_FONT: LazyCell<HFONT> = LazyCell::new(|| unsafe {
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            &mut ncm as *mut NONCLIENTMETRICSW as *mut _,
            Default::default(),
        );
        CreateFontIndirectW(&ncm.lfMessageFont)
    });
}

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
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            w!("BUTTON"),
            text_u16.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_PUSHBUTTON as u32),
            x,
            y,
            width,
            height,
            hwndparent,
            hmenu.unwrap_or(std::ptr::null_mut()),
            hinstance,
            std::ptr::null(),
        )
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(hwnd, WM_SETFONT, (**font) as WPARAM, 1 as LPARAM);
        });
    }
    hwnd
}

pub fn create_label(
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    hwndparent: HWND,
    hinstance: HINSTANCE,
) -> HWND {
    let text_u16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            w!("STATIC"),
            text_u16.as_ptr(),
            WS_VISIBLE | WS_CHILD,
            x,
            y,
            width,
            height,
            hwndparent,
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null(),
        )
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(hwnd, WM_SETFONT, (**font) as WPARAM, 1 as LPARAM);
        });
    }
    hwnd
}

pub fn create_checkbox(
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
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            w!("BUTTON"),
            text_u16.as_ptr(),
            WS_CHILD | WS_VISIBLE | (BS_AUTOCHECKBOX as u32),
            x,
            y,
            width,
            height,
            hwndparent,
            hmenu.unwrap_or(std::ptr::null_mut()),
            hinstance,
            std::ptr::null(),
        )
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(hwnd, WM_SETFONT, (**font) as WPARAM, 1 as LPARAM);
        });
    }
    hwnd
}

pub fn create_edit(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    hwndparent: HWND,
    hmenu: Option<HMENU>,
    hinstance: HINSTANCE,
) -> HWND {
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            std::ptr::null(),
            WS_VISIBLE | WS_CHILD | WS_BORDER | (ES_AUTOHSCROLL as u32),
            x,
            y,
            width,
            height,
            hwndparent,
            hmenu.unwrap_or(std::ptr::null_mut()),
            hinstance,
            std::ptr::null(),
        )
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(hwnd, WM_SETFONT, (**font) as WPARAM, 1 as LPARAM);
        });
    }
    hwnd
}
