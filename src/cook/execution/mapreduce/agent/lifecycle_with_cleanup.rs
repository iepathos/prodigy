//! Agent lifecycle management with integrated cleanup
//!
//! This module extends the default lifecycle manager to include
//! automatic worktree cleanup functionality.

use super::lifecycle::{
    AgentLifecycleManager, DefaultLifecycleManager, LifecycleError, LifecycleResult,
};
use super::types::{AgentConfig, AgentHandle};
use crate::cook::execution::mapreduce::cleanup::{
    WorktreeCleanupConfig, WorktreeCleanupCoordinator,
};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use crate::worktree::WorktreeManager;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Lifecycle manager with integrated cleanup support
pub struct CleanupAwareLifecycleManager {
    /// Base lifecycle manager
    base_manager: DefaultLifecycleManager,
    /// Cleanup coordinator
    cleanup_coordinator: Arc<WorktreeCleanupCoordinator>,
    /// Job ID for tracking
    job_id: String,
    /// Cleanup configuration
    cleanup_config: WorktreeCleanupConfig,
}

impl CleanupAwareLifecycleManager {
    /// Create a new lifecycle manager with cleanup support
    pub fn new(
        worktree_manager: Arc<WorktreeManager>,
        cleanup_config: WorktreeCleanupConfig,
        worktree_base_path: PathBuf,
        job_id: String,
    ) -> Self {
        let base_manager = DefaultLifecycleManager::new(worktree_manager);
        let cleanup_coordinator = Arc::new(WorktreeCleanupCoordinator::new(
            cleanup_config.clone(),
            worktree_base_path,
        ));

        Self {
            base_manager,
            cleanup_coordinator,
            job_id,
            cleanup_config,
        }
    }

    /// Start the cleanup coordinator
    pub async fn start_cleanup_coordinator(&self) {
        self.cleanup_coordinator.start().await;
    }

    /// Stop the cleanup coordinator
    pub async fn stop_cleanup_coordinator(&self) {
        self.cleanup_coordinator.stop().await;
    }

    /// Clean up all worktrees for the current job
    pub async fn cleanup_job_worktrees(&self) -> LifecycleResult<usize> {
        self.cleanup_coordinator
            .cleanup_job(&self.job_id)
            .await
            .map_err(|e| LifecycleError::CleanupError(e.to_string()))
    }
}

#[async_trait]
impl AgentLifecycleManager for CleanupAwareLifecycleManager {
    async fn create_agent(
        &self,
        config: AgentConfig,
        commands: Vec<WorkflowStep>,
    ) -> LifecycleResult<AgentHandle> {
        // Create the agent using the base manager
        let handle = self
            .base_manager
            .create_agent(config.clone(), commands)
            .await?;

        // Register the worktree with the cleanup coordinator
        let _cleanup_guard = self
            .cleanup_coordinator
            .register_worktree(
                &self.job_id,
                &config.id,
                handle.worktree_session.path.clone(),
            )
            .await;

        info!(
            "Created agent {} with worktree at {} (cleanup enabled)",
            config.id,
            handle.worktree_session.path.display()
        );

        Ok(handle)
    }

    async fn create_agent_branch(
        &self,
        worktree_path: &Path,
        branch_name: &str,
    ) -> LifecycleResult<()> {
        self.base_manager
            .create_agent_branch(worktree_path, branch_name)
            .await
    }

    async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        env: &ExecutionEnvironment,
    ) -> LifecycleResult<()> {
        self.base_manager
            .merge_agent_to_parent(agent_branch, env)
            .await
    }

    async fn handle_merge_and_cleanup(
        &self,
        is_successful: bool,
        env: &ExecutionEnvironment,
        worktree_path: &Path,
        worktree_name: &str,
        branch_name: &str,
        template_steps: &[WorkflowStep],
        item_id: &str,
    ) -> LifecycleResult<bool> {
        // First handle merge using base manager
        let merge_result = self
            .base_manager
            .handle_merge_and_cleanup(
                is_successful,
                env,
                worktree_path,
                worktree_name,
                branch_name,
                template_steps,
                item_id,
            )
            .await?;

        // If auto cleanup is enabled, schedule cleanup
        if self.cleanup_config.auto_cleanup && is_successful {
            debug!(
                "Scheduling cleanup for agent {} worktree after {} seconds",
                item_id, self.cleanup_config.cleanup_delay_secs
            );

            // Schedule cleanup after configured delay
            let cleanup_task = crate::cook::execution::mapreduce::cleanup::CleanupTask::Scheduled {
                worktree_path: worktree_path.to_path_buf(),
                delay: Duration::from_secs(self.cleanup_config.cleanup_delay_secs),
            };

            if let Err(e) = self
                .cleanup_coordinator
                .schedule_cleanup(cleanup_task)
                .await
            {
                warn!("Failed to schedule cleanup for agent {}: {}", item_id, e);
            }
        } else if !is_successful && self.cleanup_config.auto_cleanup {
            // Immediate cleanup for failed agents
            debug!("Immediately cleaning up failed agent {} worktree", item_id);

            let cleanup_task = crate::cook::execution::mapreduce::cleanup::CleanupTask::Immediate {
                worktree_path: worktree_path.to_path_buf(),
                job_id: self.job_id.clone(),
            };

            if let Err(e) = self
                .cleanup_coordinator
                .schedule_cleanup(cleanup_task)
                .await
            {
                warn!(
                    "Failed to schedule cleanup for failed agent {}: {}",
                    item_id, e
                );
            }
        }

        Ok(merge_result)
    }

    async fn cleanup_agent(&self, handle: AgentHandle) -> LifecycleResult<()> {
        // If auto cleanup is disabled, use base manager
        if !self.cleanup_config.auto_cleanup {
            return self.base_manager.cleanup_agent(handle).await;
        }

        // Schedule immediate cleanup
        let cleanup_task = crate::cook::execution::mapreduce::cleanup::CleanupTask::Immediate {
            worktree_path: handle.worktree_session.path.clone(),
            job_id: self.job_id.clone(),
        };

        self.cleanup_coordinator
            .schedule_cleanup(cleanup_task)
            .await
            .map_err(|e| LifecycleError::CleanupError(e.to_string()))?;

        Ok(())
    }

    async fn get_worktree_commits(&self, worktree_path: &Path) -> LifecycleResult<Vec<String>> {
        self.base_manager.get_worktree_commits(worktree_path).await
    }

    async fn get_modified_files(&self, worktree_path: &Path) -> LifecycleResult<Vec<String>> {
        self.base_manager.get_modified_files(worktree_path).await
    }
}
