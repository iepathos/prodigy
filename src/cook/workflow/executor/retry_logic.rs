//! Retry logic for command execution
//!
//! This module contains retry handling for shell and test commands,
//! extracted from commands.rs to reduce its size and improve separation of concerns.

use crate::config::command::TestDebugConfig;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::checkpoint;
use crate::cook::workflow::NormalizedWorkflow;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use tempfile::NamedTempFile;

use super::{StepResult, WorkflowContext, WorkflowExecutor};

// ============================================================================
// Pure Helper Functions
// ============================================================================

/// Determine if output should be written to a temp file (pure function)
pub fn should_use_temp_file(stdout_len: usize, stderr_len: usize) -> bool {
    stdout_len + stderr_len > 10000
}

/// Format shell output for display or storage (pure function)
pub fn format_shell_output(stdout: &str, stderr: &str) -> String {
    format!("=== STDOUT ===\n{}\n\n=== STDERR ===\n{}", stdout, stderr)
}

/// Format smaller shell output for inline display (pure function)
pub fn format_inline_output(stdout: &str, stderr: &str) -> String {
    format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr)
}

/// Build shell-specific context variables (pure function)
pub fn build_shell_context_vars(
    attempt: u32,
    exit_code: Option<i32>,
    output: String,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("shell.attempt".to_string(), attempt.to_string());
    vars.insert(
        "shell.exit_code".to_string(),
        exit_code.unwrap_or(-1).to_string(),
    );
    vars.insert("shell.output".to_string(), output);
    vars
}

/// Check if max attempts have been reached (pure function)
pub fn has_reached_max_attempts(attempt: u32, max_attempts: u32) -> bool {
    attempt >= max_attempts
}

/// Determine if workflow should fail on max attempts (pure function)
pub fn should_fail_workflow_on_max_attempts(fail_workflow: bool) -> bool {
    fail_workflow
}

// ============================================================================
// I/O Helper Functions
// ============================================================================

/// Create a temp file with shell output
pub fn create_output_temp_file(stdout: &str, stderr: &str) -> Result<NamedTempFile> {
    let temp_file = NamedTempFile::new()?;
    let combined_output = format_shell_output(stdout, stderr);
    fs::write(temp_file.path(), &combined_output)?;
    Ok(temp_file)
}

// ============================================================================
// WorkflowExecutor Retry Methods
// ============================================================================

impl WorkflowExecutor {
    /// Execute a shell command with retry logic (for shell commands with on_failure)
    pub(crate) async fn execute_shell_with_retry(
        &self,
        command: &str,
        on_failure: Option<&TestDebugConfig>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(command);
        self.log_variable_resolutions(&resolutions);

        let mut attempt = 0;
        loop {
            attempt += 1;
            self.user_interaction.display_progress(&format!(
                "Running shell command (attempt {attempt}): {interpolated_cmd}"
            ));

            env_vars.insert("SHELL_ATTEMPT".to_string(), attempt.to_string());

            let shell_result = self
                .execute_shell_command(&interpolated_cmd, env, env_vars.clone(), timeout)
                .await?;

            if shell_result.success {
                self.user_interaction
                    .display_success(&format!("Shell command succeeded on attempt {attempt}"));
                return Ok(shell_result);
            }

            if let Some(debug_config) = on_failure {
                if has_reached_max_attempts(attempt, debug_config.max_attempts) {
                    self.user_interaction.display_error(&format!(
                        "Shell command failed after {} attempts",
                        debug_config.max_attempts
                    ));

                    if should_fail_workflow_on_max_attempts(debug_config.fail_workflow) {
                        return Err(anyhow!(
                            "Shell command failed after {} attempts and fail_workflow is true",
                            debug_config.max_attempts
                        ));
                    } else {
                        return Ok(shell_result);
                    }
                }

                let temp_file =
                    if should_use_temp_file(shell_result.stdout.len(), shell_result.stderr.len()) {
                        Some(create_output_temp_file(
                            &shell_result.stdout,
                            &shell_result.stderr,
                        )?)
                    } else {
                        None
                    };

                let output_path = temp_file
                    .as_ref()
                    .map(|f| f.path().to_string_lossy().to_string());

                let mut debug_cmd = debug_config.claude.clone();

                let output = if let Some(output_file) = output_path {
                    output_file
                } else {
                    format_inline_output(&shell_result.stdout, &shell_result.stderr)
                };

                let shell_vars = build_shell_context_vars(attempt, shell_result.exit_code, output);
                for (key, value) in shell_vars {
                    ctx.variables.insert(key, value);
                }

                let (interpolated_debug_cmd, debug_resolutions) =
                    ctx.interpolate_with_tracking(&debug_cmd);
                self.log_variable_resolutions(&debug_resolutions);
                debug_cmd = interpolated_debug_cmd;

                self.user_interaction.display_info(&format!(
                    "Shell command failed, running: {} (attempt {}/{})",
                    debug_cmd, attempt, debug_config.max_attempts
                ));

                let debug_result = self
                    .execute_claude_command(&debug_cmd, env, env_vars.clone())
                    .await?;

                if !debug_result.success {
                    self.user_interaction
                        .display_error("Debug command failed, but continuing with retry");
                }

                drop(temp_file);
            } else {
                return Ok(shell_result);
            }
        }
    }

