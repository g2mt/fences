use std::sync::OnceLock;

use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber::prelude::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod app;
mod config;
mod desktop_cover;
mod fence;
mod geo;
mod paths;
mod prompt;
mod window;

use crate::app::APP;
use crate::config::save_thread::SaveThread;
use crate::desktop_cover::DesktopCover;
use crate::paths::{app_file, LOG_PATH};

fn main() -> Result<()> {
    APP.get_or_init(|| app::App {
        cover: OnceLock::new(),
        save_thread: OnceLock::new(),
    });

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
        APP.get().unwrap().cover.get_or_init(|| cover.clone());
        let save_thread = SaveThread::new();
        APP.get().unwrap().save_thread.set(save_thread).unwrap();
        if let Err(e) = APP.get().unwrap().load_state() {
            error!("{}", e.to_string());
        }
        unsafe {
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            info!("Message loop stopped");
        }
        if let Err(e) = APP.get().unwrap().save_state() {
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
