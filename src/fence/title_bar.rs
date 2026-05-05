use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;
use crate::geo::Area;
use crate::window::{register_classname, Base, BaseRef, Window};

pub struct TitleBar {
    base: BaseRef,
    title: Mutex<Arc<str>>,
}

impl TitleBar {
    pub fn new(parent_hwnd: HWND, title: Arc<str>, fence_area: &Area<i32>) -> Result<Arc<Self>> {
        let hinstance = unsafe { GetModuleHandleW(None).unwrap_or_default() };
        let area = Self::area_from_fence_area(fence_area);
        Base::create_window(
            WINDOW_EX_STYLE(0),
            register_classname("FenceTitleBar"),
            PCWSTR(
                title
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect::<Vec<_>>()
                    .as_ptr(),
            ),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            area.x,
            area.y,
            area.width,
            area.height,
            parent_hwnd,
            None,
            hinstance.into(),
            |base| {
                Ok(Arc::new(Self {
                    base,
                    title: Mutex::new(title),
                }))
            },
        )
    }

    pub fn area_from_fence_area(fence_area: &Area<i32>) -> Area<i32> {
        let title_bar_height = App::config().fence.title_bar_height;
        Area::new(0, 0, fence_area.width, title_bar_height)
    }

    pub fn title(&self) -> Arc<str> {
        self.title.lock().clone()
    }

    pub fn set_title(&self, title: Arc<str>) {
        *self.title.lock() = title;
        self.base.redraw(true);
    }

    fn paint(&self, hdc: HDC) {
        let hwnd = self.base().hwnd();
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };

        let config = App::config();
        unsafe {
            config.fence.title_bar_bg_color.paint_background(hdc, &rect);
        }

        let mut text_rect = rect;
        text_rect.left += 5;
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(config.fence.title_text_color.bgr()));
        }
        App::get().draw_text(
            hdc,
            &self.title.lock(),
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

impl Window for TitleBar {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                self.paint(hdc);
                let _ = EndPaint(hwnd, &ps);

                if App::config().use_layered_window {
                    let _ = PostMessageW(
                        GetParent(hwnd).ok(),
                        crate::fence::fence::WM_USER_PAINT_WITH_ALPHA,
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
