use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::geo::Area;

#[derive(Serialize, Deserialize)]
pub struct AppState {
    pub fences: Vec<FenceState>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FenceState {
    pub title: Arc<str>,
    pub area: Area<i32>,
    pub icons: Vec<IconState>,
    pub imported_from: Option<Arc<str>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IconState {
    pub title: Arc<str>,
    pub path: Option<Arc<str>>,
}
