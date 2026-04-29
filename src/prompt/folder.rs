use tracing::info;
use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::*;

use crate::utils::HWNDWrapper;

fn browse_for_folder_sync(parent_hwnd_w: HWNDWrapper) -> Option<String> {
    unsafe {
        let mut path_buf = [0u16; MAX_PATH as usize];
        let mut bi = BROWSEINFOW {
            hwndOwner: parent_hwnd_w.0,
            pidlRoot: std::ptr::null_mut(),
            pszDisplayName: windows::core::PWSTR(path_buf.as_mut_ptr()),
            lpszTitle: w!("Select a folder"),
            ulFlags: (BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE) as u32,
            lpfn: None,
            lParam: LPARAM(0),
            iImage: 0,
        };

        let pidl = SHBrowseForFolderW(&mut bi);
        if pidl.is_null() {
            None
        } else {
            let mut path_str_buf = [0u16; MAX_PATH as usize];
            SHGetPathFromIDListW(pidl, &mut path_str_buf);
            CoTaskMemFree(Some(pidl as *const core::ffi::c_void));
            Some(String::from_utf16_lossy(
                &path_str_buf[..path_str_buf.iter().position(|&c| c == 0).unwrap_or(0)],
            ))
        }
    }
}

pub fn browse_for_folder<F>(parent_hwnd: HWND, f: F)
where
    F: FnOnce(Option<String>, HWND) + Send + 'static,
{
    let parent_hwnd_w = HWNDWrapper(parent_hwnd);
    std::thread::spawn(move || {
        let result = browse_for_folder_sync(parent_hwnd_w);
        f(result, parent_hwnd_w.0);
    });
}
