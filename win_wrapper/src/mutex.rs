use std::panic::Location;
#[cfg(not(feature = "parking_lot"))]
use std::sync as inner;
use std::time::Duration;

#[cfg(feature = "parking_lot")]
use parking_lot as inner;

/// A platform-abstracted mutex that delegates to `parking_lot::Mutex` when the
/// `parking_lot` feature is enabled, falling back to `std::sync::Mutex`
/// otherwise.
#[derive(Default)]
pub struct Mutex<T> {
    inner: inner::Mutex<T>,
}

#[cfg(feature = "parking_lot")]
pub type MutexGuard<'a, T> = parking_lot::MutexGuard<'a, T>;

#[cfg(not(feature = "parking_lot"))]
pub type MutexGuard<'a, T> = std::sync::MutexGuard<'a, T>;

const LOCK_TIMEOUT: Duration = Duration::from_secs(5);

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub fn new(val: T) -> Self {
        Self {
            inner: inner::Mutex::new(val),
        }
    }

    /// Acquires a mutex, blocking the current thread for up to 5 seconds.
    ///
    /// Upon returning, the returned [`MutexGuard`] will release the lock when
    /// it is dropped.
    ///
    /// # Panics
    ///
    /// Panics if the lock is not acquired within 5 seconds (deadlock or severe
    /// contention). When the `parking_lot` feature is **not** enabled, this
    /// also panics if another thread panicked while holding the lock
    /// (poisoning).
    #[inline]
    #[track_caller]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        fn lock_timed_out(caller: &'static Location<'static>) -> ! {
            panic!("mutex lock timed out at {caller}");
        }

        #[cfg(feature = "parking_lot")]
        {
            match self.inner.try_lock_for(LOCK_TIMEOUT) {
                Some(guard) => guard,
                None => lock_timed_out(std::panic::Location::caller()),
            }
        }
        #[cfg(not(feature = "parking_lot"))]
        {
            let deadline = std::time::Instant::now() + LOCK_TIMEOUT;
            loop {
                match self.inner.try_lock() {
                    Ok(guard) => return guard,
                    Err(std::sync::TryLockError::Poisoned(e)) => return e.into_inner(),
                    Err(std::sync::TryLockError::WouldBlock) => {
                        if std::time::Instant::now() >= deadline {
                            lock_timed_out(std::panic::Location::caller());
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        }
    }
}
