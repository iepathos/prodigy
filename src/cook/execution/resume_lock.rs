//! Resume lock management for concurrent resume protection
//!
//! Provides RAII-based locking to ensure only one resume process can execute
//! per session/job at a time. Includes stale lock detection and platform-specific
//! process existence checking.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use tracing::{info, warn};

/// Metadata stored in lock file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeLockData {
    pub job_id: String,
    pub process_id: u32,
    pub hostname: String,
    pub acquired_at: DateTime<Utc>,
}

impl ResumeLockData {
    pub fn new(job_id: String) -> Self {
        Self {
            job_id,
            process_id: std::process::id(),
            hostname: get_hostname(),
            acquired_at: Utc::now(),
        }
    }
}

/// Get current hostname
fn get_hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Manager for resume lock acquisition and release
#[derive(Clone, Debug)]
pub struct ResumeLockManager {
    locks_dir: PathBuf,
}

impl ResumeLockManager {
    /// Create new lock manager
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        let locks_dir = storage_dir.join("resume_locks");

        // Ensure locks directory exists
        std::fs::create_dir_all(&locks_dir)
            .with_context(|| format!("Failed to create locks directory: {:?}", locks_dir))?;

        Ok(Self { locks_dir })
    }

    /// Acquire exclusive lock for job/session
    ///
    /// Returns Ok(ResumeLock) if lock acquired successfully.
    /// Returns Err if lock already held by active process.
    pub fn acquire_lock<'a>(
        &'a self,
        job_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ResumeLock>> + Send + 'a>> {
        Box::pin(async move {
            let lock_path = self.get_lock_path(job_id);

            // Try to create lock file atomically
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true) // Atomic: fails if file exists
                .open(&lock_path)
                .await
            {
                Ok(mut file) => {
                    // Write lock metadata
                    let lock_data = ResumeLockData::new(job_id.to_string());
                    let json = serde_json::to_string_pretty(&lock_data)?;

                    tokio::io::AsyncWriteExt::write_all(&mut file, json.as_bytes())
                        .await
                        .context("Failed to write lock data")?;

                    info!("Acquired resume lock for {}", job_id);

                    Ok(ResumeLock {
                        job_id: job_id.to_string(),
                        lock_path,
                        manager: self.clone(),
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Lock exists - check if stale
                    match self.check_and_cleanup_stale_lock(job_id).await {
                        Ok(true) => {
                            // Stale lock removed, retry
                            warn!("Removed stale lock for {}, retrying", job_id);
                            return self.acquire_lock(job_id).await;
                        }
                        Ok(false) => {
                            // Active lock
                            let lock_info = self
                                .read_lock_info(job_id)
                                .await
                                .unwrap_or_else(|_| "unknown process".to_string());

                            Err(anyhow!(
                                "Resume already in progress for job {}\n\
                                 Lock held by: {}\n\
                                 Please wait for the other process to complete, or use --force to override.",
                                job_id,
                                lock_info
                            ))
                        }
                        Err(cleanup_err) => Err(anyhow!(
                            "Failed to check lock status for {}: {}",
                            job_id,
                            cleanup_err
                        )),
                    }
                }
                Err(e) => Err(e.into()),
            }
        })
    }

    /// Check if lock is stale and clean up if so
    ///
    /// Returns Ok(true) if stale lock was removed
    /// Returns Ok(false) if lock is active
    async fn check_and_cleanup_stale_lock(&self, job_id: &str) -> Result<bool> {
        let lock_path = self.get_lock_path(job_id);

        // Read lock data
        let contents = tokio::fs::read_to_string(&lock_path)
            .await
            .context("Failed to read lock file")?;

        let lock_data: ResumeLockData =
            serde_json::from_str(&contents).context("Failed to parse lock data")?;

        // Check if process is still running
        if !is_process_running(lock_data.process_id) {
            warn!(
                "Removing stale lock for {} (PID {} no longer running)",
                job_id, lock_data.process_id
            );

            tokio::fs::remove_file(&lock_path)
                .await
                .context("Failed to remove stale lock")?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Read human-readable lock information
    async fn read_lock_info(&self, job_id: &str) -> Result<String> {
        let lock_path = self.get_lock_path(job_id);
        let contents = tokio::fs::read_to_string(&lock_path).await?;
        let lock_data: ResumeLockData = serde_json::from_str(&contents)?;

        Ok(format!(
            "PID {} on {} (acquired {})",
            lock_data.process_id,
            lock_data.hostname,
            lock_data.acquired_at.format("%Y-%m-%d %H:%M:%S UTC")
        ))
    }

    fn get_lock_path(&self, job_id: &str) -> PathBuf {
        self.locks_dir.join(format!("{}.lock", job_id))
    }
}

/// RAII guard for resume lock
///
/// Automatically releases lock when dropped
#[derive(Debug)]
pub struct ResumeLock {
    job_id: String,
    lock_path: PathBuf,
    #[allow(dead_code)]
    manager: ResumeLockManager,
}

impl Drop for ResumeLock {
    fn drop(&mut self) {
        // Clean up lock file
        if let Err(e) = std::fs::remove_file(&self.lock_path) {
            warn!("Failed to release lock for {}: {}", self.job_id, e);
        } else {
            info!("Released resume lock for {}", self.job_id);
        }
    }
}

/// Check if a process with given PID is running
///
/// Platform-specific implementation
pub fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;

        // Use kill -0 to check process existence without killing it
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        use std::process::Command;

        // Use tasklist to check process existence
        Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid), "/NH"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.contains(&pid.to_string()))
            })
            .unwrap_or(false)
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Unsupported platform - assume process is running to be safe
        warn!("Process detection not supported on this platform");
        true
    }
}
