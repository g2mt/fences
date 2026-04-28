use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const PW_RENDERFULLCONTENT: u32 = 0x00000002;

unsafe fn find_desktop_window() -> HWND {
    // Progman hosts the desktop. On systems with an active wallpaper slideshow/
    // Windows 10+, a WorkerW window behind the icons may hold the wallpaper,
    // but Progman + PW_RENDERFULLCONTENT still renders wallpaper + icons fine.
    FindWindowW(windows_sys::w!("Progman"), null())
}

unsafe fn capture_desktop_window(width: i32, height: i32) -> HDC {
    let screen_dc = GetDC(null_mut());
    let mem_dc = CreateCompatibleDC(screen_dc);
    let mem_bmp = CreateCompatibleBitmap(screen_dc, width, height);
    SelectObject(mem_dc, mem_bmp as HGDIOBJ);
    ReleaseDC(null_mut(), screen_dc);

    let desktop_hwnd = find_desktop_window();
    if !desktop_hwnd.is_null() {
        PrintWindow(desktop_hwnd, mem_dc, PW_RENDERFULLCONTENT);
    }

    mem_dc
}

unsafe fn draw_overlay(hdc: HDC) {
    let rect_w = 1920;
    let rect_h = 1080;
    let rect_x = 0;
    let rect_y = 0;

    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, rect_w, rect_h);
    SelectObject(mem_dc, mem_bmp as HGDIOBJ);

    // Tint color: a green tint. COLORREF is 0x00bbggrr.
    let brush = CreateSolidBrush(0x0000AA00);
    let rect = RECT {
        left: 0,
        top: 0,
        right: rect_w,
        bottom: rect_h,
    };
    FillRect(mem_dc, &rect, brush);
    DeleteObject(brush as HGDIOBJ);

    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 102, // ~0.4 opacity
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
            SCREEN_W = GetSystemMetrics(SM_CXSCREEN);
            SCREEN_H = GetSystemMetrics(SM_CYSCREEN);

            DESKTOP_DC = capture_desktop_window(SCREEN_W, SCREEN_H);
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            // 1. Draw the captured desktop-only bitmap
            BitBlt(hdc, 0, 0, SCREEN_W, SCREEN_H, DESKTOP_DC, 0, 0, SRCCOPY);

            // 2. Draw the tinted overlay
            draw_overlay(hdc);

            EndPaint(hwnd, &ps);
            0
        }
        WM_KEYDOWN => {
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
        let class_name = windows_sys::w!("DesktopOnlyCoverClass");

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

        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);

        let _hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            windows_sys::w!("Desktop Only Cover"),
            WS_POPUP | WS_VISIBLE,
            0,
            0,
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
