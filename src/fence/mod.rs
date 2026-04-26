use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod icon;
use crate::window::WinHandle;
use icon::FenceIcon;

pub const BORDER_THICKNESS: i32 = 3;
pub const TITLE_BAR_HEIGHT: i32 = 24;

pub fn register_classes() {
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());

        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.hInstance = h_instance;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);

        wc.lpszClassName = w!("FenceTitleBar");
        wc.lpfnWndProc = Some(title_bar_wndproc);
        RegisterClassW(&wc);

        wc.lpszClassName = w!("FenceScrollArea");
        wc.lpfnWndProc = Some(scroll_area_wndproc);
        RegisterClassW(&wc);

        icon::register_class();
    }
}

pub unsafe extern "system" fn title_bar_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);

            let title_brush = CreateSolidBrush(0x00323232);
            FillRect(hdc, &rect, title_brush);
            DeleteObject(title_brush);

            let mut edge_rect = rect;
            edge_rect.bottom += 2;
            DrawEdge(hdc, &mut edge_rect, EDGE_RAISED, BF_RECT);

            SetBkMode(hdc, TRANSPARENT as _);
            SetTextColor(hdc, 0x00FFFFFF);

            let mut title = vec![0u16; 256];
            let len = GetWindowTextW(hwnd, title.as_mut_ptr(), 256);

            let mut text_rect = rect;
            text_rect.left += 5;
            DrawTextW(
                hdc,
                title.as_ptr(),
                len,
                &mut text_rect,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );

            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub unsafe extern "system" fn scroll_area_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);

            let scroll_brush = CreateSolidBrush(0x007D7D7D);
            FillRect(hdc, &rect, scroll_brush);
            DeleteObject(scroll_brush);

            let mut edge_rect = rect;
            edge_rect.top -= 2;
            DrawEdge(hdc, &mut edge_rect, EDGE_RAISED, BF_RECT);

            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

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
    pub title_handle: WinHandle,
    pub scroll_handle: WinHandle,
}

impl Fence {
    pub fn new(parent_hwnd: HWND, x: i32, y: i32) -> Self {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };

        let title = "Untitled";
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        let hwnd_title = unsafe {
            CreateWindowExW(
                0,
                w!("FenceTitleBar"),
                title_u16.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                x,
                y,
                300,
                TITLE_BAR_HEIGHT,
                parent_hwnd,
                std::ptr::null_mut(),
                h_instance,
                std::ptr::null(),
            )
        };

        let hwnd_scroll = unsafe {
            CreateWindowExW(
                0,
                w!("FenceScrollArea"),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE,
                x,
                y + TITLE_BAR_HEIGHT,
                300,
                150 - TITLE_BAR_HEIGHT,
                parent_hwnd,
                std::ptr::null_mut(),
                h_instance,
                std::ptr::null(),
            )
        };

        let mut fence = Self {
            rect: RECT {
                left: x,
                top: y,
                right: x + 300,
                bottom: y + 150,
            },
            title: title.to_string(),
            icons: Vec::new(),
            title_handle: WinHandle(hwnd_title),
            scroll_handle: WinHandle(hwnd_scroll),
        };

        fence
            .icons
            .push(FenceIcon::new(hwnd_scroll, "Test Icon", 10, 10));
        fence
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

    pub fn update_layout(&self) {
        let width = self.rect.right - self.rect.left;
        let height = self.rect.bottom - self.rect.top;

        unsafe {
            SetWindowPos(
                self.title_handle.0,
                std::ptr::null_mut(),
                self.rect.left,
                self.rect.top,
                width,
                TITLE_BAR_HEIGHT,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            SetWindowPos(
                self.scroll_handle.0,
                std::ptr::null_mut(),
                self.rect.left,
                self.rect.top + TITLE_BAR_HEIGHT,
                width,
                height - TITLE_BAR_HEIGHT,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn bring_to_front(&self) {
        unsafe {
            SetWindowPos(
                self.title_handle.0,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
            SetWindowPos(
                self.scroll_handle.0,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }
}

impl Drop for Fence {
    fn drop(&mut self) {
        unsafe {
            if self.title_handle.0 != std::ptr::null_mut() {
                DestroyWindow(self.title_handle.0);
            }
            if self.scroll_handle.0 != std::ptr::null_mut() {
                DestroyWindow(self.scroll_handle.0);
            }
        }
    }
}
