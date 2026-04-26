use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::*;

mod icon;
use icon::FenceIcon;

pub const BORDER_THICKNESS: i32 = 3;
pub const TITLE_BAR_HEIGHT: i32 = 24;

#[derive(Clone, Copy, PartialEq)]
pub enum HitTest {
    TitleBar,
    Client,
    Icon(usize),
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
    pub icons: Vec<FenceIcon>,
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
            icons: vec![FenceIcon::new("Test Icon", 10, 10)],
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<HitTest> {
        if x < self.rect.left || x >= self.rect.right || y < self.rect.top || y >= self.rect.bottom
        {
            return None;
        }

        let on_left = x < self.rect.left + BORDER_THICKNESS;
        let on_right = x >= self.rect.right - BORDER_THICKNESS;
        let on_top = y < self.rect.top + BORDER_THICKNESS;
        let on_bottom = y >= self.rect.bottom - BORDER_THICKNESS;

        let hit = match (on_left, on_right, on_top, on_bottom) {
            (true, _, true, _) => HitTest::TopLeft,
            (_, true, true, _) => HitTest::TopRight,
            (true, _, _, true) => HitTest::BottomLeft,
            (_, true, _, true) => HitTest::BottomRight,
            (true, _, _, _) => HitTest::Left,
            (_, true, _, _) => HitTest::Right,
            (_, _, true, _) => HitTest::Top,
            (_, _, _, true) => HitTest::Bottom,
            _ => {
                if y < self.rect.top + TITLE_BAR_HEIGHT {
                    HitTest::TitleBar
                } else {
                    let rel_x = x - self.rect.left;
                    let rel_y = y - (self.rect.top + TITLE_BAR_HEIGHT);
                    let mut icon_hit = None;
                    for (i, icon) in self.icons.iter().enumerate() {
                        if icon.hit_test(rel_x, rel_y) {
                            icon_hit = Some(HitTest::Icon(i));
                            break;
                        }
                    }
                    icon_hit.unwrap_or(HitTest::Client)
                }
            }
        };
        Some(hit)
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

            // Draw title bar (dark grey)
            let title_brush = CreateSolidBrush(0x00404040);
            let title_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: TITLE_BAR_HEIGHT,
            };
            FillRect(mem_dc, &title_rect, title_brush);
            DeleteObject(title_brush);

            // Draw scroll area (lighter grey)
            let scroll_brush = CreateSolidBrush(0x00A0A0A0);
            let scroll_rect = RECT {
                left: 0,
                top: TITLE_BAR_HEIGHT,
                right: width,
                bottom: height,
            };
            FillRect(mem_dc, &scroll_rect, scroll_brush);
            DeleteObject(scroll_brush);

            // Draw edge to make it look like a control
            let mut edge_rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            DrawEdge(mem_dc, &mut edge_rect, EDGE_RAISED, BF_RECT);

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 200, // ~78% transparent
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
            let mut rect = RECT {
                left: self.rect.left + 5,
                top: self.rect.top,
                right: self.rect.right,
                bottom: self.rect.top + TITLE_BAR_HEIGHT,
            };
            DrawTextW(
                hdc,
                title_u16.as_ptr(),
                title_u16.len() as i32,
                &mut rect as *mut RECT,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            // Draw icons in the scroll area
            for icon in &self.icons {
                icon.draw(hdc, self.rect.left, self.rect.top + TITLE_BAR_HEIGHT);
            }

            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
        }
    }
}
