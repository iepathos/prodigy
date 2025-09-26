//! Global storage implementation for Prodigy

use super::error::StorageResult;
use super::types::HealthStatus;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Global storage implementation using file system
pub struct GlobalStorage {
    /// Base directory for all storage (default: ~/.prodigy)
    base_dir: PathBuf,
}

impl GlobalStorage {
    /// Create a new global storage instance
    pub fn new() -> StorageResult<Self> {
        let base_dir = super::get_default_storage_dir()?;
        Ok(Self { base_dir })
    }

    /// Create a new global storage instance with a custom base directory (for testing)
    #[cfg(test)]
    pub fn new_with_root(base_dir: PathBuf) -> StorageResult<Self> {
        Ok(Self { base_dir })
    }

    /// Get the base directory for storage
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Get the global events directory for a repository
    pub async fn get_events_dir(&self, repo_name: &str, job_id: &str) -> Result<PathBuf> {
        let path = self.base_dir.join("events").join(repo_name).join(job_id);

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global events directory")?;

        Ok(path)
    }

    /// Get the global DLQ directory for a repository
    pub async fn get_dlq_dir(&self, repo_name: &str, job_id: &str) -> Result<PathBuf> {
        let path = self.base_dir.join("dlq").join(repo_name).join(job_id);

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global DLQ directory")?;

        Ok(path)
    }

    /// Get the global state directory for a repository
    pub async fn get_state_dir(&self, repo_name: &str, job_id: &str) -> Result<PathBuf> {
        let path = self.base_dir.join("state").join(repo_name).join(job_id);

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global state directory")?;

        Ok(path)
    }

    /// Get the global checkpoints directory for a repository
    pub async fn get_checkpoints_dir(&self, repo_name: &str) -> Result<PathBuf> {
        let path = self
            .base_dir
            .join("state")
            .join(repo_name)
            .join("checkpoints");

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global checkpoints directory")?;

        Ok(path)
    }

    /// List all job IDs with DLQ data for a repository
    pub async fn list_dlq_job_ids(&self, repo_name: &str) -> Result<Vec<String>> {
        let dlq_repo_dir = self.base_dir.join("dlq").join(repo_name);

        if !dlq_repo_dir.exists() {
            return Ok(vec![]);
        }

        let mut job_ids = Vec::new();
        let mut entries = fs::read_dir(dlq_repo_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(job_id) = entry.file_name().to_str() {
                    // Check if this job has any DLQ items
                    let items_dir = entry.path().join("items");
                    if items_dir.exists() {
                        let mut items_entries = fs::read_dir(&items_dir).await?;
                        if items_entries.next_entry().await?.is_some() {
                            job_ids.push(job_id.to_string());
                        }
                    }
                }
            }
        }

        // Sort by name (which includes timestamps)
        job_ids.sort();
        job_ids.reverse(); // Most recent first
        Ok(job_ids)
    }

    /// Health check for the storage
    pub async fn health_check(&self) -> StorageResult<HealthStatus> {
        Ok(HealthStatus {
            healthy: true,
            backend_type: "global".to_string(),
            message: Some("Global file storage is operational".to_string()),
            details: None,
        })
    }

    // Storage interface methods have been removed as part of the unified session refactor.
    // All storage operations are now handled through the unified session manager.
}
