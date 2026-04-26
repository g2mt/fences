use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, Weak};

use anyhow::{anyhow, Result};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

// Class names

static REGISTERED_CLASSNAMES: LazyLock<Mutex<HashSet<ClassName>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ClassName(Arc<PCWSTR>);
unsafe impl Send for ClassName {}

pub fn register_classname(name: PCWSTR) -> ClassName {
    pub unsafe extern "system" fn base_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            if msg == WM_NCCREATE {
                let cs = lparam as *const CREATESTRUCTW;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*cs).lpCreateParams as isize);
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let userdata = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if userdata == 0 {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let base = &*(userdata as *const () as *const Base);
            if let Some(window) = base.window.get().map(|w| w.upgrade()).flatten() {
                window.wndproc(msg, wparam, lparam)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
    let mut registered: MutexGuard<'_, HashSet<ClassName>> = REGISTERED_CLASSNAMES.lock().unwrap();
    let class_name = ClassName(Arc::new(name));
    if let Some(existing) = registered.get(&class_name) {
        return existing.clone();
    }
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.hInstance = h_instance;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        wc.lpszClassName = name;
        wc.lpfnWndProc = Some(base_wndproc);
        RegisterClassW(&wc);
    }
    registered.insert(class_name.clone());
    class_name
}

// Base

pub struct Base {
    hwnd: HWND,
    window: OnceLock<Weak<dyn Window>>,
    rect: Mutex<RECT>,
}

unsafe impl Sync for Base {}

pub type BaseRef = Pin<Box<Base>>; // pinned for win32

impl Base {
    pub fn create_window<W, F>(
        dwexstyle: WINDOW_EX_STYLE,
        classname: ClassName,
        lpwindowname: PCWSTR,
        dwstyle: WINDOW_STYLE,
        x: i32,
        y: i32,
        nwidth: i32,
        nheight: i32,
        hwndparent: HWND,
        hmenu: HMENU,
        hinstance: HINSTANCE,
        f: F,
    ) -> Result<Arc<W>>
    where
        F: FnOnce(BaseRef) -> Result<Arc<W>>,
        W: Window,
    {
        let base = unsafe {
            Self::create_window_uninit(
                dwexstyle,
                classname,
                lpwindowname,
                dwstyle,
                x,
                y,
                nwidth,
                nheight,
                hwndparent,
                hmenu,
                hinstance,
            )?
        };
        let window = f(base)?;
        let window_weak: Weak<dyn Window> = Arc::downgrade(&(window.clone() as Arc<dyn Window>));
        window.base().window.get_or_init(|| window_weak);
        Ok(window)
    }

    unsafe fn create_window_uninit(
        dwexstyle: WINDOW_EX_STYLE,
        classname: ClassName,
        lpwindowname: PCWSTR,
        dwstyle: WINDOW_STYLE,
        x: i32,
        y: i32,
        nwidth: i32,
        nheight: i32,
        hwndparent: HWND,
        hmenu: HMENU,
        hinstance: HINSTANCE,
    ) -> Result<BaseRef> {
        let mut self_ref = Box::into_pin(Box::new(Self {
            hwnd: std::ptr::null_mut(),
            window: OnceLock::new(),
            rect: Mutex::new(RECT {
                left: x,
                top: y,
                right: x + nwidth,
                bottom: y + nheight,
            }),
        }));
        let hwnd = unsafe {
            CreateWindowExW(
                dwexstyle,
                *(classname.0),
                lpwindowname,
                dwstyle,
                x,
                y,
                nwidth,
                nheight,
                hwndparent,
                hmenu,
                hinstance,
                &*self_ref as *const Base as *const _,
            )
        };
        if hwnd.is_null() {
            return Err(anyhow!("CreateWindowExW failed"));
        }
        unsafe {
            let mut_ref = Pin::get_unchecked_mut(self_ref.as_mut());
            mut_ref.hwnd = hwnd;
        }
        Ok(self_ref)
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
    }

    pub fn rect(&self) -> RECT {
        *self.rect.lock().unwrap()
    }

    fn set_window_pos(&self) {
        let rect = self.rect.lock().unwrap();
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn resize_to(&self, left: i32, top: i32, right: i32, bottom: i32) {
        *self.rect.lock().unwrap() = RECT {
            left,
            top,
            right,
            bottom,
        };
        self.set_window_pos();
    }

    pub fn resize_by(&self, dl: i32, dt: i32, dr: i32, db: i32) {
        {
            let mut rect = self.rect.lock().unwrap();
            rect.left += dl;
            rect.top += dt;
            rect.right += dr;
            rect.bottom += db;
        }
        self.set_window_pos();
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        unsafe {
            if self.hwnd != std::ptr::null_mut() {
                DestroyWindow(self.hwnd);
            }
            self.hwnd = std::ptr::null_mut();
        }
    }
}

unsafe impl Send for Base {}

// Window

pub trait Window: Send + Sync + 'static {
    /// Returns the BaseRef contained in the Window struct
    fn base<'a>(&'a self) -> &'a BaseRef;

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}
