//! Storage abstraction layer for Prodigy
//!
//! This module provides a unified storage interface for managing
//! Prodigy's data in a global ~/.prodigy directory structure.

pub mod config;
pub mod error;
pub mod factory;
pub mod global;
pub mod lock;
pub mod migrate;
pub mod types;

#[cfg(test)]
mod tests;

pub use config::{BackendConfig, BackendType};
pub use error::{StorageError, StorageResult};
pub use factory::StorageFactory;
pub use global::GlobalStorage;
pub use lock::{StorageLock, StorageLockGuard};
pub use migrate::{MigrationConfig, MigrationStats, StorageMigrator};
pub use types::{
    CheckpointFilter, DLQFilter, EventFilter, EventStats, EventStream, EventSubscription,
    HealthStatus, SessionFilter, SessionId, SessionState, WorkflowFilter,
};

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

/// Initialize the storage subsystem
pub async fn init_from_env() -> StorageResult<GlobalStorage> {
    StorageFactory::from_env().await
}

/// Legacy storage configuration for custom paths (deprecated - use config::StorageConfig)
#[derive(Debug, Clone)]
pub struct LegacyStorageConfig {
    /// Base directory for storage (default: ~/.prodigy)
    pub base_dir: PathBuf,
}

impl Default for LegacyStorageConfig {
    fn default() -> Self {
        Self {
            base_dir: get_default_storage_dir().unwrap_or_else(|_| PathBuf::from(".prodigy")),
        }
    }
}

impl LegacyStorageConfig {
    /// Create config from environment variables
    pub fn from_env() -> Self {
        if let Ok(dir) = std::env::var("PRODIGY_STORAGE_DIR") {
            Self {
                base_dir: PathBuf::from(dir),
            }
        } else {
            Self::default()
        }
    }

    /// Create config with custom base directory
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
}

/// Extract repository name from a project path
/// This matches the logic used by WorktreeManager
pub fn extract_repo_name(repo_path: &Path) -> Result<String> {
    // Canonicalize the path to handle symlinks
    let canonical_path = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    // Extract the last component as the repository name
    canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            anyhow!(
                "Could not determine repository name from path: {}",
                repo_path.display()
            )
        })
}

