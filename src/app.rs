use std::sync::{Arc, OnceLock};

use anyhow::Result;
use tracing::{info, warn};

use crate::config::state::AppState;
use crate::desktop_cover::DesktopCover;
use crate::paths;

pub struct App {
    pub cover: OnceLock<Arc<DesktopCover>>,
}

/// Assume that the APP is always initialized and use the following to get the App:
/// ```
/// APP.get().unwrap()
/// ```
pub static APP: OnceLock<App> = OnceLock::new();

impl App {
    pub fn save_state(&self) -> Result<()> {
        let cover = self.cover.get().unwrap();
        let state = cover.state();
        let path = paths::get_state_path()?;
        let json = serde_json::to_string_pretty(&state)?;
        std::fs::write(&path, json)?;
        info!("State saved to {:?}", path);
        Ok(())
    }

    pub fn load_state(&self) -> Result<()> {
        let path = paths::get_state_path()?;
        if !path.exists() {
            warn!("No state file found at {:?}", path);
            return Ok(());
        }
        let json = std::fs::read_to_string(&path)?;
        let state: AppState = serde_json::from_str(&json)?;
        info!("Loading state from {:?}", path);
        let cover = self.cover.get().unwrap();
        cover.set_state(&state)?;
        Ok(())
    }
}
