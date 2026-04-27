use std::path::PathBuf;

use anyhow::Result;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::Shell::*;

pub static LOG_PATH: &'static str = "log.txt";
pub static STATE_PATH: &'static str = "state.json";

pub fn app_dir() -> Result<PathBuf> {
    let mut path = vec![0u16; MAX_PATH as usize];
    unsafe {
        if SHGetSpecialFolderPathW(
            std::ptr::null_mut(),
            path.as_mut_ptr(),
            CSIDL_PERSONAL as _,
            FALSE,
        ) == FALSE
        {
            return Err(anyhow::anyhow!("Failed to get Documents folder"));
        }
    }
    let path_str = String::from_utf16_lossy(
        &path
            .iter()
            .take_while(|&&c| c != 0)
            .cloned()
            .collect::<Vec<_>>(),
    );
    let mut config_path = PathBuf::from(path_str);
    config_path.push("FencesConf");
    if !config_path.exists() {
        std::fs::create_dir_all(&config_path)?;
    }
    Ok(config_path)
}

pub fn app_file(file: &str) -> Result<PathBuf> {
    let mut path = app_dir()?;
    path.push(file);
    Ok(path)
}
