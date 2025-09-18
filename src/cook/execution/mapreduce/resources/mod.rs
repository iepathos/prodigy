//! Resource management module for MapReduce executor
//!
//! Provides centralized control over worktree sessions, git operations,
//! and resource lifecycle management.

pub mod agent;
pub mod cleanup;
pub mod git;
pub mod pool;
pub mod worktree;

#[cfg(test)]
mod tests;

// Re-export key types
pub use agent::{AgentContext, AgentResourceManager};
pub use cleanup::{CleanupPriority, CleanupRegistry, CleanupTask};
pub use git::GitOperations;
pub use pool::{PoolMetrics, ResourcePool};
pub use worktree::WorktreeResourceManager;

use crate::cook::execution::errors::MapReduceResult;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::worktree::{WorktreeManager, WorktreePool, WorktreeSession};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// Agent resource manager
    pub agent_manager: Arc<AgentResourceManager>,
    /// Metrics tracking
    metrics: Arc<ResourceMetricsTracker>,
}

impl ResourceManager {
    /// Create a new resource manager
    pub fn new(worktree_pool: Option<Arc<WorktreePool>>) -> Self {
        let cleanup_registry = Arc::new(CleanupRegistry::new());
        let git_ops = Arc::new(GitOperations::new());
        let worktree_manager = Arc::new(WorktreeResourceManager::new(worktree_pool.clone()));
        let agent_manager = Arc::new(AgentResourceManager::new());
        let metrics = Arc::new(ResourceMetricsTracker::new());

        Self {
            worktree_pool,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_registry,
            git_ops,
            worktree_manager,
            agent_manager,
            metrics,
        }
    }

    /// Create with existing worktree manager
    pub fn with_worktree_manager(
        worktree_pool: Option<Arc<WorktreePool>>,
        worktree_manager: Arc<WorktreeManager>,
    ) -> Self {
        let cleanup_registry = Arc::new(CleanupRegistry::new());
        let git_ops = Arc::new(GitOperations::new());
        let worktree_resource_manager = Arc::new(WorktreeResourceManager::with_manager(
            worktree_pool.clone(),
            worktree_manager,
        ));
        let agent_manager = Arc::new(AgentResourceManager::new());
        let metrics = Arc::new(ResourceMetricsTracker::new());

        Self {
            worktree_pool,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_registry,
            git_ops,
            worktree_manager: worktree_resource_manager,
            agent_manager,
            metrics,
        }
    }

    /// Acquire worktree session for an agent
    pub async fn acquire_worktree_session(
        &self,
        agent_id: &str,
        env: &ExecutionEnvironment,
    ) -> MapReduceResult<WorktreeSession> {
        // Acquire session through worktree manager
        let session = self.worktree_manager.acquire_session(agent_id, env).await?;

        // Register with active sessions
        self.register_session(agent_id.to_string(), session.clone())
            .await;

        // Update metrics
        self.metrics.increment_created();

        Ok(session)
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

    /// Cleanup orphaned worktrees from failed agents
    pub async fn cleanup_orphaned_resources(&self, worktree_names: &[String]) {
        if !worktree_names.is_empty() {
            // Use worktree manager for cleanup
            self.worktree_manager
                .cleanup_orphaned_worktrees(worktree_names)
                .await;

            // Register cleanup tasks
            for name in worktree_names {
                let cleanup_task = Box::new(cleanup::WorktreeCleanupTask::new(name.clone(), None));
                self.cleanup_registry.register(cleanup_task).await;
                log::info!("Registered cleanup task for orphaned worktree: {}", name);
            }
        }
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
    pub async fn get_metrics(&self) -> ResourceMetrics {
        let active_sessions = self.active_sessions.read().await.len();
        ResourceMetrics {
            active_sessions,
            total_created: self.metrics.total_created(),
            total_reused: self.metrics.total_reused(),
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

/// Internal metrics tracker
struct ResourceMetricsTracker {
    created: AtomicUsize,
    reused: AtomicUsize,
}

impl ResourceMetricsTracker {
    fn new() -> Self {
        Self {
            created: AtomicUsize::new(0),
            reused: AtomicUsize::new(0),
        }
    }

    fn increment_created(&self) {
        self.created.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    fn increment_reused(&self) {
        self.reused.fetch_add(1, Ordering::Relaxed);
    }

    fn total_created(&self) -> usize {
        self.created.load(Ordering::Relaxed)
    }

    fn total_reused(&self) -> usize {
        self.reused.load(Ordering::Relaxed)
    }
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
