//! Platform-specific exit status handling
//!
//! Provides a cross-platform way to create `ExitStatus` from raw values

#[cfg(unix)]
pub use std::os::unix::process::ExitStatusExt;

#[cfg(not(unix))]
pub trait ExitStatusExt {
    fn from_raw(raw: i32) -> Self;
}

#[cfg(not(unix))]
impl ExitStatusExt for std::process::ExitStatus {
    fn from_raw(_raw: i32) -> Self {
        // On non-Unix platforms, we can't create ExitStatus from raw value
        // Return a default success status
        std::process::Command::new("true")
            .status()
            .unwrap_or_else(|_| {
                std::process::Command::new("cmd")
                    .args(&["/c", "exit 0"])
                    .status()
                    .unwrap()
            })
    }
}
