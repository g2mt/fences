use std::sync::Arc;

use parking_lot::Mutex;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::fut::{PromptFuture, PromptState};

pub fn confirm(
    _hwnd: Option<windows::Win32::Foundation::HWND>,
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
    let text_str = unsafe { text.to_string().unwrap_or_default() };
    let caption_str = unsafe { caption.to_string().unwrap_or_default() };

    std::thread::spawn(move || {
        let result = unsafe {
            MessageBoxW(
                None,
                PCWSTR(HSTRING::from(&text_str).as_ptr()),
                PCWSTR(HSTRING::from(&caption_str).as_ptr()),
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
