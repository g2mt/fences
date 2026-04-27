use serde::{Deserialize, Serialize};

use crate::fence::FenceState;

#[derive(Serialize, Deserialize)]
pub struct AppState {
    fences: Vec<FenceState>,
}
