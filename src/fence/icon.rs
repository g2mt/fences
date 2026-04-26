use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::window::{WinHandle, Window};

pub fn register_class() {
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.hInstance = h_instance;
        wc.lpszClassName = w!("FenceIcon");
        wc.lpfnWndProc = Some(icon_wndproc);
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        RegisterClassW(&wc);
    }
}

pub struct Icon {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub selected: bool,
    pub handle: WinHandle,
}

impl Window for Icon {
    fn handle(&self) -> WinHandle {
        self.handle
    }

    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.handle.0;
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);

                let selected = GetWindowLongPtrW(hwnd, GWLP_USERDATA) != 0;

                if selected {
                    let brush = CreateSolidBrush(0x00FFAA44); // Light blue
                    FillRect(hdc, &rect, brush);
                    DeleteObject(brush);
                }

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

pub unsafe extern "system" fn icon_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mut icon = Icon {
        title: String::new(),
        x: 0,
        y: 0,
        selected: false,
        handle: WinHandle(hwnd),
    };
    icon.wndproc(msg, wparam, lparam)
}

impl Icon {
    pub fn new(parent_hwnd: HWND, title: &str, x: i32, y: i32) -> Self {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        let hwnd = unsafe {
            CreateWindowExW(
                0,
                w!("FenceIcon"),
                title_u16.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                x,
                y,
                64,
                64,
                parent_hwnd,
                std::ptr::null_mut(),
                h_instance,
                std::ptr::null(),
            )
        };

        Self {
            title: title.to_string(),
            x,
            y,
            selected: false,
            handle: WinHandle(hwnd),
        }
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
        unsafe {
            SetWindowLongPtrW(self.handle.0, GWLP_USERDATA, if selected { 1 } else { 0 });
            InvalidateRect(self.handle.0, std::ptr::null(), TRUE);
        }
    }

    pub fn hit_test(&self, rel_x: i32, rel_y: i32) -> bool {
        let width = 64;
        let height = 64;
        rel_x >= self.x && rel_x < self.x + width && rel_y >= self.y && rel_y < self.y + height
    }
}

impl Drop for Icon {
    fn drop(&mut self) {
        unsafe {
            if self.handle.0 != std::ptr::null_mut() {
                DestroyWindow(self.handle.0);
            }
        }
    }
}
