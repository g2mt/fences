#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::OnceLock;

use anyhow::Result;
use parking_lot::Mutex;
use tracing::{error, info};
use tracing_subscriber::prelude::*;
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
use crate::paths::{app_file, init_app_dir, LOG_PATH};

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

        Ok(())
    })();
    if let Err(e) = r {
        error!("{}", e.to_string());
        Err(e)
    } else {
        Ok(())
    }
}
