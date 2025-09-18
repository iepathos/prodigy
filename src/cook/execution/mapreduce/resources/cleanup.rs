//! Resource cleanup coordination for MapReduce

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Priority levels for cleanup tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CleanupPriority {
    /// Critical cleanup that must happen immediately
    Critical = 0,
    /// High priority cleanup
    High = 1,
    /// Normal priority cleanup
    Normal = 2,
    /// Low priority cleanup that can be deferred
    Low = 3,
}

/// Trait for cleanup tasks
#[async_trait]
pub trait CleanupTask: Send + Sync {
    /// Execute the cleanup task
    async fn cleanup(&self) -> MapReduceResult<()>;

    /// Get the priority of this cleanup task
    fn priority(&self) -> CleanupPriority {
        CleanupPriority::Normal
    }

    /// Get a description of this cleanup task
    fn description(&self) -> String {
        "Cleanup task".to_string()
    }
}

/// Registry for cleanup tasks
pub struct CleanupRegistry {
    tasks: Arc<RwLock<Vec<Box<dyn CleanupTask>>>>,
}

impl CleanupRegistry {
    /// Create a new cleanup registry
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a cleanup task
    pub async fn register(&self, task: Box<dyn CleanupTask>) {
        let mut tasks = self.tasks.write().await;
        tasks.push(task);
    }

    /// Execute all cleanup tasks in priority order
    pub async fn execute_all(&self) -> MapReduceResult<()> {
        let mut tasks = {
            let mut tasks = self.tasks.write().await;
            tasks.drain(..).collect::<Vec<_>>()
        };

        // Sort tasks by priority (Critical first)
        tasks.sort_by(|a, b| a.priority().cmp(&b.priority()));

        let mut errors = Vec::new();

        for task in tasks {
            let description = task.description();
            debug!("Executing cleanup task: {}", description);

            if let Err(e) = task.cleanup().await {
                error!("Cleanup task '{}' failed: {}", description, e);
                errors.push(format!("{}: {}", description, e));
            } else {
                info!("Cleanup task '{}' completed successfully", description);
            }
        }

        if !errors.is_empty() {
            return Err(MapReduceError::General {
                message: format!(
                    "Cleanup failed with {} errors: {}",
                    errors.len(),
                    errors.join("; ")
                ),
                source: None,
            });
        }

        Ok(())
    }

    /// Execute cleanup tasks with a specific priority
    pub async fn execute_priority(&self, priority: CleanupPriority) -> MapReduceResult<()> {
        // Filter and execute tasks with matching priority
        let tasks = self.tasks.read().await;

        let mut errors = Vec::new();

        for task in tasks.iter() {
            if task.priority() == priority {
                let description = task.description();
                debug!(
                    "Executing priority {} cleanup task: {}",
                    priority as u8, description
                );

                if let Err(e) = task.cleanup().await {
                    error!("Cleanup task '{}' failed: {}", description, e);
                    errors.push(format!("{}: {}", description, e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(MapReduceError::General {
                message: format!(
                    "Priority {} cleanup failed with {} errors: {}",
                    priority as u8,
                    errors.len(),
                    errors.join("; ")
                ),
                source: None,
            });
        }

        Ok(())
    }

    /// Clear all registered tasks without executing them
    pub async fn clear(&self) {
        let mut tasks = self.tasks.write().await;
        tasks.clear();
    }

    /// Get the number of registered tasks
    pub async fn count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.len()
    }
}

/// Cleanup task for worktree resources
pub struct WorktreeCleanupTask {
    worktree_name: String,
    worktree_path: Option<std::path::PathBuf>,
}

impl WorktreeCleanupTask {
    /// Create a new worktree cleanup task
    pub fn new(worktree_name: String, worktree_path: Option<std::path::PathBuf>) -> Self {
        Self {
            worktree_name,
            worktree_path,
        }
    }
}

#[async_trait]
impl CleanupTask for WorktreeCleanupTask {
    async fn cleanup(&self) -> MapReduceResult<()> {
        // Attempt to remove the worktree using git command
        let output = tokio::process::Command::new("git")
            .args(["worktree", "remove", "--force", &self.worktree_name])
            .output()
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to execute git worktree remove: {}", e),
                source: None,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // It's ok if the worktree doesn't exist
            if !stderr.contains("not a valid path") && !stderr.contains("is not a working tree") {
                return Err(MapReduceError::General {
                    message: format!(
                        "Failed to cleanup worktree {}: {}",
                        self.worktree_name, stderr
                    ),
                    source: None,
                });
            }
        }

        // If we have a path, also try to remove the directory
        if let Some(path) = &self.worktree_path {
            if path.exists() {
                if let Err(e) = tokio::fs::remove_dir_all(path).await {
                    debug!("Failed to remove worktree directory {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    fn priority(&self) -> CleanupPriority {
        CleanupPriority::Normal
    }

    fn description(&self) -> String {
        format!("Cleanup worktree: {}", self.worktree_name)
    }
}

/// Cleanup task for git branches
pub struct GitBranchCleanupTask {
    branch_name: String,
    worktree_path: std::path::PathBuf,
}

impl GitBranchCleanupTask {
    /// Create a new git branch cleanup task
    pub fn new(branch_name: String, worktree_path: std::path::PathBuf) -> Self {
        Self {
            branch_name,
            worktree_path,
        }
    }
}

#[async_trait]
impl CleanupTask for GitBranchCleanupTask {
    async fn cleanup(&self) -> MapReduceResult<()> {
        let output = tokio::process::Command::new("git")
            .args(["branch", "-D", &self.branch_name])
            .current_dir(&self.worktree_path)
            .output()
            .await
            .map_err(|e| MapReduceError::General {
                message: format!("Failed to delete git branch: {}", e),
                source: None,
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // It's ok if the branch doesn't exist
            if !stderr.contains("not found") {
                debug!("Failed to delete branch {}: {}", self.branch_name, stderr);
            }
        }

        Ok(())
    }

    fn priority(&self) -> CleanupPriority {
        CleanupPriority::Low
    }

    fn description(&self) -> String {
        format!("Cleanup git branch: {}", self.branch_name)
    }
}

/// Generic cleanup task implementation
pub struct GenericCleanupTask<F>
where
    F: Fn() -> futures::future::BoxFuture<'static, MapReduceResult<()>> + Send + Sync,
{
    cleanup_fn: F,
    priority: CleanupPriority,
    description: String,
}

impl<F> GenericCleanupTask<F>
where
    F: Fn() -> futures::future::BoxFuture<'static, MapReduceResult<()>> + Send + Sync,
{
    /// Create a new generic cleanup task
    pub fn new(cleanup_fn: F, priority: CleanupPriority, description: String) -> Self {
        Self {
            cleanup_fn,
            priority,
            description,
        }
    }
}

#[async_trait]
impl<F> CleanupTask for GenericCleanupTask<F>
where
    F: Fn() -> futures::future::BoxFuture<'static, MapReduceResult<()>> + Send + Sync,
{
    async fn cleanup(&self) -> MapReduceResult<()> {
        (self.cleanup_fn)().await
    }

    fn priority(&self) -> CleanupPriority {
        self.priority
    }

    fn description(&self) -> String {
        self.description.clone()
    }
}
