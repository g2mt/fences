use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_DESTROY => {
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
        WM_LBUTTONDOWN => {
            let x = (lparam & 0xFFFF) as i16;
            let y = ((lparam >> 16) & 0xFFFF) as i16;
            println!("Mouse click at: x={}, y={}", x, y);
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
