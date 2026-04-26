use anyhow::{anyhow, Result};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::fence::Fence;
use crate::window::{WinHandle, Window};

// Menus
pub const IDM_EXIT: usize = 101;
pub const IDM_ADD_FENCE: usize = 102;

// Custom events
pub const WM_USER_SHELLICON: u32 = WM_USER + 1;

pub struct DesktopCover {
    handle: WinHandle,
    fences: Vec<Fence>,
    dragging_idx: Option<usize>,
    last_mouse_pos: POINT,
}

impl DesktopCover {
    pub unsafe fn new(wndproc: WNDPROC) -> Result<Box<DesktopCover>> {
        let h_instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let class_name = w!("BottomWindowClass");
        unsafe {
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: wndproc,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_instance,
                hIcon: std::ptr::null_mut(),
                hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
                hbrBackground: GetStockObject(BLACK_BRUSH) as HBRUSH,
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name,
            };
            if RegisterClassW(&wc) == 0 {
                return Err(anyhow!("RegisterClassW failed"));
            }
        }

        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_LAYERED,
                class_name,
                w!("Desktop Cover"),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                width,
                height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                h_instance,
                std::ptr::null(),
            )
        };
        if hwnd == std::ptr::null_mut() {
            return Err(anyhow!("CreateWindowExW failed"));
        }
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
        }
        unsafe {
            SetLayeredWindowAttributes(hwnd, 0x00000000, 0, LWA_COLORKEY);
        }
        unsafe {
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
        Ok(Box::new(DesktopCover {
            handle: WinHandle(hwnd),
            fences: Vec::new(),
            dragging_idx: None,
            last_mouse_pos: POINT { x: 0, y: 0 },
        }))
    }
}

impl Window for DesktopCover {
    fn handle(&self) -> WinHandle {
        self.handle
    }

    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.handle.0;
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
                let hdc = unsafe { BeginPaint(hwnd, &mut ps) };

                for fence in &self.fences {
                    unsafe {
                        fence.draw(hdc);
                    }
                }

                unsafe {
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_SETCURSOR => {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe { GetCursorPos(&mut pt) };
                unsafe { ScreenToClient(hwnd, &mut pt) };

                let over_fence = self.fences.iter().any(|f| f.contains(pt.x, pt.y));
                if over_fence {
                    unsafe {
                        let cursor = LoadCursorW(std::ptr::null_mut(), IDC_SIZEALL);
                        SetCursor(cursor);
                    }
                    return TRUE as LRESULT;
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_LBUTTONDOWN => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                for (i, fence) in self.fences.iter().enumerate().rev() {
                    if fence.contains(x, y) {
                        self.dragging_idx = Some(i);
                        self.last_mouse_pos = POINT { x, y };
                        unsafe { SetCapture(hwnd) };
                        break;
                    }
                }
                0
            }
            WM_MOUSEMOVE => {
                if let Some(idx) = self.dragging_idx {
                    let x = (lparam & 0xFFFF) as i16 as i32;
                    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                    let dx = x - self.last_mouse_pos.x;
                    let dy = y - self.last_mouse_pos.y;

                    self.fences[idx].move_by(dx, dy);
                    self.last_mouse_pos = POINT { x, y };

                    unsafe { InvalidateRect(hwnd, std::ptr::null(), TRUE) };
                }
                0
            }
            WM_LBUTTONUP => {
                if self.dragging_idx.is_some() {
                    self.dragging_idx = None;
                    unsafe { ReleaseCapture() };
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
                match wparam & 0xFFFF {
                    IDM_EXIT => unsafe {
                        DestroyWindow(hwnd);
                    },
                    IDM_ADD_FENCE => {
                        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
                        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
                        self.fences
                            .push(Fence::new(width / 2 - 150, height / 2 - 75));
                        unsafe { InvalidateRect(hwnd, std::ptr::null(), TRUE) };
                    }
                    _ => {}
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
