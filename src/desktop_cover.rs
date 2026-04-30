use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use tracing::{debug, error, info};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;
use crate::config::state::{AppState, FenceStickyPosition};
use crate::fence::{Fence, HitTest};
use crate::prompt;
use crate::utils::HWNDWrapper;
use crate::window::{register_classname, Base, BaseRef, Window};

// Menus
pub const IDM_EXIT: usize = 101;
pub const IDM_ADD_FENCE: usize = 102;
pub const IDM_ADD_FENCE_FROM_FOLDER: usize = 103;
pub const IDM_DELETE_FENCE: usize = 104;
pub const IDM_ADD_ICON: usize = 105;
pub const IDM_RUN_ICON: usize = 106;
pub const IDM_DELETE_ICON: usize = 107;
pub const IDM_RENAME_FENCE: usize = 108;
pub const IDM_RENAME_ICON: usize = 109;
pub const IDM_SET_ICON_PATH: usize = 110;
pub const IDM_IMPORT: usize = 111;
pub const IDM_IMPORT_FROM: usize = 112;
pub const IDM_OPEN_EXPLORER: usize = 113;
pub const IDM_STICKY_NONE: usize = 114;
pub const IDM_STICKY_TOPLEFT: usize = 115;
pub const IDM_STICKY_TOPRIGHT: usize = 116;
pub const IDM_STICKY_BOTTOMLEFT: usize = 117;
pub const IDM_STICKY_BOTTOMRIGHT: usize = 118;

// Custom events
pub const WM_USER_SHELLICON: u32 = WM_USER + 1;
pub const WM_USER_WAKE_FUTURE: u32 = WM_USER + 2;

pub struct DesktopCover {
    base: BaseRef,
    inner: Mutex<DesktopCoverInner>,
    executor: crate::fut::AsyncExecutor,
}

struct DesktopCoverInner {
    /// List of fences currently managed by the desktop cover.
    fences: Vec<Arc<Fence>>,
    /// The type of hit test result from the last interaction, used for dragging or context menus.
    hit_type: Option<HitTest>,
    /// The last recorded mouse position in client coordinates.
    last_mouse_pos: POINT,
    /// Width of the screen
    screen_width: i32,
    /// Height of the screen
    screen_height: i32,
}

