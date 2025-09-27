//! Worktree cleanup coordinator
//!
//! Manages the lifecycle and cleanup of worktrees created during MapReduce operations.

use super::config::WorktreeCleanupConfig;
use super::error::{CleanupError, CleanupResult};
use super::monitor::{CleanupRecommendation, WorktreeResourceMonitor};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

/// Task for cleanup operations
#[derive(Debug, Clone)]
pub enum CleanupTask {
    /// Clean up immediately
    Immediate {
        worktree_path: PathBuf,
        job_id: String,
    },
    /// Clean up after a delay
    Scheduled {
        worktree_path: PathBuf,
        delay: Duration,
    },
    /// Clean up multiple worktrees
    Batch { worktree_paths: Vec<PathBuf> },
}

/// Handle for managing cleanup lifecycle
pub struct CleanupGuard {
    worktree_path: PathBuf,
    coordinator: Arc<WorktreeCleanupCoordinator>,
}

impl CleanupGuard {
    /// Schedule cleanup after a delay
    pub async fn schedule_cleanup(self, delay: Duration) -> CleanupResult<()> {
        self.coordinator
            .schedule_cleanup(CleanupTask::Scheduled {
                worktree_path: self.worktree_path,
                delay,
            })
            .await
    }

    /// Perform immediate cleanup
    pub async fn immediate_cleanup(self) -> CleanupResult<()> {
        self.coordinator
            .cleanup_worktree(&self.worktree_path, true)
            .await?;
        Ok(())
    }
}

/// Coordinator for worktree cleanup operations
pub struct WorktreeCleanupCoordinator {
    /// Configuration
    config: WorktreeCleanupConfig,
    /// Active worktrees tracked by job ID
    active_worktrees: Arc<RwLock<HashMap<String, Vec<WorktreeHandle>>>>,
    /// Cleanup task queue
    cleanup_queue: Arc<Mutex<VecDeque<CleanupTask>>>,
    /// Background cleanup task handle
    cleanup_worker: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Resource monitor
    resource_monitor: Arc<Mutex<WorktreeResourceMonitor>>,
    /// Base path for worktrees
    worktree_base_path: PathBuf,
}

/// Handle for a tracked worktree
#[derive(Debug, Clone)]
struct WorktreeHandle {
    path: PathBuf,
    created_at: Instant,
    job_id: String,
    agent_id: String,
}

impl WorktreeCleanupCoordinator {
    /// Create a new cleanup coordinator
    pub fn new(config: WorktreeCleanupConfig, worktree_base_path: PathBuf) -> Self {
        let resource_monitor = Arc::new(Mutex::new(WorktreeResourceMonitor::new(
            config.disk_usage_threshold_mb,
            config.max_worktrees_per_job,
            config.max_total_worktrees,
        )));

        let coordinator = Self {
            config,
            active_worktrees: Arc::new(RwLock::new(HashMap::new())),
            cleanup_queue: Arc::new(Mutex::new(VecDeque::new())),
            cleanup_worker: Arc::new(Mutex::new(None)),
            resource_monitor,
            worktree_base_path,
        };

        coordinator
    }

    /// Start the background cleanup worker
    pub async fn start(&self) {
        let mut worker_guard = self.cleanup_worker.lock().await;

        // Don't start if already running
        if worker_guard.is_some() {
            return;
        }

        let queue = Arc::clone(&self.cleanup_queue);
        let config = self.config.clone();
        let coordinator = Arc::new(self.clone());

        let handle = tokio::spawn(async move {
            loop {
                // Process cleanup tasks
                let task = {
                    let mut queue_guard = queue.lock().await;
                    queue_guard.pop_front()
                };

                if let Some(task) = task {
                    if let Err(e) = Self::process_cleanup_task(task, &coordinator).await {
                        error!("Cleanup task failed: {}", e);
                    }
                } else {
                    // No tasks, sleep briefly
                    sleep(Duration::from_secs(1)).await;
                }

                // Check if we should perform periodic cleanup
                if config.enable_monitoring {
                    if let Err(e) = coordinator.periodic_cleanup_check().await {
                        warn!("Periodic cleanup check failed: {}", e);
                    }
                }
            }
        });

        *worker_guard = Some(handle);
    }

