use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing::{error, info};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

use crate::app::App;
use crate::commands::*;
use crate::config::state::IconState;
use crate::mutex::Mutex;
use crate::window::{Base, BaseRef, Window, register_classname};

pub struct Icon {
    base: BaseRef,
    state: Mutex<IconState>,
    selected: AtomicBool,
}

impl Icon {
    pub fn new(parent_hwnd: HWND, title: &str, path: Option<&str>, x: i32, y: i32) -> Arc<Self> {
        let hinstance = unsafe { GetWindowLongPtrW(parent_hwnd, GWLP_HINSTANCE) as HINSTANCE };
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();

        let state = Mutex::new(IconState {
            title: Arc::from(title),
            path: path.map(|s| Arc::from(s)),
        });

        let icon_size = App::config().icon.size;

        Base::create_window(
            0,
            register_classname("FenceIcon"),
            title_u16.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            x,
            y,
            icon_size,
            icon_size,
            parent_hwnd,
            None,
            hinstance,
            |base| {
                Ok(Arc::new(Self {
                    base,
                    state,
                    selected: AtomicBool::new(false),
                }))
            },
        )
        .expect("Failed to create Icon window")
    }

    pub fn set_selected(&self, selected: bool) {
        self.selected.store(selected, Ordering::SeqCst);
        unsafe {
            let _ = InvalidateRect(self.base.hwnd(), std::ptr::null(), 1);
        }
    }

    pub fn contains_point(&self, rel_x: i32, rel_y: i32) -> bool {
        let rect = self.base.rect();
        rel_x >= rect.left && rel_x < rect.right && rel_y >= rect.top && rel_y < rect.bottom
    }

    pub fn title(&self) -> Arc<str> {
        self.state.lock().title.clone()
    }

    pub fn set_title(&self, title: Arc<str>) {
        {
            let mut s = self.state.lock();
            s.title = title.clone();
        }
        let hwnd = self.base.hwnd();
        let title_u16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let _ = SetWindowTextW(hwnd, title_u16.as_ptr());
        }
        self.base.redraw(true);
    }

    pub fn path(&self) -> Option<Arc<str>> {
        self.state.lock().path.as_ref().map(|arc| arc.clone())
    }

    pub fn set_path(&self, path: Option<Arc<str>>) {
        let _ = std::mem::replace(&mut self.state.lock().path, path);
        self.base.redraw(true);
    }

    #[cfg(windows)]
    pub fn run(&self) {
        use std::os::windows::process::CommandExt;

        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

        if let Some(path) = self.path() {
            info!("Running {}", path);
            let mut command = Command::new("cmd");
            command.args(["/C", &path]);
            command.creation_flags(CREATE_NO_WINDOW);
            let _ = command.spawn();
        } else {
            error!("No path specified for {}", self.title());
        }
    }

    fn paint(&self, hdc: HDC) {
        let hwnd = self.base().hwnd();
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        let _ = unsafe { GetClientRect(hwnd, &mut rect) };

        let mut pt = POINT { x: 0, y: 0 };
        let _ = unsafe { ClientToScreen(hwnd, &mut pt) };

        let config = App::config();
        let selected = self.selected.load(Ordering::SeqCst);

        let bg_color = if selected {
            config.icon.selected_bg_color
        } else {
            config.icon.unselected_bg_color
        };
        if !config.use_layered_window && bg_color.a() < 255 {
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
            unsafe {
                config.fence.fence_bg_color.paint_background(hdc, &rect);
            }
        }
        unsafe {
            bg_color.paint_background(hdc, &rect);
        }

        let icon_draw_size = config.icon.icon_size_draw;
        let width = rect.right - rect.left;

        let state = self.state.lock();
        let path = state.path.clone();

        let mut hicon = HICON::default();
        if let Some(ref path) = path {
            let path_u16: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let mut shfi: SHFILEINFOW = unsafe { std::mem::zeroed() };
            unsafe {
                SHGetFileInfoW(
                    path_u16.as_ptr(),
                    0,
                    &mut shfi,
                    std::mem::size_of::<SHFILEINFOW>() as u32,
                    SHGFI_ICON | SHGFI_LARGEICON,
                );
            }
            hicon = shfi.hIcon;
        }

        if hicon == std::ptr::null_mut() {
            hicon = unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) };
        }

        let _ = unsafe {
            DrawIconEx(
                hdc,
                (width - icon_draw_size) / 2,
                0,
                hicon,
                icon_draw_size,
                icon_draw_size,
                0,
                std::ptr::null_mut(),
                DI_NORMAL,
            )
        };

        if path.is_some() {
            let _ = unsafe { DestroyIcon(hicon) };
        }

        unsafe {
            SetBkMode(hdc, TRANSPARENT as i32);
            SetTextColor(hdc, config.icon.text_color.bgr());
        }

        let mut text_rect = rect;
        text_rect.top += icon_draw_size;
        App::get().draw_text(
            hdc,
            &state.title,
            &mut text_rect,
            DT_CENTER | DT_WORDBREAK | DT_NOPREFIX,
        );
    }

    pub fn set_info_from_selector(&self) {
        let mut file_buf = [0u16; MAX_PATH as usize];
        let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = self.base.hwnd();
        ofn.lpstrFile = file_buf.as_mut_ptr();
        ofn.nMaxFile = MAX_PATH;
        ofn.lpstrFilter = w!("All Files\0*.*\0\0");
        ofn.nFilterIndex = 1;
        ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;

        if unsafe { GetOpenFileNameW(&mut ofn) } != 0 {
            let path_str = String::from_utf16_lossy(
                &file_buf[..file_buf.iter().position(|&c| c == 0).unwrap_or(0)],
            );
            let path_stem: Option<String> = std::path::Path::new(&path_str)
                .file_stem()
                .and_then(|s| s.to_str().map(|s| s.to_string()));
            self.set_path(Some(path_str.into()));

            if let Some(name) = path_stem {
                let result = unsafe {
                    MessageBoxW(
                        self.base.hwnd(),
                        w!("Do you want to update the icon name to match the file?"),
                        w!("Update Name"),
                        MB_YESNO | MB_ICONQUESTION,
                    )
                };
                if result == IDYES {
                    self.set_title(Arc::from(name));
                }
            }
        }
    }

    /// Shows the context menu at absolute mouse position x, y
    pub fn show_context_menu(&self, x: i32, y: i32, fence_hwnd: HWND) {
        let h_menu = unsafe { CreatePopupMenu() };

        unsafe {
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_RUN_ICON, w!("&Run"));
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_RENAME_ICON, w!("Re&name"));
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_SET_ICON_PATH, w!("Set &path"));
            let _ = AppendMenuW(h_menu, MF_STRING, IDM_DELETE_ICON, w!("&Delete"));

            let _ = SetForegroundWindow(fence_hwnd);
            let _ = TrackPopupMenu(
                h_menu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                x,
                y,
                0,
                fence_hwnd,
                std::ptr::null(),
            );
            let _ = DestroyMenu(h_menu);
        }
    }
}

impl Window for Icon {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().hwnd();
        match msg {
            WM_NCHITTEST => HTTRANSPARENT as isize,
            WM_PAINT => unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                self.paint(hdc);
                let _ = EndPaint(hwnd, &ps);
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
