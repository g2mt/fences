#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Result, anyhow};
use std::path::Path;
use tracing::{error, info};
use tracing_subscriber::prelude::*;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Controls::{
    ICC_LISTVIEW_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod app;
mod commands;
mod config;
mod desktop_cover;
mod desktop_mirror;
mod fence;
mod paths;

use crate::app::App;
use crate::config::save_thread::SaveThread;
use crate::desktop_cover::DesktopCover;
use crate::paths::{ID_PATH, LOG_PATH, app_file, init_app_dir};

fn ensure_single_instance() -> Result<()> {
    let id_path = App::get().id_path.get().unwrap();
    if !id_path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&id_path).unwrap_or_default();
    let pid: u32 = if let Ok(pid) = content.trim().parse() {
        pid
    } else {
        return Ok(());
    };

    let process = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
            0,
            pid,
        )
    };
    if process.is_null() {
        return Err(anyhow!("Unable to open existing process with pid {}", pid));
    }
    let mut image_path = vec![0u16; 32_768];
    let mut image_path_len = image_path.len() as u32;
    let queried_path = unsafe {
        let success = QueryFullProcessImageNameW(
            process,
            0,
            image_path.as_mut_ptr(),
            &mut image_path_len,
        );
        success != 0
    };
    if !queried_path {
        unsafe { CloseHandle(process) };
        return Err(anyhow!("Unable to get executable path for pid {}", pid));
    }

    let path_match = (|| -> Result<bool> {
        let process_path = String::from_utf16_lossy(&image_path[..image_path_len as usize]);
        let process_path = std::fs::canonicalize(Path::new(&process_path))?;
        let current_path = std::fs::canonicalize(std::env::current_exe()?)?;
        Ok(process_path
            .to_string_lossy()
            .eq_ignore_ascii_case(&current_path.to_string_lossy()))
    })();
    match path_match {
        Ok(true) => {}
        Ok(false) => {
            unsafe { CloseHandle(process) };
            info!("PID {} belongs to a different executable; not terminating it", pid);
            return Ok(());
        }
        Err(e) => {
            unsafe { CloseHandle(process) };
            return Err(e);
        }
    }

    info!("Found existing instance with pid {}; terminating it", pid);
    let (terminated, wait_result) = unsafe {
        let terminated = TerminateProcess(process, 1) != 0;
        let wait_result = if terminated {
            Some(WaitForSingleObject(process, 10_000))
        } else {
            None
        };
        CloseHandle(process);
        (terminated, wait_result)
    };
    if !terminated {
        return Err(anyhow!("Unable to terminate existing process with pid {}", pid));
    }
    if wait_result != Some(0) {
        return Err(anyhow!("Timed out waiting for process {} to terminate", pid));
    }
    match std::fs::remove_file(&id_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn main() -> Result<()> {
    unsafe {
        let mut icc = INITCOMMONCONTROLSEX::default();
        icc.dwSize = std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32;
        icc.dwICC = ICC_LISTVIEW_CLASSES;
        let _ = InitCommonControlsEx(&icc);
    }
    let _ = init_app_dir();

    let log_path = app_file(LOG_PATH)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)?;

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file),
        )
        .init();

    let r: Result<()> = (|| {
        info!("Starting Desktop Cover");

        {
            let id_path = app_file(ID_PATH)?;
            App::get().id_path.get_or_init(|| id_path);
        }
        if let Err(e) = ensure_single_instance() {
            error!("ensure_single_instance: {}", e);
        }
        {
            let id_path = App::get().id_path.get().unwrap();
            let pid = std::process::id();
            std::fs::write(id_path, pid.to_string())?;
            info!("Wrote pid {} to {:?}", pid, id_path);
        }

        let cover = DesktopCover::new()?;
        App::get().cover.get_or_init(|| cover.clone());
        App::get().mirror.lock().update();
        let save_thread = SaveThread::new();
        App::get().save_thread.set(save_thread).unwrap();

        App::get().load_config()?;

        if let Err(e) = App::get().load_state() {
            error!("{}", e.to_string());
        }
        unsafe {
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            info!("Message loop stopped");
        }
        if let Err(e) = App::get().save_state() {
            error!("{}", e.to_string());
        }
        if let Err(e) = App::get().save_config() {
            error!("{}", e.to_string());
        }
        App::get().remove_id_path();
        Ok(())
    })();
    if let Err(e) = r {
        error!("{}", e.to_string());
        Err(e)
    } else {
        Ok(())
    }
}
