//! Storage cleanup module for managing Prodigy storage lifecycle
//!
//! Provides comprehensive cleanup capabilities for all Prodigy storage types:
//! - Worktrees (session and MapReduce)
//! - Session state and checkpoints
//! - Claude execution logs
//! - MapReduce job state
//! - Event logs
//! - Dead Letter Queue data

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

use super::GlobalStorage;

/// Configuration for cleanup operations
#[derive(Debug, Clone, Default)]
pub struct CleanupConfig {
    /// Only clean items older than this duration
    pub older_than: Option<Duration>,
    /// Preview what would be cleaned without making changes
    pub dry_run: bool,
    /// Skip all confirmations
    pub force: bool,
}

/// Statistics from a cleanup operation
#[derive(Debug, Clone, Default)]
pub struct CleanupStats {
    /// Number of items scanned
    pub items_scanned: usize,
    /// Number of items removed
    pub items_removed: usize,
    /// Bytes reclaimed from cleanup
    pub bytes_reclaimed: u64,
    /// Errors encountered during cleanup
    pub errors: Vec<String>,
}

impl CleanupStats {
    /// Create a new empty stats struct
    pub fn new() -> Self {
        Self::default()
    }

    /// Add another stats instance to this one
    pub fn merge(&mut self, other: &CleanupStats) {
        self.items_scanned += other.items_scanned;
        self.items_removed += other.items_removed;
        self.bytes_reclaimed += other.bytes_reclaimed;
        self.errors.extend(other.errors.clone());
    }
}

/// Storage statistics across all types
#[derive(Debug, Clone, Default)]
pub struct StorageStats {
    /// Bytes used by worktrees
    pub worktrees_bytes: u64,
    /// Bytes used by session state
    pub sessions_bytes: u64,
    /// Bytes used by Claude logs
    pub logs_bytes: u64,
    /// Bytes used by MapReduce state
    pub state_bytes: u64,
    /// Bytes used by event logs
    pub events_bytes: u64,
    /// Bytes used by DLQ data
    pub dlq_bytes: u64,
    /// Total bytes across all storage
    pub total_bytes: u64,
}

impl StorageStats {
    /// Create a new empty stats struct
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate total from individual components
    pub fn calculate_total(&mut self) {
        self.total_bytes = self.worktrees_bytes
            + self.sessions_bytes
            + self.logs_bytes
            + self.state_bytes
            + self.events_bytes
            + self.dlq_bytes;
    }

    /// Format bytes as human-readable string
    pub fn format_bytes(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.2} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

/// Manager for storage cleanup operations
pub struct StorageCleanupManager {
    storage: GlobalStorage,
    repo_name: String,
}

impl StorageCleanupManager {
    /// Create a new cleanup manager for a specific repository
    pub fn new(storage: GlobalStorage, repo_name: String) -> Self {
        Self { storage, repo_name }
    }

    /// Get storage statistics for current repository
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        let mut stats = StorageStats::new();

        // Calculate worktrees size
        let worktrees_dir = self.storage.get_worktrees_dir(&self.repo_name).await?;
        stats.worktrees_bytes = calculate_dir_size(&worktrees_dir).await?;

        // Calculate sessions size (stored in state dir)
        if let Ok(state_base_dir) = self.storage.get_state_base_dir(&self.repo_name).await {
            stats.sessions_bytes = calculate_dir_size(&state_base_dir).await?;
        }

        // Calculate logs size (Claude logs are in ~/.prodigy/logs)
        if let Ok(logs_dir) = self.storage.get_logs_dir(&self.repo_name).await {
            stats.logs_bytes = calculate_dir_size(&logs_dir).await?;
        }

        // Calculate state size (MapReduce state)
        if let Ok(state_base_dir) = self.storage.get_state_base_dir(&self.repo_name).await {
            let mapreduce_dir = state_base_dir.join("mapreduce");
            if mapreduce_dir.exists() {
                stats.state_bytes = calculate_dir_size(&mapreduce_dir).await?;
            }
        }

