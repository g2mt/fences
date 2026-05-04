use std::sync::Arc;

use anyhow::Result;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;
use crate::geo::Area;
use crate::window::{register_classname, Base, BaseRef, Window};

use crate::fence::fence::WM_USER_PAINT_WITH_ALPHA;

pub struct ScrollArea {
    base: BaseRef,
}

impl ScrollArea {
    pub fn new(parent_hwnd: HWND, fence_area: &Area<i32>) -> Result<Arc<Self>> {
        let hinstance = unsafe {
            HINSTANCE(GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as *mut core::ffi::c_void)
        };
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
            WINDOW_EX_STYLE(0),
            register_classname("FenceScrollArea"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_VSCROLL,
            area.x,
            area.y,
            area.width,
            area.height,
            parent_hwnd,
            None,
            hinstance,
            |base| Ok(Arc::new(Self { base })),
        )
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
        #[cfg(not(feature = "use-UpdateLayeredWindow"))]
        if config.fence.scroll_area_bg_color.a() < 255 {
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
                    Some(mirror.hdc()),
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
                if res == LRESULT(HTCLIENT as isize) {
                    LRESULT(HTTRANSPARENT as isize)
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
                match SCROLLBAR_COMMAND((wparam.0 & 0xFFFF) as i32) {
                    SB_TOP => si.nPos = si.nMin,
                    SB_BOTTOM => si.nPos = si.nMax,
                    SB_LINEUP => si.nPos -= 10,
                    SB_LINEDOWN => si.nPos += 10,
                    SB_PAGEUP => si.nPos -= si.nPage as i32,
                    SB_PAGEDOWN => si.nPos += si.nPage as i32,
                    SB_THUMBTRACK => si.nPos = (wparam.0 >> 16) as i16 as i32,
                    _ => {}
                }

                si.fMask = SIF_POS;
                let _ = SetScrollInfo(hwnd, SB_VERT, &si, true);
                let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);

                if si.nPos != cur_pos {
                    let _ = ScrollWindowEx(
                        hwnd,
                        0,
                        cur_pos - si.nPos,
                        None,
                        None,
                        None,
                        None,
                        SW_ERASE | SW_INVALIDATE | SW_SCROLLCHILDREN,
                    );
                    let parent = GetParent(hwnd);
                    if let Ok(parent) = parent {
                        let _ = InvalidateRect(Some(parent), None, true);
                    }
                }
                LRESULT(0)
            },
            WM_MOUSEWHEEL => unsafe {
                let delta = (wparam.0 >> 16) as i16 as i32;
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
                        Some(WPARAM(
                            ((new_pos as usize) << 16) | SB_THUMBTRACK.0 as usize,
                        )),
                        Some(LPARAM(0)),
                    );
                }
                LRESULT(0)
            },
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                self.paint(hdc);
                let _ = EndPaint(hwnd, &ps);

                #[cfg(feature = "use-UpdateLayeredWindow")]
                {
                    let _ = PostMessageW(
                        GetParent(hwnd).ok(),
                        WM_USER_PAINT_WITH_ALPHA,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }

                LRESULT(0)
            },
            WM_PRINTCLIENT => {
                self.paint(HDC(wparam.0 as _));
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
