//! Extended environment for workflow execution with checkpoint support
//!
//! This module provides ExecutionEnv, an extension of WorkflowEnv that includes
//! session context, checkpoint management, and variable storage for Effect-based
//! workflow execution.

use super::environment::WorkflowEnv;
use crate::cook::workflow::checkpoint::CheckpointManager;
use crate::cook::workflow::variables::VariableStore;
use std::path::PathBuf;
use std::sync::Arc;

/// Extended environment for workflow execution with checkpoint support
#[derive(Clone)]
pub struct ExecutionEnv {
    /// Base workflow environment (Claude/shell runners, patterns)
    pub workflow_env: WorkflowEnv,
    /// Session identifier
    pub session_id: String,
    /// Workflow file path (for checkpoint)
    pub workflow_path: PathBuf,
    /// Checkpoint manager
    pub checkpoint_manager: Arc<CheckpointManager>,
    /// Variable store for captured outputs
    pub variable_store: VariableStore,
    /// Verbosity level
    pub verbosity: u8,
}

impl std::fmt::Debug for ExecutionEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionEnv")
            .field("workflow_env", &self.workflow_env)
            .field("session_id", &self.session_id)
            .field("workflow_path", &self.workflow_path)
            .field("checkpoint_manager", &"<CheckpointManager>")
            .field("variable_store", &self.variable_store)
            .field("verbosity", &self.verbosity)
            .finish()
    }
}

impl ExecutionEnv {
    /// Create builder for ExecutionEnv
    pub fn builder(workflow_env: WorkflowEnv) -> ExecutionEnvBuilder {
        ExecutionEnvBuilder::new(workflow_env)
    }
}

/// Builder for ExecutionEnv
pub struct ExecutionEnvBuilder {
    workflow_env: WorkflowEnv,
    session_id: Option<String>,
    workflow_path: Option<PathBuf>,
    checkpoint_manager: Option<Arc<CheckpointManager>>,
    variable_store: Option<VariableStore>,
    verbosity: u8,
}

impl ExecutionEnvBuilder {
    /// Create new builder with workflow environment
    pub fn new(workflow_env: WorkflowEnv) -> Self {
        Self {
            workflow_env,
            session_id: None,
            workflow_path: None,
            checkpoint_manager: None,
            variable_store: None,
            verbosity: 0,
        }
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set workflow path
    pub fn with_workflow_path(mut self, path: PathBuf) -> Self {
        self.workflow_path = Some(path);
        self
    }

    /// Set checkpoint manager
    pub fn with_checkpoint_manager(mut self, manager: Arc<CheckpointManager>) -> Self {
        self.checkpoint_manager = Some(manager);
        self
    }

    /// Set variable store
    pub fn with_variable_store(mut self, store: VariableStore) -> Self {
        self.variable_store = Some(store);
        self
    }

    /// Set verbosity level
    pub fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Build ExecutionEnv
    ///
    /// Returns error if required fields are missing
    pub fn build(self) -> Result<ExecutionEnv, String> {
        Ok(ExecutionEnv {
            workflow_env: self.workflow_env,
            session_id: self
                .session_id
                .ok_or_else(|| "session_id is required".to_string())?,
            workflow_path: self
                .workflow_path
                .ok_or_else(|| "workflow_path is required".to_string())?,
            checkpoint_manager: self
                .checkpoint_manager
                .ok_or_else(|| "checkpoint_manager is required".to_string())?,
            variable_store: self.variable_store.unwrap_or_default(),
            verbosity: self.verbosity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::workflow::checkpoint_path::CheckpointStorage;

    fn create_test_workflow_env() -> WorkflowEnv {
        use crate::cook::workflow::effects::environment::{ClaudeRunner, RunnerOutput};
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::path::Path;
        use std::sync::Arc;

        struct MockClaudeRunner;

        #[async_trait]
        impl ClaudeRunner for MockClaudeRunner {
            async fn run(
                &self,
                _command: &str,
                _working_dir: &Path,
                _env_vars: HashMap<String, String>,
            ) -> anyhow::Result<RunnerOutput> {
                Ok(RunnerOutput::success("test output".to_string()))
            }
        }

        WorkflowEnv::builder()
            .with_claude_runner(Arc::new(MockClaudeRunner))
            .build()
    }

    fn create_test_checkpoint_manager() -> Arc<CheckpointManager> {
        Arc::new(CheckpointManager::with_storage(
            CheckpointStorage::Session {
                session_id: "test-session".to_string(),
            },
        ))
    }

    #[test]
    fn test_builder_with_all_fields() {
        let workflow_env = create_test_workflow_env();
        let checkpoint_manager = create_test_checkpoint_manager();

        let result = ExecutionEnv::builder(workflow_env)
            .with_session_id("test-session")
            .with_workflow_path(PathBuf::from("/tmp/workflow.yml"))
            .with_checkpoint_manager(checkpoint_manager)
            .with_verbosity(2)
            .build();

        assert!(result.is_ok());
        let env = result.unwrap();
        assert_eq!(env.session_id, "test-session");
        assert_eq!(env.workflow_path, PathBuf::from("/tmp/workflow.yml"));
        assert_eq!(env.verbosity, 2);
    }

    #[test]
    fn test_builder_missing_session_id() {
        let workflow_env = create_test_workflow_env();
        let checkpoint_manager = create_test_checkpoint_manager();

        let result = ExecutionEnv::builder(workflow_env)
            .with_workflow_path(PathBuf::from("/tmp/workflow.yml"))
            .with_checkpoint_manager(checkpoint_manager)
            .build();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session_id is required"));
    }

    #[test]
    fn test_builder_missing_workflow_path() {
        let workflow_env = create_test_workflow_env();
        let checkpoint_manager = create_test_checkpoint_manager();

        let result = ExecutionEnv::builder(workflow_env)
            .with_session_id("test-session")
            .with_checkpoint_manager(checkpoint_manager)
            .build();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("workflow_path is required"));
    }

    #[test]
    fn test_builder_default_variable_store() {
        let workflow_env = create_test_workflow_env();
        let checkpoint_manager = create_test_checkpoint_manager();

        let result = ExecutionEnv::builder(workflow_env)
            .with_session_id("test-session")
            .with_workflow_path(PathBuf::from("/tmp/workflow.yml"))
            .with_checkpoint_manager(checkpoint_manager)
            .build();

        assert!(result.is_ok());
        // Variable store should be initialized with default
    }
}
