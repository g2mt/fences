use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use crate::window::{Window, WinHandle};

pub struct App {
    pub windows: BTreeMap<WinHandle, Mutex<Box<dyn Window>>>,
}

impl App {
    pub fn add_window(&mut self, window: Box<dyn Window>) {
        self.windows.insert(window.handle(), Mutex::new(window));
    }
}

pub static APP: OnceLock<Mutex<App>> = OnceLock::new();
