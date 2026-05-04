use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use tracing::{error, info, warn};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::*;

use crate::config::config::Config;
use crate::config::save_thread::SaveThread;
use crate::config::state::{AppState, FenceState, FenceStickyPosition};
use crate::desktop_cover::DesktopCover;
use crate::desktop_mirror::DesktopMirror;
use crate::fence::import_dialog::ImportDialog;
use crate::fence::{Fence, HitType};
use crate::geo::Bounds;
use crate::paths::{app_file, STATE_PATH};
use crate::utils::HWNDWrapper;
use crate::window::Window;

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
    pub fences: Mutex<AppFences>,
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

#[derive(Default)]
pub struct AppFences {
    /// List of fences currently managed by the desktop cover.
    items: Vec<Arc<Fence>>,
    /// The type of hit test result from the last interaction, used for dragging or context menus.
    hit_type: Option<HitType>,
}

impl AppFences {
    pub fn items(&self) -> &[Arc<Fence>] {
        &self.items
    }

    pub fn hit_type(&self) -> Option<HitType> {
        self.hit_type
    }

    pub fn select(&mut self, x: i32, y: i32) -> Option<(&Arc<Fence>, HitType)> {
        // Clear existing selections on all fences
        for fence in &self.items {
            fence.clear_selection();
        }

        // Find fence in reverse order
        let mut hit_idx: Option<(usize, HitType)> = None;
        for (i, fence) in self.items.iter().enumerate().rev() {
            if let Some(hit) = fence.hit_test(x, y) {
                hit_idx = Some((i, hit));
                break;
            }
        }

        if let Some((idx, hit)) = hit_idx {
            // Remove the fence at this index
            let fence = self.items.remove(idx);

            // Select the icon inside the fence if applicable
            if let HitType::Icon(icon_idx) = hit {
                fence.select_icon(icon_idx);
            }

            // Bring the window to front and push to end of items list
            fence.base().bring_to_front();
            self.items.push(fence);

            self.hit_type = Some(hit);
            Some((self.items.last().unwrap(), hit))
        } else {
            self.hit_type = None;
            None
        }
    }

    #[must_use]
    pub fn release_hit_type(&mut self) -> Option<HitType> {
        self.hit_type.take()
    }

    pub fn set_state(&mut self, cover: &DesktopCover, fence_states: &[FenceState]) -> Result<()> {
        self.items.clear();
        for f_state in fence_states {
            let fence = Fence::from_state(cover, f_state.clone())?;
            self.items.push(fence);
        }
        Ok(())
    }

    pub fn add(&mut self, fence: Arc<Fence>) {
        self.items.push(fence);
        self.hit_type = None;
    }

    pub fn remove(&mut self, fence: &Arc<Fence>) {
        if let Some(pos) = self.items.iter().position(|f| Arc::ptr_eq(f, fence)) {
            self.items.remove(pos);
        }
        self.hit_type = None;
    }

    pub fn rearrange(&mut self, old_screen_width: i32, old_screen_height: i32) {
        let bounds = App::get().screen_bounds();
        let new_width = bounds.width.load(Ordering::Relaxed);
        let new_height = bounds.height.load(Ordering::Relaxed);

        if old_screen_width == new_width && old_screen_height == new_height {
            return;
        }

        info!(
            "rearranging from {}x{} to {}x{}",
            old_screen_width, old_screen_height, new_width, new_height
        );
        for fence in &self.items {
            if let Some(sticky) = fence.sticky() {
                let area = fence.get_state().area;
                let (new_x, new_y) = match sticky {
                    FenceStickyPosition::TopLeft => (area.x, area.y),
                    FenceStickyPosition::TopRight => {
                        let offset_from_right = old_screen_width - (area.x + area.width);
                        (new_width - area.width - offset_from_right, area.y)
                    }
                    FenceStickyPosition::BottomLeft => {
                        let offset_from_bottom = old_screen_height - (area.y + area.height);
                        (area.x, new_height - area.height - offset_from_bottom)
                    }
                    FenceStickyPosition::BottomRight => {
                        let offset_from_right = old_screen_width - (area.x + area.width);
                        let offset_from_bottom = old_screen_height - (area.y + area.height);
                        (
                            new_width - area.width - offset_from_right,
                            new_height - area.height - offset_from_bottom,
                        )
                    }
                };
                fence.base().move_to(new_x, new_y);
            }
        }

        App::get().save_thread.get().unwrap().set_unsaved();
    }
}
