//! Global storage management for events, DLQ, and job state
//!
//! This module provides centralized storage under ~/.prodigy/ to enable
//! cross-worktree data sharing and persistent job state management.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Global storage paths configuration
pub struct GlobalStorage {
    /// Base directory for all global storage (~/.prodigy)
    base_dir: PathBuf,
    /// Repository name extracted from project path
    repo_name: String,
}

impl GlobalStorage {
    /// Create a new global storage instance for a repository
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo_name = extract_repo_name(repo_path)?;
        let base_dir = get_global_base_dir()?;

        Ok(Self {
            base_dir,
            repo_name,
        })
    }

    /// Get the global events directory for this repository
    pub async fn get_events_dir(&self, job_id: &str) -> Result<PathBuf> {
        let path = self
            .base_dir
            .join("events")
            .join(&self.repo_name)
            .join(job_id);

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global events directory")?;

        Ok(path)
    }

    /// Get the global DLQ directory for this repository
    pub async fn get_dlq_dir(&self, job_id: &str) -> Result<PathBuf> {
        let path = self.base_dir.join("dlq").join(&self.repo_name).join(job_id);

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global DLQ directory")?;

        Ok(path)
    }

    /// Get the global state directory for this repository
    pub async fn get_state_dir(&self, job_id: &str) -> Result<PathBuf> {
        let path = self
            .base_dir
            .join("state")
            .join(&self.repo_name)
            .join(job_id);

        fs::create_dir_all(&path)
            .await
            .context("Failed to create global state directory")?;

        Ok(path)
    }

    /// Get repository name for this storage instance
    pub fn repo_name(&self) -> &str {
        &self.repo_name
    }

    /// List all job IDs with DLQ data for this repository
    pub async fn list_dlq_job_ids(&self) -> Result<Vec<String>> {
        let dlq_repo_dir = self.base_dir.join("dlq").join(&self.repo_name);

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

    /// Check if we should use global storage based on environment
    pub fn should_use_global() -> bool {
        // Check for opt-out environment variable
        if std::env::var("PRODIGY_USE_LOCAL_STORAGE").unwrap_or_default() == "true" {
            return false;
        }

        // Default to global storage
        true
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

/// Get the global base directory (~/.prodigy)
pub fn get_global_base_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not determine home directory"))
        .map(|home| home.join(".prodigy"))
}

/// List DLQ job IDs from local storage (fallback for legacy support)
pub async fn list_local_dlq_job_ids(project_root: &Path) -> Result<Vec<String>> {
    let local_dlq_dir = project_root.join(".prodigy").join("dlq");

    if !local_dlq_dir.exists() {
        return Ok(vec![]);
    }

    let mut job_ids = Vec::new();
    let mut entries = fs::read_dir(local_dlq_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        if let Some(name) = entry.file_name().to_str() {
            if name.ends_with(".json") {
                job_ids.push(name.trim_end_matches(".json").to_string());
            }
        }
    }

    job_ids.sort();
    job_ids.reverse(); // Most recent first
    Ok(job_ids)
}

/// Discover all available DLQ job IDs, checking both global and local storage
pub async fn discover_dlq_job_ids(project_root: &Path) -> Result<Vec<String>> {
    if GlobalStorage::should_use_global() {
        let storage = GlobalStorage::new(project_root)?;
        storage.list_dlq_job_ids().await
    } else {
        list_local_dlq_job_ids(project_root).await
    }
}

/// Create a new event logger with global storage
pub async fn create_global_event_logger(
    repo_path: &Path,
    job_id: &str,
) -> Result<crate::cook::execution::events::EventLogger> {
    use crate::cook::execution::events::{EventLogger, EventWriter, JsonlEventWriter};

    let storage = GlobalStorage::new(repo_path)?;
    let events_dir = storage.get_events_dir(job_id).await?;

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

    let storage = GlobalStorage::new(repo_path)?;
    let dlq_dir = storage.get_dlq_dir(job_id).await?;

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
    use tracing::{info, warn};

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

        let storage = GlobalStorage::new(project_path)?;
        info!(
            "Migrating local storage to global for repository: {}",
            storage.repo_name()
        );

        // Migrate events
        let local_events = local_dir.join("events");
        if local_events.exists() {
            migrate_directory(
                &local_events,
                &storage.base_dir.join("events").join(&storage.repo_name),
            )
            .await?;
        }

        // Migrate DLQ
        let local_dlq = local_dir.join("dlq");
        if local_dlq.exists() {
            migrate_directory(
                &local_dlq,
                &storage.base_dir.join("dlq").join(&storage.repo_name),
            )
            .await?;
        }

        // Migrate state
        let local_state = local_dir.join("state");
        if local_state.exists() {
            migrate_directory(
                &local_state,
                &storage.base_dir.join("state").join(&storage.repo_name),
            )
            .await?;
        }

        info!("Migration completed successfully");

        // Optionally remove local directory
        if std::env::var("PRODIGY_REMOVE_LOCAL_AFTER_MIGRATION").unwrap_or_default() == "true" {
            fs::remove_dir_all(&local_dir)
                .await
                .context("Failed to remove local storage after migration")?;
            info!("Removed local storage directory");
        } else {
            warn!(
                "Local storage directory preserved at: {}",
                local_dir.display()
            );
        }

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
mod tests {
    use super::*;
    use tempfile::TempDir;

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

        let storage = GlobalStorage::new(&repo_path).unwrap();

        // Test events directory creation
        let events_dir = storage.get_events_dir("job-123").await.unwrap();
        assert!(events_dir.exists());
        assert!(events_dir.ends_with("events/test-repo/job-123"));

        // Test DLQ directory creation
        let dlq_dir = storage.get_dlq_dir("job-123").await.unwrap();
        assert!(dlq_dir.exists());
        assert!(dlq_dir.ends_with("dlq/test-repo/job-123"));

        // Test state directory creation
        let state_dir = storage.get_state_dir("job-123").await.unwrap();
        assert!(state_dir.exists());
        assert!(state_dir.ends_with("state/test-repo/job-123"));
    }

    #[tokio::test]
    async fn test_cross_worktree_event_aggregation() {
        use serde_json::json;

        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        std::fs::create_dir(&repo_path).unwrap();

        // Simulate multiple worktrees accessing the same job
        let worktree1_path = temp_dir.path().join("worktree1").join("test-repo");
        std::fs::create_dir_all(&worktree1_path).unwrap();
        let worktree2_path = temp_dir.path().join("worktree2").join("test-repo");
        std::fs::create_dir_all(&worktree2_path).unwrap();

        let job_id = "shared-job-123";

        // Create storage instances for each worktree
        let storage1 = GlobalStorage::new(&worktree1_path).unwrap();
        let storage2 = GlobalStorage::new(&worktree2_path).unwrap();

        // Both should resolve to the same global event directory
        let events_dir1 = storage1.get_events_dir(job_id).await.unwrap();
        let events_dir2 = storage2.get_events_dir(job_id).await.unwrap();

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
        let temp_dir = TempDir::new().unwrap();

        // Create two different repositories
        let repo1_path = temp_dir.path().join("repo-one");
        let repo2_path = temp_dir.path().join("repo-two");
        std::fs::create_dir(&repo1_path).unwrap();
        std::fs::create_dir(&repo2_path).unwrap();

        let storage1 = GlobalStorage::new(&repo1_path).unwrap();
        let storage2 = GlobalStorage::new(&repo2_path).unwrap();

        let job_id = "same-job-id";

        // Get event directories for the same job ID but different repos
        let events_dir1 = storage1.get_events_dir(job_id).await.unwrap();
        let events_dir2 = storage2.get_events_dir(job_id).await.unwrap();

        // Ensure they are different (isolated by repo name)
        assert_ne!(events_dir1, events_dir2);
        assert!(events_dir1.ends_with("events/repo-one/same-job-id"));
        assert!(events_dir2.ends_with("events/repo-two/same-job-id"));
    }

    #[tokio::test]
    async fn test_list_dlq_job_ids() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        std::fs::create_dir(&repo_path).unwrap();

        // Create a GlobalStorage instance with a test base directory
        let storage = GlobalStorage {
            base_dir: temp_dir.path().join("test-global"),
            repo_name: "test-repo".to_string(),
        };

        // Initially no job IDs
        let job_ids = storage.list_dlq_job_ids().await.unwrap();
        assert!(job_ids.is_empty());

        // Create some DLQ directories with items
        for i in 1..=3 {
            let job_id = format!("job-{}", i);
            let dlq_dir = storage.get_dlq_dir(&job_id).await.unwrap();
            let items_dir = dlq_dir.join("items");
            fs::create_dir_all(&items_dir).await.unwrap();

            // Create a dummy item file
            let item_file = items_dir.join("item-1.json");
            fs::write(&item_file, "{}").await.unwrap();
        }

        // Create a directory without items (should not be listed)
        let empty_job_dir = storage.get_dlq_dir("empty-job").await.unwrap();
        fs::create_dir_all(&empty_job_dir.join("items"))
            .await
            .unwrap();

        let job_ids = storage.list_dlq_job_ids().await.unwrap();
        assert_eq!(job_ids.len(), 3);

        // Should be sorted in reverse order (most recent first)
        assert_eq!(job_ids, vec!["job-3", "job-2", "job-1"]);
    }

    #[tokio::test]
    async fn test_local_dlq_discovery() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().join("test-project");
        std::fs::create_dir(&project_path).unwrap();

        // Initially no job IDs
        let job_ids = list_local_dlq_job_ids(&project_path).await.unwrap();
        assert!(job_ids.is_empty());

        // Create local DLQ structure
        let dlq_dir = project_path.join(".prodigy").join("dlq");
        fs::create_dir_all(&dlq_dir).await.unwrap();

        // Create job files
        for i in 1..=3 {
            let job_file = dlq_dir.join(format!("job-{}.json", i));
            fs::write(&job_file, "{}").await.unwrap();
        }

        let job_ids = list_local_dlq_job_ids(&project_path).await.unwrap();
        assert_eq!(job_ids.len(), 3);
        assert_eq!(job_ids, vec!["job-3", "job-2", "job-1"]);
    }

    #[tokio::test]
    async fn test_discover_dlq_job_ids() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().join("test-project");
        std::fs::create_dir(&project_path).unwrap();

        // Test with local storage (override global default)
        std::env::set_var("PRODIGY_USE_LOCAL_STORAGE", "true");

        let job_ids = discover_dlq_job_ids(&project_path).await.unwrap();
        assert!(job_ids.is_empty());

        // Create local DLQ structure
        let dlq_dir = project_path.join(".prodigy").join("dlq");
        fs::create_dir_all(&dlq_dir).await.unwrap();
        let job_file = dlq_dir.join("test-job.json");
        fs::write(&job_file, "{}").await.unwrap();

        let job_ids = discover_dlq_job_ids(&project_path).await.unwrap();
        assert_eq!(job_ids.len(), 1);
        assert_eq!(job_ids[0], "test-job");

        // Clean up environment variable
        std::env::remove_var("PRODIGY_USE_LOCAL_STORAGE");
    }
}