/// Get the default storage directory (~/.prodigy)
pub fn get_default_storage_dir() -> Result<PathBuf> {
    // During tests, use a temp directory to avoid filesystem issues
    #[cfg(test)]
    {
        use std::sync::OnceLock;
        static TEST_DIR: OnceLock<PathBuf> = OnceLock::new();
        let test_dir = TEST_DIR.get_or_init(|| {
            let temp_dir =
                std::env::temp_dir().join(format!("prodigy-test-{}", std::process::id()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            temp_dir
        });
        Ok(test_dir.clone())
    }

    #[cfg(not(test))]
    {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not determine home directory"))
            .map(|home| home.join(".prodigy"))
    }
}

/// Discover all available DLQ job IDs
pub async fn discover_dlq_job_ids(project_root: &Path) -> Result<Vec<String>> {
    let storage = GlobalStorage::new()?;
    let repo_name = extract_repo_name(project_root)?;
    storage.list_dlq_job_ids(&repo_name).await
}

/// Create a new event logger with global storage
pub async fn create_global_event_logger(
    repo_path: &Path,
    job_id: &str,
) -> Result<crate::cook::execution::events::EventLogger> {
    use crate::cook::execution::events::{EventLogger, EventWriter, JsonlEventWriter};

    let storage = GlobalStorage::new()?;
    let repo_name = extract_repo_name(repo_path)?;
    let events_dir = storage.get_events_dir(&repo_name, job_id).await?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let event_file = events_dir.join(format!("events-{}.jsonl", timestamp));

    let writer: Box<dyn EventWriter> = Box::new(
        JsonlEventWriter::new(event_file)
            .await
            .context("Failed to create global event writer")?,
    );

    Ok(EventLogger::new(vec![writer]))
}

/// Create a new DLQ with global storage
pub async fn create_global_dlq(
    repo_path: &Path,
    job_id: &str,
    event_logger: Option<std::sync::Arc<crate::cook::execution::events::EventLogger>>,
) -> Result<crate::cook::execution::dlq::DeadLetterQueue> {
    use crate::cook::execution::dlq::DeadLetterQueue;

    let storage = GlobalStorage::new()?;
    let repo_name = extract_repo_name(repo_path)?;
    let dlq_dir = storage.get_dlq_dir(&repo_name, job_id).await?;

    DeadLetterQueue::new(
        job_id.to_string(),
        dlq_dir,
        1000, // max_items
        30,   // retention_days
        event_logger,
    )
    .await
}

/// Migration utilities for transitioning from local to global storage
pub mod migration {
    use super::*;
    use tokio::fs;
    use tracing::info;

    /// Check if a project has local storage that needs migration
    pub async fn has_local_storage(project_path: &Path) -> bool {
        let local_dir = project_path.join(".prodigy");
        local_dir.exists() && local_dir.is_dir()
    }

    /// Migrate local storage to global storage
    pub async fn migrate_to_global(project_path: &Path) -> Result<()> {
        let local_dir = project_path.join(".prodigy");

        if !local_dir.exists() {
            return Ok(());
        }

        let storage = GlobalStorage::new()?;
        let repo_name = extract_repo_name(project_path)?;

        info!(
            "Migrating local storage to global for repository: {}",
            repo_name
        );

        // Migrate events
        let local_events = local_dir.join("events");
        if local_events.exists() {
            migrate_directory(
                &local_events,
                &storage.base_dir().join("events").join(&repo_name),
            )
            .await?;
        }

        // Migrate DLQ
        let local_dlq = local_dir.join("dlq");
        if local_dlq.exists() {
            migrate_directory(&local_dlq, &storage.base_dir().join("dlq").join(&repo_name)).await?;
        }

        // Migrate state
        let local_state = local_dir.join("state");
        if local_state.exists() {
            migrate_directory(
                &local_state,
                &storage.base_dir().join("state").join(&repo_name),
            )
            .await?;
        }

        // Migrate checkpoints
        let local_checkpoints = local_dir.join("checkpoints");
        if local_checkpoints.exists() {
            migrate_directory(
                &local_checkpoints,
                &storage
                    .base_dir()
                    .join("state")
                    .join(&repo_name)
                    .join("checkpoints"),
            )
            .await?;
        }

        // Migrate session state files
        let session_state = local_dir.join("session_state.json");
        if session_state.exists() {
            let target_dir = storage
                .base_dir()
                .join("state")
                .join(&repo_name)
                .join("sessions");
            fs::create_dir_all(&target_dir).await?;
            fs::copy(&session_state, target_dir.join("session_state.json")).await?;
        }

        info!("Migration completed successfully");

        // Always remove local directory after successful migration
        fs::remove_dir_all(&local_dir)
            .await
            .context("Failed to remove local storage after migration")?;
        info!("Removed local storage directory after successful migration");

        Ok(())
    }

    async fn migrate_directory(from: &Path, to: &Path) -> Result<()> {
        use tokio::fs;

        if !from.exists() {
            return Ok(());
        }

        // Create target directory
        fs::create_dir_all(to)
            .await
            .context("Failed to create target directory")?;

        // Copy contents recursively
        copy_dir_recursive(from, to).await?;

        info!("Migrated {} to {}", from.display(), to.display());
        Ok(())
    }

    async fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
        let mut entries = fs::read_dir(from)
            .await
            .context("Failed to read source directory")?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_name = entry.file_name();
            let target_path = to.join(&file_name);

            if path.is_dir() {
                fs::create_dir_all(&target_path).await?;
                Box::pin(copy_dir_recursive(&path, &target_path)).await?;
            } else {
                fs::copy(&path, &target_path).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod mod_tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::fs;

    #[test]
    fn test_extract_repo_name() {
        // Test with simple path
        let path = Path::new("/home/user/projects/my-repo");
        assert_eq!(extract_repo_name(path).unwrap(), "my-repo");

        // Test with trailing slash
        let path = Path::new("/home/user/projects/another-repo/");
        assert_eq!(extract_repo_name(path).unwrap(), "another-repo");
    }

    #[tokio::test]
    async fn test_global_storage_paths() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        std::fs::create_dir(&repo_path).unwrap();

        let storage = GlobalStorage::new().unwrap();
        let repo_name = "test-repo";

        // Test events directory creation
        let events_dir = storage.get_events_dir(repo_name, "job-123").await.unwrap();
        assert!(events_dir.exists());
        assert!(events_dir.ends_with("events/test-repo/job-123"));

        // Test DLQ directory creation
        let dlq_dir = storage.get_dlq_dir(repo_name, "job-123").await.unwrap();
        assert!(dlq_dir.exists());
        assert!(dlq_dir.ends_with("dlq/test-repo/job-123"));

        // Test state directory creation
        let state_dir = storage.get_state_dir(repo_name, "job-123").await.unwrap();
        assert!(state_dir.exists());
        assert!(state_dir.ends_with("state/test-repo/job-123"));
    }

    #[tokio::test]
    async fn test_cross_worktree_event_aggregation() {
        use serde_json::json;

        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        std::fs::create_dir(&repo_path).unwrap();

        let job_id = "shared-job-123";
        let repo_name = "test-repo";

        // Create storage instances for each worktree
        let storage1 = GlobalStorage::new().unwrap();
        let storage2 = GlobalStorage::new().unwrap();

        // Both should resolve to the same global event directory
        let events_dir1 = storage1.get_events_dir(repo_name, job_id).await.unwrap();
        let events_dir2 = storage2.get_events_dir(repo_name, job_id).await.unwrap();

        assert_eq!(events_dir1, events_dir2);
        assert!(events_dir1.ends_with("events/test-repo/shared-job-123"));

        // Write events from both worktrees
        let event_file1 = events_dir1.join("events-wt1.jsonl");
        let event_file2 = events_dir2.join("events-wt2.jsonl");

        fs::write(
            &event_file1,
            json!({"worktree": 1, "event": "test"}).to_string() + "\n",
        )
        .await
        .unwrap();
        fs::write(
            &event_file2,
            json!({"worktree": 2, "event": "test"}).to_string() + "\n",
        )
        .await
        .unwrap();

        // Verify both event files exist in the same directory
        assert!(event_file1.exists());
        assert!(event_file2.exists());

        // Read all events from the shared directory
        let mut entries = fs::read_dir(&events_dir1).await.unwrap();
        let mut event_files = Vec::new();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                event_files.push(entry.path());
            }
        }

        assert_eq!(event_files.len(), 2);
    }

    #[tokio::test]
    async fn test_global_storage_isolation_between_repos() {
        let _temp_dir = TempDir::new().unwrap();

        let storage1 = GlobalStorage::new().unwrap();
        let storage2 = GlobalStorage::new().unwrap();

        let job_id = "same-job-id";

        // Get event directories for the same job ID but different repos
        let events_dir1 = storage1.get_events_dir("repo-one", job_id).await.unwrap();
        let events_dir2 = storage2.get_events_dir("repo-two", job_id).await.unwrap();

        // Ensure they are different (isolated by repo name)
        assert_ne!(events_dir1, events_dir2);
        assert!(events_dir1.ends_with("events/repo-one/same-job-id"));
        assert!(events_dir2.ends_with("events/repo-two/same-job-id"));
    }

    #[tokio::test]
    async fn test_list_dlq_job_ids() {
        let _temp_dir = TempDir::new().unwrap();
        let storage = GlobalStorage::new().unwrap();
        let repo_name = "test-repo";

        // Initially no job IDs
        let job_ids = storage.list_dlq_job_ids(repo_name).await.unwrap();
        assert!(job_ids.is_empty());

        // Create some DLQ directories with items
        for i in 1..=3 {
            let job_id = format!("job-{}", i);
            let dlq_dir = storage.get_dlq_dir(repo_name, &job_id).await.unwrap();
            let items_dir = dlq_dir.join("items");
            fs::create_dir_all(&items_dir).await.unwrap();

            // Create a dummy item file
            let item_file = items_dir.join("item-1.json");
            fs::write(&item_file, "{}").await.unwrap();
        }

        // Create a directory without items (should not be listed)
        let empty_job_dir = storage.get_dlq_dir(repo_name, "empty-job").await.unwrap();
        fs::create_dir_all(&empty_job_dir.join("items"))
            .await
            .unwrap();

        let job_ids = storage.list_dlq_job_ids(repo_name).await.unwrap();
        assert_eq!(job_ids.len(), 3);

        // Should be sorted in reverse order (most recent first)
        assert_eq!(job_ids, vec!["job-3", "job-2", "job-1"]);
    }

    #[tokio::test]
    async fn test_discover_dlq_job_ids() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().join("test-project");
        std::fs::create_dir(&project_path).unwrap();

        // Always uses global storage now
        let job_ids = discover_dlq_job_ids(&project_path).await.unwrap();
        assert!(job_ids.is_empty());

        // Create global DLQ structure
        let storage = GlobalStorage::new().unwrap();
        let repo_name = extract_repo_name(&project_path).unwrap();

        // Create job directory with items
        let job_dir = storage.get_dlq_dir(&repo_name, "test-job").await.unwrap();
        let items_dir = job_dir.join("items");
        fs::create_dir_all(&items_dir).await.unwrap();

        // Add an item to make it a valid DLQ job
        let item_file = items_dir.join("item-1.json");
        fs::write(&item_file, "{}").await.unwrap();

        let job_ids = storage.list_dlq_job_ids(&repo_name).await.unwrap();
        assert_eq!(job_ids.len(), 1);
        assert_eq!(job_ids[0], "test-job");
    }
}
