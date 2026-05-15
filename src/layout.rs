use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::MoveWindow;

use crate::utils::HWNDWrapper;

pub enum Orientation {
    Vertical,
    Horizontal,
}

pub enum Item {
    Fixed { hwnd: HWNDWrapper, size: i32 },
    Fill { hwnd: HWNDWrapper, min: i32 },
    Nested { layout: Box<Layout>, size: i32 },
}

pub struct Layout {
    pub orientation: Orientation,
    pub margin: i32,
    pub gap: i32,
    pub items: Vec<Item>,
}

impl Layout {
    pub fn arrange(&self, mut rect: RECT) {
        rect.left += self.margin;
        rect.right -= self.margin;
        rect.top += self.margin;
        rect.bottom -= self.margin;
        match self.orientation {
            Orientation::Vertical => self.arrange_vertical(rect),
            Orientation::Horizontal => self.arrange_horizontal(rect),
        }
    }

    fn arrange_vertical(&self, rect: RECT) {
        let content_w = rect.right - rect.left;
        let content_h = (rect.bottom - rect.top - 2 * self.margin).max(0);
        let item_count = self.items.len();

        let mut fixed_total = 0i32;
        let mut fill_count = 0i32;
        for item in &self.items {
            match item {
                Item::Fixed { size, .. } | Item::Nested { size, .. } => {
                    fixed_total += size;
                }
                Item::Fill { .. } => fill_count += 1,
            }
        }
        let gap_total = if item_count > 0 {
            (item_count - 1) as i32 * self.gap
        } else {
            0
        };
        let fill_size = if fill_count > 0 {
            (content_h - fixed_total - gap_total).max(0) / fill_count
        } else {
            0
        };

        let mut y = rect.top + self.margin;
        for (i, item) in self.items.iter().enumerate() {
            let is_last = i == item_count - 1;
            match item {
                Item::Fixed { hwnd, size } => {
                    if hwnd.0 != std::ptr::null_mut() && content_w > 0 && *size > 0 {
                        unsafe {
                            let _ = MoveWindow(hwnd.0, rect.left, y, content_w, *size, 1);
                        }
                    }
                    y += size;
                }
                Item::Fill { hwnd, min } => {
                    let item_h = fill_size.max(*min);
                    if hwnd.0 != std::ptr::null_mut() && content_w > 0 && item_h > 0 {
                        unsafe {
                            let _ = MoveWindow(hwnd.0, rect.left, y, content_w, item_h, 1);
                        }
                    }
                    y += item_h;
                }
                Item::Nested { layout, size } => {
                    if content_w > 0 && *size > 0 {
                        let child = RECT {
                            left: rect.left,
                            top: y,
                            right: rect.left + content_w,
                            bottom: y + size,
                        };
                        layout.arrange(child.clone());
                    }
                    y += size;
                }
            }
            if !is_last {
                y += self.gap;
            }
        }
    }

    fn arrange_horizontal(&self, rect: RECT) {
        let content_w = (rect.right - rect.left - 2 * self.margin).max(0);
        let content_h = rect.bottom - rect.top;
        let item_count = self.items.len();

        let mut fixed_total = 0i32;
        let mut fill_count = 0i32;
        for item in &self.items {
            match item {
                Item::Fixed { size, .. } | Item::Nested { size, .. } => {
                    fixed_total += size;
                }
                Item::Fill { .. } => fill_count += 1,
            }
        }
        let gap_total = if item_count > 0 {
            (item_count - 1) as i32 * self.gap
        } else {
            0
        };
        let fill_size = if fill_count > 0 {
            (content_w - fixed_total - gap_total).max(0) / fill_count
        } else {
            0
        };

        let mut x = rect.left + self.margin;
        for (i, item) in self.items.iter().enumerate() {
            let is_last = i == item_count - 1;
            match item {
                Item::Fixed { hwnd, size } => {
                    if hwnd.0 != std::ptr::null_mut() && *size > 0 && content_h > 0 {
                        unsafe {
                            let _ = MoveWindow(hwnd.0, x, rect.top, *size, content_h, 1);
                        }
                    }
                    x += size;
                }
                Item::Fill { hwnd, min } => {
                    let item_w = fill_size.max(*min);
                    if hwnd.0 != std::ptr::null_mut() && item_w > 0 && content_h > 0 {
                        unsafe {
                            let _ = MoveWindow(hwnd.0, x, rect.top, item_w, content_h, 1);
                        }
                    }
                    x += item_w;
                }
                Item::Nested { layout, size } => {
                    if *size > 0 && content_h > 0 {
                        let child = RECT {
                            left: x,
                            top: rect.top,
                            right: x + size,
                            bottom: rect.top + content_h,
                        };
                        layout.arrange(child.clone());
                    }
                    x += size;
                }
            }
            if !is_last {
                x += self.gap;
            }
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            orientation: Orientation::Vertical,
            margin: 5,
            gap: 3,
            items: vec![],
        }
    }
}
