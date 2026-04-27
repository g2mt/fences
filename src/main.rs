use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber::prelude::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod app_state;
mod desktop_cover;
mod fence;
mod geo;
mod paths;
mod window;

use crate::app_state::save_thread::SaveThread;
use crate::desktop_cover::DesktopCover;

fn main() -> Result<()> {
    let log_path = paths::get_log_path()?;
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

        unsafe {
            let cover = DesktopCover::new()?;
            let save_thread = SaveThread::new(cover.clone());
            cover.set_save_thread(save_thread);
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            info!("Message loop stopped");
            cover.save_state();

            // dropped at the end of program
            drop(cover);
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
