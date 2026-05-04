use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use tracing::{debug, error, info};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::app::App;
use crate::commands::*;
use crate::config::state::{AppState, FenceStickyPosition};
use crate::fence::{Fence, HitType};
use crate::fut::AsyncExecutor;
use crate::utils::HWNDWrapper;
use crate::window::{register_classname, Base, BaseRef, Window};

// Custom events
pub const WM_USER_SHELLICON: u32 = WM_USER + 1;
pub const WM_USER_WAKE_FUTURE: u32 = WM_USER + 2;

pub struct DesktopCover {
    base: BaseRef,
    last_mouse_pos: Mutex<POINT>,
    executor: AsyncExecutor,
}

impl DesktopCover {
    pub fn new() -> Result<Arc<Self>> {
        let hinstance = unsafe { GetModuleHandleW(None).unwrap_or_default() };

        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        {
            let app = App::get();
            let bounds = app.screen_bounds();
            bounds.width.store(width, Ordering::Relaxed);
            bounds.height.store(height, Ordering::Relaxed);
            app.hwnd_shell.get_or_init(|| unsafe {
                // https://stackoverflow.com/a/32589338
                let progman = FindWindowW(w!("Progman"), PCWSTR::null()).unwrap_or_default();
                HWNDWrapper(
                    FindWindowExW(
                        Some(progman),
                        Some(HWND::default()),
                        w!("SHELLDLL_DefView"),
                        PCWSTR::null(),
                    )
                    .unwrap_or_default(),
                )
            });
        }

        Base::create_window(
            WS_EX_NOACTIVATE | WS_EX_LAYERED,
            register_classname("BottomWindowClass"),
            w!("Desktop Cover"),
            WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN,
            0,
            0,
            width,
            height,
            App::get().hwnd_shell.get().unwrap().0,
            None,
            hinstance.into(),
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
                    nid.hIcon = LoadIconW(Some(hinstance.into()), PCWSTR(1usize as *const u16))
                        .or_else(|_| LoadIconW(None, IDI_APPLICATION))
                        .unwrap_or_default();
                    let tip: Vec<u16> = "Desktop Cover"
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let len = tip.len().min(nid.szTip.len());
                    nid.szTip[..len].copy_from_slice(&tip[..len]);
                    let _ = Shell_NotifyIconW(NIM_ADD, &nid);

                    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0x00000000), 0, LWA_COLORKEY);

