use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::window::{register_classname, Base, BaseRef, Window};

pub const ICON_SIZE: i32 = 64;

#[derive(Serialize, Deserialize, Clone)]
pub struct IconState {
    pub title: Arc<str>,
    pub path: Option<Arc<str>>,
}

pub struct Icon {
    base: BaseRef,
    state: Mutex<IconState>,
    selected: AtomicBool,
}

impl Icon {
    pub fn new(parent_hwnd: HWND, title: &str, path: Option<&str>, x: i32, y: i32) -> Arc<Self> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        let state = Mutex::new(IconState {
            title: Arc::from(title),
            path: path.map(|s| Arc::from(s)),
        });

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
                    state,
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

    pub fn title(&self) -> Arc<str> {
        self.state.lock().unwrap().title.clone()
    }

    pub fn set_title(&self, title: Arc<str>) {
        {
            let mut s = self.state.lock().unwrap();
            s.title = title.clone();
        }
        let hwnd = self.base.hwnd();
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SetWindowTextW(hwnd, title_u16.as_ptr());
        }
        self.base.redraw();
    }

    pub fn path(&self) -> Option<Arc<str>> {
        self.state
            .lock()
            .unwrap()
            .path
            .as_ref()
            .map(|arc| arc.clone())
    }

    pub fn set_path(&self, path: Option<Arc<str>>) {
        let _ = std::mem::replace(&mut self.state.lock().unwrap().path, path);
        self.base.redraw();
    }

    pub fn set_info_from_selector(&self) {
        let mut file_buf = [0u16; MAX_PATH as usize];
        let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = self.base.hwnd();
        ofn.lpstrFile = file_buf.as_mut_ptr();
        ofn.nMaxFile = MAX_PATH;
        ofn.lpstrFilter = w!("All Files\0*.*\0\0");
        ofn.nFilterIndex = 1;
        ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;

        if unsafe { GetOpenFileNameW(&mut ofn) } != 0 {
            let path_str = String::from_utf16_lossy(
                &file_buf[..file_buf.iter().position(|&c| c == 0).unwrap_or(0)],
            );
            let path = std::path::Path::new(&path_str);
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                self.set_title(Arc::from(name));
            }
            self.set_path(Some(Arc::from(path_str)));
        }
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

                let state = self.state.lock().unwrap();
                let path = state.path.clone();

                let mut hicon = std::ptr::null_mut();
                if let Some(ref path) = path {
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

                if path.is_some() {
                    DestroyIcon(hicon);
                }

                SetBkMode(hdc, TRANSPARENT as _);
                SetTextColor(hdc, 0x00FFFFFF); // White text

                let title_utf16: Vec<u16> = state.title.encode_utf16().collect();
                let mut text_rect = rect;
                text_rect.top += icon_height;
                DrawTextW(
                    hdc,
                    title_utf16.as_ptr(),
                    title_utf16.len() as _,
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
