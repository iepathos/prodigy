//! Shell command executor for MapReduce operations

use super::executor::{CommandError, CommandExecutor, CommandResult, ExecutionContext};
use crate::cook::workflow::{CommandType, WorkflowStep};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use tokio::process::Command;
use tokio::time::{timeout as tokio_timeout, Duration};

/// Executor for shell commands
pub struct ShellCommandExecutor;

impl ShellCommandExecutor {
    /// Create a new shell command executor
    pub fn new() -> Self {
        Self
    }

    /// Build a command with environment and working directory
    fn build_command(command_text: &str, context: &ExecutionContext) -> Command {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command_text]);
        cmd.current_dir(&context.worktree_path);

        // Set standard environment variables
        cmd.env("PRODIGY_WORKTREE", &context.worktree_name);
        cmd.env("PRODIGY_ITEM_ID", &context.item_id);
        cmd.env("PRODIGY_AUTOMATION", "true");

        // Add context environment variables
        for (key, value) in &context.environment {
            cmd.env(key, value);
        }

        cmd
    }

    /// Execute command with optional timeout
    async fn execute_with_timeout(
        mut cmd: Command,
        timeout_secs: Option<u64>,
    ) -> Result<std::process::Output, CommandError> {
        if let Some(secs) = timeout_secs {
            let duration = Duration::from_secs(secs);
            match tokio_timeout(duration, cmd.output()).await {
                Ok(result) => result.map_err(|e| CommandError::ExecutionFailed(e.to_string())),
                Err(_) => Err(CommandError::Timeout(format!(
                    "Shell command timed out after {} seconds",
                    secs
                ))),
            }
        } else {
            cmd.output()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))
        }
    }

    /// Build result from command output
    fn build_result(
        output: std::process::Output,
        start: Instant,
    ) -> CommandResult {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        CommandResult {
            output: Some(stdout.clone()),
            exit_code: output.status.code().unwrap_or(-1),
            variables: HashMap::new(),
            duration: start.elapsed(),
            success: output.status.success(),
            stderr,
        }
    }
}

#[async_trait]
impl CommandExecutor for ShellCommandExecutor {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &ExecutionContext,
    ) -> Result<CommandResult, CommandError> {
        let start = Instant::now();

        // Extract shell command
        let command = step.shell.as_ref().ok_or_else(|| {
            CommandError::InvalidConfiguration("No shell command in step".to_string())
        })?;

        // Build and execute command
        let cmd = Self::build_command(command, context);
        let output = Self::execute_with_timeout(cmd, step.timeout).await?;

        // Build and return result
        Ok(Self::build_result(output, start))
    }

    fn supports(&self, command_type: &CommandType) -> bool {
        matches!(command_type, CommandType::Shell(_) | CommandType::Test(_))
    }
}

impl Default for ShellCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}
