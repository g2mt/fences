use anyhow::Result;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::Shell::*;

pub fn get_config_dir() -> Result<PathBuf> {
    let mut path = vec![0u16; MAX_PATH as usize];
    unsafe {
        if SHGetSpecialFolderPathW(std::ptr::null_mut(), path.as_mut_ptr(), CSIDL_PERSONAL as _, FALSE) == FALSE {
            return Err(anyhow::anyhow!("Failed to get Documents folder"));
        }
    }
    let path_str = String::from_utf16_lossy(&path.iter().take_while(|&&c| c != 0).cloned().collect::<Vec<_>>());
    let mut config_path = PathBuf::from(path_str);
    config_path.push("FencesConf");
    if !config_path.exists() {
        std::fs::create_dir_all(&config_path)?;
    }
    Ok(config_path)
}

pub fn get_log_path() -> Result<PathBuf> {
    let mut path = get_config_dir()?;
    path.push("log.txt");
    Ok(path)
}

pub fn get_state_path() -> Result<PathBuf> {
    let mut path = get_config_dir()?;
    path.push("state.json");
    Ok(path)
}
