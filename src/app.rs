use std::path::PathBuf;
use std::sync::atomic::AtomicI32;
use std::sync::{Arc, LazyLock, OnceLock};

use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use tracing::{error, info, warn};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;

use crate::config::config::Config;
use crate::config::save_thread::SaveThread;
use crate::config::state::AppState;
use crate::desktop_cover::DesktopCover;
use crate::desktop_mirror::DesktopMirror;
use crate::fence::{FenceList, ImportDialog};
use crate::geo::Bounds;
use crate::paths::{STATE_PATH, app_file};
use crate::utils::HWNDWrapper;

#[derive(Default)]
pub struct App {
    pub cover: OnceLock<Arc<DesktopCover>>,
    pub mirror: Mutex<DesktopMirror>,
    pub screen_bounds: OnceLock<Bounds<AtomicI32>>,
    pub hwnd_shell: OnceLock<HWNDWrapper>,
    pub save_thread: OnceLock<SaveThread>,
    pub config: OnceLock<Arc<Config>>,
    pub import_dialog: Mutex<Option<Arc<ImportDialog>>>,
    pub id_path: OnceLock<PathBuf>,
    pub fences: Mutex<FenceList>,
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
            let json = serde_json::to_string_pretty(&cfg)?;
            self.config
                .set(Arc::new(cfg))
                .map_err(|_| anyhow!("Config already set"))?;
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

    pub fn draw_text(&self, hdc: HDC, text: &str, rect: &mut RECT, format: DRAW_TEXT_FORMAT) {
        let config = Self::config();
        let font_name_u16: Vec<u16> = config
            .font
            .family
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            let hfont = CreateFontW(
                -config.font.size,
                0,
                0,
                0,
                FW_NORMAL.0 as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                VARIABLE_PITCH.0 as u32,
                windows::core::PCWSTR(font_name_u16.as_ptr()),
            );

            let old_font = SelectObject(hdc, hfont.into());

            let mut text_u16: Vec<u16> = text.encode_utf16().collect();
            DrawTextW(hdc, &mut text_u16, rect, format);

            SelectObject(hdc, old_font.into());
            let _ = DeleteObject(hfont.into());
        }
    }
}
