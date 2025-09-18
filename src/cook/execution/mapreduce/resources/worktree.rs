//! Worktree session management for MapReduce agents

use crate::cook::execution::errors::{MapReduceError, MapReduceResult};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::worktree::{WorktreeManager, WorktreePool, WorktreeRequest, WorktreeSession};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Manages worktree resources for MapReduce agents
pub struct WorktreeResourceManager {
    pool: Option<Arc<WorktreePool>>,
    worktree_manager: Option<Arc<WorktreeManager>>,
}

impl WorktreeResourceManager {
    /// Create a new worktree resource manager
    pub fn new(pool: Option<Arc<WorktreePool>>) -> Self {
        Self {
            pool,
            worktree_manager: None,
        }
    }

    /// Create with both pool and manager
    pub fn with_manager(pool: Option<Arc<WorktreePool>>, manager: Arc<WorktreeManager>) -> Self {
        Self {
            pool,
            worktree_manager: Some(manager),
        }
    }

    /// Acquire a worktree session for an agent
    pub async fn acquire_session(
        &self,
        agent_id: &str,
        _env: &ExecutionEnvironment,
    ) -> MapReduceResult<WorktreeSession> {
        // Try to use worktree pool first
        if let Some(pool) = &self.pool {
            let handle = pool
                .acquire(WorktreeRequest::Anonymous)
                .await
                .map_err(|e| self.create_worktree_error(agent_id, e.to_string()))?;

            // Extract the session from the handle
            let session = handle
                .session()
                .ok_or_else(|| {
                    self.create_worktree_error(
                        agent_id,
                        "No session in worktree handle".to_string(),
                    )
                })?
                .clone();
            info!(
                "Acquired worktree session for agent {}: {}",
                agent_id,
                session.path.display()
            );

            Ok(session)
        } else if let Some(manager) = &self.worktree_manager {
            // Fallback to direct worktree manager
            let session = manager
                .create_session()
                .await
                .map_err(|e| self.create_worktree_error(agent_id, e.to_string()))?;

            info!(
                "Created worktree session for agent {} at: {}",
                agent_id,
                session.path.display()
            );

            Ok(session)
        } else {
            Err(self.create_worktree_error(
                agent_id,
                "No worktree pool or manager available".to_string(),
            ))
        }
    }

    /// Release a worktree session
    pub async fn release_session(&self, agent_id: &str, session: WorktreeSession) {
        debug!(
            "Releasing worktree session for agent {}: {}",
            agent_id,
            session.path.display()
        );

        // Session cleanup is handled by Drop trait
        drop(session);
    }

    /// Cleanup orphaned worktree by name
    pub async fn cleanup_by_name(&self, worktree_name: &str) -> MapReduceResult<()> {
        if let Some(pool) = &self.pool {
            pool.cleanup_by_name(worktree_name)
                .await
                .map_err(|e| MapReduceError::General {
                    message: format!("Failed to cleanup worktree {}: {}", worktree_name, e),
                    source: None,
                })?;
        } else if let Some(manager) = &self.worktree_manager {
            manager
                .cleanup_session(worktree_name, true)
                .await
                .map_err(|e| MapReduceError::General {
                    message: format!("Failed to cleanup worktree {}: {}", worktree_name, e),
                    source: None,
                })?;
        }
        Ok(())
    }

    /// Get worktree path from session
    pub fn get_worktree_path(session: &WorktreeSession) -> PathBuf {
        session.path.clone()
    }

    /// Check if a worktree exists
    pub async fn worktree_exists(&self, name: &str) -> bool {
        if let Some(_manager) = &self.worktree_manager {
            // Check if worktree exists using git command
            let output = std::process::Command::new("git")
                .args(["worktree", "list"])
                .output()
                .ok();

            if let Some(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains(name);
            }
        }
        false
    }

    /// Cleanup all orphaned worktrees from a list
    pub async fn cleanup_orphaned_worktrees(&self, worktree_names: &[String]) {
        for name in worktree_names {
            if let Err(e) = self.cleanup_by_name(name).await {
                warn!("Failed to cleanup orphaned worktree {}: {}", name, e);
            } else {
                debug!("Cleaned up orphaned worktree: {}", name);
            }
        }
    }

    fn create_worktree_error(&self, agent_id: &str, message: String) -> MapReduceError {
        MapReduceError::WorktreeCreationFailed {
            agent_id: agent_id.to_string(),
            reason: message.clone(),
            source: std::io::Error::other(message),
        }
    }
}

/// Information about a worktree session
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
}

impl WorktreeInfo {
    /// Create new worktree info
    pub fn new(name: String, path: PathBuf, branch: Option<String>) -> Self {
        Self { name, path, branch }
    }

    /// Create from a WorktreeSession
    pub fn from_session(session: &WorktreeSession) -> Self {
        Self {
            name: session.name.clone(),
            path: session.path.clone(),
            branch: Some(session.branch.clone()),
        }
    }
}
