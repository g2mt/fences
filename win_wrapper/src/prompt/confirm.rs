use std::sync::Arc;

use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::fut::{PromptFuture, PromptState};
use crate::mutex::Mutex;

pub fn confirm(
    text: &str,
    caption: &str,
    utype: MESSAGEBOX_STYLE,
) -> PromptFuture<MESSAGEBOX_RESULT> {
    let state = Arc::new(Mutex::new(PromptState {
        result: None,
        waker: None,
        completed: false,
    }));

    let state_clone = state.clone();
    let text_u16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let caption_u16: Vec<u16> = caption.encode_utf16().chain(std::iter::once(0)).collect();

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
