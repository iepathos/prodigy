//! Resource management module for MapReduce executor
//!
//! Provides centralized control over worktree sessions, git operations,
//! and resource lifecycle management.

pub mod cleanup;
pub mod git;
pub mod pool;
pub mod worktree;

// Re-export key types
pub use cleanup::{CleanupPriority, CleanupRegistry, CleanupTask};
pub use git::GitOperations;
pub use pool::{PoolMetrics, ResourcePool};
pub use worktree::WorktreeResourceManager;

use crate::cook::execution::errors::MapReduceResult;
use crate::worktree::{WorktreePool, WorktreeSession};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Central resource manager for MapReduce execution
pub struct ResourceManager {
    /// Worktree pool for session management
    pub worktree_pool: Option<Arc<WorktreePool>>,
    /// Active sessions tracked by agent ID
    pub active_sessions: Arc<RwLock<HashMap<String, WorktreeSession>>>,
    /// Registry for cleanup tasks
    pub cleanup_registry: Arc<CleanupRegistry>,
    /// Git operations handler
    pub git_ops: Arc<GitOperations>,
    /// Worktree resource manager
    pub worktree_manager: Arc<WorktreeResourceManager>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new(worktree_pool: Option<Arc<WorktreePool>>) -> Self {
        let cleanup_registry = Arc::new(CleanupRegistry::new());
        let git_ops = Arc::new(GitOperations::new());
        let worktree_manager = Arc::new(WorktreeResourceManager::new(worktree_pool.clone()));

        Self {
            worktree_pool,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_registry,
            git_ops,
            worktree_manager,
        }
    }

    /// Register an active session
    pub async fn register_session(&self, agent_id: String, session: WorktreeSession) {
        let mut sessions = self.active_sessions.write().await;
        sessions.insert(agent_id, session);
    }

    /// Unregister a session
    pub async fn unregister_session(&self, agent_id: &str) -> Option<WorktreeSession> {
        let mut sessions = self.active_sessions.write().await;
        sessions.remove(agent_id)
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> Vec<(String, WorktreeSession)> {
        let sessions = self.active_sessions.read().await;
        sessions
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Cleanup all resources
    pub async fn cleanup_all(&self) -> MapReduceResult<()> {
        // First cleanup all active sessions
        let sessions = {
            let mut sessions = self.active_sessions.write().await;
            let all_sessions: HashMap<String, WorktreeSession> = sessions.drain().collect();
            all_sessions
        };

        // Cleanup each session
        for (_agent_id, _session) in sessions {
            // Session cleanup handled by Drop trait
        }

        // Execute all registered cleanup tasks
        self.cleanup_registry.execute_all().await?;

        Ok(())
    }

    /// Get resource usage metrics
    pub fn get_metrics(&self) -> ResourceMetrics {
        ResourceMetrics {
            active_sessions: 0, // Will be populated async
            total_created: 0,
            total_reused: 0,
        }
    }
}

/// Resource usage metrics
#[derive(Debug, Clone, Default)]
pub struct ResourceMetrics {
    pub active_sessions: usize,
    pub total_created: usize,
    pub total_reused: usize,
}

/// RAII guard for resources
pub struct ResourceGuard<T> {
    resource: Option<T>,
    cleanup: Option<Box<dyn FnOnce(T) + Send>>,
}

impl<T> ResourceGuard<T> {
    /// Create a new resource guard
    pub fn new<F>(resource: T, cleanup: F) -> Self
    where
        F: FnOnce(T) + Send + 'static,
    {
        Self {
            resource: Some(resource),
            cleanup: Some(Box::new(cleanup)),
        }
    }

    /// Get a reference to the resource
    pub fn get(&self) -> Option<&T> {
        self.resource.as_ref()
    }

    /// Get a mutable reference to the resource
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.resource.as_mut()
    }

    /// Take the resource, consuming the guard without running cleanup
    pub fn take(mut self) -> Option<T> {
        self.cleanup = None;
        self.resource.take()
    }
}

impl<T> Drop for ResourceGuard<T> {
    fn drop(&mut self) {
        if let (Some(resource), Some(cleanup)) = (self.resource.take(), self.cleanup.take()) {
            cleanup(resource);
        }
    }
}

impl<T> std::ops::Deref for ResourceGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource
            .as_ref()
            .expect("Resource guard already consumed")
    }
}

impl<T> std::ops::DerefMut for ResourceGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource
            .as_mut()
            .expect("Resource guard already consumed")
    }
}
