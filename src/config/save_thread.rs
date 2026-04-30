use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::app::App;

#[derive(Debug)]
pub struct SaveThread {
    flag: Arc<AtomicBool>,
    _handle: thread::JoinHandle<()>,
}

impl SaveThread {
    /// Creates a new `SaveThread` that will periodically save the state of
    /// `cover` (at most once every five seconds) whenever `set_unsaved()`
    /// has been called.
    pub fn new() -> Self {
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();

        let handle = thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(3));
                if flag_clone
                    .compare_exchange(true, false, Ordering::Acquire, Ordering::Acquire)
                    .is_ok()
                {
                    let _ = App::get().save_state();
                }
            }
        });

        SaveThread {
            flag,
            _handle: handle,
        }
    }

    /// Marks the state as unsaved. The background thread will persist the
    /// state on its next scheduled run (within five seconds).
    pub fn set_unsaved(&self) {
        self.flag.store(true, Ordering::Release);
    }
}
