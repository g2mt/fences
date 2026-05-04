use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::fut::{PromptFuture, PromptState};

const ID_EDIT: u32 = 101;
const ID_OK: u32 = 1;
const ID_CANCEL: u32 = 2;

struct InputDialogData {
    message_utf16: Vec<u16>,
    default_text: String,
    edit_hwnd: HWND,
    state: Arc<Mutex<crate::fut::PromptState<Option<String>>>>,
}

unsafe extern "system" fn input_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();

            // Store the InputDialogData pointer passed through lParam
            let cs = lparam.0 as *const CREATESTRUCTW;
            let data = &mut *((*cs).lpCreateParams as *mut InputDialogData);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as *mut InputDialogData as isize);

            // Create a static message label
            let _ = CreateWindowExW(
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
            let _ = crate::utils::create_button(
                "OK",
                50,
                80,
                60,
                25,
                hwnd,
                Some(HMENU(ID_OK as *mut core::ffi::c_void)),
                hinstance.into(),
            );

            // Create Cancel button
            let _ = crate::utils::create_button(
                "Cancel",
                130,
                80,
                60,
                25,
                hwnd,
                Some(HMENU(ID_CANCEL as *mut core::ffi::c_void)),
                hinstance.into(),
            );

            // Set the edit's initial text
            let default_utf16: Vec<u16> = data
                .default_text
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = SetWindowTextW(data.edit_hwnd, PCWSTR(default_utf16.as_ptr()));
            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_DESTROY => LRESULT(0),
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

                        let mut state = data.state.lock();
                        state.result = Some(Some(s));
                        state.completed = true;
                        if let Some(waker) = state.waker.take() {
                            waker.wake();
                        }
                        let _ = DestroyWindow(hwnd);
                    }
                    ID_CANCEL => {
                        let mut state = data.state.lock();
                        state.result = Some(None);
                        state.completed = true;
                        if let Some(waker) = state.waker.take() {
                            waker.wake();
                        }
                        let _ = DestroyWindow(hwnd);
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

/// Shows a non-blocking input dialog.
pub fn input(title: &str, message: &str, default: &str) -> PromptFuture<Option<String>> {
    let state = Arc::new(Mutex::new(PromptState {
        result: None,
        waker: None,
        completed: false,
    }));

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();

        if CLASS_REGISTERED
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
        {
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.style = CS_HREDRAW | CS_VREDRAW;
            wc.lpfnWndProc = Some(input_wndproc);
            wc.hInstance = hinstance.into();
            wc.hbrBackground = HBRUSH((COLOR_BTNFACE.0 + 1) as *mut core::ffi::c_void);
            wc.lpszClassName = w!("InputDialogClass");
            RegisterClassW(&wc);
        }

        let data_ptr = Box::into_raw(Box::new(InputDialogData {
            message_utf16: message.encode_utf16().chain(std::iter::once(0)).collect(),
            default_text: default.to_string(),
            edit_hwnd: HWND::default(),
            state: state.clone(),
        }));

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
            Some(hinstance.into()),
            Some(data_ptr as *mut core::ffi::c_void),
        );

        if let Ok(hwnd) = hwnd {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = UpdateWindow(hwnd);
        } else {
            let mut s = state.lock();
            s.completed = true;
            s.result = Some(None);
        }
    }

    PromptFuture { state }
}