        // Calculate events size
        if let Ok(events_base_dir) = self.storage.get_events_base_dir(&self.repo_name).await {
            stats.events_bytes = calculate_dir_size(&events_base_dir).await?;
        }

        // Calculate DLQ size
        if let Ok(dlq_base_dir) = self.storage.get_dlq_base_dir(&self.repo_name).await {
            stats.dlq_bytes = calculate_dir_size(&dlq_base_dir).await?;
        }

        stats.calculate_total();
        Ok(stats)
    }

    /// Clean worktrees older than specified duration
    pub async fn clean_worktrees(&self, config: &CleanupConfig) -> Result<CleanupStats> {
        let worktrees_dir = self.storage.get_worktrees_dir(&self.repo_name).await?;
        clean_directory_by_age(&worktrees_dir, config).await
    }

    /// Clean session state older than specified duration
    pub async fn clean_sessions(&self, config: &CleanupConfig) -> Result<CleanupStats> {
        let state_dir = self.storage.get_state_base_dir(&self.repo_name).await?;
        clean_directory_by_age(&state_dir, config).await
    }

    /// Clean Claude execution logs older than specified duration
    pub async fn clean_logs(&self, config: &CleanupConfig) -> Result<CleanupStats> {
        let logs_dir = self.storage.get_logs_dir(&self.repo_name).await?;
        clean_directory_by_age(&logs_dir, config).await
    }

    /// Clean MapReduce job state older than specified duration
    pub async fn clean_state(&self, config: &CleanupConfig) -> Result<CleanupStats> {
        let state_base_dir = self.storage.get_state_base_dir(&self.repo_name).await?;
        let mapreduce_dir = state_base_dir.join("mapreduce");

        if !mapreduce_dir.exists() {
            return Ok(CleanupStats::new());
        }

        clean_directory_by_age(&mapreduce_dir, config).await
    }

    /// Clean event logs older than specified duration
    pub async fn clean_events(&self, config: &CleanupConfig) -> Result<CleanupStats> {
        let events_dir = self.storage.get_events_base_dir(&self.repo_name).await?;
        clean_directory_by_age(&events_dir, config).await
    }

    /// Clean DLQ data older than specified duration
    pub async fn clean_dlq(&self, config: &CleanupConfig) -> Result<CleanupStats> {
        let dlq_dir = self.storage.get_dlq_base_dir(&self.repo_name).await?;
        clean_directory_by_age(&dlq_dir, config).await
    }

    /// Clean all storage types
    pub async fn clean_all(&self, config: &CleanupConfig) -> Result<HashMap<String, CleanupStats>> {
        let mut results = HashMap::new();

        results.insert("worktrees".to_string(), self.clean_worktrees(config).await?);
        results.insert("sessions".to_string(), self.clean_sessions(config).await?);
        results.insert("logs".to_string(), self.clean_logs(config).await?);
        results.insert("state".to_string(), self.clean_state(config).await?);
        results.insert("events".to_string(), self.clean_events(config).await?);
        results.insert("dlq".to_string(), self.clean_dlq(config).await?);

        Ok(results)
    }
}

/// Calculate total size of a directory recursively
async fn calculate_dir_size(dir: &Path) -> Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }

    let mut total_size = 0u64;
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries = match fs::read_dir(&current).await {
            Ok(entries) => entries,
            Err(_) => continue, // Skip directories we can't read
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = match fs::metadata(&path).await {
                Ok(m) => m,
                Err(_) => continue, // Skip entries we can't stat
            };

            if metadata.is_dir() {
                stack.push(path);
            } else {
                total_size += metadata.len();
            }
        }
    }

    Ok(total_size)
}

