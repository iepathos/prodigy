//! Environment types for workflow effect execution
//!
//! This module defines environment types used with Stillwater's Effect pattern
//! for dependency injection in workflow operations.
//!
//! # Architecture
//!
//! The environment types provide access to:
//! - Command runners (Claude, shell)
//! - Output parsing patterns
//! - Working directory context
//!
//! # Testing
//!
//! Mock implementations can be created for testing without actual I/O:
//!
//! ```ignore
//! let mock_env = WorkflowEnv::builder()
//!     .with_mock_claude_runner(MockClaudeRunner::new())
//!     .with_mock_shell_runner(MockShellRunner::new())
//!     .build();
//! ```

use crate::cook::execution::ClaudeExecutor;
use crate::cook::workflow::pure::OutputPattern;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Output from running a command
#[derive(Debug, Clone)]
pub struct RunnerOutput {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code
    pub exit_code: Option<i32>,
    /// Whether the command succeeded
    pub success: bool,
    /// Optional JSON log location (for Claude commands)
    pub json_log_location: Option<String>,
}

impl RunnerOutput {
    /// Create a successful output
    pub fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
            json_log_location: None,
        }
    }

    /// Create a failed output
    pub fn failure(stderr: String, exit_code: i32) -> Self {
        Self {
            stdout: String::new(),
            stderr,
            exit_code: Some(exit_code),
            success: false,
            json_log_location: None,
        }
    }
}

/// Trait for running Claude commands
#[async_trait]
pub trait ClaudeRunner: Send + Sync {
    /// Run a Claude command and return the output
    async fn run(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: HashMap<String, String>,
    ) -> anyhow::Result<RunnerOutput>;
}

/// Trait for running shell commands
#[async_trait]
pub trait ShellRunner: Send + Sync {
    /// Run a shell command and return the output
    async fn run(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> anyhow::Result<RunnerOutput>;
}

/// Adapter to use ClaudeExecutor as ClaudeRunner
pub struct ClaudeExecutorAdapter {
    executor: Arc<dyn ClaudeExecutor>,
}

impl ClaudeExecutorAdapter {
    /// Create a new adapter from a ClaudeExecutor
    pub fn new(executor: Arc<dyn ClaudeExecutor>) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl ClaudeRunner for ClaudeExecutorAdapter {
    async fn run(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: HashMap<String, String>,
    ) -> anyhow::Result<RunnerOutput> {
        let result = self
            .executor
            .execute_claude_command(command, working_dir, env_vars)
            .await?;

        // Extract json_log_location before moving other fields
        let json_log_location = result.json_log_location().map(|s| s.to_string());

        Ok(RunnerOutput {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            success: result.success,
            json_log_location,
        })
    }
}

/// Default shell runner using tokio::process
pub struct DefaultShellRunner;

impl DefaultShellRunner {
    /// Create a new default shell runner
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultShellRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ShellRunner for DefaultShellRunner {
    async fn run(
        &self,
        command: &str,
        working_dir: &Path,
        env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> anyhow::Result<RunnerOutput> {
        use tokio::process::Command;
        use tokio::time::{timeout as tokio_timeout, Duration};

        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd.current_dir(working_dir);

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let output = if let Some(timeout_secs) = timeout {
            let duration = Duration::from_secs(timeout_secs);
            match tokio_timeout(duration, cmd.output()).await {
                Ok(result) => result?,
                Err(_) => {
                    return Ok(RunnerOutput {
                        stdout: String::new(),
                        stderr: format!("Command timed out after {} seconds", timeout_secs),
                        exit_code: Some(-1),
                        success: false,
                        json_log_location: None,
                    });
                }
            }
        } else {
            cmd.output().await?
        };

        Ok(RunnerOutput {
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            json_log_location: None,
        })
    }
}

/// Environment for workflow command execution
///
/// Provides dependencies needed for executing workflow commands including
/// Claude runner, shell runner, and output parsing patterns.
#[derive(Clone)]
pub struct WorkflowEnv {
    /// Claude command runner
    pub claude_runner: Arc<dyn ClaudeRunner>,
    /// Shell command runner
    pub shell_runner: Arc<dyn ShellRunner>,
    /// Output patterns for variable extraction
    pub output_patterns: Vec<OutputPattern>,
    /// Working directory for command execution
    pub working_dir: PathBuf,
    /// Environment variables to pass to commands
    pub env_vars: HashMap<String, String>,
}

impl std::fmt::Debug for WorkflowEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowEnv")
            .field("output_patterns", &self.output_patterns)
            .field("working_dir", &self.working_dir)
            .field("env_vars", &self.env_vars)
            .finish_non_exhaustive()
    }
}

impl WorkflowEnv {
    /// Create a new builder for WorkflowEnv
    pub fn builder() -> WorkflowEnvBuilder {
        WorkflowEnvBuilder::new()
    }
}

/// Builder for constructing WorkflowEnv
pub struct WorkflowEnvBuilder {
    claude_runner: Option<Arc<dyn ClaudeRunner>>,
    shell_runner: Option<Arc<dyn ShellRunner>>,
    output_patterns: Vec<OutputPattern>,
    working_dir: Option<PathBuf>,
    env_vars: HashMap<String, String>,
}

impl WorkflowEnvBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            claude_runner: None,
            shell_runner: None,
            output_patterns: Vec::new(),
            working_dir: None,
            env_vars: HashMap::new(),
        }
    }

    /// Set the Claude runner
    pub fn with_claude_runner(mut self, runner: Arc<dyn ClaudeRunner>) -> Self {
        self.claude_runner = Some(runner);
        self
    }

    /// Set the Claude runner from a ClaudeExecutor
    pub fn with_claude_executor(self, executor: Arc<dyn ClaudeExecutor>) -> Self {
        let adapter = Arc::new(ClaudeExecutorAdapter::new(executor));
        self.with_claude_runner(adapter)
    }

    /// Set the shell runner
    pub fn with_shell_runner(mut self, runner: Arc<dyn ShellRunner>) -> Self {
        self.shell_runner = Some(runner);
        self
    }

    /// Set output patterns for variable extraction
    pub fn with_output_patterns(mut self, patterns: Vec<OutputPattern>) -> Self {
        self.output_patterns = patterns;
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Set environment variables
    pub fn with_env_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.env_vars = vars;
        self
    }

    /// Add a single environment variable
    pub fn add_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    /// Build the WorkflowEnv
    ///
    /// # Panics
    ///
    /// Panics if required fields are not set
    pub fn build(self) -> WorkflowEnv {
        WorkflowEnv {
            claude_runner: self.claude_runner.expect("claude_runner must be set"),
            shell_runner: self
                .shell_runner
                .unwrap_or_else(|| Arc::new(DefaultShellRunner::new())),
            output_patterns: self.output_patterns,
            working_dir: self.working_dir.unwrap_or_else(|| PathBuf::from(".")),
            env_vars: self.env_vars,
        }
    }

    /// Build the WorkflowEnv, returning an error if required fields are missing
    pub fn try_build(self) -> Result<WorkflowEnv, String> {
        let claude_runner = self
            .claude_runner
            .ok_or_else(|| "claude_runner must be set".to_string())?;

        Ok(WorkflowEnv {
            claude_runner,
            shell_runner: self
                .shell_runner
                .unwrap_or_else(|| Arc::new(DefaultShellRunner::new())),
            output_patterns: self.output_patterns,
            working_dir: self.working_dir.unwrap_or_else(|| PathBuf::from(".")),
            env_vars: self.env_vars,
        })
    }
}

