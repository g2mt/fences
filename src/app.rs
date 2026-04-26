use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::window::{WinHandle, Window};

pub struct App {
    windows: BTreeMap<WinHandle, Pin<Arc<dyn Window>>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            windows: BTreeMap::new(),
        }
    }

    pub fn add_window<T: Window>(&mut self, window: T) -> Pin<Arc<T>> {
        let handle = window.handle();
        let r = Arc::pin(window);
        self.windows.insert(handle, r.clone());
        r
    }

    pub fn window(&self, handle: WinHandle) -> Option<&Pin<Arc<dyn Window>>> {
        self.windows.get(&handle)
    }
}

pub static APP: OnceLock<Mutex<App>> = OnceLock::new();

pub fn lock_app() -> MutexGuard<'static, App> {
    APP.get().unwrap().lock().unwrap()
}
