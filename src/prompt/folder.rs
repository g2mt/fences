use tracing::info;
use windows_sys::core::w;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::*;

pub fn browse_for_folder<F>(parent_hwnd: HWND, f: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    std::thread::spawn(move || {
        let result = unsafe {
            let mut path_buf = [0u16; MAX_PATH as usize];
            let mut bi = BROWSEINFOW {
                hwndOwner: parent_hwnd,
                pidlRoot: std::ptr::null_mut(),
                pszDisplayName: path_buf.as_mut_ptr(),
                lpszTitle: w!("Select a folder"),
                ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
                lpfn: None,
                lParam: 0,
                iImage: 0,
            };

            let pidl = SHBrowseForFolderW(&mut bi);
            if pidl.is_null() {
                None
            } else {
                let mut path_str_buf = [0u16; MAX_PATH as usize];
                SHGetPathFromIDListW(pidl, path_str_buf.as_mut_ptr());
                CoTaskMemFree(pidl as _);
                Some(String::from_utf16_lossy(
                    &path_str_buf
                        [..path_str_buf
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(0)],
                ))
            }
        };
        f(result);
    });
}
