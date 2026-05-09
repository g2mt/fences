use std::cell::LazyCell;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, HFONT};
use windows::Win32::UI::WindowsAndMessaging::*;

thread_local! {
    static CONTROL_FONT: LazyCell<HFONT> = LazyCell::new(|| unsafe {
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
            Some(&mut ncm as *mut NONCLIENTMETRICSW as *mut _),
            Default::default(),
        ).unwrap();
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
            WINDOW_EX_STYLE(0),
            w!("BUTTON"),
            PCWSTR(text_u16.as_ptr()),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
            x,
            y,
            width,
            height,
            Some(hwndparent),
            hmenu,
            Some(hinstance.into()),
            None,
        )
        .unwrap()
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM((*font).0 as _)),
                Some(LPARAM(1)),
            );
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
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            PCWSTR(text_u16.as_ptr()),
            WS_VISIBLE | WS_CHILD,
            x,
            y,
            width,
            height,
            Some(hwndparent),
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap()
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM((*font).0 as _)),
                Some(LPARAM(1)),
            );
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
            None,
            WS_VISIBLE | WS_CHILD | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
            x,
            y,
            width,
            height,
            Some(hwndparent),
            hmenu,
            Some(hinstance.into()),
            None,
        )
        .unwrap()
    };
    unsafe {
        CONTROL_FONT.with(|font| {
            SendMessageW(
                hwnd,
                WM_SETFONT,
                Some(WPARAM((*font).0 as _)),
                Some(LPARAM(1)),
            );
        });
    }
    hwnd
}
