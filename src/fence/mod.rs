use anyhow::Result;
use tracing::info;
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod icon;
mod import_dialog;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use import_dialog::{ImportDialog, ImportItem};

use crate::app::App;
use crate::config::state::{FenceState, IconState};
use crate::fence::icon::Icon;
use crate::geo::Area;
use crate::prompt;
use crate::window::{register_classname, Base, BaseRef, Window};

pub struct TitleBar {
    base: BaseRef,
    title: Mutex<Arc<str>>,
}

impl TitleBar {
    pub fn new(parent_hwnd: HWND, title: Arc<str>, fence_area: &Area<i32>) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        let area = Self::area_from_fence_area(fence_area);
        Base::create_window(
            0,
            register_classname(w!("FenceTitleBar")),
            title
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
                .as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            area.x,
            area.y,
            area.width,
            area.height,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
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

    pub fn set_title(&self, title: Arc<str>) {
        *self.title.lock().unwrap() = title;
        self.base.redraw();
    }
}

impl Window for TitleBar {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);

                let config = App::config();
                config.fence.title_bar_bg_color.paint_background(hdc, &rect);
                SetBkMode(hdc, TRANSPARENT as _);
                SetTextColor(hdc, config.fence.title_text_color.0);

                let title_utf16: Vec<u16> = self.title.lock().unwrap().encode_utf16().collect();
                let mut text_rect = rect;
                text_rect.left += 5;
                DrawTextW(
                    hdc,
                    title_utf16.as_ptr(),
                    title_utf16.len() as _,
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
    pub fn new(parent_hwnd: HWND, fence_area: &Area<i32>) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
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
            register_classname(w!("FenceScrollArea")),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_VSCROLL,
            area.x,
            area.y,
            area.width,
            area.height,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
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
                let mut si: SCROLLINFO = std::mem::zeroed();
                si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                si.fMask = SIF_ALL;
                GetScrollInfo(hwnd, SB_VERT, &mut si);

                let scroll_amount = (delta / WHEEL_DELTA as i32) * 30;
                let new_pos = (si.nPos - scroll_amount).clamp(si.nMin, si.nMax - si.nPage as i32);

                if new_pos != si.nPos {
                    SendMessageW(
                        hwnd,
                        WM_VSCROLL,
                        ((new_pos as usize) << 16) | SB_THUMBTRACK as usize,
                        0,
                    );
                }
                0
            },
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);

                let mut pt = POINT { x: 0, y: 0 };
                ClientToScreen(hwnd, &mut pt);

                let config = App::config();
                if config.fence.scroll_area_bg_color.a() < 255 {
                    let mirror = App::get().mirror.lock().unwrap();
                    let screen_left = GetSystemMetrics(SM_XVIRTUALSCREEN);
                    let screen_top = GetSystemMetrics(SM_YVIRTUALSCREEN);
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
                    );
                }
                config
                    .fence
                    .scroll_area_bg_color
                    .paint_background(hdc, &rect);

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
    title: Arc<str>,
    icons: Vec<Arc<Icon>>,
    imported_from: Option<Arc<str>>,
}

impl Fence {
    pub fn new(parent_hwnd: HWND, x: i32, y: i32, title: &str) -> Result<Arc<Self>> {
        Self::from_state(
            parent_hwnd,
            FenceState {
                title: Arc::from(title),
                area: Area::new(x, y, 300, 150),
                icons: Vec::new(),
                imported_from: None,
            },
        )
    }

    pub fn from_state(parent_hwnd: HWND, state: FenceState) -> Result<Arc<Self>> {
        let h_instance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        Base::create_window(
            0,
            register_classname(w!("Fence")),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
            state.area.x,
            state.area.y,
            state.area.width,
            state.area.height,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            |base| {
                let title_bar = TitleBar::new(base.hwnd(), state.title.clone(), &state.area)?;
                let scroll_area = ScrollArea::new(base.hwnd(), &state.area)?;

                let fence = Arc::new(Self {
                    base,
                    inner: Mutex::new(FenceInner {
                        title: state.title.clone(),
                        icons: Vec::new(),
                        imported_from: state.imported_from.clone(),
                    }),
                    title_bar,
                    scroll_area,
                });

                for icon_state in state.icons {
                    fence.add_icon_with_path(&icon_state.title, icon_state.path.as_deref());
                }
                Ok(fence)
            },
        )
    }

