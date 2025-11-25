//! Mock environment builders for testing
//!
//! This module provides builder patterns for creating test environments
//! without requiring full production dependencies. Use these builders
//! to create minimal mock environments for unit testing Reader pattern
//! effects.
//!
//! # Example
//!
//! ```ignore
//! use crate::cook::execution::mapreduce::mock_environment::MockMapEnvBuilder;
//!
//! #[tokio::test]
//! async fn test_agent_execution() {
//!     let env = MockMapEnvBuilder::new()
//!         .with_max_parallel(4)
//!         .with_job_id("test-job-123")
//!         .with_config("debug", json!(true))
//!         .build();
//!
//!     let effect = execute_agent(item);
//!     let result = effect.run_async(&env).await;
//!     assert!(result.is_ok());
//! }
//! ```

use crate::commands::CommandRegistry;
use crate::cook::execution::claude::ClaudeExecutor;
use crate::cook::execution::mapreduce::agent_command_executor::AgentCommandExecutor;
use crate::cook::execution::mapreduce::checkpoint::storage::{
    CheckpointStorage, FileCheckpointStorage,
};
use crate::cook::execution::mapreduce::environment::{MapEnv, MapEnvParams, PhaseEnv};
use crate::cook::execution::ExecutionResult;
use crate::cook::session::{
    SessionInfo, SessionManager, SessionState, SessionSummary, SessionUpdate,
};
use crate::cook::workflow::WorkflowStep;
use crate::subprocess::runner::{ExitStatus, ProcessCommand, ProcessOutput, ProcessRunner};
use crate::subprocess::SubprocessManager;
use crate::worktree::WorktreeManager;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// =============================================================================
// Mock Dependencies for Testing
// =============================================================================

/// Mock ClaudeExecutor that always succeeds
#[derive(Clone)]
struct MockClaudeExecutor;

#[async_trait]
impl ClaudeExecutor for MockClaudeExecutor {
    async fn execute_claude_command(
        &self,
        command: &str,
        _project_path: &Path,
        _env_vars: HashMap<String, String>,
    ) -> anyhow::Result<ExecutionResult> {
        Ok(ExecutionResult {
            success: true,
            stdout: format!("Mock output for: {}", command),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        })
    }

    async fn check_claude_cli(&self) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn get_claude_version(&self) -> anyhow::Result<String> {
        Ok("mock-1.0.0".to_string())
    }
}

/// Mock ProcessRunner that always succeeds
#[derive(Clone)]
struct MockProcessRunner;

#[async_trait]
impl ProcessRunner for MockProcessRunner {
    async fn run(
        &self,
        _command: ProcessCommand,
    ) -> Result<ProcessOutput, crate::subprocess::error::ProcessError> {
        Ok(ProcessOutput {
            status: ExitStatus::Success,
            stdout: "mock output".to_string(),
            stderr: String::new(),
            duration: std::time::Duration::from_millis(1),
        })
    }

    async fn run_streaming(
        &self,
        _command: ProcessCommand,
    ) -> Result<crate::subprocess::runner::ProcessStream, crate::subprocess::error::ProcessError>
    {
        unimplemented!("Streaming not implemented for mock")
    }
}

/// Mock SessionManager for testing
#[derive(Clone)]
struct MockSessionManager;

