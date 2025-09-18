//! Distributed locking mechanisms for storage coordination

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use super::error::{StorageError, StorageResult};

/// Storage lock information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageLock {
    /// Unique lock key
    pub key: String,
    /// Lock holder identifier
    pub holder: String,
    /// When the lock was acquired
    pub acquired_at: DateTime<Utc>,
    /// Time to live for the lock
    pub ttl: Duration,
    /// Lock token for verification
    pub token: String,
}

impl StorageLock {
    /// Create a new storage lock
    pub fn new(key: String, holder: String, ttl: Duration) -> Self {
        Self {
            key,
            holder,
            acquired_at: Utc::now(),
            ttl,
            token: Uuid::new_v4().to_string(),
        }
    }

    /// Check if the lock has expired
    pub fn is_expired(&self) -> bool {
        let expiry = self.acquired_at + chrono::Duration::from_std(self.ttl).unwrap();
        Utc::now() > expiry
    }

    /// Remaining time before lock expires
    pub fn remaining_ttl(&self) -> Option<Duration> {
        let expiry = self.acquired_at + chrono::Duration::from_std(self.ttl).unwrap();
        let remaining = expiry - Utc::now();

        if remaining > chrono::Duration::zero() {
            remaining.to_std().ok()
        } else {
            None
        }
    }
}

/// Lock guard that automatically releases the lock when dropped
#[async_trait]
pub trait StorageLockGuard: Send + Sync {
    /// Get the lock information
    fn lock_info(&self) -> &StorageLock;

    /// Explicitly release the lock
    async fn release(self: Box<Self>) -> StorageResult<()>;

    /// Extend the lock TTL
    async fn extend(&mut self, additional_ttl: Duration) -> StorageResult<()>;

    /// Check if the lock is still valid
    async fn is_valid(&self) -> StorageResult<bool>;
}

/// File-based lock guard implementation
pub struct FileLockGuard {
    lock: StorageLock,
    lock_file: std::path::PathBuf,
}

impl FileLockGuard {
    /// Create a new file lock guard
    pub fn new(lock: StorageLock, lock_file: std::path::PathBuf) -> Self {
        Self { lock, lock_file }
    }
}

#[async_trait]
impl StorageLockGuard for FileLockGuard {
    fn lock_info(&self) -> &StorageLock {
        &self.lock
    }

    async fn release(self: Box<Self>) -> StorageResult<()> {
        tokio::fs::remove_file(&self.lock_file)
            .await
            .map_err(|e| StorageError::lock(format!("Failed to release lock: {}", e)))?;
        Ok(())
    }

    async fn extend(&mut self, additional_ttl: Duration) -> StorageResult<()> {
        self.lock.ttl += additional_ttl;

        // Update lock file modification time
        let _metadata = tokio::fs::metadata(&self.lock_file).await?;
        let _modified = std::time::SystemTime::now();

        // Note: Setting modification time is platform-specific
        // For now, we'll just update the lock info
        Ok(())
    }

    async fn is_valid(&self) -> StorageResult<bool> {
        if self.lock.is_expired() {
            return Ok(false);
        }

        // Check if lock file still exists
        Ok(self.lock_file.exists())
    }
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        // Best-effort cleanup in drop
        if self.lock_file.exists() {
            let _ = std::fs::remove_file(&self.lock_file);
        }
    }
}

/// Lock manager for coordinating distributed locks
pub struct LockManager {
    backend: Arc<dyn LockBackend>,
}

impl LockManager {
    /// Create a new lock manager
    pub fn new(backend: Arc<dyn LockBackend>) -> Self {
        Self { backend }
    }

    /// Acquire a lock with retry logic
    pub async fn acquire_with_retry(
        &self,
        key: &str,
        holder: &str,
        ttl: Duration,
        max_retries: u32,
        retry_delay: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>> {
        let mut attempts = 0;

        loop {
            match self.backend.try_acquire(key, holder, ttl).await {
                Ok(guard) => return Ok(guard),
                Err(e) if e.is_conflict() && attempts < max_retries => {
                    attempts += 1;
                    tokio::time::sleep(retry_delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Wait for a lock to become available
    pub async fn wait_for_lock(
        &self,
        key: &str,
        holder: &str,
        ttl: Duration,
        timeout: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>> {
        let deadline = tokio::time::Instant::now() + timeout;
        let retry_delay = Duration::from_millis(100);

        while tokio::time::Instant::now() < deadline {
            match self.backend.try_acquire(key, holder, ttl).await {
                Ok(guard) => return Ok(guard),
                Err(e) if e.is_conflict() => {
                    tokio::time::sleep(retry_delay).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(StorageError::Timeout(timeout))
    }
}

/// Backend trait for different lock implementations
#[async_trait]
pub trait LockBackend: Send + Sync {
    /// Try to acquire a lock
    async fn try_acquire(
        &self,
        key: &str,
        holder: &str,
        ttl: Duration,
    ) -> StorageResult<Box<dyn StorageLockGuard>>;

    /// Check if a lock exists
    async fn exists(&self, key: &str) -> StorageResult<bool>;

    /// Force release a lock (admin operation)
    async fn force_release(&self, key: &str) -> StorageResult<()>;

    /// List all active locks
    async fn list_locks(&self) -> StorageResult<Vec<StorageLock>>;
}
