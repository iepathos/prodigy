//! Agent lifecycle management
//!
//! This module handles the creation, initialization, and cleanup of agents
//! within the MapReduce framework. It manages worktree creation, branch
//! operations, and merging results back to the parent.

use super::types::{AgentConfig, AgentHandle};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::WorkflowStep;
use crate::worktree::WorktreeManager;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// Error type for lifecycle operations
#[derive(Debug, thiserror::Error)]
pub enum LifecycleError {
    #[error("Failed to create worktree: {0}")]
    WorktreeCreation(String),
    #[error("Failed to create branch: {0}")]
    BranchCreation(String),
    #[error("Failed to merge branch: {0}")]
    MergeError(String),
    #[error("Git operation failed: {0}")]
    GitError(String),
    #[error("Cleanup failed: {0}")]
    CleanupError(String),
}

/// Result type for lifecycle operations
pub type LifecycleResult<T> = Result<T, LifecycleError>;

/// Trait for managing agent lifecycle
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait AgentLifecycleManager: Send + Sync {
    /// Create a new agent with a dedicated worktree
    async fn create_agent(
        &self,
        config: AgentConfig,
        commands: Vec<WorkflowStep>,
    ) -> LifecycleResult<AgentHandle>;

    /// Create a branch for an agent in its worktree
    async fn create_agent_branch(
        &self,
        worktree_path: &Path,
        branch_name: &str,
    ) -> LifecycleResult<()>;

    /// Merge an agent's changes back to the parent
    async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        env: &ExecutionEnvironment,
    ) -> LifecycleResult<()>;

    /// Handle merge and cleanup after agent completion
    async fn handle_merge_and_cleanup(
        &self,
        is_successful: bool,
        env: &ExecutionEnvironment,
        worktree_path: &Path,
        worktree_name: &str,
        branch_name: &str,
        template_steps: &[WorkflowStep],
        item_id: &str,
    ) -> LifecycleResult<bool>;

    /// Clean up an agent's resources
    async fn cleanup_agent(&self, handle: AgentHandle) -> LifecycleResult<()>;

    /// Get commits from a worktree
    async fn get_worktree_commits(&self, worktree_path: &Path) -> LifecycleResult<Vec<String>>;

    /// Get modified files in a worktree
    async fn get_modified_files(&self, worktree_path: &Path) -> LifecycleResult<Vec<String>>;
}

/// Default implementation of the lifecycle manager
pub struct DefaultLifecycleManager {
    worktree_manager: Arc<WorktreeManager>,
}

impl DefaultLifecycleManager {
    /// Create a new lifecycle manager
    pub fn new(worktree_manager: Arc<WorktreeManager>) -> Self {
        Self { worktree_manager }
    }
}

#[async_trait]
impl AgentLifecycleManager for DefaultLifecycleManager {
    async fn create_agent(
        &self,
        config: AgentConfig,
        commands: Vec<WorkflowStep>,
    ) -> LifecycleResult<AgentHandle> {
        // Create a worktree for this agent
        let session_id = format!("mapreduce-agent-{}", config.id);

        let worktree_session = self
            .worktree_manager
            .create_session_with_id(&session_id)
            .await
            .map_err(|e| LifecycleError::WorktreeCreation(e.to_string()))?;

        // Create the agent handle with initial state
        let handle = AgentHandle::new(config, worktree_session, commands);

        Ok(handle)
    }

    async fn create_agent_branch(
        &self,
        worktree_path: &Path,
        branch_name: &str,
    ) -> LifecycleResult<()> {
        use tokio::process::Command;

        // Create branch from current HEAD
        let output = Command::new("git")
            .args(["checkout", "-b", branch_name])
            .current_dir(worktree_path)
            .output()
            .await
            .map_err(|e| LifecycleError::GitError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LifecycleError::BranchCreation(format!(
                "Failed to create branch {}: {}",
                branch_name, stderr
            )));
        }

        Ok(())
    }

    async fn merge_agent_to_parent(
        &self,
        agent_branch: &str,
        env: &ExecutionEnvironment,
    ) -> LifecycleResult<()> {
        use tokio::process::Command;
        // Get parent worktree path (always use working_dir since we always use worktrees)
        let parent_worktree_path = &env.working_dir;

        // First, fetch the agent branch in the parent worktree
        let output = Command::new("git")
            .args(["fetch", ".", &format!("{}:{}", agent_branch, agent_branch)])
            .current_dir(&**parent_worktree_path)
            .output()
            .await
            .map_err(|e| LifecycleError::GitError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LifecycleError::MergeError(format!(
                "Failed to fetch branch {}: {}",
                agent_branch, stderr
            )));
        }

        // Now merge the branch
        let output = Command::new("git")
            .args(["merge", "--no-ff", agent_branch])
            .current_dir(&**parent_worktree_path)
            .output()
            .await
            .map_err(|e| LifecycleError::GitError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LifecycleError::MergeError(format!(
                "Failed to merge branch {}: {}",
                agent_branch, stderr
            )));
        }

        Ok(())
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
        if is_successful && env.worktree_name.is_some() {
            // Create and checkout branch
            self.create_agent_branch(worktree_path, branch_name).await?;

            // Try to merge
            match self.merge_agent_to_parent(branch_name, env).await {
                Ok(()) => {
                    info!("Successfully merged agent {} to parent worktree", item_id);
                    self.worktree_manager
                        .cleanup_session(worktree_name, true)
                        .await
                        .map_err(|e| LifecycleError::CleanupError(e.to_string()))?;
                    Ok(true)
                }
                Err(e) => {
                    warn!("Failed to merge agent {} to parent: {}", item_id, e);
                    Ok(false)
                }
            }
        } else {
            // Cleanup if no parent or failed
            if !template_steps.is_empty() {
                self.worktree_manager
                    .cleanup_session(worktree_name, true)
                    .await
                    .map_err(|e| LifecycleError::CleanupError(e.to_string()))?;
            }
            Ok(false)
        }
    }

    async fn cleanup_agent(&self, handle: AgentHandle) -> LifecycleResult<()> {
        // Clean up the worktree
        self.worktree_manager
            .cleanup_session(&handle.worktree_session.name, true)
            .await
            .map_err(|e| LifecycleError::CleanupError(e.to_string()))?;

        Ok(())
    }

    async fn get_worktree_commits(&self, worktree_path: &Path) -> LifecycleResult<Vec<String>> {
        use crate::cook::execution::mapreduce::resources::git_operations::{
            GitOperationsConfig, GitOperationsService, GitResultExt,
        };

        let mut service = GitOperationsService::new(GitOperationsConfig::default());
        match service
            .get_worktree_commits(worktree_path, None, None)
            .await
        {
            Ok(commits) => Ok(commits.to_string_list()),
            Err(e) => {
                warn!("Failed to get worktree commits: {}", e);
                Ok(vec![])
            }
        }
    }

    async fn get_modified_files(&self, worktree_path: &Path) -> LifecycleResult<Vec<String>> {
        use crate::cook::execution::mapreduce::resources::git_operations::{
            GitOperationsConfig, GitOperationsService, GitResultExt,
        };

        let mut service = GitOperationsService::new(GitOperationsConfig::default());
        match service
            .get_worktree_modified_files(worktree_path, None)
            .await
        {
            Ok(files) => Ok(files.to_string_list()),
            Err(e) => {
                warn!("Failed to get modified files: {}", e);
                Ok(vec![])
            }
        }
    }
}
