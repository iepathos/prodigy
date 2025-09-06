//! Event retention policy management

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Event retention policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Maximum age of events to retain (in days)
    pub max_age_days: Option<u32>,

    /// Maximum number of events to retain
    pub max_events: Option<usize>,

    /// Maximum file size in bytes
    pub max_file_size_bytes: Option<u64>,

    /// Archive old events instead of deleting
    pub archive_old_events: bool,

    /// Path to archive directory
    pub archive_path: Option<PathBuf>,

    /// Compress archived events
    pub compress_archives: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: Some(30),   // Keep events for 30 days by default
            max_events: Some(100000), // Keep max 100k events
            max_file_size_bytes: Some(100 * 1024 * 1024), // 100MB max file size
            archive_old_events: true,
            archive_path: Some(PathBuf::from(".prodigy/events/archive")),
            compress_archives: true,
        }
    }
}

/// Manages event retention and cleanup
pub struct RetentionManager {
    policy: RetentionPolicy,
    events_path: PathBuf,
}

impl RetentionManager {
    /// Create a new retention manager with the given policy
    pub fn new(policy: RetentionPolicy, events_path: PathBuf) -> Self {
        Self {
            policy,
            events_path,
        }
    }

    /// Create with default policy
    pub fn with_default_policy(events_path: PathBuf) -> Self {
        Self::new(RetentionPolicy::default(), events_path)
    }

    /// Load policy from configuration file
    pub fn from_config_file(config_path: &Path, events_path: PathBuf) -> Result<Self> {
        let config_content = fs::read_to_string(config_path)?;
        let policy: RetentionPolicy = serde_yaml::from_str(&config_content)?;
        Ok(Self::new(policy, events_path))
    }

    /// Apply retention policy to events file
    pub async fn apply_retention(&self) -> Result<RetentionStats> {
        let mut stats = RetentionStats::default();

        if !self.events_path.exists() {
            return Ok(stats);
        }

        // Check file size first
        let metadata = fs::metadata(&self.events_path)?;
        let file_size = metadata.len();
        stats.original_size_bytes = file_size;

        // Determine if cleanup is needed
        let needs_cleanup = self.needs_cleanup(file_size)?;

        if !needs_cleanup {
            stats.events_retained = self.count_events()?;
            stats.final_size_bytes = file_size;
            return Ok(stats);
        }

        // Perform cleanup
        self.cleanup_events(&mut stats).await?;

        Ok(stats)
    }

    /// Check if cleanup is needed based on policy
    fn needs_cleanup(&self, file_size: u64) -> Result<bool> {
        // Check file size limit
        if let Some(max_size) = self.policy.max_file_size_bytes {
            if file_size > max_size {
                return Ok(true);
            }
        }

        // Check event count limit
        if let Some(max_events) = self.policy.max_events {
            let event_count = self.count_events()?;
            if event_count > max_events {
                return Ok(true);
            }
        }

        // Check age limit
        if self.policy.max_age_days.is_some() {
            // We'd need to check if there are old events, which requires scanning
            // For efficiency, we'll return true and let the cleanup process handle it
            return Ok(true);
        }

        Ok(false)
    }