    pub fn get_state(&self) -> FenceState {
        let inner = self.inner.lock().unwrap();
        let area = self.base.area();
        FenceState {
            title: inner.title.clone(),
            area: Area::new(
                area.x.load(Ordering::Relaxed),
                area.y.load(Ordering::Relaxed),
                area.width.load(Ordering::Relaxed),
                area.height.load(Ordering::Relaxed),
            ),
            icons: inner
                .icons
                .iter()
                .map(|i| IconState {
                    title: i.title(),
                    path: i.path(),
                })
                .collect(),
            imported_from: inner.imported_from.clone(),
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<HitTest> {
        let config = App::config();
        let border = config.fence.border_thickness;
        let title_h = config.fence.title_bar_height;

        let rect = self.base.rect();
        if x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom {
            return None;
        }

        let on_left = x < rect.left + border;
        let on_right = x >= rect.right - border;
        let on_top = y < rect.top + border;
        let on_bottom = y >= rect.bottom - border;

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
                if y < rect.top + title_h {
                    HitTest::TitleBar
                } else {
                    let mut si: SCROLLINFO = unsafe { std::mem::zeroed() };
                    si.cbSize = std::mem::size_of::<SCROLLINFO>() as u32;
                    si.fMask = SIF_POS;
                    unsafe { GetScrollInfo(self.scroll_area.base().hwnd(), SB_VERT, &mut si) };

                    let rel_x = x - rect.left;
                    let rel_y = y - (rect.top + title_h) + si.nPos;
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

    pub fn title(&self) -> Arc<str> {
        self.inner.lock().unwrap().title.clone()
    }

    pub fn set_title(&self, title: Arc<str>) {
        let mut inner = self.inner.lock().unwrap();
        self.title_bar.set_title(title.clone());
        inner.title = title;
    }

    pub fn add_icon(&self, title: &str) {
        self.add_icon_with_path(title, None);
    }

    pub fn add_icon_with_path(&self, title: &str, path: Option<&str>) {
        let mut inner = self.inner.lock().unwrap();
        inner
            .icons
            .push(Icon::new(self.scroll_area.base().hwnd(), title, path, 0, 0));
        drop(inner);
        self.reflow_icons();
    }

    pub fn remove_icon(&self, index: usize) {
        let mut inner = self.inner.lock().unwrap();
        if index < inner.icons.len() {
            inner.icons.remove(index);
        }
        drop(inner);
        self.reflow_icons();
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

    pub fn icon_by_index(&self, index: usize) -> Option<Arc<Icon>> {
        self.inner.lock().unwrap().icons.get(index).cloned()
    }

    pub fn reflow_icons(&self) {
        let config = App::config();
        let icon_size = config.icon.size;
        let fence_padding = config.fence.padding;
        let fence_spacing = config.fence.spacing;

        let rect = self.base.rect();
        let width = rect.right - rect.left;

        let available_width = width - (fence_padding * 2);
        let cols = (available_width / (icon_size + fence_spacing)).max(1);

        let inner = self.inner.lock().unwrap();
        for (i, icon) in inner.icons.iter().enumerate() {
            let col = i as i32 % cols;
            let row = i as i32 / cols;

            let x = fence_padding + col * (icon_size + fence_spacing);
            let y = fence_padding + row * (icon_size + fence_spacing);

            icon.base().resize_to(x, y, icon_size, icon_size);
        }
        drop(inner);
        self.update_scroll_info();
    }

    pub fn imported_from(&self) -> Option<Arc<str>> {
        self.inner.lock().unwrap().imported_from.clone()
    }

    pub fn set_imported_from(&self, imported_from: Option<Arc<str>>) {
        self.inner.lock().unwrap().imported_from = imported_from;
    }

    pub fn show_import_existing_dialog(self: &Arc<Self>, parent_hwnd: HWND) {
        let imported_from = if let Some(p) = self.imported_from() {
            p
        } else {
            return;
        };

        let folder_path = std::path::Path::new(imported_from.as_ref());

        // Read all .lnk files from the directory
        let mut dir_items: Vec<(String, String)> = Vec::new(); // (title, path)
        if let Ok(entries) = std::fs::read_dir(folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("lnk") {
                    if let (Some(name), Some(path_str)) = (
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string()),
                        path.to_str().map(|s| s.to_string()),
                    ) {
                        dir_items.push((name, path_str));
                    }
                }
            }
        }

        // Build import items: existing icons get Keep/Remove based on whether
        // they still exist in the directory; new items from directory get Keep.
        let mut import_items: Vec<ImportItem> = Vec::new();

        {
            let inner = self.inner.lock().unwrap();
            for icon in &inner.icons {
                let icon_path = icon.path().map(|p| p.to_string()).unwrap_or_default();
                let still_present = dir_items.iter().any(|(_, dp)| *dp == icon_path);
                import_items.push(ImportItem {
                    title: icon.title(),
                    path: Arc::from(icon_path.as_str()),
                    action: if still_present {
                        import_dialog::ACTION_KEEP
                    } else {
                        import_dialog::ACTION_REMOVE
                    },
                });
            }
        }

        // Add new items from directory not already in the fence
        {
            let inner = self.inner.lock().unwrap();
            for (name, path_str) in &dir_items {
                let already_present = inner
                    .icons
                    .iter()
                    .any(|i| i.path().map(|p| p.to_string()).unwrap_or_default() == *path_str);
                if !already_present {
                    import_items.push(ImportItem {
                        title: Arc::from(name.as_str()),
                        path: Arc::from(path_str.as_str()),
                        action: import_dialog::ACTION_KEEP,
                    });
                }
            }
        }

        let fence = self.clone();
        ImportDialog::show(parent_hwnd, import_items, move |kept_items| {
            // Remove all existing icons
            let count = fence.icon_count();
            for _ in 0..count {
                fence.remove_icon(0);
            }
            // Add kept items
            for item in kept_items {
                fence.add_icon_with_path(&item.title, Some(&item.path));
            }
        });
    }

    pub fn show_import_from_dialog(self: &Arc<Self>, parent_hwnd: HWND) {
        let self_arc = self.clone();
        prompt::browse_for_folder(parent_hwnd, move |opt| {
            if let Some(path_str) = opt {
                self_arc.set_imported_from(Some(Arc::from(path_str.as_str())));
                self_arc.show_import_existing_dialog(parent_hwnd);
            }
        });
    }

    pub fn from_folder_selector(parent_hwnd: HWND) -> Result<Option<Arc<Self>>> {
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        prompt::browse_for_folder(parent_hwnd, move |opt| {
            let _ = tx.send(opt);
        });
        let path_str = match rx.recv() {
            Ok(Some(s)) => s,
            _ => return Ok(None),
        };
        let folder_path = std::path::Path::new(&path_str);
        let title = folder_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Folder Fence");

        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let fence = Self::new(parent_hwnd, width / 2 - 150, height / 2 - 75, title)?;
        fence.set_imported_from(Some(Arc::from(path_str.as_str())));

        if let Ok(entries) = std::fs::read_dir(folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("lnk") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        fence.add_icon_with_path(name, path.to_str());
                    }
                }
            }
        }

        Ok(Some(fence))
    }