/// Clean a directory by removing items older than specified duration
async fn clean_directory_by_age(dir: &Path, config: &CleanupConfig) -> Result<CleanupStats> {
    let mut stats = CleanupStats::new();

    if !dir.exists() {
        return Ok(stats);
    }

    let cutoff_time = config.older_than.map(|duration| Utc::now() - duration);

    let mut entries = fs::read_dir(dir)
        .await
        .context("Failed to read directory")?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        stats.items_scanned += 1;

        // Get modification time
        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) => {
                stats
                    .errors
                    .push(format!("Failed to stat {}: {}", path.display(), e));
                continue;
            }
        };

        // Check if item is old enough to clean
        if let Some(cutoff) = cutoff_time {
            if let Ok(modified) = metadata.modified() {
                let modified_time: DateTime<Utc> = modified.into();
                if modified_time >= cutoff {
                    continue; // Item is too new, skip
                }
            }
        }

        // Calculate size before deletion
        let size = if metadata.is_dir() {
            calculate_dir_size(&path).await.unwrap_or(0)
        } else {
            metadata.len()
        };

        // Delete if not dry-run
        if !config.dry_run {
            if let Err(e) = remove_path(&path).await {
                stats
                    .errors
                    .push(format!("Failed to remove {}: {}", path.display(), e));
                continue;
            }
        }

        stats.items_removed += 1;
        stats.bytes_reclaimed += size;
    }

    Ok(stats)
}

/// Remove a file or directory recursively
async fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path).await?;

    if metadata.is_dir() {
        fs::remove_dir_all(path).await?;
    } else {
        fs::remove_file(path).await?;
    }

    Ok(())
}

/// Parse duration string (e.g., "7d", "24h", "30m")
pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (num_str, unit) = s.split_at(s.len().saturating_sub(1));

    let num: i64 = num_str
        .parse()
        .with_context(|| format!("Invalid duration number: {}", num_str))?;

    match unit {
        "s" => Ok(Duration::seconds(num)),
        "m" => Ok(Duration::minutes(num)),
        "h" => Ok(Duration::hours(num)),
        "d" => Ok(Duration::days(num)),
        _ => Err(anyhow::anyhow!(
            "Invalid duration unit: {}. Use s, m, h, or d",
            unit
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::seconds(30));
        assert_eq!(parse_duration("15m").unwrap(), Duration::minutes(15));
        assert_eq!(parse_duration("24h").unwrap(), Duration::hours(24));
        assert_eq!(parse_duration("7d").unwrap(), Duration::days(7));

        assert!(parse_duration("invalid").is_err());
        assert!(parse_duration("7x").is_err());
    }

    #[tokio::test]
    async fn test_calculate_dir_size() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create test files
        fs::write(dir.join("file1.txt"), "hello").await.unwrap();
        fs::write(dir.join("file2.txt"), "world").await.unwrap();

        let size = calculate_dir_size(dir).await.unwrap();
        assert_eq!(size, 10); // "hello" + "world" = 10 bytes
    }

    #[tokio::test]
    async fn test_clean_directory_by_age() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create test files
        fs::write(dir.join("old1.txt"), "old content")
            .await
            .unwrap();
        fs::write(dir.join("old2.txt"), "more old").await.unwrap();

        let config = CleanupConfig {
            older_than: Some(Duration::seconds(0)), // Clean everything
            dry_run: false,
            force: true,
        };

        let stats = clean_directory_by_age(dir, &config).await.unwrap();
        assert_eq!(stats.items_scanned, 2);
        assert_eq!(stats.items_removed, 2);
    }

    #[tokio::test]
    async fn test_dry_run_doesnt_delete() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create test file
        let test_file = dir.join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        let config = CleanupConfig {
            older_than: Some(Duration::seconds(0)),
            dry_run: true,
            force: true,
        };

        let stats = clean_directory_by_age(dir, &config).await.unwrap();
        assert_eq!(stats.items_scanned, 1);
        assert_eq!(stats.items_removed, 1);

        // File should still exist
        assert!(test_file.exists());
    }
}
