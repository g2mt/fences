#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Result, anyhow};
use tracing::{error, info};
use tracing_subscriber::prelude::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

mod app;
mod config;
mod desktop_cover;
mod desktop_mirror;
mod fence;
mod fut;
mod geo;
mod paths;
mod prompt;
mod utils;
mod window;

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
    info!(
        "Found existing instance with pid {}, signaling it to exit",
        pid
    );

    unsafe {
        let hwnd = FindWindowExW(None, None, w!("BottomWindowClass"), PCWSTR::null())
            .unwrap_or(HWND::default());
        let win_pid = GetWindowThreadProcessId(hwnd, None);
        if win_pid != pid {
            return Err(anyhow!("Handle not owned by PID"));
        }
        if !hwnd.is_invalid() {
            let _ = PostMessageW(Some(hwnd), WM_DESTROY, WPARAM(0), LPARAM(0));
        } else {
            return Err(anyhow!("Unable to find desktop cover class"));
        }
    }

    // Wait up to ~10 seconds for the id file to be deleted
    let start = std::time::Instant::now();
    while id_path.exists() {
        if start.elapsed() > std::time::Duration::from_secs(2) {
            return Err(anyhow!("Timed out waiting for existing instance to exit"));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    return Ok(());
}

fn main() -> Result<()> {
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
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
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
