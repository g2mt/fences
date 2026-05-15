use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{debug, info};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::core::w;

use crate::fut::{PromptFuture, PromptState};

pub fn browse_for_folder() -> PromptFuture<Option<String>> {
    let state = Arc::new(Mutex::new(PromptState {
        result: None,
        waker: None,
        completed: false,
    }));

    let state_clone = state.clone();
    std::thread::spawn(move || {
        debug!("spawned thread");
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
        hwndOwner: HWND(std::ptr::null_mut()),
        pidlRoot: std::ptr::null_mut(),
        pszDisplayName: windows_sys::core::PWSTR(std::ptr::null_mut()),
        lpszTitle: w!("Select a folder"),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: None,
        lParam: LPARAM(0),
        iImage: 0,
    };

    let pidl = unsafe { SHBrowseForFolderW(&mut browse_info) };

    if pidl.is_null() {
        info!("User cancelled folder browse dialog");
        return None;
    }

    let mut path = [0u16; MAX_PATH as usize];
    let success = unsafe { SHGetPathFromIDListW(pidl, &mut path) };

    unsafe {
        CoTaskMemFree(Some(pidl as *mut _));
    }

    if success.as_bool() {
        let path_str = String::from_utf16_lossy(
            &path[..path.iter().position(|&c| c == 0).unwrap_or(path.len())],
        );
        info!("User selected folder: {}", path_str);
        Some(path_str)
    } else {
        info!("Failed to get path from folder browse dialog");
        None
    }
}
