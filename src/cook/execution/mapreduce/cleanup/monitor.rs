//! Resource monitoring for worktree usage

use super::error::{CleanupError, CleanupResult};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, warn};

/// Recommendation for cleanup actions
#[derive(Debug, Clone)]
pub enum CleanupRecommendation {
    /// No action needed
    None,
    /// Cleanup old worktrees
    CleanupOld { threshold: Duration },
    /// Cleanup failed worktrees
    CleanupFailed,
    /// Emergency cleanup due to resource limits
    EmergencyCleanup { reason: String },
}

/// Metrics for worktree resource usage
#[derive(Debug, Clone, Default)]
pub struct WorktreeMetrics {
    /// Number of active worktrees
    pub active_worktrees: usize,
    /// Total disk usage in bytes
    pub total_disk_usage: u64,
    /// Number of cleanup operations performed
    pub cleanup_operations: usize,
    /// Number of failed cleanup attempts
    pub cleanup_failures: usize,
    /// Average cleanup time
    pub average_cleanup_time: Duration,
    /// Number of orphaned worktrees
    pub orphaned_worktrees: usize,
}

/// Monitor for worktree resource usage
pub struct WorktreeResourceMonitor {
    /// Disk usage threshold in bytes
    disk_usage_threshold: u64,
    /// Maximum worktrees per job
    max_worktrees_per_job: usize,
    /// Maximum total worktrees
    max_total_worktrees: usize,
    /// Current metrics
    metrics: WorktreeMetrics,
}

impl WorktreeResourceMonitor {
    /// Create a new resource monitor
    pub fn new(
        disk_usage_threshold_mb: u64,
        max_worktrees_per_job: usize,
        max_total_worktrees: usize,
    ) -> Self {
        Self {
            disk_usage_threshold: disk_usage_threshold_mb * 1024 * 1024,
            max_worktrees_per_job,
            max_total_worktrees,
            metrics: WorktreeMetrics::default(),
        }
    }

    /// Check resource limits
    pub fn check_limits(&self) -> CleanupResult<()> {
        if self.metrics.active_worktrees >= self.max_total_worktrees {
            return Err(CleanupError::ResourceLimitExceeded(format!(
                "Total worktree limit exceeded: {} >= {}",
                self.metrics.active_worktrees, self.max_total_worktrees
            )));
        }

        if self.metrics.total_disk_usage >= self.disk_usage_threshold {
            return Err(CleanupError::ResourceLimitExceeded(format!(
                "Disk usage threshold exceeded: {} bytes >= {} bytes",
                self.metrics.total_disk_usage, self.disk_usage_threshold
            )));
        }

        Ok(())
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> WorktreeMetrics {
        self.metrics.clone()
    }

    /// Update metrics after scanning worktrees
    pub fn update_metrics(&mut self, metrics: WorktreeMetrics) {
        self.metrics = metrics;
    }

    /// Get cleanup recommendation based on current state
    pub fn cleanup_recommendation(&self) -> CleanupRecommendation {
        // Emergency cleanup if over limits
        if self.metrics.active_worktrees >= self.max_total_worktrees {
            return CleanupRecommendation::EmergencyCleanup {
                reason: format!(
                    "Active worktrees ({}) exceeded limit ({})",
                    self.metrics.active_worktrees, self.max_total_worktrees
                ),
            };
        }

        // Recommend cleanup if disk usage is high
        if self.metrics.total_disk_usage > (self.disk_usage_threshold * 80 / 100) {
            return CleanupRecommendation::CleanupOld {
                threshold: Duration::from_secs(3600), // 1 hour
            };
        }

        // Cleanup failed worktrees if there are many failures
        if self.metrics.cleanup_failures > 10 {
            return CleanupRecommendation::CleanupFailed;
        }

        // Check for high percentage of active worktrees
        let usage_percentage =
            (self.metrics.active_worktrees * 100) / self.max_total_worktrees.max(1);
        if usage_percentage > 75 {
            return CleanupRecommendation::CleanupOld {
                threshold: Duration::from_secs(7200), // 2 hours
            };
        }

        CleanupRecommendation::None
    }

    /// Calculate disk usage for a path
    pub async fn calculate_disk_usage(path: &Path) -> u64 {
        // Simple implementation - in production would use walkdir
        // and accumulate sizes
        match tokio::fs::metadata(path).await {
            Ok(metadata) => {
                if metadata.is_dir() {
                    // Estimate based on directory count
                    // In production, would recursively calculate
                    1024 * 1024 * 10 // 10MB estimate per worktree
                } else {
                    metadata.len()
                }
            }
            Err(e) => {
                debug!("Failed to get metadata for {:?}: {}", path, e);
                0
            }
        }
    }

    /// Scan worktree directory and update metrics
    pub async fn scan_worktree_directory(&mut self, base_path: &Path) -> CleanupResult<()> {
        let mut active_count = 0;
        let mut total_disk_usage = 0u64;
        let mut orphaned_count = 0;

        // Scan the worktree directory
        match tokio::fs::read_dir(base_path).await {
            Ok(mut entries) => {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        active_count += 1;
                        total_disk_usage += Self::calculate_disk_usage(&path).await;

                        // Check if worktree is orphaned (simplified check)
                        if Self::is_orphaned_worktree(&path).await {
                            orphaned_count += 1;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to scan worktree directory: {}", e);
            }
        }

        // Update metrics
        self.metrics.active_worktrees = active_count;
        self.metrics.total_disk_usage = total_disk_usage;
        self.metrics.orphaned_worktrees = orphaned_count;

        Ok(())
    }

    /// Check if a worktree is orphaned
    async fn is_orphaned_worktree(path: &Path) -> bool {
        // Check if .git directory exists
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            return true;
        }

        // Additional checks could include:
        // - Check if branch still exists
        // - Check last modification time
        // - Check if parent process is still running

        false
    }

    /// Record cleanup operation
    pub fn record_cleanup(&mut self, success: bool, duration: Duration) {
        self.metrics.cleanup_operations += 1;

        if !success {
            self.metrics.cleanup_failures += 1;
        }

        // Update average cleanup time
        let total_time =
            self.metrics.average_cleanup_time.as_secs() * self.metrics.cleanup_operations as u64;
        let new_total = total_time + duration.as_secs();

        self.metrics.average_cleanup_time =
            Duration::from_secs(new_total / (self.metrics.cleanup_operations as u64).max(1));
    }

    /// Get worktree paths that should be cleaned
    pub async fn get_cleanup_candidates(
        base_path: &Path,
        max_age: Duration,
    ) -> CleanupResult<Vec<PathBuf>> {
        let mut candidates = Vec::new();
        let now = std::time::SystemTime::now();

        match tokio::fs::read_dir(base_path).await {
            Ok(mut entries) => {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_dir() {
                        // Check modification time
                        if let Ok(metadata) = entry.metadata().await {
                            if let Ok(modified) = metadata.modified() {
                                if let Ok(age) = now.duration_since(modified) {
                                    if age > max_age {
                                        candidates.push(path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(CleanupError::CoordinatorError(format!(
                    "Failed to read worktree directory: {}",
                    e
                )));
            }
        }

        Ok(candidates)
    }
}
