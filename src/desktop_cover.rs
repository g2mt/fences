use anyhow::{anyhow, Result};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::window::{WinHandle, Window};

// Menus
pub const IDM_EXIT: usize = 101;

// Custom events
pub const WM_USER_SHELLICON: u32 = WM_USER + 1;

pub struct DesktopCover {
    handle: WinHandle,
}

impl DesktopCover {
    pub unsafe fn new(h_instance: HINSTANCE, class_name: PCWSTR) -> Result<Box<DesktopCover>> {
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
                let mut rect: RECT = unsafe { std::mem::zeroed() };
                unsafe { GetClientRect(hwnd, &mut rect) };
                let center_x = (rect.right - rect.left) / 2;
                let center_y = (rect.bottom - rect.top) / 2;
                let radius = 100;
                let h_brush = unsafe { CreateSolidBrush(0x000000FF) };
                let old_brush = unsafe { SelectObject(hdc, h_brush) };
                unsafe {
                    Ellipse(
                        hdc,
                        center_x - radius,
                        center_y - radius,
                        center_x + radius,
                        center_y + radius,
                    );
                    SelectObject(hdc, old_brush);
                    DeleteObject(h_brush);
                    EndPaint(hwnd, &ps);
                }
                0
            }
            WM_LBUTTONDOWN => {
                let x = (lparam & 0xFFFF) as i16;
                let y = ((lparam >> 16) & 0xFFFF) as i16;
                println!("Mouse click at: x={}, y={}", x, y);
                0
            }
            WM_USER_SHELLICON => {
                if lparam as u32 == WM_RBUTTONUP || lparam as u32 == WM_LBUTTONUP {
                    let mut pt = POINT { x: 0, y: 0 };
                    unsafe { GetCursorPos(&mut pt) };
                    let h_menu = unsafe { CreatePopupMenu() };
                    unsafe {
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
                if (wparam & 0xFFFF) == IDM_EXIT {
                    unsafe { DestroyWindow(hwnd) };
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }
}
