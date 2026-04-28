use std::ptr::{null, null_mut};

use windows_sys::core::w;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

unsafe fn get_desktop_window() -> HWND {
    let mut desktop_hwnd = FindWindowW(w!("Progman"), null());

    let mut workerw: HWND = null_mut();
    unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: isize) -> BOOL {
        let p = lparam as *mut HWND;
        let defview = FindWindowExW(hwnd, null_mut(), w!("SHELLDLL_DefView"), null());
        if defview != null_mut() {
            *p = hwnd;
            return 0;
        }
        1
    }

    EnumWindows(Some(enum_windows_proc), &mut workerw as *mut _ as isize);

    if workerw != null_mut() {
        desktop_hwnd = workerw;
    }

    desktop_hwnd
}

unsafe fn capture_desktop_only(width: i32, height: i32, left: i32, top: i32) -> HDC {
    let desktop_hwnd = get_desktop_window();
    let desktop_dc = GetDC(desktop_hwnd);
    
    let screen_dc = GetDC(null_mut());
    let mem_dc = CreateCompatibleDC(screen_dc);
    let mem_bmp = CreateCompatibleBitmap(screen_dc, width, height);
    SelectObject(mem_dc, mem_bmp as HGDIOBJ);

    let mut pt = POINT { x: left, y: top };
    ScreenToClient(desktop_hwnd, &mut pt);

    BitBlt(mem_dc, 0, 0, width, height, desktop_dc, pt.x, pt.y, SRCCOPY);

    ReleaseDC(desktop_hwnd, desktop_dc);
    ReleaseDC(null_mut(), screen_dc);

    mem_dc
}

unsafe fn draw_overlay(hdc: HDC, width: i32, height: i32) {
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, width, height);
    SelectObject(mem_dc, mem_bmp as HGDIOBJ);

    let brush = CreateSolidBrush(0x00210000); // RGB(0, 0, 33)
    let rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    FillRect(mem_dc, &rect, brush);
    DeleteObject(brush as HGDIOBJ);

    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 102, // 0.4 opacity
        AlphaFormat: 0,
    };

    GdiAlphaBlend(hdc, 0, 0, width, height, mem_dc, 0, 0, width, height, blend);

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

            DESKTOP_DC = capture_desktop_only(SCREEN_W, SCREEN_H, left, top);
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            BitBlt(hdc, 0, 0, SCREEN_W, SCREEN_H, DESKTOP_DC, 0, 0, SRCCOPY);
            draw_overlay(hdc, SCREEN_W, SCREEN_H);

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
        let class_name = w!("DesktopOnlyCoverClass");

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

        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);

        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Desktop Only Cover"),
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
