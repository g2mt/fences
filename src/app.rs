use std::path::PathBuf;
use std::sync::atomic::AtomicI32;
use std::sync::{Arc, LazyLock, OnceLock};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use tracing::{error, info, warn};

use crate::config::config::Config;
use crate::config::save_thread::SaveThread;
use crate::config::state::AppState;
use crate::desktop_cover::DesktopCover;
use crate::desktop_mirror::DesktopMirror;
use crate::fence::import_dialog::ImportDialog;
use crate::geo::Bounds;
use crate::paths::{app_file, STATE_PATH};

#[derive(Default)]
pub struct App {
    pub cover: OnceLock<Arc<DesktopCover>>,
    pub mirror: Mutex<DesktopMirror>,
    pub save_thread: OnceLock<SaveThread>,
    pub config: OnceLock<Arc<Config>>,
    pub import_dialog: Mutex<Option<Arc<ImportDialog>>>,
    pub id_path: OnceLock<PathBuf>,
    pub screen_bounds: OnceLock<Bounds<AtomicI32>>,
}

/// Assume that the singleton is always initialized and the [App::get()] api to access.
static SINGLETON: LazyLock<App> = LazyLock::new(|| App::default());

impl App {
    pub fn get() -> &'static Self {
        &SINGLETON
    }

    pub fn screen_bounds(&self) -> &Bounds<AtomicI32> {
        self.screen_bounds.get_or_init(move || Bounds {
            width: AtomicI32::new(0),
            height: AtomicI32::new(0),
        })
    }

    pub fn config() -> Arc<Config> {
        Self::get().config.get().expect("Config not loaded").clone()
    }

    pub fn remove_id_path(&self) {
        let id_path = self.id_path.get().unwrap();
        if let Err(e) = std::fs::remove_file(&id_path) {
            error!("Failed to remove id file: {}", e);
        } else {
            info!("Removed id file {:?}", id_path);
        }
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
