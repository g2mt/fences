use std::collections::HashSet;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, LazyLock, OnceLock, Weak};

use anyhow::{Result, anyhow};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

use crate::geo::Area;
use crate::mutex::{Mutex, MutexGuard};

// Class names

static REGISTERED_CLASSNAMES: LazyLock<Mutex<HashSet<ClassName>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ClassName(Arc<Vec<u16>>);
unsafe impl Send for ClassName {}

pub fn register_classname_ex(name: &str, mut wc: WNDCLASSW) -> ClassName {
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
            if let Some(window) = base.window.get().and_then(|w| w.upgrade()) {
                window.wndproc(msg, wparam, lparam)
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }
    let mut registered: MutexGuard<'_, HashSet<ClassName>> = REGISTERED_CLASSNAMES.lock();
    let name_u16: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let class_name = ClassName(Arc::new(name_u16));
    if let Some(existing) = registered.get(&class_name) {
        return existing.clone();
    }
    unsafe {
        let hinstance = GetModuleHandleW(std::ptr::null());
        wc.hInstance = hinstance;
        wc.lpszClassName = class_name.0.as_ptr();
        wc.lpfnWndProc = Some(base_wndproc);
        if RegisterClassW(&wc) == 0 {
            panic!("RegisterClassW returned 0");
        }
    }
    registered.insert(class_name.clone());
    class_name
}

pub fn register_classname(name: &str) -> ClassName {
    register_classname_ex(name, unsafe {
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.style = CS_DBLCLKS | CS_HREDRAW | CS_VREDRAW;
        wc.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        wc
    })
}

// Base

pub struct Base {
    hwnd: HWND,
    window: OnceLock<Weak<dyn Window>>,
    area: Area<AtomicI32>,
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
        hmenu: Option<HMENU>,
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
        let window_ = Arc::downgrade(&window);
        window.base().window.get_or_init(|| window_);
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
        hmenu: Option<HMENU>,
        hinstance: HINSTANCE,
    ) -> Result<BaseRef> {
        let mut self_ref = Box::into_pin(Box::new(Self {
            hwnd: HWND::default(),
            window: OnceLock::new(),
            area: Area::new(x, y, nwidth, nheight),
        }));
        let hwnd = unsafe {
            CreateWindowExW(
                dwexstyle,
                classname.0.as_ptr(),
                lpwindowname,
                dwstyle,
                x,
                y,
                nwidth,
                nheight,
                hwndparent,
                hmenu.unwrap_or(std::ptr::null_mut()),
                hinstance,
                &*self_ref as *const Base as *const _,
            )
        };
        if hwnd == std::ptr::null_mut() {
            return Err(anyhow!(
                "CreateWindowExW failed, GetLastError()={:?}",
                unsafe { GetLastError() }
            ));
        }
        unsafe {
            let mut_ref = Pin::get_unchecked_mut(self_ref.as_mut());
            mut_ref.hwnd = hwnd;
        }
        Ok(self_ref)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn redraw(&self, immediate: bool) {
        unsafe {
            let _ = InvalidateRect(self.hwnd, std::ptr::null(), 1);
            if immediate {
                let _ = UpdateWindow(self.hwnd);
            }
        }
    }

    pub fn area(&self) -> &Area<AtomicI32> {
        &self.area
    }

    pub fn rect(&self) -> RECT {
        (&self.area).into()
    }

    fn set_window_pos(&self) {
        let x = self.area.x.load(Ordering::Relaxed);
        let y = self.area.y.load(Ordering::Relaxed);
        let width = self.area.width.load(Ordering::Relaxed);
        let height = self.area.height.load(Ordering::Relaxed);
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn move_to(&self, x: i32, y: i32) {
        self.area.x.store(x, Ordering::Relaxed);
        self.area.y.store(y, Ordering::Relaxed);
        self.set_window_pos();
    }

    pub fn move_by(&self, dl: i32, dt: i32) {
        self.area.x.fetch_add(dl, Ordering::Relaxed);
        self.area.y.fetch_add(dt, Ordering::Relaxed);
        self.set_window_pos();
    }

    pub fn resize_to(&self, left: i32, top: i32, width: i32, height: i32) {
        self.area.x.store(left, Ordering::Relaxed);
        self.area.y.store(top, Ordering::Relaxed);
        self.area.width.store(width, Ordering::Relaxed);
        self.area.height.store(height, Ordering::Relaxed);
        self.set_window_pos();
    }

    pub fn add_area(&self, dl: i32, dt: i32, dw: i32, dh: i32) {
        self.area.x.fetch_add(dl, Ordering::Relaxed);
        self.area.y.fetch_add(dt, Ordering::Relaxed);
        self.area.width.fetch_add(dw, Ordering::Relaxed);
        self.area.height.fetch_add(dh, Ordering::Relaxed);
        self.set_window_pos();
    }

    pub unsafe fn resize_to_deferred(
        &self,
        left: i32,
        top: i32,
        width: i32,
        height: i32,
        hwinposinfo: HDWP,
    ) -> HDWP {
        self.area.x.store(left, Ordering::Relaxed);
        self.area.y.store(top, Ordering::Relaxed);
        self.area.width.store(width, Ordering::Relaxed);
        self.area.height.store(height, Ordering::Relaxed);
        unsafe {
            DeferWindowPos(
                hwinposinfo,
                self.hwnd,
                std::ptr::null_mut(),
                left,
                top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
    }
}

impl Drop for Base {
    fn drop(&mut self) {
        unsafe {
            if self.hwnd != HWND::default() {
                let _ = DestroyWindow(self.hwnd);
            }
            self.hwnd = HWND::default();
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