impl DesktopCover {
    pub fn new() -> Result<Arc<Self>> {
        let h_instance = unsafe { GetModuleHandleW(None).unwrap_or_default() };
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        Base::create_window(
            WS_EX_NOACTIVATE | WS_EX_LAYERED,
            register_classname("BottomWindowClass"),
            w!("Desktop Cover"),
            WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN,
            0,
            0,
            width,
            height,
            HWND::default(),
            None,
            h_instance.into(),
            |base| {
                let hwnd = base.hwnd();
                unsafe {
                    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                    nid.hWnd = hwnd;
                    nid.uID = 1;
                    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
                    nid.uCallbackMessage = WM_USER_SHELLICON;
                    // winresource puts icon at ID 1
                    nid.hIcon = LoadIconW(Some(h_instance.into()), PCWSTR(1usize as *const u16))
                        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
                        .unwrap_or_default();
                    let tip: Vec<u16> = "Desktop Cover"
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let len = tip.len().min(nid.szTip.len());
                    nid.szTip[..len].copy_from_slice(&tip[..len]);
                    Shell_NotifyIconW(NIM_ADD, &nid);

                    SetLayeredWindowAttributes(hwnd, COLORREF(0x00000000), 0, LWA_COLORKEY);
                    SetWindowPos(
                        hwnd,
                        Some(HWND_BOTTOM),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }

                let cover = Arc::new(Self {
                    base,
                    inner: Mutex::new(DesktopCoverInner {
                        fences: Vec::new(),
                        hit_type: None,
                        last_mouse_pos: POINT { x: 0, y: 0 },
                        screen_width: width,
                        screen_height: height,
                    }),
                    executor: crate::fut::AsyncExecutor::new(),
                });

                Ok(cover)
            },
        )
    }

    pub fn state(&self) -> AppState {
        let inner = self.inner.lock();
        AppState {
            fences: inner.fences.iter().map(|f| f.get_state()).collect(),
            screen_width: inner.screen_width,
            screen_height: inner.screen_height,
        }
    }

    pub fn set_state(&self, state: &AppState) -> Result<()> {
        let mut fences = Vec::new();
        for f_state in &state.fences {
            let fence = Fence::from_state(self, f_state.clone())?;
            fences.push(fence);
        }
        let mut inner = self.inner.lock();
        inner.fences = fences;
        drop(inner);

        self.rearrange_fences(state.screen_width, state.screen_height);
        Ok(())
    }

    pub fn rearrange_fences(&self, old_screen_width: i32, old_screen_height: i32) {
        let inner = self.inner.lock();
        let new_width = inner.screen_width;
        let new_height = inner.screen_height;

        if old_screen_width == new_width && old_screen_height == new_height {
            return;
        }

        info!(
            "rearranging from {}x{} to {}x{}",
            old_screen_width, old_screen_height, new_width, new_height
        );
        for fence in &inner.fences {
            if let Some(sticky) = fence.sticky() {
                let area = fence.get_state().area;
                let (new_x, new_y) = match sticky {
                    FenceStickyPosition::TopLeft => (area.x, area.y),
                    FenceStickyPosition::TopRight => {
                        let offset_from_right = old_screen_width - (area.x + area.width);
                        (new_width - area.width - offset_from_right, area.y)
                    }
                    FenceStickyPosition::BottomLeft => {
                        let offset_from_bottom = old_screen_height - (area.y + area.height);
                        (area.x, new_height - area.height - offset_from_bottom)
                    }
                    FenceStickyPosition::BottomRight => {
                        let offset_from_right = old_screen_width - (area.x + area.width);
                        let offset_from_bottom = old_screen_height - (area.y + area.height);
                        (
                            new_width - area.width - offset_from_right,
                            new_height - area.height - offset_from_bottom,
                        )
                    }
                };
                fence.base().move_to(new_x, new_y);
            }
        }

        App::get().save_thread.get().unwrap().set_unsaved();
    }

    fn on_display_change(&self) -> LRESULT {
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        info!("Screen resolution changed to {}x{}", width, height);

        let mut inner = self.inner.lock();
        let old_width = inner.screen_width;
        let old_height = inner.screen_height;
        inner.screen_width = width;
        inner.screen_height = height;
        drop(inner);

        self.rearrange_fences(old_width, old_height);

        unsafe {
            SetWindowPos(
                self.base().hwnd(),
                None,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }

        App::get().save_thread.get().unwrap().set_unsaved();
        LRESULT(0)
    }

    fn on_destroy(&self) -> LRESULT {
        let hwnd = self.base().hwnd();
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &nid);
            PostQuitMessage(0);
        }
        LRESULT(0)
    }

