use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing::error;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;

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
    match msg {
        WM_NCCREATE => unsafe {
            // Store the InputDialogData pointer passed through lParam
            let cs = lparam.0 as *const CREATESTRUCTW;
            let data = &mut *((*cs).lpCreateParams as *mut InputDialogData);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as *mut InputDialogData as isize);

            // Create a static message label
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                PCWSTR(data.message_utf16.as_ptr()),
                WS_VISIBLE | WS_CHILD,
                10,
                10,
                200,
                20,
                Some(hwnd),
                None,
                Some(GetModuleHandleW(None).unwrap_or_default().into()),
                None,
            );

            // Create edit control
            let edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                None,
                WS_VISIBLE | WS_CHILD | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                10,
                40,
                200,
                20,
                Some(hwnd),
                Some(HMENU(ID_EDIT as *mut core::ffi::c_void)),
                Some(GetModuleHandleW(None).unwrap_or_default().into()),
                None,
            )
            .unwrap_or_default();
            data.edit_hwnd = edit;

            // Create OK button
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("OK"),
                WS_VISIBLE | WS_CHILD | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
                50,
                80,
                60,
                25,
                Some(hwnd),
                Some(HMENU(ID_OK as *mut core::ffi::c_void)),
                Some(GetModuleHandleW(None).unwrap_or_default().into()),
                None,
            );

            // Create Cancel button
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("Cancel"),
                WS_VISIBLE | WS_CHILD,
                130,
                80,
                60,
                25,
                Some(hwnd),
                Some(HMENU(ID_CANCEL as *mut core::ffi::c_void)),
                Some(GetModuleHandleW(None).unwrap_or_default().into()),
                None,
            );

            // Set the edit's initial text
            if let Some(default) = &data.result {
                let default_utf16: Vec<u16> =
                    default.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(hwnd, PCWSTR(default_utf16.as_ptr()));
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_DESTROY => unsafe {
            PostQuitMessage(0);
            LRESULT(0)
        },
        WM_COMMAND => unsafe {
            let id = (wparam.0 as u32) & 0xFFFF;
            let hi = ((wparam.0 as u32) >> 16) as u16;
            if hi == BN_CLICKED as u16 {
                let data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut InputDialogData;
                let data = &mut *data_ptr;
                match id {
                    ID_OK => {
                        let len = GetWindowTextLengthW(data.edit_hwnd);
                        let mut buf: Vec<u16> = vec![0; (len + 1) as usize];
                        GetWindowTextW(data.edit_hwnd, &mut buf);
                        let s = String::from_utf16_lossy(&buf[..len as usize]);
                        data.result = Some(s);
                        data.ok_clicked.store(true, Ordering::SeqCst);
                        DestroyWindow(data.edit_hwnd);
                        DestroyWindow(hwnd);
                    }
                    ID_CANCEL => {
                        data.ok_clicked.store(false, Ordering::SeqCst);
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        },
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

static CLASS_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Shows a modal input dialog. Returns `None` if the user cancelled, otherwise `Some(String)`.
pub fn input_sync(title: &str, message: &str, default: &str) -> Option<String> {
    unsafe {
        let h_instance = GetModuleHandleW(None).unwrap_or_default();

        // Register a window class for the dialog
        if CLASS_REGISTERED
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
        {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.style = CS_HREDRAW | CS_VREDRAW;
            wc.lpfnWndProc = Some(input_wndproc);
            wc.hInstance = h_instance.into();
            wc.hbrBackground = HBRUSH((COLOR_BTNFACE.0 + 1) as *mut core::ffi::c_void);
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
            edit_hwnd: HWND::default(),
            result: Some(default.to_string()),
            ok_clicked: AtomicBool::new(false),
        }));

        // Create the dialog window
        let title_utf16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("InputDialogClass"),
            PCWSTR(title_utf16.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            240,
            150,
            None,
            None,
            Some(h_instance.into()),
            Some(data_ptr as *mut core::ffi::c_void),
        );
        if hwnd.is_err() {
            error!("hwnd == null, {}", GetLastError().0);
            return Some(default.to_string());
        }
        let hwnd = hwnd.unwrap();

        // Show and run modal loop
        ShowWindow(hwnd, SW_SHOWNORMAL);
        UpdateWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
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
