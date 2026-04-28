use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use std::ptr::null_mut;

fn main() {
    unsafe {
        let instance = GetModuleHandleW(null_mut());
        let class_name = "DesktopCoverClass\0".encode_utf16().collect::<Vec<u16>>();

        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: 0,
            hCursor: LoadCursorW(0, IDC_ARROW),
            hbrBackground: 0,
            lpszMenuName: null_mut(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wnd_class);

        let screen_width = GetSystemMetrics(SM_CXSCREEN);
        let screen_height = GetSystemMetrics(SM_CYSCREEN);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name.as_ptr(),
            null_mut(),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
            screen_width,
            screen_height,
            0,
            0,
            instance,
            null_mut(),
        );

        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let screen_width = GetSystemMetrics(SM_CXSCREEN);
            let screen_height = GetSystemMetrics(SM_CYSCREEN);

            // Copy desktop to background
            let hdc_screen = GetDC(0);
            BitBlt(hdc, 0, 0, screen_width, screen_height, hdc_screen, 0, 0, SRCCOPY);
            ReleaseDC(0, hdc_screen);

            // Draw 300x200 rgba(0,0,33,0.4) rectangle at center
            // Note: Gdi AlphaBlend is used for transparency. 0.4 alpha is approx 102/255.
            let rect_width = 300;
            let rect_height = 200;
            let x = (screen_width - rect_width) / 2;
            let y = (screen_height - rect_height) / 2;

            let mem_dc = CreateCompatibleDC(hdc);
            let mem_bm = CreateCompatibleBitmap(hdc, rect_width, rect_height);
            SelectObject(mem_dc, mem_bm);

            // Fill with color (0, 0, 33)
            let brush = CreateSolidBrush(RGB(0, 0, 33));
            let rect = RECT { left: 0, top: 0, right: rect_width, bottom: rect_height };
            FillRect(mem_dc, &rect, brush);
            DeleteObject(brush);

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 102, // 0.4 * 255
                AlphaFormat: 0,
            };

            AlphaBlend(
                hdc, x, y, rect_width, rect_height,
                mem_dc, 0, 0, rect_width, rect_height,
                blend
            );

            DeleteObject(mem_bm);
            DeleteDC(mem_dc);

            EndPaint(hwnd, &ps);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn RGB(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}