    /// Stop the background cleanup worker
    pub async fn stop(&self) {
        let mut worker_guard = self.cleanup_worker.lock().await;

        if let Some(handle) = worker_guard.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Register a job for tracking
    pub async fn register_job(&self, job_id: &str) -> CleanupGuard {
        let mut active = self.active_worktrees.write().await;
        active.entry(job_id.to_string()).or_insert_with(Vec::new);

        // Create a dummy guard for the job
        CleanupGuard {
            worktree_path: self.worktree_base_path.clone(),
            coordinator: Arc::new(self.clone()),
        }
    }

    /// Register a worktree for a specific job
    pub async fn register_worktree(
        &self,
        job_id: &str,
        agent_id: &str,
        worktree_path: PathBuf,
    ) -> CleanupGuard {
        let handle = WorktreeHandle {
            path: worktree_path.clone(),
            created_at: Instant::now(),
            job_id: job_id.to_string(),
            agent_id: agent_id.to_string(),
        };

        let mut active = self.active_worktrees.write().await;
        active
            .entry(job_id.to_string())
            .or_insert_with(Vec::new)
            .push(handle);

        // Check resource limits
        if self.config.enable_monitoring {
            if let Err(e) = self.check_resource_limits().await {
                warn!("Resource limit check failed: {}", e);
            }
        }

        CleanupGuard {
            worktree_path,
            coordinator: Arc::new(self.clone()),
        }
    }

    /// Schedule a cleanup task
    pub async fn schedule_cleanup(&self, task: CleanupTask) -> CleanupResult<()> {
        // Handle scheduled tasks with delay
        if let CleanupTask::Scheduled {
            worktree_path,
            delay,
        } = task
        {
            let queue = Arc::clone(&self.cleanup_queue);
            let job_id = String::new(); // Would extract from path in real impl

            tokio::spawn(async move {
                sleep(delay).await;
                let mut queue_guard = queue.lock().await;
                queue_guard.push_back(CleanupTask::Immediate {
                    worktree_path,
                    job_id,
                });
            });

            return Ok(());
        }

        // Add other tasks directly to queue
        let mut queue = self.cleanup_queue.lock().await;
        queue.push_back(task);

        Ok(())
    }

    /// Force cleanup all worktrees for a job
    pub async fn cleanup_job(&self, job_id: &str) -> CleanupResult<usize> {
        let handles = {
            let mut active = self.active_worktrees.write().await;
            active.remove(job_id).unwrap_or_default()
        };

        let count = handles.len();
        info!("Cleaning up {} worktrees for job {}", count, job_id);

        for handle in handles {
            if let Err(e) = self.cleanup_worktree(&handle.path, false).await {
                warn!(
                    "Failed to cleanup worktree {}: {}",
                    handle.path.display(),
                    e
                );
            }
        }

        Ok(count)
    }

    /// Clean up orphaned worktrees older than specified duration
    pub async fn cleanup_orphaned_worktrees(&self, max_age: Duration) -> CleanupResult<usize> {
        let candidates =
            WorktreeResourceMonitor::get_cleanup_candidates(&self.worktree_base_path, max_age)
                .await?;

        let mut cleaned = 0;

        for path in candidates {
            // Check if this worktree is tracked
            let is_tracked = {
                let active = self.active_worktrees.read().await;
                active
                    .values()
                    .any(|handles| handles.iter().any(|h| h.path == path))
            };

            if !is_tracked {
                info!("Cleaning orphaned worktree: {}", path.display());
                if let Err(e) = self.cleanup_worktree(&path, true).await {
                    warn!("Failed to cleanup orphaned worktree: {}", e);
                } else {
                    cleaned += 1;
                }
            }
        }

        Ok(cleaned)
    }

    /// Clean up a specific worktree
    pub async fn cleanup_worktree(&self, worktree_path: &Path, force: bool) -> CleanupResult<()> {
        info!("Cleaning up worktree: {}", worktree_path.display());

        // Check if worktree exists
        if !worktree_path.exists() {
            debug!(
                "Worktree doesn't exist, skipping: {}",
                worktree_path.display()
            );
            return Ok(());
        }

        // Check if worktree is active (unless forced)
        if !force && self.is_worktree_active(worktree_path).await? {
            return Err(CleanupError::WorktreeActive);
        }

        let start_time = Instant::now();

        // Remove git worktree
        let result = timeout(
            Duration::from_secs(self.config.cleanup_timeout_secs),
            self.remove_git_worktree(worktree_path),
        )
        .await;

        match result {
            Ok(Ok(())) => {
                // Remove from tracking
                self.untrack_worktree(worktree_path).await;

                // Record success
                let mut monitor = self.resource_monitor.lock().await;
                monitor.record_cleanup(true, start_time.elapsed());

                info!(
                    "Successfully cleaned up worktree: {}",
                    worktree_path.display()
                );
                Ok(())
            }
            Ok(Err(e)) => {
                // Record failure
                let mut monitor = self.resource_monitor.lock().await;
                monitor.record_cleanup(false, start_time.elapsed());

                Err(e)
            }
            Err(_) => {
                // Timeout occurred
                let mut monitor = self.resource_monitor.lock().await;
                monitor.record_cleanup(false, start_time.elapsed());

                Err(CleanupError::Timeout {
                    timeout: Duration::from_secs(self.config.cleanup_timeout_secs),
                })
            }
        }
    }

    /// Remove a git worktree
    async fn remove_git_worktree(&self, worktree_path: &Path) -> CleanupResult<()> {
        // First try git worktree remove
        let worktree_name = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| CleanupError::GitError("Invalid worktree path".to_string()))?;

        let output = Command::new("git")
            .args(["worktree", "remove", worktree_name, "--force"])
            .current_dir(
                &self
                    .worktree_base_path
                    .parent()
                    .unwrap_or(&self.worktree_base_path),
            )
            .output()
            .await
            .map_err(|e| CleanupError::GitError(format!("Failed to run git command: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // If git command failed, try manual removal
            warn!(
                "Git worktree remove failed: {}, attempting manual removal",
                stderr
            );

            // Remove directory manually
            tokio::fs::remove_dir_all(worktree_path)
                .await
                .map_err(|e| CleanupError::RemovalFailed {
                    path: worktree_path.to_path_buf(),
                    source: e,
                })?;

            // Prune worktree list
            let _ = Command::new("git")
                .args(["worktree", "prune"])
                .current_dir(
                    &self
                        .worktree_base_path
                        .parent()
                        .unwrap_or(&self.worktree_base_path),
                )
                .output()
                .await;
        }

        Ok(())
    }

    /// Check if a worktree is still active
    async fn is_worktree_active(&self, worktree_path: &Path) -> CleanupResult<bool> {
        // Check if any git operations are running in the worktree
        // This is a simplified check - in production would be more thorough

        let git_lock = worktree_path.join(".git/index.lock");
        if git_lock.exists() {
            return Ok(true);
        }

        Ok(false)
    }

    /// Remove worktree from tracking
    async fn untrack_worktree(&self, worktree_path: &Path) {
        let mut active = self.active_worktrees.write().await;

        for handles in active.values_mut() {
            handles.retain(|h| h.path != worktree_path);
        }

        // Remove empty job entries
        active.retain(|_, handles| !handles.is_empty());
    }

    /// Check resource limits and trigger cleanup if needed
    async fn check_resource_limits(&self) -> CleanupResult<()> {
        let mut monitor = self.resource_monitor.lock().await;

        // Scan current state
        monitor
            .scan_worktree_directory(&self.worktree_base_path)
            .await?;

        // Check limits
        if let Err(e) = monitor.check_limits() {
            warn!("Resource limit exceeded: {}", e);

            // Get cleanup recommendation
            match monitor.cleanup_recommendation() {
                CleanupRecommendation::EmergencyCleanup { reason } => {
                    warn!("Emergency cleanup triggered: {}", reason);
                    // Trigger immediate cleanup of old worktrees
                    let _ = self
                        .cleanup_orphaned_worktrees(Duration::from_secs(300))
                        .await;
                }
                CleanupRecommendation::CleanupOld { threshold } => {
                    let _ = self.cleanup_orphaned_worktrees(threshold).await;
                }
                CleanupRecommendation::CleanupFailed => {
                    // Would retry failed cleanups here
                }
                CleanupRecommendation::None => {}
            }
        }

        Ok(())
    }

    /// Periodic cleanup check
    async fn periodic_cleanup_check(&self) -> CleanupResult<()> {
        static LAST_CHECK: std::sync::OnceLock<Mutex<Instant>> = std::sync::OnceLock::new();

        let last_check = LAST_CHECK.get_or_init(|| Mutex::new(Instant::now()));
        let mut last = last_check.lock().await;

        // Only check every 60 seconds
        if last.elapsed() < Duration::from_secs(60) {
            return Ok(());
        }

        *last = Instant::now();

        // Perform resource check
        self.check_resource_limits().await?;

        // Clean up old orphaned worktrees
        let cleaned = self
            .cleanup_orphaned_worktrees(Duration::from_secs(3600))
            .await?;

        if cleaned > 0 {
            info!("Periodic cleanup removed {} orphaned worktrees", cleaned);
        }

        Ok(())
    }

    /// Process a cleanup task
    async fn process_cleanup_task(
        task: CleanupTask,
        coordinator: &Arc<WorktreeCleanupCoordinator>,
    ) -> CleanupResult<()> {
        match task {
            CleanupTask::Immediate { worktree_path, .. } => {
                coordinator.cleanup_worktree(&worktree_path, false).await?;
            }
            CleanupTask::Scheduled { .. } => {
                // Already handled in schedule_cleanup
            }
            CleanupTask::Batch { worktree_paths } => {
                for path in worktree_paths {
                    if let Err(e) = coordinator.cleanup_worktree(&path, false).await {
                        warn!("Failed to cleanup worktree in batch: {}", e);
                    }
                }
            }
        }

        Ok(())
    }
}

// Implement Clone manually to handle Arc fields
impl Clone for WorktreeCleanupCoordinator {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            active_worktrees: Arc::clone(&self.active_worktrees),
            cleanup_queue: Arc::clone(&self.cleanup_queue),
            cleanup_worker: Arc::clone(&self.cleanup_worker),
            resource_monitor: Arc::clone(&self.resource_monitor),
            worktree_base_path: self.worktree_base_path.clone(),
        }
    }
}
