use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_USER_SHELLICON: u32 = WM_USER + 1;
const IDM_EXIT: usize = 101;

trait Window {
    fn hwnd(&self) -> HWND;
    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}

struct App {
    windows: BTreeMap<HWND, RefCell<Box<dyn Window>>>,
}

static APP: OnceLock<Mutex<App>> = OnceLock::new();

struct DesktopCover {
    hwnd: HWND,
}

impl DesktopCover {
    unsafe fn new(h_instance: HINSTANCE, class_name: PCWSTR) -> Box<DesktopCover> {
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);

        let mut dc = Box::new(DesktopCover {
            hwnd: std::ptr::null_mut(),
        });

        // Create window with WS_EX_NOACTIVATE | WS_EX_LAYERED
        let hwnd = CreateWindowExW(
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
            std::ptr::null_mut(), // no lpCreateParams – we will use the global map
        );

        if hwnd == std::ptr::null_mut() {
            panic!("CreateWindowExW failed");
        }

        dc.hwnd = hwnd;

        // Add system tray icon
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

        // Make the black background color transparent
        SetLayeredWindowAttributes(hwnd, 0x00000000, 0, LWA_COLORKEY);

        // Push the window to the bottom initially
        SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        dc
    }
}

impl Window for DesktopCover {
    fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn wndproc(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let hwnd = self.hwnd;
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
                // Force the window to stay at the bottom of the Z-order
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

                let h_brush = unsafe { CreateSolidBrush(0x000000FF) }; // Red in BGR
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

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Look up the window object using the global map.
    if let Some(app) = APP.get() {
        if let Ok(mut guard) = app.lock() {
            if let Some(window_refcell) = guard.windows.get(&hwnd) {
                let mut window = window_refcell.borrow_mut();
                return window.wndproc(msg, wparam, lparam);
            }
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn main() {
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());
        let class_name = w!("BottomWindowClass");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
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
            return;
        }

        // Initialise the global map before creating any windows.
        APP.get_or_init(|| Mutex::new(App { windows: BTreeMap::new() }));

        let dc = DesktopCover::new(h_instance, class_name);
        let hwnd = dc.hwnd;

        // Insert the window into the global map.
        {
            let app = APP.get().unwrap();
            let mut guard = app.lock().unwrap();
            guard.windows.insert(hwnd, RefCell::new(dc));
        }

        // Standard message loop
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
