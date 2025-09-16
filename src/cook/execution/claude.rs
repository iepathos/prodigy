//! Claude CLI execution implementation

use super::{CommandExecutor, CommandRunner, ExecutionContext, ExecutionResult};
use crate::testing::config::TestConfiguration;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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
    test_config: Option<Arc<TestConfiguration>>,
}

impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    /// Create a new Claude executor
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            test_config: None,
        }
    }

    /// Create a new Claude executor with test configuration
    pub fn with_test_config(runner: R, test_config: Arc<TestConfiguration>) -> Self {
        Self {
            runner,
            test_config: Some(test_config),
        }
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
        let test_mode = self.test_config.as_ref().map_or(false, |c| c.test_mode);
        if test_mode {
            return self.handle_test_mode_execution(command).await;
        }

        let mut context = ExecutionContext::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            context.working_directory = project_path.to_path_buf();
            context.env_vars = env_vars.clone();
        }

        // Check for timeout configuration passed via environment variable
        if let Some(timeout_str) = env_vars.get("PRODIGY_COMMAND_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                context.timeout_seconds = Some(timeout_secs);
                tracing::debug!("Claude command timeout set to {} seconds", timeout_secs);
            }
        }

        // Claude requires some input on stdin to work properly
        context.stdin = Some("".to_string());

        let args = vec![
            "--print".to_string(),
            "--dangerously-skip-permissions".to_string(),
            command.to_string(),
        ];
        tracing::debug!("Executing claude command with args: {:?}", args);

        // Print progress indicator for long-running Claude commands
        println!("🤖 Claude is processing: {}", command);
        println!("⏳ This may take a few minutes...");

        // Start a progress indicator task
        let progress_handle = tokio::spawn(async {
            let mut count = 0;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                count += 10;
                println!("   ⏱️  Still working... ({} seconds elapsed)", count);
            }
        });

        let result = self
            .runner
            .run_with_context("claude", &args, &context)
            .await;

        // Stop the progress indicator
        progress_handle.abort();

        if let Err(ref e) = result {
            tracing::error!("Claude command failed: {:?}", e);
            println!("❌ Claude command failed: {:?}", e);
        } else if result.as_ref().map(|r| r.success).unwrap_or(false) {
            println!("✅ Claude command completed successfully");
        }

        result
    }

    async fn check_claude_cli(&self) -> Result<bool> {
        // Always return true in test mode
        let test_mode = self.test_config.as_ref().map_or(false, |c| c.test_mode);
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

        // Check if we should simulate no changes
        if let Some(config) = &self.test_config {
            let command_name = command.trim_start_matches('/');
            // Extract just the command name, ignoring arguments
            let command_name = command_name
                .split_whitespace()
                .next()
                .unwrap_or(command_name);
            if config
                .no_changes_commands
                .iter()
                .any(|cmd| cmd.trim() == command_name)
            {
                println!("[TEST MODE] Simulating no changes for: {command_name}");
                // Return success but the orchestrator will detect no commits were made
                return Ok(ExecutionResult {
                    success: true,
                    stdout: format!("Test mode - no changes for {command}"),
                    stderr: String::new(),
                    exit_code: Some(0),
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
        let env_vars = HashMap::new();

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "Command executed");
    }
}
