use std::sync::Arc;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::core::w;

use crate::fut::{PromptFuture, PromptState};
use crate::mutex::Mutex;

pub fn browse_for_folder() -> PromptFuture<Option<String>> {
    let state = Arc::new(Mutex::new(PromptState {
        result: None,
        waker: None,
        completed: false,
    }));

    let state_clone = state.clone();
    std::thread::spawn(move || {
        let res = browse_for_folder_sync();
        let mut s = state_clone.lock();
        s.result = Some(res);
        s.completed = true;
        if let Some(waker) = s.waker.take() {
            waker.wake();
        }
    });

    PromptFuture { state }
}

fn browse_for_folder_sync() -> Option<String> {
    let mut browse_info = BROWSEINFOW {
        hwndOwner: std::ptr::null_mut(),
        pidlRoot: std::ptr::null_mut(),
        pszDisplayName: std::ptr::null_mut(),
        lpszTitle: w!("Select a folder"),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: None,
        lParam: 0,
        iImage: 0,
    };

    let pidl = unsafe { SHBrowseForFolderW(&mut browse_info) };

    if pidl.is_null() {
        return None;
    }

    let mut path = [0u16; MAX_PATH as usize];
    let success = unsafe { SHGetPathFromIDListW(pidl, path.as_mut_ptr()) };

    unsafe {
        CoTaskMemFree(pidl as *const core::ffi::c_void);
    }

    if success != 0 {
        let path_str = String::from_utf16_lossy(
            &path[..path.iter().position(|&c| c == 0).unwrap_or(path.len())],
        );

        Some(path_str)
    } else {
        None
    }
}
