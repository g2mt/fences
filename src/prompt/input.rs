use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

use crate::fut::{PromptFuture, PromptState};
use crate::layout::{Item, Layout, Orientation};
use crate::utils::HWNDWrapper;

const ID_EDIT: u32 = 101;
const ID_OK: u32 = 1;
const ID_CANCEL: u32 = 2;

const DLG_WIDTH: i32 = 360;
const DLG_HEIGHT: i32 = 170;

const LABEL_HEIGHT: i32 = 20;
const EDIT_MIN_HEIGHT: i32 = 22;
const BTN_WIDTH: i32 = 70;
const BTN_HEIGHT: i32 = 26;

struct InputDialogData {
    message: String,
    default_text: String,
    layout: Layout,
    edit_hwnd: HWND,
    state: Arc<Mutex<crate::fut::PromptState<Option<String>>>>,
}

fn layout_widgets(hwnd: HWND, data: &InputDialogData) {
    let mut rect = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut rect);
    };
    data.layout.arrange(&rect);
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

            let cs = lparam.0 as *const CREATESTRUCTW;
            let data = &mut *((*cs).lpCreateParams as *mut InputDialogData);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as *mut InputDialogData as isize);

            // Create a static message label
            let label = crate::controls::create_label(
                &data.message,
                0,
                0,
                0,
                0,
                hwnd,
                hinstance.into(),
            );

            // Create edit control
            let edit = crate::controls::create_edit(
                0,
                0,
                0,
                0,
                hwnd,
                Some(HMENU(ID_EDIT as *mut core::ffi::c_void)),
                hinstance.into(),
            );
            data.edit_hwnd = edit;

            // Create OK button
            let ok_btn = crate::controls::create_button(
                "OK",
                0,
                0,
                0,
                0,
                hwnd,
                Some(HMENU(ID_OK as *mut core::ffi::c_void)),
                hinstance.into(),
            );

            // Create Cancel button
            let cancel_btn = crate::controls::create_button(
                "Cancel",
                0,
                0,
                0,
                0,
                hwnd,
                Some(HMENU(ID_CANCEL as *mut core::ffi::c_void)),
                hinstance.into(),
            );

            data.layout = Layout {
                orientation: Orientation::Vertical,
                margin: 10,
                gap: 6,
                items: vec![
                    Item::Fixed {
                        hwnd: HWNDWrapper(label),
                        size: LABEL_HEIGHT,
                    },
                    Item::Fill {
                        hwnd: HWNDWrapper(edit),
                        min: EDIT_MIN_HEIGHT,
                    },
                    Item::Nested {
                        layout: Box::new(Layout {
                            orientation: Orientation::Horizontal,
                            margin: 10,
                            gap: 10,
                            items: vec![
                                Item::Fill {
                                    hwnd: HWNDWrapper(HWND::default()),
                                    min: 0,
                                },
                                Item::Fixed {
                                    hwnd: HWNDWrapper(ok_btn),
                                    size: BTN_WIDTH,
                                },
                                Item::Fixed {
                                    hwnd: HWNDWrapper(cancel_btn),
                                    size: BTN_WIDTH,
                                },
                            ],
                        }),
                        size: BTN_HEIGHT,
                    },
                ],
            };

            // Set the edit's initial text
            let default_utf16: Vec<u16> = data
                .default_text
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = SetWindowTextW(data.edit_hwnd, PCWSTR(default_utf16.as_ptr()));

            layout_widgets(hwnd, data);

            DefWindowProcW(hwnd, msg, wparam, lparam)
        },
        WM_SIZE => unsafe {
            let data_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut InputDialogData;
            if !data_ptr.is_null() {
                let data = &*data_ptr;
                layout_widgets(hwnd, data);
            }
            LRESULT(0)
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

        // Center dialog on screen
        let bounds = crate::app::App::get().screen_bounds();
        let screen_w = bounds.width.load(std::sync::atomic::Ordering::Relaxed);
        let screen_h = bounds.height.load(std::sync::atomic::Ordering::Relaxed);
        let dlg_x = (screen_w - DLG_WIDTH) / 2;
        let dlg_y = (screen_h - DLG_HEIGHT) / 2;

        let data_ptr = Box::into_raw(Box::new(InputDialogData {
            message: message.to_string(),
            default_text: default.to_string(),
            layout: Layout {
                orientation: Orientation::Vertical,
                margin: 0,
                gap: 0,
                items: vec![],
            },
            edit_hwnd: HWND::default(),
            state: state.clone(),
        }));

        let title_utf16: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("InputDialogClass"),
            PCWSTR(title_utf16.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            dlg_x,
            dlg_y,
            DLG_WIDTH,
            DLG_HEIGHT,
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
