use anyhow::{anyhow, Result};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::fence::{Fence, HitTest};
use crate::window::{WinHandle, Window};

// Menus
pub const IDM_EXIT: usize = 101;
pub const IDM_ADD_FENCE: usize = 102;

// Custom events
pub const WM_USER_SHELLICON: u32 = WM_USER + 1;

pub struct DesktopCover {
    /// The window handle for the desktop cover.
    handle: WinHandle,
    /// The list of fences managed by this cover.
    fences: Vec<Fence>,
    /// The type of hit test result for the currently focused/dragged fence.
    hit_type: Option<HitTest>,
    /// The last recorded mouse position during a drag operation.
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
            hit_type: None,
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

                for fence in self.fences.iter().rev() {
                    let hit = fence.hit_test(pt.x, pt.y);
                    if hit != HitTest::None {
                        let cursor_id = match hit {
                            HitTest::Inside => IDC_SIZEALL,
                            HitTest::Left | HitTest::Right => IDC_SIZEWE,
                            HitTest::Top | HitTest::Bottom => IDC_SIZENS,
                            HitTest::TopLeft | HitTest::BottomRight => IDC_SIZENWSE,
                            HitTest::TopRight | HitTest::BottomLeft => IDC_SIZENESW,
                            HitTest::None => IDC_ARROW,
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
            WM_LBUTTONDOWN => {
                let x = (lparam & 0xFFFF) as i16 as i32;
                let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                let mut hit_idx = None;
                for (i, fence) in self.fences.iter().enumerate().rev() {
                    let hit = fence.hit_test(x, y);
                    if hit != HitTest::None {
                        hit_idx = Some((i, hit));
                        break;
                    }
                }

                if let Some((idx, hit)) = hit_idx {
                    let fence = self.fences.remove(idx);
                    self.fences.push(fence);
                    self.hit_type = Some(hit);
                    self.last_mouse_pos = POINT { x, y };
                    unsafe {
                        SetCapture(hwnd);
                        InvalidateRect(hwnd, std::ptr::null(), TRUE);
                    };
                }
                0
            }
            WM_MOUSEMOVE => {
                if let Some(hit_type) = self.hit_type {
                    let x = (lparam & 0xFFFF) as i16 as i32;
                    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

                    let dx = x - self.last_mouse_pos.x;
                    let dy = y - self.last_mouse_pos.y;

                    if let Some(fence) = self.fences.last_mut() {
                        match hit_type {
                            HitTest::Inside => fence.move_by(dx, dy),
                            HitTest::Left => { fence.rect.left += dx; }
                            HitTest::Right => { fence.rect.right += dx; }
                            HitTest::Top => { fence.rect.top += dy; }
                            HitTest::Bottom => { fence.rect.bottom += dy; }
                            HitTest::TopLeft => { fence.rect.left += dx; fence.rect.top += dy; }
                            HitTest::TopRight => { fence.rect.right += dx; fence.rect.top += dy; }
                            HitTest::BottomLeft => { fence.rect.left += dx; fence.rect.bottom += dy; }
                            HitTest::BottomRight => { fence.rect.right += dx; fence.rect.bottom += dy; }
                            HitTest::None => {}
                        }
                    }

                    self.last_mouse_pos = POINT { x, y };
                    unsafe { InvalidateRect(hwnd, std::ptr::null(), TRUE) };
                }
                0
            }
            WM_LBUTTONUP => {
                if self.hit_type.is_some() {
                    self.hit_type = None;
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
