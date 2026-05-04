use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use crate::app::App;
use crate::config::state::{FenceState, FenceStickyPosition};
use crate::desktop_cover::DesktopCover;
use crate::fence::fence::Fence;
use crate::window::Window;

#[derive(Default)]
pub struct Fences {
    /// List of fences currently managed by the desktop cover.
    items: Vec<Arc<Fence>>,
}

impl Fences {
    pub fn items(&self) -> &[Arc<Fence>] {
        &self.items
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
    }

    pub fn remove(&mut self, fence: &Arc<Fence>) {
        if let Some(pos) = self.items.iter().position(|f| Arc::ptr_eq(f, fence)) {
            self.items.remove(pos);
        }
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
