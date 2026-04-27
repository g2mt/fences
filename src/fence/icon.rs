use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::window::{register_classname, Base, BaseRef, Window};

pub const ICON_SIZE: i32 = 64;

#[derive(Serialize, Deserialize, Clone)]
pub struct IconState {
    pub title: String,
    pub path: Option<String>,
}

pub struct Icon {
    base: BaseRef,
    title: String,
    path: Option<String>,
    selected: AtomicBool,
}

impl Icon {
    pub fn new(parent_hwnd: HWND, title: &str, path: Option<&str>, x: i32, y: i32) -> Arc<Self> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        Base::create_window(
            0,
            register_classname(w!("FenceIcon")),
            title_u16.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            x,
            y,
            ICON_SIZE,
            ICON_SIZE,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            |base| {
                Ok(Arc::new(Self {
                    base,
                    title: title.to_string(),
                    path: path.map(|s| s.to_string()),
                    selected: AtomicBool::new(false),
                }))
            },
        )
        .expect("Failed to create Icon window")
    }

    pub fn set_selected(&self, selected: bool) {
        self.selected.store(selected, Ordering::SeqCst);
        unsafe {
            InvalidateRect(self.base.hwnd(), std::ptr::null(), TRUE);
        }
    }

    pub fn hit_test(&self, rel_x: i32, rel_y: i32) -> bool {
        let rect = self.base.rect();
        rel_x >= rect.left && rel_x < rect.right && rel_y >= rect.top && rel_y < rect.bottom
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: String) {
        self.title = title;
        unsafe {
            InvalidateRect(self.base.hwnd(), std::ptr::null(), TRUE);
        }
    }

    pub fn path(&self) -> Option<&String> {
        self.path.as_ref()
    }
}

impl Window for Icon {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);

                let selected = self.selected.load(Ordering::SeqCst);

                let bg_color = if selected { 0x00FFAA44 } else { 0x007D7D7D };
                let brush = CreateSolidBrush(bg_color);
                FillRect(hdc, &rect, brush);
                DeleteObject(brush);

                let icon_width = 32;
                let icon_height = 32;
                let width = rect.right - rect.left;

                let mut hicon = std::ptr::null_mut();
                if let Some(ref path) = self.path {
                    let path_u16: Vec<u16> =
                        path.encode_utf16().chain(std::iter::once(0)).collect();
                    let mut shfi: SHFILEINFOW = std::mem::zeroed();
                    SHGetFileInfoW(
                        path_u16.as_ptr(),
                        0,
                        &mut shfi,
                        std::mem::size_of::<SHFILEINFOW>() as u32,
                        SHGFI_ICON | SHGFI_LARGEICON,
                    );
                    hicon = shfi.hIcon;
                }

                if hicon.is_null() {
                    hicon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
                }

                DrawIconEx(
                    hdc,
                    (width - icon_width) / 2,
                    0,
                    hicon,
                    icon_width,
                    icon_height,
                    0,
                    std::ptr::null_mut(),
                    DI_NORMAL,
                );

                if !self.path.is_none() {
                    DestroyIcon(hicon);
                }

                SetBkMode(hdc, TRANSPARENT as _);
                SetTextColor(hdc, 0x00FFFFFF); // White text

                let mut title = vec![0u16; 256];
                let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 256);

                let mut text_rect = rect;
                text_rect.top += icon_height;
                DrawTextW(
                    hdc,
                    title.as_ptr(),
                    len,
                    &mut text_rect,
                    DT_CENTER | DT_WORDBREAK | DT_NOPREFIX,
                );

                EndPaint(hwnd, &ps);
                0
            },
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