#[async_trait]
impl SessionManager for MockSessionManager {
    async fn start_session(&self, _session_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn update_session(&self, _update: SessionUpdate) -> anyhow::Result<()> {
        Ok(())
    }

    async fn complete_session(&self) -> anyhow::Result<SessionSummary> {
        Ok(SessionSummary {
            iterations: 0,
            files_changed: 0,
        })
    }

    fn get_state(&self) -> anyhow::Result<SessionState> {
        Ok(SessionState::new(
            "mock-session".to_string(),
            std::env::temp_dir(),
        ))
    }

    async fn save_state(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_state(&self, _path: &Path) -> anyhow::Result<()> {
        Ok(())
    }

    async fn load_session(&self, _session_id: &str) -> anyhow::Result<SessionState> {
        Ok(SessionState::new(
            "mock-session".to_string(),
            std::env::temp_dir(),
        ))
    }

    async fn save_checkpoint(&self, _state: &SessionState) -> anyhow::Result<()> {
        Ok(())
    }

    async fn list_resumable(&self) -> anyhow::Result<Vec<SessionInfo>> {
        Ok(vec![])
    }

    async fn get_last_interrupted(&self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
}

/// Create a mock AgentCommandExecutor for testing
fn create_mock_command_executor() -> Arc<AgentCommandExecutor> {
    let mock_claude: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor);
    let mock_runner = Arc::new(MockProcessRunner);
    let mock_subprocess = Arc::new(SubprocessManager::new(mock_runner));
    let mock_session: Arc<dyn SessionManager> = Arc::new(MockSessionManager);
    let mock_registry = Arc::new(CommandRegistry::new());

    Arc::new(AgentCommandExecutor::new(
        mock_claude,
        mock_subprocess,
        mock_session,
        mock_registry,
    ))
}

// =============================================================================
// MockMapEnvBuilder
// =============================================================================

/// Builder for creating mock MapEnv instances for testing.
///
/// This builder provides a fluent API for constructing test environments
/// with minimal boilerplate. All fields have sensible defaults.
///
/// # Example
///
/// ```ignore
/// let env = MockMapEnvBuilder::new()
///     .with_max_parallel(4)
///     .with_job_id("test-job")
///     .build();
/// ```
pub struct MockMapEnvBuilder {
    max_parallel: usize,
    job_id: String,
    config: HashMap<String, Value>,
    workflow_env: HashMap<String, Value>,
    agent_template: Vec<WorkflowStep>,
    worktree_base_path: PathBuf,
}

impl Default for MockMapEnvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockMapEnvBuilder {
    /// Create a new mock environment builder with default values.
    pub fn new() -> Self {
        Self {
            max_parallel: 5,
            job_id: "mock-job-000".to_string(),
            config: HashMap::new(),
            workflow_env: HashMap::new(),
            agent_template: vec![],
            worktree_base_path: std::env::temp_dir().join("prodigy-test-worktrees"),
        }
    }

    /// Set the maximum number of parallel agents.
    pub fn with_max_parallel(mut self, max_parallel: usize) -> Self {
        self.max_parallel = max_parallel;
        self
    }

    /// Set the job ID.
    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = job_id.into();
        self
    }

    /// Add a configuration value.
    pub fn with_config(mut self, key: impl Into<String>, value: Value) -> Self {
        self.config.insert(key.into(), value);
        self
    }

    /// Add a workflow environment variable.
    pub fn with_workflow_env(mut self, key: impl Into<String>, value: Value) -> Self {
        self.workflow_env.insert(key.into(), value);
        self
    }

    /// Set the agent template.
    pub fn with_agent_template(mut self, template: Vec<WorkflowStep>) -> Self {
        self.agent_template = template;
        self
    }

    /// Set the worktree base path.
    pub fn with_worktree_base_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.worktree_base_path = path.into();
        self
    }

    /// Enable debug mode in config.
    pub fn with_debug(mut self) -> Self {
        self.config
            .insert("debug".to_string(), serde_json::json!(true));
        self
    }

    /// Enable verbose mode in config.
    pub fn with_verbose(mut self) -> Self {
        self.config
            .insert("verbose".to_string(), serde_json::json!(true));
        self
    }

    /// Build the mock MapEnv.
    ///
    /// # Panics
    ///
    /// Panics if WorktreeManager cannot be created for the test path.
    /// This should not happen in normal test conditions.
    pub fn build(self) -> MapEnv {
        // Ensure the test directory exists
        let _ = std::fs::create_dir_all(&self.worktree_base_path);

        let subprocess = SubprocessManager::production();
        let worktree_manager = Arc::new(
            WorktreeManager::new(self.worktree_base_path, subprocess)
                .expect("Failed to create test worktree manager"),
        );

        // Create file-based storage for testing (uses temp directory)
        let checkpoint_path = std::env::temp_dir().join("prodigy-test-checkpoints");
        let _ = std::fs::create_dir_all(&checkpoint_path);
        let storage: Arc<dyn CheckpointStorage> =
            Arc::new(FileCheckpointStorage::new(checkpoint_path, false));

        // Create a mock command executor for testing
        let command_executor = create_mock_command_executor();

        MapEnv::new(MapEnvParams {
            worktree_manager,
            command_executor,
            storage,
            agent_template: self.agent_template,
            job_id: self.job_id,
            max_parallel: self.max_parallel,
            workflow_env: self.workflow_env,
            config: self.config,
        })
    }
}

// =============================================================================
// MockPhaseEnvBuilder
// =============================================================================

/// Builder for creating mock PhaseEnv instances for testing.
///
/// # Example
///
/// ```ignore
/// let env = MockPhaseEnvBuilder::new()
///     .with_variable("count", json!(42))
///     .build();
/// ```
pub struct MockPhaseEnvBuilder {
    variables: HashMap<String, Value>,
    workflow_env: HashMap<String, Value>,
}

impl Default for MockPhaseEnvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPhaseEnvBuilder {
    /// Create a new mock phase environment builder.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            workflow_env: HashMap::new(),
        }
    }

    /// Add a variable.
    pub fn with_variable(mut self, name: impl Into<String>, value: Value) -> Self {
        self.variables.insert(name.into(), value);
        self
    }

    /// Add multiple variables.
    pub fn with_variables(mut self, vars: HashMap<String, Value>) -> Self {
        self.variables.extend(vars);
        self
    }

    /// Add a workflow environment variable.
    pub fn with_workflow_env(mut self, key: impl Into<String>, value: Value) -> Self {
        self.workflow_env.insert(key.into(), value);
        self
    }

    /// Build the mock PhaseEnv.
    pub fn build(self) -> PhaseEnv {
        // Create file-based storage for testing (uses temp directory)
        let checkpoint_path = std::env::temp_dir().join("prodigy-test-checkpoints");
        let _ = std::fs::create_dir_all(&checkpoint_path);
        let storage: Arc<dyn CheckpointStorage> =
            Arc::new(FileCheckpointStorage::new(checkpoint_path, false));

        // Create a mock command executor for testing
        let command_executor = create_mock_command_executor();

        PhaseEnv::new(command_executor, storage, self.variables, self.workflow_env)
    }
}