                    let _ = SetWindowPos(
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
                    last_mouse_pos: Mutex::new(POINT { x: 0, y: 0 }),
                    executor: AsyncExecutor::new(HWNDWrapper(hwnd)),
                });
                /*
                #[cfg(feature = "use-UpdateLayeredWindow")]
                {
                    cover.paint_with_alpha();
                } */
                Ok(cover)
            },
        )
    }

    pub fn executor(&self) -> &AsyncExecutor {
        &self.executor
    }

    pub fn state(&self) -> AppState {
        let fences = App::get().fences.lock();
        let bounds = App::get().screen_bounds();
        AppState {
            fences: fences.items().iter().map(|f| f.get_state()).collect(),
            screen_width: bounds.width.load(Ordering::Relaxed),
            screen_height: bounds.height.load(Ordering::Relaxed),
        }
    }

    pub fn set_state(&self, state: &AppState) -> Result<()> {
        let mut fences = Vec::new();
        App::get().fences.lock();
        fences.set_state(self, &state.fences);
        fences.rearrange(state.screen_width, state.screen_height);
        Ok(())
    }

    fn on_display_change(&self) -> LRESULT {
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        info!("Screen resolution changed to {}x{}", width, height);

        let bounds = App::get().screen_bounds();
        let old_width = bounds.width.swap(width, Ordering::Relaxed);
        let old_height = bounds.height.swap(height, Ordering::Relaxed);
        App::get().fences.lock().rearrange(old_width, old_height);

        unsafe {
            let _ = SetWindowPos(
                self.base().hwnd(),
                None,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }

        /*
        #[cfg(feature = "use-UpdateLayeredWindow")]
        {
            self.paint_with_alpha();
        } */

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
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
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
            let _ = FillRect(hdc, &ps.rcPaint, brush);
            let _ = DeleteObject(brush.into());

            let _ = EndPaint(hwnd, &ps);
        }
        LRESULT(0)
    }

    fn on_set_cursor(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let mut pt = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut pt);
        };
        unsafe {
            let _ = ScreenToClient(hwnd, &mut pt);
        };

        let fences = App::get().fences.lock();
        for fence in fences.items().iter().rev() {
            if let Some(hit) = fence.hit_test(pt.x, pt.y) {
                let cursor_id = match hit {
                    HitType::TitleBar => IDC_SIZEALL,
                    HitType::Client | HitType::Icon(_) => IDC_ARROW,
                    HitType::Left | HitType::Right => IDC_SIZEWE,
                    HitType::Top | HitType::Bottom => IDC_SIZENS,
                    HitType::TopLeft | HitType::BottomRight => IDC_SIZENWSE,
                    HitType::TopRight | HitType::BottomLeft => IDC_SIZENESW,
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

        let mut fences = App::get().fences.lock();
        for fence in fences.items().iter().rev() {
            if let Some(hit @ HitType::Icon(_)) = fence.hit_test(x, y) {
                fences.hit_type = Some(hit);
                drop(fences);
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

        let mut fences = App::get().fences.lock();
        let mut hit_idx = None;
        for (i, fence) in fences.items().iter().enumerate().rev() {
            if let Some(hit) = fence.hit_test(x, y) {
                hit_idx = Some((i, hit));
                break;
            }
        }

        for fence in &fences.items() {
            fence.clear_selection();
        }

        if let Some((idx, hit)) = hit_idx {
            let fence = fences.items.remove(idx);

            if let HitType::Icon(icon_idx) = hit {
                fence.select_icon(icon_idx);
            }

            fence.base().bring_to_front();
            fences.items.push(fence);

            match hit {
                HitType::Client | HitType::Icon(_) => {
                    fences.hit_type = Some(hit);
                }
                _ => {
                    fences.hit_type = Some(hit);
                    let mut last = self.last_mouse_pos.lock();
                    *last = POINT { x, y };
                    drop(last);
                    unsafe {
                        SetCapture(hwnd);
                    };
                }
            }
        }
        LRESULT(0)
    }

    fn on_mouse_move(&self, lparam: LPARAM) -> LRESULT {
        let mut fences = App::get().fences.lock();
        if let Some(hit_type) = fences.hit_type() {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            let mut last = self.last_mouse_pos.lock();
            let dx = x - last.x;
            let dy = y - last.y;

            if let Some(fence) = fences.items().last() {
                match hit_type {
                    HitType::TitleBar => fence.base().move_by(dx, dy),
                    HitType::Left => fence.add_area(dx, 0, -dx, 0),
                    HitType::Right => fence.add_area(0, 0, dx, 0),
                    HitType::Top => fence.add_area(0, dy, 0, -dy),
                    HitType::Bottom => fence.add_area(0, 0, 0, dy),
                    HitType::TopLeft => fence.add_area(dx, dy, -dx, -dy),
                    HitType::TopRight => fence.add_area(0, dy, dx, -dy),
                    HitType::BottomLeft => fence.add_area(dx, 0, -dx, dy),
                    HitType::BottomRight => fence.add_area(0, 0, dx, dy),
                    HitType::Client => (),
                    HitType::Icon(_) => (),
                }
            }

            self.base.redraw();
            App::get().save_thread.get().unwrap().set_unsaved();
            *last = POINT { x, y };
        }
        LRESULT(0)
    }

    fn on_lbutton_up(&self) -> LRESULT {
        let mut fences = App::get().fences.lock();
        if fences.release_hit_type().is_some() {
            unsafe {
                let _ = ReleaseCapture();
            };
        }
        LRESULT(0)
    }

    fn on_rbutton_up(&self, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let x = (lparam.0 & 0xFFFF) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

        let mut hit_idx = None;
        {
            let fences = App::get().fences.lock();
            for (i, fence) in fences.items().iter().enumerate().rev() {
                if let Some(hit) = fence.hit_test(x, y) {
                    hit_idx = Some((i, hit));
                    break;
                }
            }
        }

        if let Some((idx, hit)) = hit_idx {
            let mut fences = App::get().fences.lock();
            let fence = fences.items.remove(idx);

            fence.clear_selection();
            if let HitType::Icon(icon_idx) = hit {
                fence.select_icon(icon_idx);
            }

            fence.base().bring_to_front();
            fences.items.push(fence.clone());

            fences.hit_type = Some(hit);
            drop(fences);

            let mut pt = POINT { x, y };
            unsafe {
                let _ = ClientToScreen(hwnd, &mut pt);
            };
            let h_menu = unsafe { CreatePopupMenu().unwrap_or_default() };

            unsafe {
                if let HitType::Icon(_) = hit {
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_RUN_ICON, w!("&Run"));
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_RENAME_ICON, w!("Re&name"));
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_SET_ICON_PATH, w!("Set &path"));
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_DELETE_ICON, w!("&Delete"));
                } else {
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_IMPORT, w!("&Import"));
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_IMPORT_FROM, w!("Import &from..."));
                    let open_explorer_flags = if fence.imported_from().is_some() {
                        MF_STRING
                    } else {
                        MF_STRING | MF_GRAYED
                    };
                    let _ = AppendMenuW(
                        h_menu,
                        open_explorer_flags,
                        IDM_OPEN_EXPLORER,
                        w!("Open in Explorer"),
                    );
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_ADD_ICON, w!("Add &icon"));
                    let _ = AppendMenuW(h_menu, MF_SEPARATOR, 0, PCWSTR::null());

                    let h_sticky_menu = CreatePopupMenu().unwrap_or_default();
                    let checky_sticky = |pos: Option<FenceStickyPosition>| {
                        if fence.sticky() == pos {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        }
                    };

                    let _ = AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(None),
                        IDM_STICKY_NONE,
                        w!("None"),
                    );
                    let _ = AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::TopLeft)),
                        IDM_STICKY_TOPLEFT,
                        w!("Top Left"),
                    );
                    let _ = AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::TopRight)),
                        IDM_STICKY_TOPRIGHT,
                        w!("Top Right"),
                    );
                    let _ = AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::BottomLeft)),
                        IDM_STICKY_BOTTOMLEFT,
                        w!("Bottom Left"),
                    );
                    let _ = AppendMenuW(
                        h_sticky_menu,
                        MF_STRING | checky_sticky(Some(FenceStickyPosition::BottomRight)),
                        IDM_STICKY_BOTTOMRIGHT,
                        w!("Bottom Right"),
                    );

                    let _ = AppendMenuW(
                        h_menu,
                        MF_POPUP,
                        h_sticky_menu.0 as usize,
                        w!("Sticky position"),
                    );

                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_RENAME_FENCE, w!("Re&name fence"));
                    let _ = AppendMenuW(h_menu, MF_STRING, IDM_DELETE_FENCE, w!("&Delete fence"));
                }
                let _ = SetForegroundWindow(hwnd);
                let _ = TrackPopupMenu(
                    h_menu,
                    TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                    pt.x,
                    pt.y,
                    Some(0),
                    hwnd,
                    None,
                );
                let _ = DestroyMenu(h_menu);
            }
        }
        LRESULT(0)
    }

    fn on_shell_icon(&self, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        if lparam.0 as u32 == WM_RBUTTONUP || lparam.0 as u32 == WM_LBUTTONUP {
            let mut pt = POINT { x: 0, y: 0 };
            unsafe {
                let _ = GetCursorPos(&mut pt);
            };
            let h_menu = unsafe { CreatePopupMenu().unwrap_or_default() };
            unsafe {
                let _ = AppendMenuW(h_menu, MF_STRING, IDM_ADD_FENCE, w!("&Add fence"));
                let _ = AppendMenuW(
                    h_menu,
                    MF_STRING,
                    IDM_ADD_FENCE_FROM_FOLDER,
                    w!("Add fence from &folder"),
                );
                let _ = AppendMenuW(h_menu, MF_STRING, IDM_RELOAD, w!("&Reload"));
                let _ = AppendMenuW(h_menu, MF_STRING, IDM_EXIT, w!("&Exit"));
                let _ = SetForegroundWindow(hwnd);
                let _ = TrackPopupMenu(
                    h_menu,
                    TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                    pt.x,
                    pt.y,
                    Some(0),
                    hwnd,
                    None,
                );
                let _ = DestroyMenu(h_menu);
            }
        }
        LRESULT(0)
    }

    fn trigger_fence_command(&self, command: usize, hit_type: HitType) -> bool {
        let fences = App::get().fences.lock();
        if let Some(fence) = fences.items().last() {
            fence.on_command(self, command, hit_type)
        } else {
            false
        }
    }

    fn on_command(&self, wparam: WPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        let command = (wparam.0 & 0xFFFF) as u16 as usize;
        debug!("command: {}", command);

        let mut should_save = false;
        match command {
            IDM_EXIT => unsafe {
                let _ = DestroyWindow(hwnd);
            },
            IDM_ADD_FENCE => {
                let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
                match Fence::new(self, width / 2 - 150, height / 2 - 75, "Untitled") {
                    Ok(fence) => App::get().fences.lock().add(fence),
                    Err(err) => error!("spawning fence: {:?}", err),
                }
                should_save = true;
            }
            IDM_ADD_FENCE_FROM_FOLDER => {
                self.executor.spawn(async move {
                    debug!("IDM_ADD_FENCE_FROM_FOLDER async spawn");
                    let cover = App::get().cover.get().unwrap();
                    match Fence::from_folder_selector(&cover).await {
                        Ok(Some(fence)) => {
                            let app = App::get();
                            app.fences.lock().add(fence);
                            app.save_thread.get().unwrap().set_unsaved();
                        }
                        Err(e) => {
                            error!("Error adding fence: {:?}", e);
                        }
                        _ => (),
                    }
                });
            }
            IDM_ADD_ICON
            | IDM_RENAME_FENCE
            | IDM_DELETE_FENCE
            | IDM_RUN_ICON
            | IDM_RENAME_ICON
            | IDM_SET_ICON_PATH
            | IDM_DELETE_ICON
            | IDM_IMPORT
            | IDM_IMPORT_FROM
            | IDM_STICKY_NONE
            | IDM_STICKY_TOPLEFT
            | IDM_STICKY_TOPRIGHT
            | IDM_STICKY_BOTTOMLEFT
            | IDM_STICKY_BOTTOMRIGHT
            | IDM_OPEN_EXPLORER => {
                if let Some(hit_type) = App::get().fences.lock().release_hit_type() {
                    should_save = self.trigger_fence_command(command, hit_type);
                } else {
                    error!("command {} expects hit type", command);
                }
            }
            IDM_RELOAD => {
                // Spawn a new instance of the same executable
                let exe = std::env::current_exe().expect("failed to get current exe path");
                let _ = Command::new(exe).spawn();
                // Close this instance
                unsafe {
                    let _ = DestroyWindow(hwnd);
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
                self.executor.poll_all();
                LRESULT(0)
            }
            WM_COMMAND => self.on_command(wparam),
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