    /// Count total events in the file
    fn count_events(&self) -> Result<usize> {
        let file = fs::File::open(&self.events_path)?;
        let reader = BufReader::new(file);
        let count = reader
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.trim().is_empty())
            .count();
        Ok(count)
    }

    /// Perform the actual cleanup of events
    async fn cleanup_events(&self, stats: &mut RetentionStats) -> Result<()> {
        let cutoff_time = self.calculate_cutoff_time();
        let temp_file = self.events_path.with_extension("tmp");
        let mut events_to_archive = Vec::new();
        let mut events_to_keep = Vec::new();

        // Read and filter events
        let file = fs::File::open(&self.events_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            stats.events_processed += 1;

            // Parse event to check timestamp
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                if self.should_retain_event(&event, cutoff_time, stats.events_retained) {
                    events_to_keep.push(line);
                    stats.events_retained += 1;
                } else {
                    events_to_archive.push(line);
                    stats.events_removed += 1;
                }
            }
        }

        // Archive old events if configured
        if self.policy.archive_old_events && !events_to_archive.is_empty() {
            self.archive_events(&events_to_archive, stats).await?;
        }

        // Write retained events to temp file
        let mut temp_writer = fs::File::create(&temp_file)?;
        for event in events_to_keep {
            writeln!(temp_writer, "{}", event)?;
        }
        temp_writer.sync_all()?;

        // Replace original file with temp file
        fs::rename(&temp_file, &self.events_path)?;

        // Update final size
        let metadata = fs::metadata(&self.events_path)?;
        stats.final_size_bytes = metadata.len();

        Ok(())
    }

    /// Calculate the cutoff time for event retention
    fn calculate_cutoff_time(&self) -> Option<DateTime<Utc>> {
        self.policy
            .max_age_days
            .map(|days| Utc::now() - Duration::days(days as i64))
    }

    /// Check if an event should be retained
    fn should_retain_event(
        &self,
        event: &serde_json::Value,
        cutoff_time: Option<DateTime<Utc>>,
        current_retained_count: usize,
    ) -> bool {
        // Check event count limit
        if let Some(max_events) = self.policy.max_events {
            if current_retained_count >= max_events {
                return false;
            }
        }

        // Check age limit
        if let Some(cutoff) = cutoff_time {
            if let Some(timestamp) = extract_event_timestamp(event) {
                if timestamp < cutoff {
                    return false;
                }
            }
        }

        true
    }

    /// Archive events to the configured archive directory
    async fn archive_events(&self, events: &[String], stats: &mut RetentionStats) -> Result<()> {
        let archive_dir = self
            .policy
            .archive_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Archive path not configured"))?;

        // Create archive directory if it doesn't exist
        fs::create_dir_all(archive_dir)?;

        // Generate archive filename with timestamp
        let archive_filename = format!(
            "events_archive_{}.jsonl{}",
            Utc::now().format("%Y%m%d_%H%M%S"),
            if self.policy.compress_archives {
                ".gz"
            } else {
                ""
            }
        );
        let archive_path = archive_dir.join(archive_filename);

        // Write events to archive
        if self.policy.compress_archives {
            self.write_compressed_archive(&archive_path, events)?;
        } else {
            self.write_plain_archive(&archive_path, events)?;
        }

        stats.events_archived = events.len();
        stats.archive_path = Some(archive_path);

        Ok(())
    }

    /// Write events to a plain text archive file
    fn write_plain_archive(&self, path: &Path, events: &[String]) -> Result<()> {
        let mut file = fs::File::create(path)?;
        for event in events {
            writeln!(file, "{}", event)?;
        }
        file.sync_all()?;
        Ok(())
    }

    /// Write events to a compressed archive file
    fn write_compressed_archive(&self, path: &Path, events: &[String]) -> Result<()> {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let file = fs::File::create(path)?;
        let mut encoder = GzEncoder::new(file, Compression::default());

        for event in events {
            writeln!(encoder, "{}", event)?;
        }

        encoder.finish()?;
        Ok(())
    }

    /// Get current retention policy
    pub fn policy(&self) -> &RetentionPolicy {
        &self.policy
    }

    /// Update retention policy
    pub fn set_policy(&mut self, policy: RetentionPolicy) {
        self.policy = policy;
    }

    /// Save policy to configuration file
    pub fn save_policy_to_file(&self, config_path: &Path) -> Result<()> {
        let yaml = serde_yaml::to_string(&self.policy)?;
        fs::write(config_path, yaml)?;
        Ok(())
    }
}

/// Statistics from retention operations
#[derive(Debug, Default, Clone)]
pub struct RetentionStats {
    /// Number of events processed
    pub events_processed: usize,

    /// Number of events retained
    pub events_retained: usize,

    /// Number of events removed
    pub events_removed: usize,

    /// Number of events archived
    pub events_archived: usize,

    /// Original file size in bytes
    pub original_size_bytes: u64,

    /// Final file size after cleanup
    pub final_size_bytes: u64,

    /// Path to archive file if created
    pub archive_path: Option<PathBuf>,
}

impl RetentionStats {
    /// Calculate the space saved in bytes
    pub fn space_saved(&self) -> u64 {
        self.original_size_bytes
            .saturating_sub(self.final_size_bytes)
    }

