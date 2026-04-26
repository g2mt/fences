use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use crate::window::{WinHandle, Window};

pub struct App {
    windows: BTreeMap<WinHandle, Mutex<Box<dyn Window>>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            windows: BTreeMap::new(),
        }
    }

    pub fn add_window(&mut self, window: Box<dyn Window>) {
        self.windows.insert(window.handle(), Mutex::new(window));
    }

    pub fn window(&self, handle: WinHandle) -> Option<&Mutex<Box<dyn Window>>> {
        self.windows.get(&handle)
    }
}

pub static APP: OnceLock<Mutex<App>> = OnceLock::new();
