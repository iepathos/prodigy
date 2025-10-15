//! Session-Job mapping for bidirectional lookup between session IDs and MapReduce job IDs
//!
//! This module provides a mapping between workflow session IDs and MapReduce job IDs,
//! enabling resume operations to work with either ID type.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

/// Bidirectional mapping between session IDs and MapReduce job IDs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionJobMapping {
    /// The session ID (e.g., "session-1234567890")
    pub session_id: String,

    /// The MapReduce job ID (e.g., "mapreduce-1234567890")
    pub job_id: String,

    /// The name of the workflow being executed
    pub workflow_name: String,

    /// When this mapping was created
    pub created_at: DateTime<Utc>,
}

impl SessionJobMapping {
    /// Create a new session-job mapping
    pub fn new(session_id: String, job_id: String, workflow_name: String) -> Self {
        Self {
            session_id,
            job_id,
            workflow_name,
            created_at: Utc::now(),
        }
    }

    /// Store this mapping to disk
    pub async fn store(&self, storage_dir: &Path) -> Result<()> {
        let mapping_dir = storage_dir.join("mappings");
        fs::create_dir_all(&mapping_dir).await?;

        // Store by both session ID and job ID for bidirectional lookup
        let session_file = mapping_dir.join(format!("{}.json", self.session_id));
        let job_file = mapping_dir.join(format!("{}.json", self.job_id));

        let json = serde_json::to_vec_pretty(self)?;

        fs::write(&session_file, &json).await?;
        fs::write(&job_file, &json).await?;

        Ok(())
    }

    /// Load a mapping by session ID
    pub async fn load_by_session(session_id: &str, storage_dir: &Path) -> Result<Option<Self>> {
        let mapping_file = storage_dir
            .join("mappings")
            .join(format!("{}.json", session_id));

        if !mapping_file.exists() {
            return Ok(None);
        }

        let data = fs::read(&mapping_file).await?;
        let mapping: Self =
            serde_json::from_slice(&data).context("Failed to deserialize session-job mapping")?;

        Ok(Some(mapping))
    }

    /// Load a mapping by job ID
    pub async fn load_by_job(job_id: &str, storage_dir: &Path) -> Result<Option<Self>> {
        let mapping_file = storage_dir
            .join("mappings")
            .join(format!("{}.json", job_id));

        if !mapping_file.exists() {
            return Ok(None);
        }

        let data = fs::read(&mapping_file).await?;
        let mapping: Self =
            serde_json::from_slice(&data).context("Failed to deserialize session-job mapping")?;

        Ok(Some(mapping))
    }

    /// Check if a mapping exists for a session ID
    pub async fn exists_for_session(session_id: &str, storage_dir: &Path) -> bool {
        storage_dir
            .join("mappings")
            .join(format!("{}.json", session_id))
            .exists()
    }

    /// Check if a mapping exists for a job ID
    pub async fn exists_for_job(job_id: &str, storage_dir: &Path) -> bool {
        storage_dir
            .join("mappings")
            .join(format!("{}.json", job_id))
            .exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_session_job_mapping_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage_dir = temp_dir.path();

        let mapping = SessionJobMapping::new(
            "session-123".to_string(),
            "mapreduce-456".to_string(),
            "test-workflow".to_string(),
        );

        // Store the mapping
        mapping.store(&storage_dir).await.unwrap();

        // Load by session ID
        let loaded_by_session = SessionJobMapping::load_by_session("session-123", &storage_dir)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded_by_session.session_id, "session-123");
        assert_eq!(loaded_by_session.job_id, "mapreduce-456");
        assert_eq!(loaded_by_session.workflow_name, "test-workflow");

        // Load by job ID
        let loaded_by_job = SessionJobMapping::load_by_job("mapreduce-456", &storage_dir)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded_by_job.session_id, "session-123");
        assert_eq!(loaded_by_job.job_id, "mapreduce-456");
        assert_eq!(loaded_by_job.workflow_name, "test-workflow");
    }

    #[tokio::test]
    async fn test_mapping_existence_checks() {
        let temp_dir = TempDir::new().unwrap();
        let storage_dir = temp_dir.path();

        // Initially should not exist
        assert!(!SessionJobMapping::exists_for_session("session-123", &storage_dir).await);
        assert!(!SessionJobMapping::exists_for_job("mapreduce-456", &storage_dir).await);

        // Store a mapping
        let mapping = SessionJobMapping::new(
            "session-123".to_string(),
            "mapreduce-456".to_string(),
            "test-workflow".to_string(),
        );
        mapping.store(&storage_dir).await.unwrap();

        // Now should exist
        assert!(SessionJobMapping::exists_for_session("session-123", &storage_dir).await);
        assert!(SessionJobMapping::exists_for_job("mapreduce-456", &storage_dir).await);
    }

    #[tokio::test]
    async fn test_load_nonexistent_mapping() {
        let temp_dir = TempDir::new().unwrap();
        let storage_dir = temp_dir.path();

        let result = SessionJobMapping::load_by_session("nonexistent", &storage_dir)
            .await
            .unwrap();
        assert!(result.is_none());

        let result = SessionJobMapping::load_by_job("nonexistent", &storage_dir)
            .await
            .unwrap();
        assert!(result.is_none());
    }
}
