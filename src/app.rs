use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Result};
use tracing::{info, warn};

use crate::config::config::Config;
use crate::config::save_thread::SaveThread;
use crate::config::state::AppState;
use crate::desktop_cover::DesktopCover;
use crate::paths::{app_file, STATE_PATH};

pub struct App {
    pub cover: OnceLock<Arc<DesktopCover>>,
    pub save_thread: OnceLock<SaveThread>,
    pub config: OnceLock<Arc<Config>>,
}

/// Assume that the APP is always initialized and the [App::get()] api to access.
pub static APP: OnceLock<App> = OnceLock::new();

impl App {
    pub fn get() -> &'static Self {
        APP.get().expect("App not initialized")
    }

    pub fn config() -> Arc<Config> {
        Self::get().config.get().expect("Config not loaded").clone()
    }

    pub fn is_config_loaded() -> bool {
        Self::get().config.get().is_some()
    }

    pub fn save_state(&self) -> Result<()> {
        let cover = self.cover.get().unwrap();
        let state = cover.state();
        let path = app_file(STATE_PATH)?;
        let json = serde_json::to_string_pretty(&state)?;
        std::fs::write(&path, json)?;
        info!("State saved to {:?}", path);
        Ok(())
    }

    pub fn load_state(&self) -> Result<()> {
        let path = app_file(STATE_PATH)?;
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

    pub fn load_config(&self) -> Result<()> {
        let path = app_file("config.json")?;
        if path.exists() {
            let json = std::fs::read_to_string(&path)?;
            let cfg: Config = serde_json::from_str(&json)?;
            self.config
                .set(Arc::new(cfg))
                .map_err(|_| anyhow!("Config already set"))?;
        } else {
            let cfg = Config::default();
            self.config
                .set(Arc::new(cfg.clone()))
                .map_err(|_| anyhow!("Config already set"))?;
            let json = serde_json::to_string_pretty(&cfg)?;
            std::fs::write(&path, json)?;
            info!("Config file created at {:?}", path);
        }
        Ok(())
    }

    pub fn save_config(&self) -> Result<()> {
        let path = app_file("config.json")?;
        let cfg = self.config.get().expect("Config not loaded");
        let json = serde_json::to_string_pretty(&**cfg)?;
        std::fs::write(&path, json)?;
        info!("Config saved to {:?}", path);
        Ok(())
    }
}
