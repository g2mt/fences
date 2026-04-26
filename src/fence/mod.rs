use anyhow::Result;
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod icon;
use std::sync::{Arc, Mutex};

use icon::Icon;

use crate::window::{register_classname, Base, BaseRef, Window};

pub const BORDER_THICKNESS: i32 = 3;
pub const TITLE_BAR_HEIGHT: i32 = 24;

pub struct TitleBar {
    base: BaseRef,
}

impl TitleBar {
    pub fn new(parent_hwnd: HWND, title: *const u16, width: i32) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        Base::create_window(
            0,
            register_classname(w!("FenceTitleBar")),
            title,
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
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
    base: BaseRef,
}

impl ScrollArea {
    pub fn new(parent_hwnd: HWND, width: i32, height: i32) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        Base::create_window(
            0,
            register_classname(w!("FenceScrollArea")),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_VSCROLL,
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
    base: BaseRef,
    inner: Mutex<FenceInner>,
    pub title_bar: Arc<TitleBar>,
    pub scroll_area: Arc<ScrollArea>,
}

struct FenceInner {
    title: String,
    icons: Vec<Arc<Icon>>,
}

impl Fence {
    pub fn new(parent_hwnd: HWND, x: i32, y: i32) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };

        let title = "Untitled";
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        Base::create_window(
            0,
            register_classname(w!("Fence")),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            x,
            y,
            300,
            150,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            |base| {
                let title_bar = TitleBar::new(base.handle(), title_u16.as_ptr(), 300)?;
                let scroll_area = ScrollArea::new(base.handle(), 300, 150)?;

                let fence = Arc::new(Self {
                    base,
                    inner: Mutex::new(FenceInner {
                        title: title.to_string(),
                        icons: Vec::new(),
                    }),
                    title_bar,
                    scroll_area,
                });

                {
                    let mut inner = fence.inner.lock().unwrap();
                    inner.icons.push(Icon::new(
                        fence.scroll_area.base().handle(),
                        "Test Icon",
                        10,
                        10,
                    ));
                }
                fence.update_scroll_info();
                Ok(fence)
            },
        )
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<HitTest> {
        let rect = self.base.rect();
        if x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom {
            return None;
        }

        let on_left = x < rect.left + BORDER_THICKNESS;
        let on_right = x >= rect.right - BORDER_THICKNESS;
        let on_top = y < rect.top + BORDER_THICKNESS;
        let on_bottom = y >= rect.bottom - BORDER_THICKNESS;

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
                if y < rect.top + TITLE_BAR_HEIGHT {
                    HitTest::TitleBar
                } else {
                    let mut si: SCROLLINFO = unsafe { std::mem::zeroed() };
                    si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                    si.fMask = SIF_POS;
                    unsafe { GetScrollInfo(self.scroll_area.base().handle(), SB_VERT, &mut si) };

                    let rel_x = x - rect.left;
                    let rel_y = y - (rect.top + TITLE_BAR_HEIGHT) + si.nPos;
                    let mut icon_hit = None;
                    let inner = self.inner.lock().unwrap();
                    for (i, icon) in inner.icons.iter().enumerate() {
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

    pub fn add_icon(&self, title: &str) {
        let mut inner = self.inner.lock().unwrap();
        let x = 10;
        let y = 10 + (inner.icons.len() as i32 * 70);
        inner
            .icons
            .push(Icon::new(self.scroll_area.base().handle(), title, x, y));
        drop(inner);
        self.update_scroll_info();
    }

    pub fn remove_icon(&self, index: usize) {
        let mut inner = self.inner.lock().unwrap();
        if index < inner.icons.len() {
            inner.icons.remove(index);
        }
        drop(inner);
        self.update_scroll_info();
    }

    pub fn icon_count(&self) -> usize {
        self.inner.lock().unwrap().icons.len()
    }

    pub fn clear_selection(&self) {
        let inner = self.inner.lock().unwrap();
        for icon in &inner.icons {
            icon.set_selected(false);
        }
    }

    pub fn select_icon(&self, index: usize) {
        let inner = self.inner.lock().unwrap();
        if let Some(icon) = inner.icons.get(index) {
            icon.set_selected(true);
        }
    }

    pub fn resize(&self, dl: i32, dt: i32, dr: i32, db: i32) {
        self.base.resize(dl, dt, dr, db);
        self.update_layout();
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

            let inner = self.inner.lock().unwrap();
            for icon in &inner.icons {
                InvalidateRect(icon.base().handle(), std::ptr::null(), TRUE);
            }
        }
    }

    pub fn update_layout(&self) {
        let rect = self.base.rect();
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;

        self.title_bar.base().resize(0, 0, 0, 0); // Sync internal rect if needed, but here we just use SetWindowPos for children
        unsafe {
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

        let inner = self.inner.lock().unwrap();
        let mut max_y = 0;
        for icon in &inner.icons {
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
        drop(inner);
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
