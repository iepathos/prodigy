//! Configuration for worktree cleanup operations

use serde::{Deserialize, Serialize};

/// Configuration for worktree cleanup operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeCleanupConfig {
    /// Enable automatic cleanup of worktrees
    pub auto_cleanup: bool,

    /// Delay before cleaning up successful agent worktrees (in seconds)
    pub cleanup_delay_secs: u64,

    /// Maximum number of worktrees per job
    pub max_worktrees_per_job: usize,

    /// Maximum total worktrees across all jobs
    pub max_total_worktrees: usize,

    /// Disk usage threshold for triggering cleanup warnings (in MB)
    pub disk_usage_threshold_mb: u64,

    /// Enable resource monitoring
    pub enable_monitoring: bool,

    /// Cleanup timeout per worktree (in seconds)
    pub cleanup_timeout_secs: u64,

    /// Number of cleanup worker threads
    pub cleanup_workers: usize,

    /// Retry failed cleanups
    pub retry_failed_cleanup: bool,

    /// Maximum cleanup retry attempts
    pub max_cleanup_retries: u32,
}

impl Default for WorktreeCleanupConfig {
    fn default() -> Self {
        Self {
            auto_cleanup: true,
            cleanup_delay_secs: 30,
            max_worktrees_per_job: 50,
            max_total_worktrees: 200,
            disk_usage_threshold_mb: 1024, // 1GB
            enable_monitoring: true,
            cleanup_timeout_secs: 30,
            cleanup_workers: 4,
            retry_failed_cleanup: true,
            max_cleanup_retries: 3,
        }
    }
}

impl WorktreeCleanupConfig {
    /// Create a configuration with immediate cleanup
    pub fn immediate() -> Self {
        Self {
            cleanup_delay_secs: 0,
            ..Default::default()
        }
    }

    /// Create a configuration with aggressive cleanup
    pub fn aggressive() -> Self {
        Self {
            auto_cleanup: true,
            cleanup_delay_secs: 5,
            max_worktrees_per_job: 20,
            max_total_worktrees: 50,
            disk_usage_threshold_mb: 512,
            ..Default::default()
        }
    }

    /// Create a configuration with conservative cleanup
    pub fn conservative() -> Self {
        Self {
            auto_cleanup: true,
            cleanup_delay_secs: 120,
            max_worktrees_per_job: 100,
            max_total_worktrees: 500,
            disk_usage_threshold_mb: 5120, // 5GB
            ..Default::default()
        }
    }

    /// Check if a resource limit is exceeded
    pub fn is_limit_exceeded(&self, active_worktrees: usize, job_worktrees: usize) -> bool {
        active_worktrees >= self.max_total_worktrees || job_worktrees >= self.max_worktrees_per_job
    }
}
