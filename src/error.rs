use std::error::Error;
use std::fmt;
use windows_sys::Win32::Foundation::GetLastError;

/// A Win32 error wrapping the value from `GetLastError`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WinError(pub i32);

impl WinError {
    /// Creates a `WinError` from the calling thread's last-error code.
    pub fn last_error() -> Self {
        WinError(unsafe { GetLastError() as i32 })
    }
}

impl fmt::Display for WinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Win32 error {}", self.0)
    }
}

impl Error for WinError {}