    fn on_window_pos_changing(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let pos = lparam.0 as *mut WINDOWPOS;
        unsafe {
            (*pos).hwndInsertAfter = HWND_BOTTOM;

            if ((*pos).flags & SWP_NOSIZE) == SET_WINDOW_POS_FLAGS(0) {
                App::get().mirror.lock().update();
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    fn on_paint(&self) -> LRESULT {
        unsafe {
            let hwnd = self.base().hwnd();
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let brush = CreateSolidBrush(COLORREF(0x00000000));
            FillRect(hdc, &ps.rcPaint, brush);
            DeleteObject(brush.into());

            EndPaint(hwnd, &ps);
        }
        LRESULT(0)
    }

    fn on_set_cursor(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let mut pt = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut pt) };
        unsafe { ScreenToClient(hwnd, &mut pt) };

        let inner = self.inner.lock();
        for fence in inner.fences.iter().rev() {
            if let Some(hit) = fence.hit_test(pt.x, pt.y) {
                let cursor_id = match hit {
                    HitTest::TitleBar => IDC_SIZEALL,
                    HitTest::Client | HitTest::Icon(_) => IDC_ARROW,
                    HitTest::Left | HitTest::Right => IDC_SIZEWE,
                    HitTest::Top | HitTest::Bottom => IDC_SIZENS,
                    HitTest::TopLeft | HitTest::BottomRight => IDC_SIZENWSE,
                    HitTest::TopRight | HitTest::BottomLeft => IDC_SIZENESW,
                };
                unsafe {
                    let cursor = LoadCursorW(None, cursor_id).unwrap_or_default();
                    SetCursor(Some(cursor));
                }
                return LRESULT(TRUE.0 as isize);
            }
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    fn on_lbutton_dblclk(&self, lparam: LPARAM) -> LRESULT {
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

        let mut inner = self.inner.lock();
        for fence in inner.fences.iter().rev() {
            if let Some(hit @ HitTest::Icon(_)) = fence.hit_test(x, y) {
                inner.hit_type = Some(hit);
                drop(inner);
                self.on_command(WPARAM(IDM_RUN_ICON));
                return LRESULT(0);
            }
        }
        LRESULT(0)
    }

    fn on_lbutton_down(&self, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

        let mut inner = self.inner.lock();
        let mut hit_idx = None;
        for (i, fence) in inner.fences.iter().enumerate().rev() {
            if let Some(hit) = fence.hit_test(x, y) {
                hit_idx = Some((i, hit));
                break;
            }
        }

        for fence in &inner.fences {
            fence.clear_selection();
        }

        if let Some((idx, hit)) = hit_idx {
            let fence = inner.fences.remove(idx);

            if let HitTest::Icon(icon_idx) = hit {
                fence.select_icon(icon_idx);
            }

            fence.base().bring_to_front();
            inner.fences.push(fence);

            match hit {
                HitTest::Client | HitTest::Icon(_) => {
                    inner.hit_type = Some(hit);
                }
                _ => {
                    inner.hit_type = Some(hit);
                    inner.last_mouse_pos = POINT { x, y };
                    unsafe {
                        SetCapture(hwnd);
                    };
                }
            }
        }
        LRESULT(0)
    }

    fn on_mouse_move(&self, lparam: LPARAM) -> LRESULT {
        let mut inner = self.inner.lock();
        if let Some(hit_type) = inner.hit_type {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            let dx = x - inner.last_mouse_pos.x;
            let dy = y - inner.last_mouse_pos.y;

            if let Some(fence) = inner.fences.last() {
                match hit_type {
                    HitTest::TitleBar => fence.base().move_by(dx, dy),
                    HitTest::Left => fence.add_area(dx, 0, -dx, 0),
                    HitTest::Right => fence.add_area(0, 0, dx, 0),
                    HitTest::Top => fence.add_area(0, dy, 0, -dy),
                    HitTest::Bottom => fence.add_area(0, 0, 0, dy),
                    HitTest::TopLeft => fence.add_area(dx, dy, -dx, -dy),
                    HitTest::TopRight => fence.add_area(0, dy, dx, -dy),
                    HitTest::BottomLeft => fence.add_area(dx, 0, -dx, dy),
                    HitTest::BottomRight => fence.add_area(0, 0, dx, dy),
                    HitTest::Client => (),
                    HitTest::Icon(_) => (),
                }
            }

            self.base.redraw();
            App::get().save_thread.get().unwrap().set_unsaved();
            inner.last_mouse_pos = POINT { x, y };
        }
        LRESULT(0)
    }

    fn on_lbutton_up(&self) -> LRESULT {
        let mut inner = self.inner.lock();
        if inner.hit_type.is_some() {
            inner.hit_type = None;
            unsafe { ReleaseCapture() };
        }
        LRESULT(0)
    }

    fn on_rbutton_up(&self, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

        let mut hit_idx = None;
        {
            let inner = self.inner.lock();
            for (i, fence) in inner.fences.iter().enumerate().rev() {
                if let Some(hit) = fence.hit_test(x, y) {
                    hit_idx = Some((i, hit));
                    break;
                }
            }
        }

        if let Some((idx, hit)) = hit_idx {
            {
                let mut inner = self.inner.lock();
                let fence = inner.fences.remove(idx);

                fence.clear_selection();
                if let HitTest::Icon(icon_idx) = hit {
                    fence.select_icon(icon_idx);
                }

                fence.base().bring_to_front();
                inner.fences.push(fence);

                inner.hit_type = Some(hit);
            }

            let mut pt = POINT { x, y };
            unsafe { ClientToScreen(hwnd, &mut pt) };
            let h_menu = unsafe { CreatePopupMenu().unwrap_or_default() };

            unsafe {
                if let HitTest::Icon(_) = hit {
                    AppendMenuW(h_menu, MF_STRING, IDM_RUN_ICON, w!("&Run"));
                    AppendMenuW(h_menu, MF_STRING, IDM_RENAME_ICON, w!("Re&name"));
                    AppendMenuW(h_menu, MF_STRING, IDM_SET_ICON_PATH, w!("Set &path"));
                    AppendMenuW(h_menu, MF_STRING, IDM_DELETE_ICON, w!("&Delete"));
                } else {
                    AppendMenuW(h_menu, MF_STRING, IDM_IMPORT, w!("&Import"));
                    AppendMenuW(h_menu, MF_STRING, IDM_IMPORT_FROM, w!("Import &from..."));
                    let has_import_path = {
                        let inner = self.inner.lock();
                        inner
                            .fences
                            .last()
                            .map_or(false, |f| f.imported_from().is_some())
                    };
                    let open_explorer_flags = if has_import_path {
                        MF_STRING
                    } else {
                        MF_STRING | MF_GRAYED
                    };
                    AppendMenuW(
                        h_menu,
                        open_explorer_flags,
                        IDM_OPEN_EXPLORER,
                        w!("Open in Explorer"),
                    );
                    AppendMenuW(h_menu, MF_STRING, IDM_ADD_ICON, w!("Add &icon"));
                    AppendMenuW(h_menu, MF_SEPARATOR, 0, PCWSTR::null());

                    let h_sticky_menu = CreatePopupMenu().unwrap_or_default();
                    let current_sticky = self.inner.lock().fences.last().and_then(|f| f.sticky());

                    let checky_sticky = |pos: Option<FenceStickyPosition>| {
                        if current_sticky == pos {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        }
                    };

                    AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(None),
                        IDM_STICKY_NONE,
                        w!("None"),
                    );
                    AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::TopLeft)),
                        IDM_STICKY_TOPLEFT,
                        w!("Top Left"),
                    );
                    AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::TopRight)),
                        IDM_STICKY_TOPRIGHT,
                        w!("Top Right"),
                    );
                    AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::BottomLeft)),
                        IDM_STICKY_BOTTOMLEFT,
                        w!("Bottom Left"),
                    );
                    AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::BottomRight)),
                        IDM_STICKY_BOTTOMRIGHT,
                        w!("Bottom Right"),
                    );

                    AppendMenuW(
                        h_menu,
                        MF_POPUP,
                        h_sticky_menu.0 as usize,
                        w!("Sticky position"),
                    );

                    AppendMenuW(h_menu, MF_STRING, IDM_RENAME_FENCE, w!("Re&name fence"));
                    AppendMenuW(h_menu, MF_STRING, IDM_DELETE_FENCE, w!("&Delete fence"));
                }
                SetForegroundWindow(hwnd);
                TrackPopupMenu(
                    h_menu,
                    TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                    pt.x,
                    pt.y,
                    Some(0),
                    hwnd,
                    None,
                );
                DestroyMenu(h_menu);
            }
        }
        LRESULT(0)
    }

    fn on_shell_icon(&self, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        if lparam.0 as u32 == WM_RBUTTONUP || lparam.0 as u32 == WM_LBUTTONUP {
            let mut pt = POINT { x: 0, y: 0 };
            unsafe { GetCursorPos(&mut pt) };
            let h_menu = unsafe { CreatePopupMenu().unwrap_or_default() };
            unsafe {
                AppendMenuW(h_menu, MF_STRING, IDM_ADD_FENCE, w!("&Add fence"));
                AppendMenuW(
                    h_menu,
                    MF_STRING,
                    IDM_ADD_FENCE_FROM_FOLDER,
                    w!("Add fence from &folder"),
                );
                AppendMenuW(h_menu, MF_STRING, IDM_EXIT, w!("&Exit"));
                SetForegroundWindow(hwnd);
                TrackPopupMenu(
                    h_menu,
                    TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                    pt.x,
                    pt.y,
                    Some(0),
                    hwnd,
                    None,
                );
                DestroyMenu(h_menu);
            }
        }
        LRESULT(0)
    }

    fn on_command(&self, wparam: WPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let command = (wparam.0 & 0xFFFF) as u16 as usize;
        debug!("command: {}", command);
        let hit_type;
        {
            let mut inner = self.inner.lock();
            hit_type = inner.hit_type.take();
        }

        let mut should_save = false;
        match command {
            IDM_EXIT => unsafe {
                DestroyWindow(hwnd);
            },
            IDM_ADD_FENCE => {
                let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
                if let Ok(fence) = Fence::new(self, width / 2 - 150, height / 2 - 75, "Untitled") {
                    let mut inner = self.inner.lock();
                    inner.fences.push(fence);
                }
                should_save = true;
            }
            IDM_ADD_FENCE_FROM_FOLDER => {
                self.executor.spawn(self, async move {
                    debug!("IDM_ADD_FENCE_FROM_FOLDER async spawn");
                    let cover = App::get().cover.get().unwrap();
                    match Fence::from_folder_selector(&cover).await {
                        Ok(Some(fence)) => {
                            let mut inner = cover.inner.lock();
                            inner.fences.push(fence);
                            App::get().save_thread.get().unwrap().set_unsaved();
                        }
                        Err(e) => {
                            error!("Error adding fence: {:?}", e);
                        }
                        _ => (),
                    }
                });
            }
            IDM_ADD_ICON => {
                let inner = self.inner.lock();
                if let Some(fence) = inner.fences.last() {
                    let title = format!("Icon #{}", fence.icon_count());
                    fence.add_icon(&title);
                }
                should_save = true;
            }
            IDM_RENAME_FENCE => {
                let inner = self.inner.lock();
                if let Some(fence) = inner.fences.last() {
                    let fence = fence.clone();
                    let current_title = String::from(&fence.title() as &str);
                    self.executor.spawn(self, async move {
                        if let Some(new_title) =
                            prompt::input("Rename fence", "Enter new fence name:", &current_title)
                                .await
                        {
                            if !new_title.is_empty() {
                                fence.set_title(new_title.into());
                                App::get().save_thread.get().unwrap().set_unsaved();
                            }
                        }
                    });
                }
            }
            IDM_DELETE_FENCE => {
                let result = unsafe {
                    MessageBoxW(
                        Some(hwnd),
                        w!("Are you sure you want to delete this fence?"),
                        w!("Confirm Deletion"),
                        MB_YESNO | MB_ICONQUESTION,
                    )
                };
                if result == IDYES {
                    let mut inner = self.inner.lock();
                    inner.fences.pop();
                }
                should_save = true;
            }
            IDM_RUN_ICON => {
                if let Some(HitTest::Icon(icon_idx)) = hit_type {
                    let inner = self.inner.lock();
                    let icon = inner
                        .fences
                        .last()
                        .unwrap()
                        .icon_by_index(icon_idx)
                        .unwrap();
                    icon.run();
                } else {
                    error!("IDM_RUN_ICON: invalid state");
                }
            }
            IDM_RENAME_ICON => {
                if let Some(HitTest::Icon(icon_idx)) = hit_type {
                    let inner = self.inner.lock();
                    let icon = inner
                        .fences
                        .last()
                        .unwrap()
                        .icon_by_index(icon_idx)
                        .unwrap();
                    let current_title = String::from(&icon.title() as &str);
                    self.executor.spawn(self, async move {
                        if let Some(new_title) =
                            prompt::input("Rename icon", "Enter new icon name:", &current_title)
                                .await
                        {
                            if !new_title.is_empty() {
                                icon.set_title(new_title.into());
                                App::get().save_thread.get().unwrap().set_unsaved();
                            }
                        }
                    });
                } else {
                    error!("IDM_RENAME_ICON: invalid state");
                }
            }
            IDM_SET_ICON_PATH => {
                if let Some(HitTest::Icon(icon_idx)) = hit_type {
                    let inner = self.inner.lock();
                    let icon = inner
                        .fences
                        .last()
                        .unwrap()
                        .icon_by_index(icon_idx)
                        .unwrap();
                    icon.set_info_from_selector();
                } else {
                    error!("IDM_SET_ICON_PATH: invalid state");
                }
                should_save = true;
            }
            IDM_DELETE_ICON => {
                if let Some(HitTest::Icon(icon_idx)) = hit_type {
                    let inner = self.inner.lock();
                    let fence = inner.fences.last().unwrap();
                    fence.remove_icon(icon_idx);
                } else {
                    error!("IDM_DELETE_ICON: invalid state");
                }
                should_save = true;
            }
            IDM_IMPORT => {
                let inner = self.inner.lock();
                let fence = inner.fences.last().unwrap();
                if fence.imported_from().is_some() {
                    fence.show_import_existing_dialog();
                } else {
                    let fence: Arc<Fence> = fence.clone();
                    self.executor.spawn(self, async move {
                        fence.show_import_from_dialog().await;
                    });
                }
                should_save = true;
            }
            IDM_IMPORT_FROM => {
                debug!("import from");
                let fence: Arc<Fence> = self.inner.lock().fences.last().unwrap().clone();
                self.executor.spawn(self, async move {
                    fence.show_import_from_dialog().await;
                });
                should_save = true;
            }
            IDM_STICKY_NONE
            | IDM_STICKY_TOPLEFT
            | IDM_STICKY_TOPRIGHT
            | IDM_STICKY_BOTTOMLEFT
            | IDM_STICKY_BOTTOMRIGHT => {
                use crate::config::state::FenceStickyPosition;
                let sticky = match command {
                    IDM_STICKY_TOPLEFT => Some(FenceStickyPosition::TopLeft),
                    IDM_STICKY_TOPRIGHT => Some(FenceStickyPosition::TopRight),
                    IDM_STICKY_BOTTOMLEFT => Some(FenceStickyPosition::BottomLeft),
                    IDM_STICKY_BOTTOMRIGHT => Some(FenceStickyPosition::BottomRight),
                    _ => None,
                };
                let inner = self.inner.lock();
                if let Some(fence) = inner.fences.last() {
                    fence.set_sticky(sticky);
                }
                should_save = true;
            }
            IDM_OPEN_EXPLORER => {
                let fence = self.inner.lock().fences.last().cloned();
                if let Some(fence) = fence {
                    if let Some(import_path) = fence.imported_from() {
                        let path_wide: Vec<u16> = import_path
                            .encode_utf16()
                            .chain(std::iter::once(0))
                            .collect();
                        unsafe {
                            ShellExecuteW(
                                None,
                                w!("open"),
                                PCWSTR(path_wide.as_ptr()),
                                PCWSTR::null(),
                                PCWSTR::null(),
                                SW_SHOWNORMAL,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        if should_save {
            App::get().save_thread.get().unwrap().set_unsaved();
        }
        LRESULT(0)
    }
}

impl Window for DesktopCover {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_CLOSE => LRESULT(0),
            WM_DISPLAYCHANGE => self.on_display_change(),
            WM_DESTROY => self.on_destroy(),
            WM_WINDOWPOSCHANGING => self.on_window_pos_changing(msg, wparam, lparam),
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_PAINT => self.on_paint(),
            WM_SETCURSOR => self.on_set_cursor(msg, wparam, lparam),
            WM_LBUTTONDBLCLK => self.on_lbutton_dblclk(lparam),
            WM_LBUTTONDOWN => self.on_lbutton_down(lparam),
            WM_MOUSEMOVE => self.on_mouse_move(lparam),
            WM_LBUTTONUP => self.on_lbutton_up(),
            WM_RBUTTONUP => self.on_rbutton_up(lparam),
            WM_USER_SHELLICON => self.on_shell_icon(lparam),
            WM_USER_WAKE_FUTURE => {
                self.executor.poll_all(self);
                LRESULT(0)
            }
            WM_COMMAND => self.on_command(wparam),
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
