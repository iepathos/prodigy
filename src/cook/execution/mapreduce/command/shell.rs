//! Shell command executor for MapReduce operations

use super::executor::{CommandError, CommandExecutor, CommandResult, ExecutionContext};
use crate::cook::workflow::{CommandType, WorkflowStep};
use crate::subprocess::{ProcessCommandBuilder, ProcessRunner};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Executor for shell commands
pub struct ShellCommandExecutor {
    /// Subprocess runner for executing commands
    runner: Arc<dyn ProcessRunner>,
}

impl ShellCommandExecutor {
    /// Create a new shell command executor with default runner
    pub fn new() -> Self {
        Self {
            runner: Arc::new(crate::subprocess::runner::TokioProcessRunner),
        }
    }

    /// Create a new shell command executor with custom runner
    pub fn with_runner(runner: Arc<dyn ProcessRunner>) -> Self {
        Self { runner }
    }

    /// Build a command with environment and working directory
    fn build_command(
        command_text: &str,
        context: &ExecutionContext,
    ) -> crate::subprocess::ProcessCommand {
        let mut builder = ProcessCommandBuilder::new("sh")
            .args(["-c", command_text])
            .current_dir(&context.worktree_path);

        // Set standard environment variables
        builder = builder
            .env("PRODIGY_WORKTREE", &context.worktree_name)
            .env("PRODIGY_ITEM_ID", &context.item_id)
            .env("PRODIGY_AUTOMATION", "true");

        // Note: We do NOT add context.environment variables here anymore
        // to avoid passing massive map results as environment variables.
        // These variables are available through interpolation instead.

        builder.build()
    }

    /// Build result from command output
    fn build_result(output: crate::subprocess::ProcessOutput, start: Instant) -> CommandResult {
        CommandResult {
            output: Some(output.stdout.clone()),
            exit_code: output.status.code().unwrap_or(-1),
            variables: HashMap::new(),
            duration: start.elapsed(),
            success: output.status.success(),
            stderr: output.stderr,
            json_log_location: None, // Shell commands don't produce JSON logs
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

        // Build command
        let process_command = Self::build_command(command, context);

        // Execute with optional timeout
        let output = if let Some(timeout_secs) = step.timeout {
            let timeout_duration = std::time::Duration::from_secs(timeout_secs);
            let mut cmd_with_timeout = process_command.clone();
            cmd_with_timeout.timeout = Some(timeout_duration);

            self.runner
                .run(cmd_with_timeout)
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?
        } else {
            self.runner
                .run(process_command)
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?
        };

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
