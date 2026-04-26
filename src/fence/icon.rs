use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

use crate::window::{Base, BaseRef, Window, register_classname};

pub const ICON_SIZE: i32 = 64;

pub struct Icon {
    base: BaseRef,
    pub title: String,
    selected: AtomicBool,
}

impl Icon {
    pub fn new(parent_hwnd: HWND, title: &str, x: i32, y: i32) -> Arc<Self> {
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
                    selected: AtomicBool::new(false),
                }))
            },
        )
        .expect("Failed to create Icon window")
    }

    pub fn set_selected(&self, selected: bool) {
        self.selected.store(selected, Ordering::SeqCst);
        unsafe {
            InvalidateRect(self.base.handle(), std::ptr::null(), TRUE);
        }
    }

    pub fn hit_test(&self, rel_x: i32, rel_y: i32) -> bool {
        let rect = self.base.rect();
        rel_x >= rect.left && rel_x < rect.right && rel_y >= rect.top && rel_y < rect.bottom
    }
}

impl Window for Icon {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().handle();
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

                let hicon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
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
