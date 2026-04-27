use std::sync::atomic::{AtomicBool, Ordering};

use tracing::{error, info};
use windows_sys::core::*;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const ID_EDIT: u32 = 101;
const ID_OK: u32 = 1;
const ID_CANCEL: u32 = 2;

struct InputDialogData {
    message_utf16: Vec<u16>,
    edit_hwnd: HWND,
    result: Option<String>,
    ok_clicked: AtomicBool,
}

unsafe extern "system" fn input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    info!("msg={} {:x} {:x}", msg, wparam, lparam);
    match msg {
        WM_NCCREATE => unsafe {
            // Store the InputDialogData pointer passed through lParam
            let data = &mut *(lparam as *mut InputDialogData);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as *mut InputDialogData as isize);

            // Create a static message label
            CreateWindowExW(
                0,
                w!("STATIC"),
                data.message_utf16.as_ptr(),
                WS_VISIBLE | WS_CHILD,
                10,
                10,
                200,
                20,
                hwnd,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );

            // Create edit control
            let edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                std::ptr::null(),
                WS_VISIBLE | WS_CHILD | WS_BORDER | ES_AUTOHSCROLL as u32,
                10,
                40,
                200,
                20,
                hwnd,
                ID_EDIT as HMENU,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );
            data.edit_hwnd = edit;

            // Create OK button
            CreateWindowExW(
                0,
                w!("BUTTON"),
                w!("OK"),
                WS_VISIBLE | WS_CHILD | BS_DEFPUSHBUTTON as u32,
                50,
                80,
                60,
                25,
                hwnd,
                ID_OK as HMENU,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );

            // Create Cancel button
            CreateWindowExW(
                0,
                w!("BUTTON"),
                w!("Cancel"),
                WS_VISIBLE | WS_CHILD,
                130,
                80,
                60,
                25,
                hwnd,
                ID_CANCEL as HMENU,
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            );

            // Set the edit's initial text
            if let Some(default) = &data.result {
                let default_utf16: Vec<u16> =
                    default.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(edit, default_utf16.as_ptr());
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_DESTROY => unsafe {
            PostQuitMessage(0);
            0
        },
        WM_COMMAND => unsafe {
            let id = (wparam as u32) & 0xFFFF;
            let hi = ((wparam as u32) >> 16) as u16;
            if hi == BN_CLICKED as u16 {
                let data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut InputDialogData;
                let data = &mut *data_ptr;
                match id {
                    ID_OK => {
                        let len = GetWindowTextLengthW(data.edit_hwnd);
                        let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
                        GetWindowTextW(data.edit_hwnd, buf.as_mut_ptr(), len + 1);
                        let s = String::from_utf16_lossy(&buf[..len as usize]);
                        data.result = Some(s);
                        data.ok_clicked.store(true, Ordering::SeqCst);
                        DestroyWindow(hwnd);
                    }
                    ID_CANCEL => {
                        data.ok_clicked.store(false, Ordering::SeqCst);
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
            }
            0
        },
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Shows a modal input dialog. Returns `None` if the user cancelled, otherwise `Some(String)`.
pub fn prompt_input(
    parent_hwnd: HWND,
    title: &str,
    message: &str,
    default: &str,
) -> Option<String> {
    unsafe {
        let h_instance = GetModuleHandleW(std::ptr::null());

        // Register a window class for the dialog
        if CLASS_REGISTERED
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
        {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.style = CS_HREDRAW | CS_VREDRAW;
            wc.lpfnWndProc = Some(input_wndproc);
            wc.hInstance = h_instance;
            wc.hbrBackground = (COLOR_BTNFACE + 1) as HBRUSH;
            wc.lpszClassName = w!("InputDialogClass");
            let atom = RegisterClassW(&wc);
            if atom == 0 {
                // Fallback: return default as stub
                error!("atom == 0");
                return Some(default.to_string());
            }
        }

        // Prepare data
        let data_ptr = Box::into_raw(Box::new(InputDialogData {
            message_utf16: message.encode_utf16().chain(std::iter::once(0)).collect(),
            edit_hwnd: std::ptr::null_mut(),
            result: Some(default.to_string()),
            ok_clicked: AtomicBool::new(false),
        }));

        // Create the dialog window
        let title_utf16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("InputDialogClass"),
            title_utf16.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            240,
            150,
            parent_hwnd,
            std::ptr::null_mut(),
            h_instance,
            data_ptr as *mut std::ffi::c_void,
        );
        if hwnd == std::ptr::null_mut() {
            error!("hwnd == null, {}", GetLastError());
            return Some(default.to_string());
        }

        // Disable parent to make it modal
        if parent_hwnd != std::ptr::null_mut() {
            EnableWindow(parent_hwnd, 0);
        }

        // Show and run modal loop
        ShowWindow(hwnd, SW_SHOWNORMAL);
        UpdateWindow(hwnd);
        info!("start event loop");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        info!("returned from event loop");

        // Re-enable parent
        if parent_hwnd != std::ptr::null_mut() {
            EnableWindow(parent_hwnd, 1);
            SetForegroundWindow(parent_hwnd);
        }

        // Retrieve result
        let data = Box::from_raw(data_ptr);
        let result = data.ok_clicked.load(Ordering::SeqCst);
        if result {
            data.result
        } else {
            None
        }
    }
}
