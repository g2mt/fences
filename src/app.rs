use std::sync::{Arc, OnceLock};

use crate::desktop_cover::DesktopCover;

pub struct App {
    pub cover: OnceLock<Arc<DesktopCover>>,
}

pub static APP: OnceLock<App> = OnceLock::new();
