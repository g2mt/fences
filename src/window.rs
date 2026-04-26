use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, Weak};

use anyhow::{anyhow, Result};
use windows_sys::Win32::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, SetWindowLongPtrW,
    CREATESTRUCTW, GWLP_USERDATA, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE, WM_NCCREATE,
};

// Class names

static REGISTERED_CLASSNAMES: LazyLock<Mutex<HashSet<Rc<PCWSTR>>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));
pub struct ClassName(Rc<PCWSTR>);
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
            let base = &*(userdata as *const Base);
            let mut window = Arc::as_ptr(&base.window) as *mut dyn Window;
            (*window).wndproc(msg, wparam, lparam)
        }
    }
    let mut registered = REGISTERED_CLASSNAMES.lock().unwrap();
    if let Some(rname) = registered.get(&name as &PCWSTR) {
        return ClassName(rname.clone());
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
    let rname = Rc::new(name);
    registered.insert(rname.clone());
    ClassName(rname)
}

// Base

pub struct Base {
    hwnd: HWND,
    window: Arc<dyn Window>,
}

unsafe impl Sync for Base {}

pub type BaseRef = Pin<Box<Base>>; // pinned for win32

impl Base {
    pub unsafe fn create_window(
        window: Arc<dyn Window>,
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
            window,
        }));
        let hwnd = unsafe {
            CreateWindowExW(
                dwexstyle,
                lpclassname,
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
        Ok(self_ref)
    }

    pub fn handle(&self) -> HWND {
        self.hwnd
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

pub trait Window: Send + Sync + 'static {
    fn base<'a>(&'a self) -> &'a ;

    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}
