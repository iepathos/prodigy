//! Session management operations for cook orchestrator
//!
//! Handles session lifecycle, state management, and resumption logic.

use crate::abstractions::git::GitOperations;
use crate::config::WorkflowConfig;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::core::{CookConfig, ExecutionEnvironment};
use crate::cook::session::{SessionManager, SessionState};
use crate::worktree::WorktreeManager;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Session operations for managing cook sessions
#[derive(Clone)]
pub struct SessionOperations {
    #[allow(dead_code)] // Will be used in resume_workflow and resume_workflow_execution
    session_manager: Arc<dyn SessionManager>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    user_interaction: Arc<dyn UserInteraction>,
    git_operations: Arc<dyn GitOperations>,
    subprocess: crate::subprocess::SubprocessManager,
}

impl SessionOperations {
    /// Create a new SessionOperations instance
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: crate::subprocess::SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            claude_executor,
            user_interaction,
            git_operations,
            subprocess,
        }
    }

    /// Generate session ID using unified format
    pub fn generate_session_id(&self) -> String {
        super::construction::generate_session_id()
    }

    /// Calculate workflow hash for validation (pure function)
    pub fn calculate_workflow_hash(workflow: &WorkflowConfig) -> String {
        let mut hasher = Sha256::new();
        let serialized = serde_json::to_string(workflow).unwrap_or_default();
        hasher.update(serialized);
        format!("{:x}", hasher.finalize())
    }

    /// Check prerequisites (standard check)
    pub async fn check_prerequisites(&self) -> Result<()> {
        // Skip checks in test mode
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return Ok(());
        }

        // Check Claude CLI
        if !self.claude_executor.check_claude_cli().await? {
            anyhow::bail!("Claude CLI is not available. Please install it first.");
        }

        // Check git repository
        if !self.git_operations.is_git_repo().await {
            anyhow::bail!("Not in a git repository. Please run from a git repository.");
        }

        Ok(())
    }

    /// Check prerequisites with config-aware git checking
    pub async fn check_prerequisites_with_config(&self, config: &CookConfig) -> Result<()> {
        // Skip checks in test mode
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return Ok(());
        }

        // Check Claude CLI
        if !self.claude_executor.check_claude_cli().await? {
            anyhow::bail!("Claude CLI is not available. Please install it first.");
        }

        // Check if this is a temporary workflow (batch/exec commands)
        let is_temp_workflow = config
            .command
            .playbook
            .to_str()
            .map(|s| s.contains("/tmp/") || s.contains("/var/folders/") || s.contains("Temp"))
            .unwrap_or(false);

        // Always check git except for temporary workflows
        if !is_temp_workflow && !self.git_operations.is_git_repo().await {
            anyhow::bail!("Not in a git repository. Please run from a git repository.");
        }

        Ok(())
    }

    /// Restore the execution environment from saved state
    pub async fn restore_environment(
        &self,
        state: &SessionState,
        config: &CookConfig,
    ) -> Result<ExecutionEnvironment> {
        let mut working_dir = Arc::new(state.working_directory.clone());
        let mut worktree_name: Option<Arc<str>> =
            state.worktree_name.as_ref().map(|s| Arc::from(s.as_str()));

        // If using a worktree, verify it still exists
        if let Some(ref name) = worktree_name {
            // Get merge config from workflow or mapreduce config
            let merge_config = config.workflow.merge.clone().or_else(|| {
                config
                    .mapreduce_config
                    .as_ref()
                    .and_then(|m| m.merge.clone())
            });

            // Get workflow environment variables
            let workflow_env = config.workflow.env.clone().unwrap_or_default();

            let worktree_manager = WorktreeManager::with_config(
                config.project_path.to_path_buf(),
                self.subprocess.clone(),
                config.command.verbosity,
                merge_config,
                workflow_env,
            )?;

            // Check if worktree still exists by trying to list sessions
            let sessions = worktree_manager.list_sessions().await?;
            if !sessions.iter().any(|s| s.name.as_str() == name.as_ref()) {
                // Recreate the worktree if it was deleted
                self.user_interaction
                    .display_warning(&format!("Worktree {} was deleted, recreating...", name));
                let session = worktree_manager.create_session().await?;
                working_dir = Arc::new(session.path.clone());
                worktree_name = Some(Arc::from(session.name.as_ref()));
            } else {
                // Get the existing worktree path
                let sessions = worktree_manager.list_sessions().await?;
                if let Some(session) = sessions.iter().find(|s| s.name.as_str() == name.as_ref()) {
                    working_dir = Arc::new(session.path.clone());
                }
            }
        }

        Ok(ExecutionEnvironment {
            working_dir,
            project_dir: Arc::clone(&config.project_path),
            worktree_name,
            session_id: Arc::from(state.session_id.as_str()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_workflow_hash_deterministic() {
        use std::collections::HashMap;
        let mut env1 = HashMap::new();
        env1.insert("TEST".to_string(), "value".to_string());

        let workflow = WorkflowConfig {
            commands: vec![],
            env: Some(env1),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let hash1 = SessionOperations::calculate_workflow_hash(&workflow);
        let hash2 = SessionOperations::calculate_workflow_hash(&workflow);

        assert_eq!(hash1, hash2, "Hash should be deterministic");
        assert!(!hash1.is_empty(), "Hash should not be empty");
    }

    #[test]
    fn test_calculate_workflow_hash_different_workflows() {
        use std::collections::HashMap;
        let mut env1 = HashMap::new();
        env1.insert("TEST1".to_string(), "value1".to_string());

        let mut env2 = HashMap::new();
        env2.insert("TEST2".to_string(), "value2".to_string());

        let workflow1 = WorkflowConfig {
            commands: vec![],
            env: Some(env1),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let workflow2 = WorkflowConfig {
            commands: vec![],
            env: Some(env2),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let hash1 = SessionOperations::calculate_workflow_hash(&workflow1);
        let hash2 = SessionOperations::calculate_workflow_hash(&workflow2);

        assert_ne!(
            hash1, hash2,
            "Different workflows should have different hashes"
        );
    }
}
