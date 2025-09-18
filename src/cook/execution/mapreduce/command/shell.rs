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

        // Create command
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);

        // Set working directory to the agent's worktree
        cmd.current_dir(&context.worktree_path);

        // Set environment variables
        cmd.env("PRODIGY_WORKTREE", &context.worktree_name);
        cmd.env("PRODIGY_ITEM_ID", &context.item_id);
        cmd.env("PRODIGY_AUTOMATION", "true");

        // Add context environment variables
        for (key, value) in &context.environment {
            cmd.env(key, value);
        }

        // Execute with optional timeout
        let output = if let Some(timeout_secs) = step.timeout {
            let duration = Duration::from_secs(timeout_secs);
            match tokio_timeout(duration, cmd.output()).await {
                Ok(result) => result.map_err(|e| CommandError::ExecutionFailed(e.to_string()))?,
                Err(_) => {
                    return Err(CommandError::Timeout(format!(
                        "Shell command timed out after {} seconds",
                        timeout_secs
                    )));
                }
            }
        } else {
            cmd.output()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(CommandResult {
            output: Some(stdout.clone()),
            exit_code,
            variables: HashMap::new(),
            duration: start.elapsed(),
            success: output.status.success(),
            stderr,
        })
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