    pub fn add_area(&self, dl: i32, dt: i32, dw: i32, dh: i32) {
        self.base.add_area(dl, dt, dw, dh);

        let bounds = self.base.area();
        let fence_area = Area::new(
            bounds.x.load(Ordering::Relaxed),
            bounds.y.load(Ordering::Relaxed),
            bounds.width.load(Ordering::Relaxed),
            bounds.height.load(Ordering::Relaxed),
        );

        let title_area = TitleBar::area_from_fence_area(&fence_area);
        let scroll_area = ScrollArea::area_from_fence_area(&fence_area);

        unsafe {
            let mut hdwp = BeginDeferWindowPos(2);
            if hdwp.is_null() {
                panic!("hdwp is null");
            }
            hdwp = self.title_bar.base().resize_to_deferred(
                title_area.x,
                title_area.y,
                title_area.width,
                title_area.height,
                hdwp,
            );
            hdwp = self.scroll_area.base().resize_to_deferred(
                scroll_area.x,
                scroll_area.y,
                scroll_area.width,
                scroll_area.height,
                hdwp,
            );
            EndDeferWindowPos(hdwp);
        }

        self.reflow_icons();
    }

    pub fn update_scroll_info(&self) {
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetClientRect(self.scroll_area.base().hwnd(), &mut rect) };
        let view_height = rect.bottom - rect.top;

        let inner = self.inner.lock().unwrap();
        let mut max_y = 0;
        for icon in &inner.icons {
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
        unsafe { SetScrollInfo(self.scroll_area.base().hwnd(), SB_VERT, &si, TRUE) };
        drop(inner);
    }
}

impl Window for Fence {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_MOVE => unsafe {
                InvalidateRect(self.scroll_area.base().hwnd(), std::ptr::null(), TRUE);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            },
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut rect: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);

                let config = App::config();
                config.fence.fence_bg_color.paint_background(hdc, &rect);

                EndPaint(hwnd, &ps);
                0
            },
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