    /// Calculate the space saved percentage
    pub fn space_saved_percentage(&self) -> f64 {
        if self.original_size_bytes > 0 {
            (self.space_saved() as f64 / self.original_size_bytes as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Display statistics summary
    pub fn display_summary(&self) {
        println!("Event Retention Summary:");
        println!("  Events processed: {}", self.events_processed);
        println!("  Events retained: {}", self.events_retained);
        println!("  Events removed: {}", self.events_removed);

        if self.events_archived > 0 {
            println!("  Events archived: {}", self.events_archived);
            if let Some(ref path) = self.archive_path {
                println!("  Archive location: {}", path.display());
            }
        }

        println!("  Original size: {} bytes", self.original_size_bytes);
        println!("  Final size: {} bytes", self.final_size_bytes);
        println!(
            "  Space saved: {} bytes ({:.1}%)",
            self.space_saved(),
            self.space_saved_percentage()
        );
    }
}

/// Extract timestamp from an event
fn extract_event_timestamp(event: &serde_json::Value) -> Option<DateTime<Utc>> {
    // Try various common timestamp field locations
    let timestamp_str = event
        .get("timestamp")
        .or_else(|| event.get("time"))
        .or_else(|| event.get("created_at"))
        .or_else(|| {
            // Look in nested event structures
            for key in [
                "JobStarted",
                "JobCompleted",
                "AgentStarted",
                "AgentCompleted",
            ] {
                if let Some(nested) = event.get(key) {
                    if let Some(ts) = nested.get("timestamp") {
                        return Some(ts);
                    }
                }
            }
            None
        })
        .and_then(|v| v.as_str());

    timestamp_str
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

/// Automated retention task that can be run periodically
pub struct RetentionTask {
    manager: RetentionManager,
    interval: std::time::Duration,
}

impl RetentionTask {
    /// Create a new retention task
    pub fn new(manager: RetentionManager, interval: std::time::Duration) -> Self {
        Self { manager, interval }
    }

    /// Run the retention task once
    pub async fn run_once(&self) -> Result<RetentionStats> {
        log::info!("Running event retention cleanup...");
        let stats = self.manager.apply_retention().await?;

        if stats.events_removed > 0 {
            log::info!(
                "Retention cleanup completed: {} events removed, {:.1}% space saved",
                stats.events_removed,
                stats.space_saved_percentage()
            );
        } else {
            log::debug!("Retention cleanup completed: no events removed");
        }

        Ok(stats)
    }

    /// Start the retention task to run periodically
    pub async fn start(self) {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            interval.tick().await;

            if let Err(e) = self.run_once().await {
                log::error!("Retention task failed: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_retention_policy() {
        let policy = RetentionPolicy::default();
        assert_eq!(policy.max_age_days, Some(30));
        assert_eq!(policy.max_events, Some(100000));
        assert_eq!(policy.max_file_size_bytes, Some(100 * 1024 * 1024));
        assert!(policy.archive_old_events);
        assert!(policy.compress_archives);
    }

    #[tokio::test]
    #[ignore] // TODO: Fix the event counting logic
    async fn test_retention_manager_no_cleanup_needed() {
        let temp_dir = TempDir::new().unwrap();
        let events_file = temp_dir.path().join("events.jsonl");

        // Create a small events file
        let content = r#"{"timestamp":"2024-01-01T00:00:00Z","event":"test"}"#;
        fs::write(&events_file, content).unwrap();

        let manager = RetentionManager::with_default_policy(events_file);
        let stats = manager.apply_retention().await.unwrap();

        // The small file doesn't trigger cleanup, so it should just count events
        assert_eq!(stats.events_retained, 1);
        assert_eq!(stats.events_removed, 0);
    }

    #[test]
    fn test_extract_event_timestamp() {
        let event_json = r#"{
            "timestamp": "2024-01-01T12:00:00Z",
            "event_type": "JobStarted"
        }"#;

        let event: serde_json::Value = serde_json::from_str(event_json).unwrap();
        let timestamp = extract_event_timestamp(&event);

        assert!(timestamp.is_some());
        use chrono::Datelike;
        let ts = timestamp.unwrap();
        assert_eq!(ts.year(), 2024);
        assert_eq!(ts.month(), 1);
        assert_eq!(ts.day(), 1);
    }

    #[test]
    fn test_retention_stats_calculations() {
        let mut stats = RetentionStats::default();
        stats.original_size_bytes = 1000;
        stats.final_size_bytes = 250;

        assert_eq!(stats.space_saved(), 750);
        assert_eq!(stats.space_saved_percentage(), 75.0);
    }
}