    /// Execute a test command with retry logic
    pub(crate) async fn execute_test_command(
        &self,
        test_cmd: crate::config::command::TestCommand,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
        _workflow: Option<&NormalizedWorkflow>,
        _step_index: Option<usize>,
    ) -> Result<StepResult> {
        let (interpolated_test_cmd, resolutions) = ctx.interpolate_with_tracking(&test_cmd.command);
        self.log_variable_resolutions(&resolutions);

        let mut failure_history: Vec<String> = Vec::new();

        let mut attempt = 0;
        loop {
            attempt += 1;
            self.user_interaction.display_progress(&format!(
                "Running test command (attempt {attempt}): {interpolated_test_cmd}"
            ));

            env_vars.insert("TEST_ATTEMPT".to_string(), attempt.to_string());

            let test_result = self
                .execute_shell_command(&interpolated_test_cmd, env, env_vars.clone(), None)
                .await?;

            if test_result.success {
                self.user_interaction
                    .display_success(&format!("Tests passed on attempt {attempt}"));
                return Ok(test_result);
            }

            if let Some(debug_config) = &test_cmd.on_failure {
                failure_history.push(format!(
                    "Attempt {}: exit code {}",
                    attempt,
                    test_result.exit_code.unwrap_or(-1)
                ));

                if attempt >= debug_config.max_attempts {
                    self.user_interaction.display_error(&format!(
                        "Tests failed after {} attempts",
                        debug_config.max_attempts
                    ));

                    if debug_config.fail_workflow {
                        return Err(anyhow!(
                            "Test command failed after {} attempts and fail_workflow is true",
                            debug_config.max_attempts
                        ));
                    } else {
                        return Ok(test_result);
                    }
                }

                // Save checkpoint after test failure but before retry
                if let (Some(workflow), Some(step_index)) =
                    (&self.current_workflow, self.current_step_index)
                {
                    let retry_state = checkpoint::RetryState {
                        current_attempt: attempt as usize,
                        max_attempts: debug_config.max_attempts as usize,
                        failure_history: failure_history.clone(),
                        in_retry_loop: true,
                    };
                    self.save_retry_checkpoint(workflow, step_index, Some(retry_state), ctx)
                        .await;
                    tracing::info!(
                        "Saved checkpoint for test retry at attempt {}/{}",
                        attempt,
                        debug_config.max_attempts
                    );
                }

                let temp_file = if test_result.stdout.len() + test_result.stderr.len() > 10000 {
                    let temp_file = NamedTempFile::new()?;
                    let combined_output =
                        format_shell_output(&test_result.stdout, &test_result.stderr);
                    fs::write(temp_file.path(), &combined_output)?;
                    Some(temp_file)
                } else {
                    None
                };

                let output_path = temp_file
                    .as_ref()
                    .map(|f| f.path().to_string_lossy().to_string());

                let mut debug_cmd = debug_config.claude.clone();

                ctx.variables
                    .insert("test.attempt".to_string(), attempt.to_string());
                ctx.variables.insert(
                    "test.exit_code".to_string(),
                    test_result.exit_code.unwrap_or(-1).to_string(),
                );

                if let Some(output_file) = output_path {
                    ctx.variables.insert("test.output".to_string(), output_file);
                } else {
                    let combined_output =
                        format_inline_output(&test_result.stdout, &test_result.stderr);
                    ctx.variables
                        .insert("test.output".to_string(), combined_output);
                }

                let (interpolated_debug_cmd, debug_resolutions) =
                    ctx.interpolate_with_tracking(&debug_cmd);
                self.log_variable_resolutions(&debug_resolutions);
                debug_cmd = interpolated_debug_cmd;

                self.user_interaction.display_info(&format!(
                    "Tests failed, running: {} (attempt {}/{})",
                    debug_cmd, attempt, debug_config.max_attempts
                ));

                let debug_result = self
                    .execute_claude_command(&debug_cmd, env, env_vars.clone())
                    .await?;

                if !debug_result.success {
                    self.user_interaction
                        .display_error("Debug command failed, but continuing with retry");
                }

                drop(temp_file);
            } else {
                return Ok(test_result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_temp_file_small_output() {
        assert!(!should_use_temp_file(100, 100));
    }

    #[test]
    fn test_should_use_temp_file_large_output() {
        assert!(should_use_temp_file(8000, 3000));
    }

    #[test]
    fn test_format_shell_output() {
        let output = format_shell_output("stdout content", "stderr content");
        assert!(output.contains("=== STDOUT ==="));
        assert!(output.contains("stdout content"));
        assert!(output.contains("=== STDERR ==="));
        assert!(output.contains("stderr content"));
    }

    #[test]
    fn test_format_inline_output() {
        let output = format_inline_output("stdout", "stderr");
        assert!(output.contains("STDOUT:"));
        assert!(output.contains("STDERR:"));
    }

    #[test]
    fn test_build_shell_context_vars() {
        let vars = build_shell_context_vars(2, Some(1), "output".to_string());
        assert_eq!(vars.get("shell.attempt"), Some(&"2".to_string()));
        assert_eq!(vars.get("shell.exit_code"), Some(&"1".to_string()));
        assert_eq!(vars.get("shell.output"), Some(&"output".to_string()));
    }

    #[test]
    fn test_has_reached_max_attempts() {
        assert!(!has_reached_max_attempts(1, 3));
        assert!(!has_reached_max_attempts(2, 3));
        assert!(has_reached_max_attempts(3, 3));
        assert!(has_reached_max_attempts(4, 3));
    }

    #[test]
    fn test_should_fail_workflow_on_max_attempts() {
        assert!(should_fail_workflow_on_max_attempts(true));
        assert!(!should_fail_workflow_on_max_attempts(false));
    }
}
