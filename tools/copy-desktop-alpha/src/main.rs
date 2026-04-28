use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

unsafe fn capture_desktop(width: i32, height: i32, left: i32, top: i32) -> HDC {
    // Get the device context for the entire virtual screen
    let desktop_dc = GetDC(null_mut());
    let mem_dc = CreateCompatibleDC(desktop_dc);
    let mem_bmp = CreateCompatibleBitmap(desktop_dc, width, height);

    SelectObject(mem_dc, mem_bmp as HGDIOBJ);
    BitBlt(mem_dc, 0, 0, width, height, desktop_dc, left, top, SRCCOPY);

    ReleaseDC(null_mut(), desktop_dc);

    // Return the memory DC containing the captured desktop
    mem_dc
}

unsafe fn draw_overlay(hdc: HDC, screen_w: i32, screen_h: i32) {
    let rect_w = 1920;
    let rect_h = 1080;
    let rect_x = 0;
    let rect_y = 0;

    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, rect_w, rect_h);
    SelectObject(mem_dc, mem_bmp as HGDIOBJ);

    // Fill the memory DC with RGB(0, 0, 33)
    // COLORREF format is 0x00bbggrr. R=0, G=0, B=33 (0x21) -> 0x00210000
    let brush = CreateSolidBrush(0x00210000);
    let rect = RECT {
        left: 0,
        top: 0,
        right: rect_w,
        bottom: rect_h,
    };
    FillRect(mem_dc, &rect, brush);
    DeleteObject(brush as HGDIOBJ);

    // Blend the rectangle onto the destination DC with 0.4 opacity (102/255)
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 102,
        AlphaFormat: 0,
    };

    GdiAlphaBlend(
        hdc, rect_x, rect_y, rect_w, rect_h, mem_dc, 0, 0, rect_w, rect_h, blend,
    );

    DeleteObject(mem_bmp as HGDIOBJ);
    DeleteDC(mem_dc);
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    static mut DESKTOP_DC: HDC = null_mut();
    static mut SCREEN_W: i32 = 0;
    static mut SCREEN_H: i32 = 0;

    match msg {
        WM_CREATE => {
            let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            SCREEN_W = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            SCREEN_H = GetSystemMetrics(SM_CYVIRTUALSCREEN);

            // Capture the desktop once when the window is created
            DESKTOP_DC = capture_desktop(SCREEN_W, SCREEN_H, left, top);
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            // 1. Draw the captured desktop background
            BitBlt(hdc, 0, 0, SCREEN_W, SCREEN_H, DESKTOP_DC, 0, 0, SRCCOPY);

            // 2. Draw the semi-transparent rectangle in the center
            draw_overlay(hdc, SCREEN_W, SCREEN_H);

            EndPaint(hwnd, &ps);
            0
        }
        WM_KEYDOWN => {
            // Exit the application if the Escape key is pressed
            if wparam == VK_ESCAPE as usize {
                PostQuitMessage(0);
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    unsafe {
        let h_instance = GetModuleHandleW(null());
        let class_name = windows_sys::w!("DesktopCoverClass");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name,
        };

        RegisterClassW(&wc);

        // Get the coordinates and dimensions of the entire virtual screen (all monitors)
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        let _hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            windows_sys::w!("Desktop Cover"),
            WS_POPUP | WS_VISIBLE,
            left,
            top,
            width,
            height,
            null_mut(),
            null_mut(),
            h_instance,
            null(),
        );

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
