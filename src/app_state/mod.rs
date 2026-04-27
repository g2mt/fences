pub mod save_thread;

use serde::{Deserialize, Serialize};

use crate::fence::FenceState;

#[derive(Serialize, Deserialize)]
pub struct AppState {
    pub fences: Vec<FenceState>,
}
