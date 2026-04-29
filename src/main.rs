#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::OnceLock;

use anyhow::Result;
use parking_lot::Mutex;
use tracing::{error, info, warn};
use tracing_subscriber::prelude::*;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;

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
use crate::desktop_mirror::DesktopMirror;
use crate::paths::{app_file, init_app_dir, ID_PATH, LOG_PATH};

fn ensure_single_instance() -> Result<()> {
    let id_path = app_file(ID_PATH)?;
    if id_path.exists() {
        let content = std::fs::read_to_string(&id_path).unwrap_or_default();
        let pid: u32 = content.trim().parse().unwrap_or(0);
        warn!("Found existing instance with pid {}, signaling it to exit", pid);

        let class_name: Vec<u16> = "BottomWindowClass"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let hwnd = FindWindowW(PCWSTR(class_name.as_ptr()), PCWSTR::null())
                .unwrap_or(HWND::default());
            if !hwnd.is_invalid() {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }

        // Wait up to ~10 seconds for the id file to be deleted
        let start = std::time::Instant::now();
        while id_path.exists() {
            if start.elapsed() > std::time::Duration::from_secs(10) {
                warn!("Timed out waiting for existing instance to exit, removing id file");
                let _ = std::fs::remove_file(&id_path);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    let pid = std::process::id();
    std::fs::write(&id_path, pid.to_string())?;
    info!("Wrote pid {} to {:?}", pid, id_path);
    Ok(())
}

fn main() -> Result<()> {
    init_app_dir();

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

        ensure_single_instance()?;

        let cover = DesktopCover::new()?;
        App::get().cover.get_or_init(|| cover.clone());
        let save_thread = SaveThread::new();
        App::get().save_thread.set(save_thread).unwrap();

        App::get().load_config()?;

        if let Err(e) = App::get().load_state() {
            error!("{}", e.to_string());
        }
        unsafe {
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
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

        if let Ok(id_path) = app_file(ID_PATH) {
            if let Err(e) = std::fs::remove_file(&id_path) {
                error!("Failed to remove id file: {}", e);
            } else {
                info!("Removed id file {:?}", id_path);
            }
        }

        Ok(())
    })();
    if let Err(e) = r {
        error!("{}", e.to_string());
        Err(e)
    } else {
        Ok(())
    }
}
