use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, Weak};

use anyhow::{anyhow, Result};
use windows_sys::Win32::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE,
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
        // TODO: retrieve the Arc<dyn Window> from userdata and call wndproc
    }
    let registered = REGISTERED_CLASSNAMES.lock().unwrap();
    if let Some(rname) = registered.get(&name) {
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
    ) -> Result<Self> {
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
                Arc::into_raw(window.clone()) as *const _,
            )
        };
        if hwnd.is_null() {
            return Err(anyhow!("CreateWindowExW failed"));
        }
        Ok(Self { hwnd, window })
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
    fn base<'a>(&'a self) -> &'a Base;

    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}
