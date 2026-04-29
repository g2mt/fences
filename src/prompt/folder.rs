use tracing::info;
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::*;

use crate::app::App;
use crate::utils::HWNDWrapper;
use crate::window::Window;

use std::sync::Arc;
use parking_lot::Mutex;
use crate::fut::{PromptFuture, PromptState};

pub fn browse_for_folder() -> PromptFuture<Option<String>> {
    let state = Arc::new(Mutex::new(PromptState {
        result: None,
        waker: None,
        completed: false,
    }));

    let state_clone = state.clone();
    std::thread::spawn(move || {
        let res = browse_for_folder_internal();
        let mut s = state_clone.lock();
        s.result = Some(res);
        s.completed = true;
        if let Some(waker) = s.waker.take() {
            waker.wake();
        }
    });

    PromptFuture { state }
}

fn browse_for_folder_internal() -> Option<String> {
    let mut browse_info = BROWSEINFOW {
        hwndOwner: HWND(std::ptr::null_mut()),
        pidlRoot: std::ptr::null_mut(),
        pszDisplayName: windows::core::PWSTR(std::ptr::null_mut()),
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
