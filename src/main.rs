use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_USER_SHELLICON: u32 = WM_USER + 1;
const IDM_EXIT: u32 = 101;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
                    AppendMenuW(h_menu, MF_STRING, IDM_EXIT as usize, w!("&Exit"));
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
            if (wparam & 0xFFFF) as u32 == IDM_EXIT {
                unsafe { DestroyWindow(hwnd) };
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
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

        // Get the full desktop dimensions
        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);

        // Create a borderless window (WS_POPUP) that doesn't steal focus (WS_EX_NOACTIVATE)
        // and supports transparency (WS_EX_LAYERED)
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
            std::ptr::null(),
        );

        if hwnd == std::ptr::null_mut() {
            return;
        }

        // Add system tray icon
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_USER_SHELLICON;
        nid.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        let tip = [
            'D' as u16, 'e' as u16, 's' as u16, 'k' as u16, 't' as u16, 'o' as u16, 'p' as u16,
            ' ' as u16, 'C' as u16, 'o' as u16, 'v' as u16, 'e' as u16, 'r' as u16, 0,
        ];
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

        // Standard message loop
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
