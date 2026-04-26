use anyhow::Result;
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod icon;
use std::sync::Arc;

use icon::Icon;

use crate::window::{register_classname, Base, BaseRef, Window};

pub const BORDER_THICKNESS: i32 = 3;
pub const TITLE_BAR_HEIGHT: i32 = 24;

pub struct TitleBar {
    pub base: BaseRef,
}

impl TitleBar {
    pub fn new(parent_hwnd: HWND, title: *const u16, width: i32) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        Base::create_window(
            0,
            register_classname(w!("FenceTitleBar")),
            title,
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            width,
            TITLE_BAR_HEIGHT,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            |base| Ok(Arc::new(Self { base })),
        )
    }
}

impl Window for TitleBar {
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
            },
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}

pub struct ScrollArea {
    pub base: BaseRef,
}

impl ScrollArea {
    pub fn new(parent_hwnd: HWND, width: i32, height: i32) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        Base::create_window(
            0,
            register_classname(w!("FenceScrollArea")),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_VSCROLL,
            0,
            TITLE_BAR_HEIGHT,
            width,
            height - TITLE_BAR_HEIGHT,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            |base| Ok(Arc::new(Self { base })),
        )
    }
}

impl Window for ScrollArea {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().handle();
        match msg {
            WM_NCHITTEST => unsafe {
                let res = DefWindowProcW(hwnd, msg, wparam, lparam);
                if res == HTCLIENT as LRESULT {
                    HTTRANSPARENT as LRESULT
                } else {
                    res
                }
            },
            WM_VSCROLL => unsafe {
                let mut si: SCROLLINFO = std::mem::zeroed();
                si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                si.fMask = SIF_ALL;
                GetScrollInfo(hwnd, SB_VERT, &mut si);

                let cur_pos = si.nPos;
                match (wparam & 0xFFFF) as i32 {
                    SB_TOP => si.nPos = si.nMin,
                    SB_BOTTOM => si.nPos = si.nMax,
                    SB_LINEUP => si.nPos -= 10,
                    SB_LINEDOWN => si.nPos += 10,
                    SB_PAGEUP => si.nPos -= si.nPage as i32,
                    SB_PAGEDOWN => si.nPos += si.nPage as i32,
                    SB_THUMBTRACK => si.nPos = (wparam >> 16) as i16 as i32,
                    _ => {}
                }

                si.fMask = SIF_POS;
                SetScrollInfo(hwnd, SB_VERT, &si, TRUE);
                GetScrollInfo(hwnd, SB_VERT, &mut si);

                if si.nPos != cur_pos {
                    ScrollWindowEx(
                        hwnd,
                        0,
                        cur_pos - si.nPos,
                        std::ptr::null(),
                        std::ptr::null(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        SW_ERASE | SW_INVALIDATE | SW_SCROLLCHILDREN,
                    );
                    let parent = GetParent(hwnd);
                    if parent != std::ptr::null_mut() {
                        InvalidateRect(parent, std::ptr::null(), TRUE);
                    }
                }
                0
            },
            WM_MOUSEWHEEL => unsafe {
                let delta = (wparam >> 16) as i16 as i32;
                let msg = if delta > 0 { SB_LINEUP } else { SB_LINEDOWN };
                SendMessageW(hwnd, WM_VSCROLL, msg as WPARAM, 0);
                0
            },
            WM_PAINT => unsafe {
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
            },
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
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
    pub base: BaseRef,
    pub rect: RECT,
    pub title: String,
    pub icons: Vec<Icon>,
    pub title_bar: Arc<TitleBar>,
    pub scroll_area: Arc<ScrollArea>,
}

impl Fence {
    pub fn new(parent_hwnd: HWND, x: i32, y: i32) -> Arc<Self> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };

        let title = "Untitled";
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        Arc::new_cyclic(|weak| {
            let base = unsafe {
                Base::create_window(
                    weak.clone(),
                    0,
                    register_classname(w!("Fence")),
                    std::ptr::null(),
                    WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                    x,
                    y,
                    300,
                    150,
                    parent_hwnd,
                    std::ptr::null_mut(),
                    h_instance,
                )
                .unwrap()
            };

            let title_bar = TitleBar::new(base.handle(), title_u16.as_ptr(), 300).unwrap();

            let scroll_area = ScrollArea::new(base.handle(), 300, 150).unwrap();

            let mut fence = Self {
                base,
                rect: RECT {
                    left: x,
                    top: y,
                    right: x + 300,
                    bottom: y + 150,
                },
                title: title.to_string(),
                icons: Vec::new(),
                title_bar,
                scroll_area,
            };

            fence.icons.push(Icon::new(
                fence.scroll_area.base().handle(),
                "Test Icon",
                10,
                10,
            ));
            fence.update_scroll_info();
            fence
        })
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
                    let mut si: SCROLLINFO = unsafe { std::mem::zeroed() };
                    si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                    si.fMask = SIF_POS;
                    unsafe { GetScrollInfo(self.scroll_area.base().handle(), SB_VERT, &mut si) };

                    let rel_x = x - self.rect.left;
                    let rel_y = y - (self.rect.top + TITLE_BAR_HEIGHT) + si.nPos;
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

    pub fn add_icon(&mut self, title: &str) {
        let x = 10;
        let y = 10 + (self.icons.len() as i32 * 70);
        self.icons
            .push(Icon::new(self.scroll_area.base().handle(), title, x, y));
        self.update_scroll_info();
    }

    pub fn move_by(&mut self, dx: i32, dy: i32) {
        self.rect.left += dx;
        self.rect.right += dx;
        self.rect.top += dy;
        self.rect.bottom += dy;
    }

    pub fn invalidate(&self) {
        unsafe {
            let parent = GetParent(self.base().handle());
            if parent != std::ptr::null_mut() {
                InvalidateRect(parent, std::ptr::null(), TRUE);
            }
            InvalidateRect(self.base().handle(), std::ptr::null(), TRUE);
            InvalidateRect(self.title_bar.base().handle(), std::ptr::null(), TRUE);
            InvalidateRect(self.scroll_area.base().handle(), std::ptr::null(), TRUE);
        }
    }

    pub fn update_layout(&self) {
        let width = self.rect.right - self.rect.left;
        let height = self.rect.bottom - self.rect.top;

        unsafe {
            SetWindowPos(
                self.base().handle(),
                std::ptr::null_mut(),
                self.rect.left,
                self.rect.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            SetWindowPos(
                self.title_bar.base().handle(),
                std::ptr::null_mut(),
                0,
                0,
                width,
                TITLE_BAR_HEIGHT,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            SetWindowPos(
                self.scroll_area.base().handle(),
                std::ptr::null_mut(),
                0,
                TITLE_BAR_HEIGHT,
                width,
                height - TITLE_BAR_HEIGHT,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.update_scroll_info();
    }

    pub fn update_scroll_info(&self) {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetClientRect(self.scroll_area.base().handle(), &mut rect) };
        let view_height = rect.bottom - rect.top;

        let mut max_y = 0;
        for icon in &self.icons {
            if icon.y + 64 > max_y {
                max_y = icon.y + 64;
            }
        }
        let content_height = max_y + 10;

        let mut si: SCROLLINFO = unsafe { std::mem::zeroed() };
        si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
        si.fMask = SIF_RANGE | SIF_PAGE | SIF_DISABLENOSCROLL;
        si.nMin = 0;
        si.nMax = content_height;
        si.nPage = view_height as u32;
        unsafe { SetScrollInfo(self.scroll_area.base().handle(), SB_VERT, &si, TRUE) };
        self.invalidate();
    }

    pub fn bring_to_front(&self) {
        unsafe {
            SetWindowPos(
                self.base().handle(),
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
        self.invalidate();
    }
}

impl Window for Fence {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().handle();
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
