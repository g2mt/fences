use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

pub struct Fence {
    pub rect: RECT,
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
        }
    }

    pub unsafe fn draw(&self, hdc: HDC) {
        let width = self.rect.right - self.rect.left;
        let height = self.rect.bottom - self.rect.top;

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

        SelectObject(mem_dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
    }
}
