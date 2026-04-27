use serde::{Deserialize, Serialize};

use crate::fence::FenceState;

pub mod save_thread;

#[derive(Serialize, Deserialize)]
pub struct AppState {
    pub fences: Vec<FenceState>,
}
