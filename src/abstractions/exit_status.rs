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
    fn from_raw(raw: i32) -> Self {
        // On non-Unix platforms, we can't create ExitStatus from raw value
        // We'll use a workaround by running a command that exits with the given code
        if raw == 0 {
            std::process::Command::new("cmd")
                .args(&["/c", "exit 0"])
                .status()
                .unwrap_or_else(|_| {
                    // Fallback for non-Windows
                    std::process::Command::new("true").status().unwrap()
                })
        } else {
            std::process::Command::new("cmd")
                .args(&["/c", &format!("exit {}", raw)])
                .status()
                .unwrap_or_else(|_| {
                    // Fallback for non-Windows
                    std::process::Command::new("false").status().unwrap()
                })
        }
    }
}
