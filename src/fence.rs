use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

pub const BORDER_THICKNESS: i32 = 3;

#[derive(Clone, Copy, PartialEq)]
pub enum HitTest {
    None,
    Inside,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub struct Fence {
    pub rect: RECT,
    pub title: String,
}

impl Fence {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            rect: RECT {
                left: x,
                top: y,
                right: x + 300,
                bottom: y + 150,
            },
            title: "Untitled".to_string(),
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> HitTest {
        if x < self.rect.left || x >= self.rect.right || y < self.rect.top || y >= self.rect.bottom
        {
            return HitTest::None;
        }

        let on_left = x < self.rect.left + BORDER_THICKNESS;
        let on_right = x >= self.rect.right - BORDER_THICKNESS;
        let on_top = y < self.rect.top + BORDER_THICKNESS;
        let on_bottom = y >= self.rect.bottom - BORDER_THICKNESS;

        match (on_left, on_right, on_top, on_bottom) {
            (true, _, true, _) => HitTest::TopLeft,
            (_, true, true, _) => HitTest::TopRight,
            (true, _, _, true) => HitTest::BottomLeft,
            (_, true, _, true) => HitTest::BottomRight,
            (true, _, _, _) => HitTest::Left,
            (_, true, _, _) => HitTest::Right,
            (_, _, true, _) => HitTest::Top,
            (_, _, _, true) => HitTest::Bottom,
            _ => HitTest::Inside,
        }
    }

    pub fn move_by(&mut self, dx: i32, dy: i32) {
        self.rect.left += dx;
        self.rect.right += dx;
        self.rect.top += dy;
        self.rect.bottom += dy;
    }

    pub unsafe fn draw(&self, hdc: HDC) {
        let width = self.rect.right - self.rect.left;
        let height = self.rect.bottom - self.rect.top;

        unsafe {
            let mem_dc = CreateCompatibleDC(hdc);
            let bitmap = CreateCompatibleBitmap(hdc, width, height);
            let old_bitmap = SelectObject(mem_dc, bitmap);

            // Fill with gray (0x808080)
            let brush = CreateSolidBrush(0x00808080);
            let rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            FillRect(mem_dc, &rect, brush);
            DeleteObject(brush);

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 128, // ~50% transparent
                AlphaFormat: 0,
            };

            AlphaBlend(
                hdc,
                self.rect.left,
                self.rect.top,
                width,
                height,
                mem_dc,
                0,
                0,
                width,
                height,
                blend,
            );

            // Draw title text
            SetBkMode(hdc, TRANSPARENT as _);
            SetTextColor(hdc, 0x00FFFFFF); // White
            let title_u16: Vec<u16> = self.title.encode_utf16().collect();
            let mut rect = self.rect;
            DrawTextW(
                hdc,
                title_u16.as_ptr(),
                title_u16.len() as i32,
                &mut rect as *mut RECT,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );

            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
        }
    }
}
