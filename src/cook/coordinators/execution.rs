//! Execution coordinator for managing command execution

use crate::cook::execution::{ClaudeExecutor, CommandExecutor};
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of attempts
    pub max_attempts: Option<u32>,
    /// Delay between attempts
    pub delay: Option<std::time::Duration>,
}

/// Command execution result
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution duration
    pub duration: std::time::Duration,
}

/// Trait for execution coordination
#[async_trait]
pub trait ExecutionCoordinator: Send + Sync {
    /// Execute a system command
    async fn execute_command(
        &self,
        command: &str,
        args: &[String],
        env: Option<HashMap<String, String>>,
        cwd: Option<&Path>,
    ) -> Result<ExecutionResult>;

    /// Execute a Claude command
    async fn execute_claude(
        &self,
        command: &str,
        args: &[String],
        env: Option<HashMap<String, String>>,
        cwd: Option<&Path>,
    ) -> Result<ExecutionResult>;

    /// Execute with retry logic
    async fn execute_with_retry(
        &self,
        command: &str,
        args: &[String],
        env: Option<HashMap<String, String>>,
        cwd: Option<&Path>,
        retry_config: &RetryConfig,
    ) -> Result<ExecutionResult>;

    /// Check if a command is available
    async fn check_command_available(&self, command: &str) -> Result<bool>;
}

/// Default implementation of execution coordinator
pub struct DefaultExecutionCoordinator {
    command_executor: Arc<dyn CommandExecutor>,
    claude_executor: Arc<dyn ClaudeExecutor>,
    #[allow(dead_code)]
    subprocess_manager: Arc<SubprocessManager>,
}

impl DefaultExecutionCoordinator {
    /// Create new execution coordinator
    pub fn new(
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        subprocess_manager: Arc<SubprocessManager>,
    ) -> Self {
        Self {
            command_executor,
            claude_executor,
            subprocess_manager,
        }
    }
}

#[async_trait]
impl ExecutionCoordinator for DefaultExecutionCoordinator {
    async fn execute_command(
        &self,
        command: &str,
        args: &[String],
        env: Option<HashMap<String, String>>,
        cwd: Option<&Path>,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

        let context = crate::cook::execution::ExecutionContext {
            env_vars: env.unwrap_or_default(),
            working_directory: cwd
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap()),
            capture_output: true,
            timeout_seconds: None,
            stdin: None,
        };

        let output = self
            .command_executor
            .execute(command, args, context)
            .await?;

        Ok(ExecutionResult {
            exit_code: output.exit_code.unwrap_or(0),
            stdout: output.stdout,
            stderr: output.stderr,
            duration: start.elapsed(),
        })
    }

    async fn execute_claude(
        &self,
        command: &str,
        args: &[String],
        env: Option<HashMap<String, String>>,
        cwd: Option<&Path>,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();

        // Convert args and build full command
        let mut full_command = format!("/{command}");
        for arg in args {
            full_command.push(' ');
            full_command.push_str(arg);
        }

        let working_dir = cwd
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap());
        let env_vars = env.unwrap_or_default();

        let output = self
            .claude_executor
            .execute_claude_command(&full_command, &working_dir, env_vars)
            .await?;

        Ok(ExecutionResult {
            exit_code: if output.success { 0 } else { 1 },
            stdout: output.stdout,
            stderr: output.stderr,
            duration: start.elapsed(),
        })
    }

    async fn execute_with_retry(
        &self,
        command: &str,
        args: &[String],
        env: Option<HashMap<String, String>>,
        cwd: Option<&Path>,
        retry_config: &RetryConfig,
    ) -> Result<ExecutionResult> {
        let mut attempts = 0;
        let max_attempts = retry_config.max_attempts.unwrap_or(3);

        loop {
            attempts += 1;

            match self.execute_command(command, args, env.clone(), cwd).await {
                Ok(result) if result.exit_code == 0 => return Ok(result),
                Ok(result) if attempts >= max_attempts => return Ok(result),
                Err(e) if attempts >= max_attempts => return Err(e),
                _ => {
                    // Wait before retry
                    let delay = retry_config
                        .delay
                        .unwrap_or(std::time::Duration::from_secs(1));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn check_command_available(&self, command: &str) -> Result<bool> {
        let context = crate::cook::execution::ExecutionContext::default();
        match self
            .command_executor
            .execute("which", &[command.to_string()], context)
            .await
        {
            Ok(output) => Ok(output.exit_code.unwrap_or(1) == 0),
            Err(_) => Ok(false),
        }
    }
}
