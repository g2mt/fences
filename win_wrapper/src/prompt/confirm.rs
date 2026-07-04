use std::sync::Arc;

use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::*;

use crate::fut::{PromptFuture, PromptState};
use crate::mutex::Mutex;

pub fn confirm(
    _hwnd: Option<windows_sys::Win32::Foundation::HWND>,
    text: PCWSTR,
    caption: PCWSTR,
    utype: MESSAGEBOX_STYLE,
) -> PromptFuture<MESSAGEBOX_RESULT> {
    let state = Arc::new(Mutex::new(PromptState {
        result: None,
        waker: None,
        completed: false,
    }));

    let state_clone = state.clone();
    let text_u16: Vec<u16> = unsafe {
        let mut len = 0;
        while *text.add(len) != 0 {
            len += 1;
        }
        std::slice::from_raw_parts(text, len).to_vec()
    };
    let caption_u16: Vec<u16> = unsafe {
        let mut len = 0;
        while *caption.add(len) != 0 {
            len += 1;
        }
        std::slice::from_raw_parts(caption, len).to_vec()
    };

    std::thread::spawn(move || {
        let result = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text_u16.as_ptr(),
                caption_u16.as_ptr(),
                utype,
            )
        };

        let mut state = state_clone.lock();
        state.result = Some(result);
        state.completed = true;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    });

    PromptFuture { state }
}
