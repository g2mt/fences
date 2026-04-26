use anyhow::Result;
use std::sync::{Arc, Mutex};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::fence::{Fence, HitTest};
use crate::window::{register_classname, Base, BaseRef, Window};

// Menus
pub const IDM_EXIT: usize = 101;
pub const IDM_ADD_FENCE: usize = 102;
pub const IDM_DELETE_FENCE: usize = 103;
pub const IDM_ADD_ICON: usize = 104;
pub const IDM_RUN_ICON: usize = 105;
pub const IDM_DELETE_ICON: usize = 106;

// Custom events
pub const WM_USER_SHELLICON: u32 = WM_USER + 1;

pub struct DesktopCover {
    base: BaseRef,
    inner: Mutex<DesktopCoverInner>,
}

struct DesktopCoverInner {
    fences: Vec<Arc<Fence>>,
    hit_type: Option<HitTest>,
    last_mouse_pos: POINT,
    context_target: Option<(usize, HitTest)>,
}

impl DesktopCover {
    pub fn new() -> Result<Arc<Self>> {
        let h_instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };

        Base::create_window(
            WS_EX_NOACTIVATE | WS_EX_LAYERED,
            register_classname(w!("BottomWindowClass")),
            w!("Desktop Cover"),
            WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN,
            0,
            0,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            h_instance,
            |base| {
                let hwnd = base.handle();
                unsafe {
                    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                    nid.hWnd = hwnd;
                    nid.uID = 1;
                    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
                    nid.uCallbackMessage = WM_USER_SHELLICON;
                    nid.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
                    let tip: Vec<u16> = "Desktop Cover"
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect();
                    let len = tip.len().min(nid.szTip.len());
                    nid.szTip[..len].copy_from_slice(&tip[..len]);
                    Shell_NotifyIconW(NIM_ADD, &nid);

                    SetLayeredWindowAttributes(hwnd, 0x00000000, 0, LWA_COLORKEY);
                    SetWindowPos(
                        hwnd,
                        HWND_BOTTOM,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }

                Ok(Arc::new(Self {
                    base,
                    inner: Mutex::new(DesktopCoverInner {
                        fences: Vec::new(),
                        hit_type: None,
                        last_mouse_pos: POINT { x: 0, y: 0 },
                        context_target: None,
                    }),
                }))
            },
        )
    }
}

