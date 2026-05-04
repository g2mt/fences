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
        let mut fences = App::get().fences.lock();
        fences.set_state(self, &state.fences)?;
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
        match msg {
            WM_CLOSE => LRESULT(0),
            WM_DISPLAYCHANGE => self.on_display_change(),
            WM_DESTROY => self.on_destroy(),
            WM_WINDOWPOSCHANGING => self.on_window_pos_changing(msg, wparam, lparam),
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_PAINT => self.on_paint(),
            WM_USER_SHELLICON => self.on_shell_icon(lparam),
            WM_USER_WAKE_FUTURE => {
                self.executor.poll_all();
                LRESULT(0)
            }
            WM_COMMAND => self.on_command(wparam),
            _ => unsafe { DefWindowProcW(self.base().hwnd(), msg, wparam, lparam) },
        }
    }
}
