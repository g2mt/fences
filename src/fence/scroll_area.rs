use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::{Mutex, MutexGuard};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;
use crate::fence::icon::Icon;
use crate::geo::Area;
use crate::window::{register_classname, Base, BaseRef, Window};

pub struct ScrollArea {
    base: BaseRef,
    icons: Mutex<Vec<Arc<Icon>>>,
}

impl ScrollArea {
    pub fn new(parent_hwnd: HWND, fence_area: &Area<i32>) -> Result<Arc<Self>> {
        let hinstance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        let config = App::config();
        let border = config.fence.border_thickness;
        let title_h = config.fence.title_bar_height;
        let area = Area::new(
            border,
            title_h,
            fence_area.width - (border * 2),
            fence_area.height - title_h - border,
        );
        Base::create_window(
            0,
            register_classname("FenceScrollArea"),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_VSCROLL,
            area.x,
            area.y,
            area.width,
            area.height,
            parent_hwnd,
            None,
            hinstance,
            |base| {
                Ok(Arc::new(Self {
                    base,
                    icons: Mutex::new(Vec::new()),
                }))
            },
        )
    }

    pub fn icons(&self) -> MutexGuard<'_, Vec<Arc<Icon>>> {
        self.icons.lock()
    }

    pub fn icon_by_index(&self, index: usize) -> Option<Arc<Icon>> {
        self.icons.lock().get(index).cloned()
    }

    pub fn add_icon(&self, title: &str, path: Option<&str>) {
        self.icons
            .lock()
            .push(Icon::new(self.base.hwnd(), title, path, 0, 0));
    }

    pub fn remove_icon(&self, index: usize) {
        let mut icons = self.icons.lock();
        if index < icons.len() {
            icons.remove(index);
        }
    }

    pub fn clear_icons(&self) {
        self.icons.lock().clear();
    }

    pub fn reflow_icons(&self) {
        let config = App::config();
        let icon_size = config.icon.size;
        let fence_padding = config.fence.padding;
        let fence_spacing = config.fence.spacing;

        let width = self.base().area().width.load(Ordering::Relaxed);

        let available_width = width - (fence_padding * 2);
        let cols = (available_width / (icon_size + fence_spacing)).max(1);

        {
            let icons = self.icons.lock();
            for (i, icon) in icons.iter().enumerate() {
                let col = i as i32 % cols;
                let row = i as i32 / cols;

                let x = fence_padding + col * (icon_size + fence_spacing);
                let y = fence_padding + row * (icon_size + fence_spacing);

                icon.base().resize_to(x, y, icon_size, icon_size);
            }
        }
        self.update_scroll_info();
    }

    pub fn area_from_fence_area(fence_area: &Area<i32>) -> Area<i32> {
        let config = App::config();
        let border = config.fence.border_thickness;
        let title_h = config.fence.title_bar_height;
        Area::new(
            border,
            title_h,
            fence_area.width - (border * 2),
            fence_area.height - title_h - border,
        )
    }

    fn paint(&self, hdc: HDC) {
        let hwnd = self.base().hwnd();
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };

        let mut pt = POINT { x: 0, y: 0 };
        let _ = unsafe { ClientToScreen(hwnd, &mut pt) };

        let config = App::config();
        if !config.use_layered_window && config.fence.scroll_area_bg_color.a() < 255 {
            let mirror = App::get().mirror.lock();
            let screen_left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let screen_top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let _ = unsafe {
                BitBlt(
                    hdc,
                    0,
                    0,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    mirror.hdc(),
                    pt.x - screen_left,
                    pt.y - screen_top,
                    SRCCOPY,
                )
            };
        }
        unsafe {
            config
                .fence
                .scroll_area_bg_color
                .paint_background(hdc, &rect);
        }
    }

    pub fn update_scroll_info(&self) {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe {
            let _ = GetClientRect(self.base().hwnd(), &mut rect);
        };
        let view_height = rect.bottom - rect.top;

        let mut max_y = 0;
        for icon in self.icons.lock().iter() {
            let irect = icon.base().rect();
            if irect.bottom > max_y {
                max_y = irect.bottom;
            }
        }
        let content_height = max_y + 10;

        let mut si: SCROLLINFO = unsafe { std::mem::zeroed() };
        si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
        si.fMask = SIF_RANGE | SIF_PAGE | SIF_DISABLENOSCROLL;
        si.nMin = 0;
        si.nMax = content_height;
        si.nPage = view_height as u32;
        unsafe { SetScrollInfo(self.base().hwnd(), SB_VERT, &si, 1) };
    }
}

impl Window for ScrollArea {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_NCHITTEST => unsafe {
                let res = DefWindowProcW(hwnd, msg, wparam, lparam);
                if res == HTCLIENT as isize {
                    HTTRANSPARENT as isize
                } else {
                    res
                }
            },
            WM_VSCROLL => unsafe {
                let mut si: SCROLLINFO = std::mem::zeroed();
                si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                si.fMask = SIF_ALL;
                let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);

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
                let _ = SetScrollInfo(hwnd, SB_VERT, &si, 1);
                let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);

                if si.nPos != cur_pos {
                    let _ = ScrollWindowEx(
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
                        let _ = InvalidateRect(parent, std::ptr::null(), 1);
                    }
                }
                0
            },
            WM_MOUSEWHEEL => unsafe {
                let delta = (wparam >> 16) as i16 as i32;
                let mut si: SCROLLINFO = std::mem::zeroed();
                si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                si.fMask = SIF_ALL;
                let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);

                let scroll_amount = (delta / WHEEL_DELTA as i32) * 30;
                let new_pos = (si.nPos - scroll_amount).clamp(si.nMin, si.nMax - si.nPage as i32);

                if new_pos != si.nPos {
                    let _ = SendMessageW(
                        hwnd,
                        WM_VSCROLL,
                        ((new_pos as WPARAM) << 16) | SB_THUMBTRACK as WPARAM,
                        0 as LPARAM,
                    );
                }
                0
            },
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                self.paint(hdc);
                let _ = EndPaint(hwnd, &ps);

                if App::config().use_layered_window {
                    let _ = PostMessageW(
                        GetParent(hwnd),
                        crate::fence::fence::WM_USER_PAINT_WITH_ALPHA,
                        0 as WPARAM,
                        0 as LPARAM,
                    );
                }

                0
            },
            WM_PRINTCLIENT => {
                self.paint(wparam as HDC);
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
