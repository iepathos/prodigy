//! Claude CLI execution implementation

use super::{CommandExecutor, CommandRunner, ExecutionContext, ExecutionResult};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;

/// Trait for executing Claude commands
#[async_trait]
pub trait ClaudeExecutor: Send + Sync {
    /// Execute a Claude command
    async fn execute_claude_command(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult>;

    /// Check if Claude CLI is available
    async fn check_claude_cli(&self) -> Result<bool>;

    /// Get Claude CLI version
    async fn get_claude_version(&self) -> Result<String>;
}

/// Implementation of Claude executor
pub struct ClaudeExecutorImpl<R: CommandRunner> {
    runner: R,
}

impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    /// Create a new Claude executor
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> ClaudeExecutor for ClaudeExecutorImpl<R> {
    async fn execute_claude_command(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        // Handle test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return self.handle_test_mode_execution(command).await;
        }

        let mut context = ExecutionContext::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            context.working_directory = project_path.to_path_buf();
            context.env_vars = env_vars;
        }

        // Set timeout for Claude commands (10 minutes by default)
        context.timeout_seconds = Some(600);

        self.runner
            .run_with_context("claude", &[command.to_string()], &context)
            .await
    }

    async fn check_claude_cli(&self) -> Result<bool> {
        // Always return true in test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return Ok(true);
        }

        match self
            .runner
            .run_command("claude", &["--version".to_string()])
            .await
        {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }

    async fn get_claude_version(&self) -> Result<String> {
        let output = self
            .runner
            .run_command("claude", &["--version".to_string()])
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            anyhow::bail!("Failed to get Claude version")
        }
    }
}

impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    /// Handle test mode execution
    async fn handle_test_mode_execution(&self, command: &str) -> Result<ExecutionResult> {
        println!("[TEST MODE] Would execute Claude command: {command}");

        // Check if we should simulate no changes/failure
        if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
            let command_name = command.trim_start_matches('/');
            if no_changes_cmds
                .split(',')
                .any(|cmd| cmd.trim() == command_name)
            {
                println!("[TEST MODE] Simulating no changes for: {command_name}");
                return Ok(ExecutionResult {
                    success: false,
                    stdout: format!("Test mode - no changes for {command}"),
                    stderr: String::new(),
                    exit_code: Some(1),
                });
            }
        }

        Ok(ExecutionResult {
            success: true,
            stdout: format!("Test mode execution of {command}"),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> CommandExecutor for ClaudeExecutorImpl<R> {
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // For Claude commands, use the Claude-specific method
        if command == "claude" && args.len() == 1 {
            self.execute_claude_command(&args[0], &context.working_directory, context.env_vars)
                .await
        } else {
            // Fallback to regular command execution
            self.runner.run_with_context(command, args, &context).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::runner::tests::MockCommandRunner;

    #[tokio::test]
    async fn test_claude_executor_check() {
        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "claude version 1.0.0".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let available = executor.check_claude_cli().await.unwrap();
        assert!(available);
    }

    #[tokio::test]
    async fn test_claude_executor_version() {
        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "claude version 1.0.0\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let version = executor.get_claude_version().await.unwrap();
        assert_eq!(version, "claude version 1.0.0");
    }

    #[tokio::test]
    async fn test_claude_command_execution() {
        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "Command executed".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let mut env_vars = HashMap::new();
        env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "Command executed");
    }
}