// =============================================================================
// Convenience Functions
// =============================================================================

/// Create a minimal mock MapEnv for simple tests.
///
/// # Example
///
/// ```ignore
/// let env = mock_map_env();
/// let effect = get_max_parallel();
/// assert_eq!(effect.run(&env).await.unwrap(), 5);
/// ```
pub fn mock_map_env() -> MapEnv {
    MockMapEnvBuilder::new().build()
}

/// Create a minimal mock PhaseEnv for simple tests.
///
/// # Example
///
/// ```ignore
/// let env = mock_phase_env();
/// let effect = get_variables();
/// assert!(effect.run(&env).await.unwrap().is_empty());
/// ```
pub fn mock_phase_env() -> PhaseEnv {
    MockPhaseEnvBuilder::new().build()
}

/// Create a mock MapEnv with debug mode enabled.
///
/// # Example
///
/// ```ignore
/// let env = mock_map_env_debug();
/// let debug = get_config_value("debug").run(&env).await.unwrap();
/// assert_eq!(debug, Some(json!(true)));
/// ```
pub fn mock_map_env_debug() -> MapEnv {
    MockMapEnvBuilder::new().with_debug().build()
}

/// Create a mock MapEnv with specific max_parallel setting.
///
/// # Example
///
/// ```ignore
/// let env = mock_map_env_with_parallel(10);
/// let max = get_max_parallel().run(&env).await.unwrap();
/// assert_eq!(max, 10);
/// ```
pub fn mock_map_env_with_parallel(max_parallel: usize) -> MapEnv {
    MockMapEnvBuilder::new()
        .with_max_parallel(max_parallel)
        .build()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::environment_helpers::*;

    #[tokio::test]
    async fn test_mock_map_env_builder_defaults() {
        let env = MockMapEnvBuilder::new().build();

        assert_eq!(env.max_parallel, 5);
        assert_eq!(env.job_id, "mock-job-000");
        assert!(env.config.is_empty());
        assert!(env.workflow_env.is_empty());
    }

    #[tokio::test]
    async fn test_mock_map_env_builder_custom() {
        let env = MockMapEnvBuilder::new()
            .with_max_parallel(20)
            .with_job_id("custom-job")
            .with_config("timeout", serde_json::json!(30))
            .with_debug()
            .build();

        assert_eq!(env.max_parallel, 20);
        assert_eq!(env.job_id, "custom-job");
        assert_eq!(env.config.get("timeout"), Some(&serde_json::json!(30)));
        assert_eq!(env.config.get("debug"), Some(&serde_json::json!(true)));
    }

    #[tokio::test]
    async fn test_mock_map_env_with_reader_pattern() {
        let env = MockMapEnvBuilder::new()
            .with_max_parallel(8)
            .with_job_id("reader-test")
            .build();

        // Test Reader pattern helpers work with mock env
        let max = get_max_parallel().run(&env).await.unwrap();
        assert_eq!(max, 8);

        let job_id = get_job_id().run(&env).await.unwrap();
        assert_eq!(job_id, "reader-test");
    }

    #[tokio::test]
    async fn test_mock_phase_env_builder() {
        let env = MockPhaseEnvBuilder::new()
            .with_variable("count", serde_json::json!(42))
            .with_variable("name", serde_json::json!("test"))
            .build();

        // Test Reader pattern helpers work with mock env
        let count = get_variable("count").run(&env).await.unwrap();
        assert_eq!(count, Some(serde_json::json!(42)));

        let name = get_variable("name").run(&env).await.unwrap();
        assert_eq!(name, Some(serde_json::json!("test")));
    }

    #[tokio::test]
    async fn test_convenience_functions() {
        let env = mock_map_env();
        assert_eq!(env.max_parallel, 5);

        let env = mock_map_env_debug();
        assert_eq!(env.config.get("debug"), Some(&serde_json::json!(true)));

        let env = mock_map_env_with_parallel(15);
        assert_eq!(env.max_parallel, 15);

        let env = mock_phase_env();
        assert!(env.variables.is_empty());
    }

    #[tokio::test]
    async fn test_local_overrides_with_mock_env() {
        let env = MockMapEnvBuilder::new().with_max_parallel(5).build();

        // Test local overrides work correctly
        let effect = with_max_parallel(50, get_max_parallel());
        let result = effect.run(&env).await.unwrap();
        assert_eq!(result, 50);

        // Original unchanged
        let effect = get_max_parallel();
        let result = effect.run(&env).await.unwrap();
        assert_eq!(result, 5);
    }
}
