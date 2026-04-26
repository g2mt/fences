use windows_sys::Win32::Foundation::RECT;

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
}
