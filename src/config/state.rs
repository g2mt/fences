use std::sync::Arc;

use serde::{Deserialize, Serialize};
use win_wrapper::geo::Area;

#[derive(Serialize, Deserialize, Default)]
pub struct AppState {
    #[serde(default)]
    pub fences: Vec<FenceState>,
    #[serde(default)]
    pub screen_width: i32,
    #[serde(default)]
    pub screen_height: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum FenceStickyPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct FenceState {
    #[serde(default)]
    pub title: Arc<str>,
    #[serde(default)]
    pub area: Area<i32>,
    #[serde(default)]
    pub icons: Vec<IconState>,
    #[serde(default)]
    pub imported_from: Option<Arc<str>>,
    #[serde(default)]
    pub sticky_pos: Option<FenceStickyPosition>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct IconState {
    #[serde(default)]
    pub title: Arc<str>,
    #[serde(default)]
    pub path: Option<Arc<str>>,
}
