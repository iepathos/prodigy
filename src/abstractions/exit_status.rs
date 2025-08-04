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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_raw_success() {
        let status = std::process::ExitStatus::from_raw(0);
        assert!(status.success());
        // On Unix, exit status might be encoded differently
        #[cfg(unix)]
        {
            // On Unix, from_raw(0) should be success
            assert!(status.success());
        }
        #[cfg(not(unix))]
        {
            assert_eq!(status.code(), Some(0));
        }
    }

    #[test]
    fn test_from_raw_failure() {
        let status = std::process::ExitStatus::from_raw(256); // Exit code 1 is typically 256 on Unix
        assert!(!status.success());
        // On Unix, exit status might be encoded differently
        #[cfg(unix)]
        {
            // from_raw(256) represents exit code 1 on Unix
            assert!(!status.success());
        }
        #[cfg(not(unix))]
        {
            assert_eq!(status.code(), Some(1));
        }
    }
}
