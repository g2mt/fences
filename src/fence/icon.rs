use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct FenceIcon {
    pub title: String,
    pub x: i32,
    pub y: i32,
}

impl FenceIcon {
    pub fn new(title: &str, x: i32, y: i32) -> Self {
        Self {
            title: title.to_string(),
            x,
            y,
        }
    }

    pub unsafe fn draw(&self, hdc: HDC, parent_x: i32, parent_y: i32) {
        let icon_width = 32;
        let icon_height = 32;
        let text_height = 32;
        let width = 64;
        let height = icon_height + text_height;

        let abs_x = parent_x + self.x;
        let abs_y = parent_y + self.y;

        // Draw icon
        let hicon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        DrawIconEx(
            hdc,
            abs_x + (width - icon_width) / 2,
            abs_y,
            hicon,
            icon_width,
            icon_height,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        );

        // Draw text
        SetBkMode(hdc, TRANSPARENT as _);
        SetTextColor(hdc, 0x00FFFFFF); // White text
        let title_u16: Vec<u16> = self.title.encode_utf16().collect();
        let mut rect = RECT {
            left: abs_x,
            top: abs_y + icon_height,
            right: abs_x + width,
            bottom: abs_y + height,
        };
        DrawTextW(
            hdc,
            title_u16.as_ptr(),
            title_u16.len() as i32,
            &mut rect as *mut RECT,
            DT_CENTER | DT_WORDBREAK | DT_NOPREFIX,
        );
    }
}