impl Window for DesktopCover {
    fn base<'a>(&'a self) -> &'a BaseRef {
        &self.base
    }

    fn wndproc(&self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.base().handle();
        match msg {
            WM_CLOSE => 0,
            WM_DESTROY => {
                let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = 1;
                unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
                unsafe { PostQuitMessage(0) };
                0
            }
            WM_WINDOWPOSCHANGING => {
                let pos = lparam as *mut WINDOWPOS;
                unsafe {
                    (*pos).hwndInsertAfter = HWND_BOTTOM;
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_PAINT => {
                let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
                unsafe {
                    BeginPaint(hwnd, &mut ps);
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_SETCURSOR => {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe { GetCursorPos(&mut pt) };
                unsafe { ScreenToClient(hwnd, &mut pt) };

                let inner = self.inner.lock().unwrap();
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
                            let cursor = LoadCursorW(std::ptr::null_mut(), cursor_id);
                            SetCursor(cursor);
                        }
                        return TRUE as LRESULT;
                    }
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_LBUTTONDBLCLK => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                let inner = self.inner.lock().unwrap();
                for fence in inner.fences.iter().rev() {
                    if let Some(HitTest::Icon(_)) = fence.hit_test(x, y) {
                        unsafe {
                            MessageBoxW(
                                hwnd,
                                w!("Clicked"),
                                w!("Test"),
                                MB_OK | MB_ICONINFORMATION,
                            );
                        }
                        break;
                    }
                }
                0
            }
            WM_LBUTTONDOWN => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                let mut inner = self.inner.lock().unwrap();
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

                    fence.bring_to_front();
                    inner.fences.push(fence);

                    match hit {
                        HitTest::Client | HitTest::Icon(_) => {
                            inner.hit_type = None;
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
                0
            }
            WM_MOUSEMOVE => {
                let mut inner = self.inner.lock().unwrap();
                if let Some(hit_type) = inner.hit_type {
                    let x = (lparam & 0xFFFF) as i16 as i32;
                    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                    let dx = x - inner.last_mouse_pos.x;
                    let dy = y - inner.last_mouse_pos.y;

                    if let Some(fence) = inner.fences.last() {
                        match hit_type {
                            HitTest::TitleBar => fence.move_by(dx, dy),
                            HitTest::Left => fence.resize(-dx, 0, dx, 0),
                            HitTest::Right => fence.resize(0, 0, dx, 0),
                            HitTest::Top => fence.resize(0, -dy, 0, dy),
                            HitTest::Bottom => fence.resize(0, 0, 0, dy),
                            HitTest::TopLeft => fence.resize(-dx, -dy, dx, dy),
                            HitTest::TopRight => fence.resize(0, -dy, dx, dy),
                            HitTest::BottomLeft => fence.resize(-dx, 0, dx, dy),
                            HitTest::BottomRight => fence.resize(0, 0, dx, dy),
                            _ => {}
                        }
                    }

                    inner.last_mouse_pos = POINT { x, y };
                }
                0
            }
            WM_LBUTTONUP => {
                let mut inner = self.inner.lock().unwrap();
                if inner.hit_type.is_some() {
                    inner.hit_type = None;
                    unsafe { ReleaseCapture() };
                }
                0
            }
            WM_RBUTTONUP => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                let mut inner = self.inner.lock().unwrap();
                let mut hit_idx = None;
                for (i, fence) in inner.fences.iter().enumerate().rev() {
                    if let Some(hit) = fence.hit_test(x, y) {
                        hit_idx = Some((i, hit));
                        break;
                    }
                }

                if let Some((idx, hit)) = hit_idx {
                    let fence = inner.fences.remove(idx);

                    fence.clear_selection();
                    if let HitTest::Icon(icon_idx) = hit {
                        fence.select_icon(icon_idx);
                    }

                    fence.bring_to_front();
                    inner.fences.push(fence);

                    let mut pt = POINT { x, y };
                    unsafe { ClientToScreen(hwnd, &mut pt) };
                    let h_menu = unsafe { CreatePopupMenu() };

                    inner.context_target = Some((inner.fences.len() - 1, hit));

                    unsafe {
                        if let HitTest::Icon(_) = hit {
                            AppendMenuW(h_menu, MF_STRING, IDM_RUN_ICON, w!("&Run"));
                            AppendMenuW(h_menu, MF_STRING, IDM_DELETE_ICON, w!("&Delete"));
                        } else {
                            AppendMenuW(h_menu, MF_STRING, IDM_ADD_ICON, w!("Add &icon"));
                            AppendMenuW(h_menu, MF_STRING, IDM_DELETE_FENCE, w!("&Delete fence"));
                        }
                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(
                            h_menu,
                            TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                            pt.x,
                            pt.y,
                            0,
                            hwnd,
                            std::ptr::null(),
                        );
                        DestroyMenu(h_menu);
                    }
                }
                0
            }
            WM_USER_SHELLICON => {
                if lparam as u32 == WM_RBUTTONUP || lparam as u32 == WM_LBUTTONUP {
                    let mut pt = POINT { x: 0, y: 0 };
                    unsafe { GetCursorPos(&mut pt) };
                    let h_menu = unsafe { CreatePopupMenu() };
                    unsafe {
                        AppendMenuW(h_menu, MF_STRING, IDM_ADD_FENCE, w!("&Add fence"));
                        AppendMenuW(h_menu, MF_STRING, IDM_EXIT, w!("&Exit"));
                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(
                            h_menu,
                            TPM_LEFTALIGN | TPM_RIGHTBUTTON,
                            pt.x,
                            pt.y,
                            0,
                            hwnd,
                            std::ptr::null(),
                        );
                        DestroyMenu(h_menu);
                    }
                }
                0
            }
            WM_COMMAND => {
                let mut inner = self.inner.lock().unwrap();
                match wparam & 0xFFFF {
                    IDM_EXIT => unsafe {
                        DestroyWindow(hwnd);
                    },
                    IDM_ADD_FENCE => {
                        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
                        if let Ok(fence) = Fence::new(hwnd, width / 2 - 150, height / 2 - 75) {
                            inner.fences.push(fence);
                        }
                    }
                    IDM_ADD_ICON => {
                        if let Some((fence_idx, _)) = inner.context_target {
                            if let Some(fence) = inner.fences.get(fence_idx) {
                                let title = format!("Icon #{}", fence.icon_count());
                                fence.add_icon(&title);
                            }
                        }
                    }
                    IDM_DELETE_FENCE => {
                        if let Some((fence_idx, _)) = inner.context_target {
                            let result = unsafe {
                                MessageBoxW(
                                    hwnd,
                                    w!("Are you sure you want to delete this fence?"),
                                    w!("Confirm Deletion"),
                                    MB_YESNO | MB_ICONQUESTION,
                                )
                            };
                            if result == IDYES {
                                if fence_idx < inner.fences.len() {
                                    inner.fences.remove(fence_idx);
                                }
                            }
                        }
                    }
                    IDM_RUN_ICON => unsafe {
                        MessageBoxW(hwnd, w!("Clicked"), w!("Test"), MB_OK | MB_ICONINFORMATION);
                    },
                    IDM_DELETE_ICON => {
                        if let Some((fence_idx, HitTest::Icon(icon_idx))) = inner.context_target {
                            if let Some(fence) = inner.fences.get(fence_idx) {
                                fence.remove_icon(icon_idx);
                            }
                        }
                    }
                    _ => {}
                }
                inner.context_target = None;
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