impl Default for WorkflowEnvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClaudeRunner {
        response: RunnerOutput,
    }

    impl MockClaudeRunner {
        fn new(response: RunnerOutput) -> Self {
            Self { response }
        }
    }

    #[async_trait]
    impl ClaudeRunner for MockClaudeRunner {
        async fn run(
            &self,
            _command: &str,
            _working_dir: &Path,
            _env_vars: HashMap<String, String>,
        ) -> anyhow::Result<RunnerOutput> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn test_runner_output_success() {
        let output = RunnerOutput::success("hello".to_string());
        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "hello");
    }

    #[test]
    fn test_runner_output_failure() {
        let output = RunnerOutput::failure("error".to_string(), 1);
        assert!(!output.success);
        assert_eq!(output.exit_code, Some(1));
        assert_eq!(output.stderr, "error");
    }

    #[test]
    fn test_workflow_env_builder() {
        let mock_runner = Arc::new(MockClaudeRunner::new(RunnerOutput::success(
            "test".to_string(),
        )));

        let env = WorkflowEnv::builder()
            .with_claude_runner(mock_runner)
            .with_working_dir(PathBuf::from("/tmp"))
            .add_env_var("KEY", "VALUE")
            .build();

        assert_eq!(env.working_dir, PathBuf::from("/tmp"));
        assert_eq!(env.env_vars.get("KEY"), Some(&"VALUE".to_string()));
    }

    #[test]
    fn test_workflow_env_builder_try_build_missing_runner() {
        let result = WorkflowEnvBuilder::new().try_build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("claude_runner"));
    }

    #[test]
    fn test_workflow_env_builder_try_build_success() {
        let mock_runner = Arc::new(MockClaudeRunner::new(RunnerOutput::success(
            "test".to_string(),
        )));

        let result = WorkflowEnvBuilder::new()
            .with_claude_runner(mock_runner)
            .try_build();

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_default_shell_runner_echo() {
        let runner = DefaultShellRunner::new();
        let result = runner
            .run(
                "echo 'hello world'",
                Path::new("/tmp"),
                HashMap::new(),
                None,
            )
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("hello world"));
    }

    #[tokio::test]
    async fn test_default_shell_runner_failure() {
        let runner = DefaultShellRunner::new();
        let result = runner
            .run("exit 1", Path::new("/tmp"), HashMap::new(), None)
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
        assert_eq!(output.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_default_shell_runner_with_env_vars() {
        let runner = DefaultShellRunner::new();
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_VAR".to_string(), "test_value".to_string());

        let result = runner
            .run("echo $TEST_VAR", Path::new("/tmp"), env_vars, None)
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.success);
        assert!(output.stdout.contains("test_value"));
    }

    #[tokio::test]
    async fn test_default_shell_runner_timeout() {
        let runner = DefaultShellRunner::new();
        let result = runner
            .run("sleep 10", Path::new("/tmp"), HashMap::new(), Some(1))
            .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.success);
        assert!(output.stderr.contains("timed out"));
    }
}
