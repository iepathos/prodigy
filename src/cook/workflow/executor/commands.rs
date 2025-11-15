//! Command execution module for workflow executor
//!
//! This module contains I/O-bound command execution logic extracted from WorkflowExecutor.
//! It provides clean separation between pure logic (in pure.rs) and I/O operations.
//!
//! ## Architecture
//!
//! - **Claude Commands**: Delegated to ClaudeExecutor trait
//! - **Shell Commands**: Direct tokio::process execution
//! - **Test Commands**: Retry logic with validation
//! - **Handler Commands**: Registry-based modular handlers
//! - **Goal-Seek Commands**: Delegated to goal_seek module (already well-separated)
//! - **Foreach Commands**: Delegated to foreach module (already well-separated)
//!
//! ## Design Principles
//!
//! 1. **I/O at Boundaries**: All async I/O operations contained here
//! 2. **Clear Interfaces**: Simple request/response data structures
//! 3. **Minimal Dependencies**: Only essential dependencies on WorkflowExecutor state
//! 4. **Testability**: Execution logic can be tested with mocks

use crate::commands::{AttributeValue, ExecutionContext};
use crate::cook::execution::{ClaudeExecutor, ExecutionResult};
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::workflow::checkpoint;
use crate::cook::workflow::on_failure::OnFailureConfig;
use crate::cook::workflow::NormalizedWorkflow;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::{CommandType, StepResult, WorkflowContext, WorkflowExecutor, WorkflowStep};

// ============================================================================
// Standalone Command Execution Functions
// ============================================================================

/// Execute a Claude CLI command
///
/// This is a thin wrapper around ClaudeExecutor that converts the result
/// to our internal StepResult format.
pub async fn execute_claude_command(
    claude_executor: &Arc<dyn ClaudeExecutor>,
    command: &str,
    working_dir: &Path,
    env_vars: HashMap<String, String>,
) -> Result<StepResult> {
    let result = claude_executor
        .execute_claude_command(command, working_dir, env_vars)
        .await
        .with_context(|| {
            format!(
                "Claude command execution failed for command: '{}' in directory: {}",
                command,
                working_dir.display()
            )
        })?;

    Ok(convert_execution_result(result))
}

/// Convert ExecutionResult to StepResult
fn convert_execution_result(result: ExecutionResult) -> StepResult {
    let json_log_location = result.json_log_location().map(|s| s.to_string());
    StepResult {
        success: result.success,
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
        json_log_location,
    }
}

// ============================================================================
// Shell Command Execution
// ============================================================================

/// Execute a shell command with optional timeout
///
/// Uses tokio::process for async shell execution. Commands are run via
/// `sh -c` on Unix-like systems.
pub async fn execute_shell_command(
    command: &str,
    working_dir: &Path,
    env_vars: HashMap<String, String>,
    timeout: Option<u64>,
) -> Result<StepResult> {
    use tokio::process::Command;
    use tokio::time::{timeout as tokio_timeout, Duration};

    // Log shell command execution details
    tracing::info!("Executing shell command: {}", command);
    tracing::info!("Working directory: {}", working_dir.display());
    if !env_vars.is_empty() {
        tracing::debug!("  With {} environment variables set", env_vars.len());
    }

    // Create command (Unix-like systems only)
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);

    // Set working directory
    cmd.current_dir(working_dir);

    // Set environment variables
    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    // Execute with optional timeout
    let output = if let Some(timeout_secs) = timeout {
        let duration = Duration::from_secs(timeout_secs);
        match tokio_timeout(duration, cmd.output()).await {
            Ok(result) => result?,
            Err(_) => {
                return Ok(StepResult {
                    success: false,
                    exit_code: Some(-1),
                    stdout: String::new(),
                    stderr: format!("Command timed out after {timeout_secs} seconds"),
                    json_log_location: None,
                });
            }
        }
    } else {
        cmd.output().await?
    };

    Ok(StepResult {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        json_log_location: None,
    })
}

// ============================================================================
// Goal-Seek Command Execution
// ============================================================================

/// Execute a goal-seek command (delegates to goal_seek module)
///
/// Goal-seeking is already well-separated into its own module with
/// GoalSeekEngine. This function provides the bridge between workflow
/// execution and the goal-seek subsystem.
pub async fn execute_goal_seek_command(
    goal_seek_config: crate::cook::goal_seek::GoalSeekConfig,
) -> Result<StepResult> {
    use crate::cook::goal_seek::{shell_executor::ShellCommandExecutor, GoalSeekEngine};

    // Create shell command executor for goal-seeking
    let executor = Box::new(ShellCommandExecutor::new());

    // Create goal-seek engine
    let mut engine = GoalSeekEngine::new(executor);

    // Execute goal-seeking
    let result = engine.seek(goal_seek_config.clone()).await?;

    // Convert result to StepResult
    match result {
        crate::cook::goal_seek::GoalSeekResult::Success {
            attempts,
            final_score,
            execution_time: _,
        } => Ok(StepResult {
            success: true,
            stdout: format!(
                "Goal '{}' achieved in {} attempts with score {}%",
                goal_seek_config.goal, attempts, final_score
            ),
            stderr: String::new(),
            exit_code: Some(0),
            json_log_location: None,
        }),
        crate::cook::goal_seek::GoalSeekResult::MaxAttemptsReached {
            attempts,
            best_score,
            last_output: _,
        } => {
            if goal_seek_config.fail_on_incomplete.unwrap_or(false) {
                Err(anyhow::anyhow!(
                    "Goal '{}' not achieved after {} attempts. Best score: {}%",
                    goal_seek_config.goal,
                    attempts,
                    best_score
                ))
            } else {
                Ok(StepResult {
                    success: false,
                    stdout: format!(
                        "Goal '{}' not achieved after {} attempts. Best score: {}%",
                        goal_seek_config.goal, attempts, best_score
                    ),
                    stderr: String::new(),
                    exit_code: Some(1),
                    json_log_location: None,
                })
            }
        }
        crate::cook::goal_seek::GoalSeekResult::Timeout {
            attempts,
            best_score,
            elapsed,
        } => Err(anyhow::anyhow!(
            "Goal '{}' timed out after {} attempts and {:?}. Best score: {}%",
            goal_seek_config.goal,
            attempts,
            elapsed,
            best_score
        )),
        crate::cook::goal_seek::GoalSeekResult::Converged {
            attempts,
            final_score,
            reason,
        } => {
            if goal_seek_config.fail_on_incomplete.unwrap_or(false)
                && final_score < goal_seek_config.threshold
            {
                Err(anyhow::anyhow!(
                    "Goal '{}' converged after {} attempts but didn't reach threshold. Score: {}%, Reason: {}",
                    goal_seek_config.goal, attempts, final_score, reason
                ))
            } else {
                Ok(StepResult {
                    success: final_score >= goal_seek_config.threshold,
                    stdout: format!(
                        "Goal '{}' converged after {} attempts. Score: {}%, Reason: {}",
                        goal_seek_config.goal, attempts, final_score, reason
                    ),
                    stderr: String::new(),
                    exit_code: Some(if final_score >= goal_seek_config.threshold {
                        0
                    } else {
                        1
                    }),
                    json_log_location: None,
                })
            }
        }
        crate::cook::goal_seek::GoalSeekResult::Failed { attempts, error } => Err(anyhow::anyhow!(
            "Goal '{}' failed after {} attempts: {}",
            goal_seek_config.goal,
            attempts,
            error
        )),
    }
}

// ============================================================================
// Foreach Command Execution
// ============================================================================

/// Execute a foreach command (delegates to foreach module)
///
/// Foreach execution is already well-separated into its own module.
/// This function provides the bridge between workflow execution and
/// the foreach subsystem for parallel iteration.
pub async fn execute_foreach_command(
    foreach_config: crate::config::command::ForeachConfig,
) -> Result<StepResult> {
    use crate::cook::execution::foreach::execute_foreach;

    let result = execute_foreach(&foreach_config).await?;

    // Return aggregated results
    Ok(StepResult {
        success: result.failed_items == 0,
        stdout: format!(
            "Foreach completed: {} total, {} successful, {} failed",
            result.total_items, result.successful_items, result.failed_items
        ),
        stderr: if result.failed_items > 0 {
            format!("{} items failed", result.failed_items)
        } else {
            String::new()
        },
        exit_code: Some(if result.failed_items == 0 { 0 } else { 1 }),
        json_log_location: None,
    })
}

// ============================================================================
// Write File Command Execution
// ============================================================================

/// Execute a write_file command with validation and formatting
///
/// Writes content to a file with optional validation, formatting, directory creation,
/// and permission setting. Provides clean logging without exposing file content.
pub async fn execute_write_file_command(
    config: &crate::config::command::WriteFileConfig,
    working_dir: &Path,
) -> Result<StepResult> {
    use std::fs;

    // Validate path (reject parent directory traversal)
    if config.path.contains("..") {
        return Err(anyhow!(
            "Invalid path: parent directory traversal not allowed"
        ));
    }

    // Resolve file path relative to working directory
    let file_path = working_dir.join(&config.path);

    // Create parent directories if requested
    if config.create_dirs {
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create parent directories for {}",
                    file_path.display()
                )
            })?;
        }
    }

    // Process content based on format
    let content_to_write = match config.format {
        crate::config::command::WriteFileFormat::Text => {
            // Plain text - no processing
            config.content.clone()
        }
        crate::config::command::WriteFileFormat::Json => {
            // Validate and pretty-print JSON
            let value: serde_json::Value =
                serde_json::from_str(&config.content).context("Invalid JSON content")?;
            serde_json::to_string_pretty(&value).context("Failed to format JSON")?
        }
        crate::config::command::WriteFileFormat::Yaml => {
            // Validate and format YAML
            let value: serde_yaml::Value =
                serde_yaml::from_str(&config.content).context("Invalid YAML content")?;
            serde_yaml::to_string(&value).context("Failed to format YAML")?
        }
    };

    // Write content to file
    fs::write(&file_path, &content_to_write)
        .with_context(|| format!("Failed to write file: {}", file_path.display()))?;

    // Set file permissions on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = u32::from_str_radix(&config.mode, 8)
            .with_context(|| format!("Invalid file mode: {}", config.mode))?;
        let permissions = fs::Permissions::from_mode(mode);
        fs::set_permissions(&file_path, permissions)
            .with_context(|| format!("Failed to set file permissions: {}", file_path.display()))?;
    }

    // Calculate content size for logging
    let content_size = content_to_write.len();

    // Log file write without exposing content
    let format_str = match config.format {
        crate::config::command::WriteFileFormat::Text => "Text",
        crate::config::command::WriteFileFormat::Json => "Json",
        crate::config::command::WriteFileFormat::Yaml => "Yaml",
    };

    Ok(StepResult {
        success: true,
        exit_code: Some(0),
        stdout: format!(
            "Wrote {} bytes to {} (format: {})",
            content_size, config.path, format_str
        ),
        stderr: String::new(),
        json_log_location: None,
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Build command description for logging/dry-run
#[allow(dead_code)] // Will be used in future refactoring phases
pub fn format_command_description(command_type: &CommandType) -> String {
    match command_type {
        CommandType::Claude(cmd) | CommandType::Legacy(cmd) => {
            format!("claude: {}", cmd)
        }
        CommandType::Shell(cmd) => format!("shell: {}", cmd),
        CommandType::Test(cmd) => format!("test: {}", cmd.command),
        CommandType::Handler { handler_name, .. } => {
            format!("handler: {}", handler_name)
        }
        CommandType::GoalSeek(cfg) => format!("goal_seek: {}", cfg.goal),
        CommandType::Foreach(cfg) => format!("foreach: {:?}", cfg.input),
        CommandType::WriteFile(cfg) => format!("write_file: {}", cfg.path),
    }
}

// ============================================================================
// WorkflowExecutor Command Execution Methods
// ============================================================================

impl WorkflowExecutor {
    /// Handle dry-run mode for a command
    async fn handle_dry_run_mode(
        &mut self,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        let command_desc = match command_type {
            CommandType::Claude(cmd) | CommandType::Legacy(cmd) => {
                format!("claude: {}", cmd)
            }
            CommandType::Shell(cmd) => format!("shell: {}", cmd),
            CommandType::Test(cmd) => format!("test: {}", cmd.command),
            CommandType::Handler { handler_name, .. } => {
                format!("handler: {}", handler_name)
            }
            CommandType::GoalSeek(cfg) => format!("goal_seek: {}", cfg.goal),
            CommandType::Foreach(cfg) => format!("foreach: {:?}", cfg.input),
            CommandType::WriteFile(cfg) => format!("write_file: {}", cfg.path),
        };

        println!("[DRY RUN] Would execute: {}", command_desc);
        self.dry_run_commands.push(command_desc.clone());

        // Track potential failure handlers
        if let Some(on_failure) = &step.on_failure {
            let handler_desc = match on_failure {
                OnFailureConfig::SingleCommand(cmd) => format!("on_failure: {}", cmd),
                OnFailureConfig::MultipleCommands(cmds) => {
                    format!("on_failure: {} commands", cmds.len())
                }
                OnFailureConfig::Advanced { claude, shell, .. } => {
                    if let Some(claude) = claude {
                        format!("on_failure: claude {}", claude)
                    } else if let Some(shell) = shell {
                        format!("on_failure: shell {}", shell)
                    } else {
                        "on_failure: unknown".to_string()
                    }
                }
                OnFailureConfig::Detailed(config) => {
                    format!("on_failure: {} handler commands", config.commands.len())
                }
                _ => "on_failure: configured".to_string(),
            };
            self.dry_run_potential_handlers.push(handler_desc);
        }

        // Handle commit_required in dry-run mode
        if step.commit_required {
            println!(
                "[DRY RUN] commit_required - assuming commit created by: {}",
                command_desc
            );
            self.assumed_commits.push(command_desc.clone());
        }

        // Simulate validation in dry-run mode since we're returning early
        // and the normal validation handling after command execution won't be reached
        if let Some(validation_config) = &step.validate {
            self.handle_validation(validation_config, env, ctx).await?;
        }

        // Simulate step validation in dry-run mode
        if let Some(step_validation) = &step.step_validate {
            if !step.skip_validation {
                let _validation_result = self
                    .handle_step_validation(step_validation, env, ctx, step)
                    .await?;
            }
        }

        // Return success result for dry-run
        Ok(StepResult {
            success: true,
            stdout: format!("[dry-run] {}", command_desc),
            stderr: String::new(),
            exit_code: Some(0),
            json_log_location: None,
        })
    }

    /// Main command dispatcher that routes to specific command handlers
    pub(super) async fn execute_command_by_type(
        &mut self,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        // Handle dry-run mode
        if self.dry_run {
            return self.handle_dry_run_mode(command_type, step, env, ctx).await;
        }

        // Add timeout to environment variables if configured for the step
        if let Some(timeout_secs) = step.timeout {
            env_vars.insert(
                "PRODIGY_COMMAND_TIMEOUT".to_string(),
                timeout_secs.to_string(),
            );
        }

        match command_type.clone() {
            CommandType::Claude(cmd) => {
                let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_claude_command(&interpolated_cmd, env, env_vars)
                    .await
            }
            CommandType::Shell(cmd) => {
                let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_shell_for_step(&interpolated_cmd, step, env, ctx, env_vars)
                    .await
            }
            CommandType::Test(test_cmd) => {
                self.execute_test_command(test_cmd, env, ctx, env_vars, None, None)
                    .await
            }
            CommandType::Legacy(cmd) => {
                let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(&cmd);
                self.log_variable_resolutions(&resolutions);
                self.execute_claude_command(&interpolated_cmd, env, env_vars)
                    .await
            }
            CommandType::Handler {
                handler_name,
                attributes,
            } => {
                self.execute_handler_command(handler_name, attributes, env, ctx, env_vars)
                    .await
            }
            CommandType::GoalSeek(goal_seek_config) => {
                self.execute_goal_seek_command(goal_seek_config, env, ctx, &env_vars)
                    .await
            }
            CommandType::Foreach(foreach_config) => {
                self.execute_foreach_command(foreach_config, env, ctx, &env_vars)
                    .await
            }
            CommandType::WriteFile(mut config) => {
                // Interpolate path and content
                let (interpolated_path, path_resolutions) =
                    ctx.interpolate_with_tracking(&config.path);
                let (interpolated_content, content_resolutions) =
                    ctx.interpolate_with_tracking(&config.content);

                // Log all variable resolutions
                self.log_variable_resolutions(&path_resolutions);
                self.log_variable_resolutions(&content_resolutions);

                // Update config with interpolated values
                config.path = interpolated_path;
                config.content = interpolated_content;

                // Execute write_file command
                execute_write_file_command(&config, &env.working_dir).await
            }
        }
    }

    /// Execute a Claude command (instance method wrapper)
    pub(crate) async fn execute_claude_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        execute_claude_command(&self.claude_executor, command, &env.working_dir, env_vars).await
    }

    /// Execute a shell command (instance method)
    pub(crate) async fn execute_shell_command(
        &self,
        command: &str,
        env: &ExecutionEnvironment,
        env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        execute_shell_command(command, &env.working_dir, env_vars, timeout).await
    }

    /// Execute shell command for a step with appropriate retry logic
    async fn execute_shell_for_step(
        &self,
        interpolated_cmd: &str,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        // Check if this shell command has test-style retry logic
        // For backward compatibility with converted test commands
        if let Some(test_cmd) = &step.test {
            if test_cmd.on_failure.is_some() {
                return self
                    .execute_shell_with_retry(
                        interpolated_cmd,
                        test_cmd.on_failure.as_ref(),
                        env,
                        ctx,
                        env_vars,
                        step.timeout,
                    )
                    .await;
            }
        }

        // Regular shell command without retry logic
        self.execute_shell_command(interpolated_cmd, env, env_vars, step.timeout)
            .await
    }

    // Pure helper functions for shell retry output management

    /// Determine if output should be written to a temp file (pure function)
    fn should_use_temp_file(stdout_len: usize, stderr_len: usize) -> bool {
        stdout_len + stderr_len > 10000
    }

    /// Format shell output for display or storage (pure function)
    fn format_shell_output(stdout: &str, stderr: &str) -> String {
        format!("=== STDOUT ===\n{}\n\n=== STDERR ===\n{}", stdout, stderr)
    }

    /// Format smaller shell output for inline display (pure function)
    fn format_inline_output(stdout: &str, stderr: &str) -> String {
        format!("STDOUT:\n{}\n\nSTDERR:\n{}", stdout, stderr)
    }

    /// Create a temp file with shell output (returns Result for I/O operation)
    fn create_output_temp_file(stdout: &str, stderr: &str) -> Result<tempfile::NamedTempFile> {
        use std::fs;

        let temp_file = tempfile::NamedTempFile::new()?;
        let combined_output = Self::format_shell_output(stdout, stderr);
        fs::write(temp_file.path(), &combined_output)?;
        Ok(temp_file)
    }

    // Pure helper functions for shell retry context management

    /// Build shell-specific context variables (pure function)
    fn build_shell_context_vars(
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

    // Pure helper functions for shell retry logic

    /// Check if max attempts have been reached (pure function)
    fn has_reached_max_attempts(attempt: u32, max_attempts: u32) -> bool {
        attempt >= max_attempts
    }

    /// Determine if workflow should fail on max attempts (pure function)
    fn should_fail_workflow_on_max_attempts(fail_workflow: bool) -> Result<(), String> {
        if fail_workflow {
            Err("fail_workflow is true".to_string())
        } else {
            Ok(())
        }
    }

    /// Execute a shell command with retry logic (for shell commands with on_failure)
    async fn execute_shell_with_retry(
        &self,
        command: &str,
        on_failure: Option<&crate::config::command::TestDebugConfig>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
        timeout: Option<u64>,
    ) -> Result<StepResult> {
        let (interpolated_cmd, resolutions) = ctx.interpolate_with_tracking(command);
        self.log_variable_resolutions(&resolutions);

        // Execute the shell command with retry logic
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.user_interaction.display_progress(&format!(
                "Running shell command (attempt {attempt}): {interpolated_cmd}"
            ));

            // Add attempt number to environment
            env_vars.insert("SHELL_ATTEMPT".to_string(), attempt.to_string());

            // Execute the shell command
            let shell_result = self
                .execute_shell_command(&interpolated_cmd, env, env_vars.clone(), timeout)
                .await?;

            // Check if command succeeded
            if shell_result.success {
                self.user_interaction
                    .display_success(&format!("Shell command succeeded on attempt {attempt}"));
                return Ok(shell_result);
            }

            // Command failed - check if we should retry
            if let Some(debug_config) = on_failure {
                // Check if max attempts reached (using pure helper)
                if Self::has_reached_max_attempts(attempt, debug_config.max_attempts) {
                    self.user_interaction.display_error(&format!(
                        "Shell command failed after {} attempts",
                        debug_config.max_attempts
                    ));

                    // Determine if workflow should fail (using pure helper)
                    if Self::should_fail_workflow_on_max_attempts(debug_config.fail_workflow)
                        .is_err()
                    {
                        return Err(anyhow!(
                            "Shell command failed after {} attempts and fail_workflow is true",
                            debug_config.max_attempts
                        ));
                    } else {
                        // Return the last result
                        return Ok(shell_result);
                    }
                }

                // Save shell output to a temp file if it's too large (using pure helper)
                let temp_file = if Self::should_use_temp_file(
                    shell_result.stdout.len(),
                    shell_result.stderr.len(),
                ) {
                    Some(Self::create_output_temp_file(
                        &shell_result.stdout,
                        &shell_result.stderr,
                    )?)
                } else {
                    None
                };

                let output_path = temp_file
                    .as_ref()
                    .map(|f| f.path().to_string_lossy().to_string());

                // Prepare the debug command with variables
                let mut debug_cmd = debug_config.claude.clone();

                // Determine output value based on size (using pure helpers)
                let output = if let Some(output_file) = output_path {
                    output_file
                } else {
                    Self::format_inline_output(&shell_result.stdout, &shell_result.stderr)
                };

                // Build and add shell-specific variables to context (using pure helper)
                let shell_vars =
                    Self::build_shell_context_vars(attempt, shell_result.exit_code, output);
                for (key, value) in shell_vars {
                    ctx.variables.insert(key, value);
                }

                // Interpolate the debug command
                let (interpolated_debug_cmd, debug_resolutions) =
                    ctx.interpolate_with_tracking(&debug_cmd);
                self.log_variable_resolutions(&debug_resolutions);
                debug_cmd = interpolated_debug_cmd;

                // Log the actual command being run
                self.user_interaction.display_info(&format!(
                    "Shell command failed, running: {} (attempt {}/{})",
                    debug_cmd, attempt, debug_config.max_attempts
                ));

                // Execute the debug command
                let debug_result = self
                    .execute_claude_command(&debug_cmd, env, env_vars.clone())
                    .await?;

                if !debug_result.success {
                    self.user_interaction
                        .display_error("Debug command failed, but continuing with retry");
                }

                // Clean up temp file
                drop(temp_file);

                // Continue to next attempt
            } else {
                // No on_failure configuration, return the failed result
                return Ok(shell_result);
            }
        }
    }

    /// Execute a test command with retry logic
    async fn execute_test_command(
        &self,
        test_cmd: crate::config::command::TestCommand,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        mut env_vars: HashMap<String, String>,
        _workflow: Option<&NormalizedWorkflow>,
        _step_index: Option<usize>,
    ) -> Result<StepResult> {
        use std::fs;
        use tempfile::NamedTempFile;

        let (interpolated_test_cmd, resolutions) = ctx.interpolate_with_tracking(&test_cmd.command);
        self.log_variable_resolutions(&resolutions);

        // Track failure history for retry state
        let mut failure_history: Vec<String> = Vec::new();
        let _max_attempts = test_cmd
            .on_failure
            .as_ref()
            .map(|f| f.max_attempts)
            .unwrap_or(1);

        // First, execute the test command
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.user_interaction.display_progress(&format!(
                "Running test command (attempt {attempt}): {interpolated_test_cmd}"
            ));

            // Add test-specific variables
            env_vars.insert("TEST_ATTEMPT".to_string(), attempt.to_string());

            // Execute the test command
            let test_result = self
                .execute_shell_command(&interpolated_test_cmd, env, env_vars.clone(), None)
                .await?;

            // Check if tests passed
            if test_result.success {
                self.user_interaction
                    .display_success(&format!("Tests passed on attempt {attempt}"));
                return Ok(test_result);
            }

            // Tests failed - check if we should retry
            if let Some(debug_config) = &test_cmd.on_failure {
                // Add failure to history
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
                        // Return the last test result
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

                // Save test output to a temp file if it's too large
                // We need to keep the temp file alive until after the debug command runs
                let temp_file = if test_result.stdout.len() + test_result.stderr.len() > 10000 {
                    // Create a temporary file for large outputs
                    let temp_file = NamedTempFile::new()?;
                    let combined_output = format!(
                        "=== STDOUT ===\n{}\n\n=== STDERR ===\n{}",
                        test_result.stdout, test_result.stderr
                    );
                    fs::write(temp_file.path(), &combined_output)?;
                    Some(temp_file)
                } else {
                    None
                };

                let output_path = temp_file
                    .as_ref()
                    .map(|f| f.path().to_string_lossy().to_string());

                // Prepare the debug command with variables
                let mut debug_cmd = debug_config.claude.clone();

                // Add test-specific variables to context
                ctx.variables
                    .insert("test.attempt".to_string(), attempt.to_string());
                ctx.variables.insert(
                    "test.exit_code".to_string(),
                    test_result.exit_code.unwrap_or(-1).to_string(),
                );

                if let Some(output_file) = output_path {
                    ctx.variables.insert("test.output".to_string(), output_file);
                } else {
                    // For smaller outputs, pass directly
                    let combined_output = format!(
                        "STDOUT:\n{}\n\nSTDERR:\n{}",
                        test_result.stdout, test_result.stderr
                    );
                    ctx.variables
                        .insert("test.output".to_string(), combined_output);
                }

                // Interpolate the debug command
                let (interpolated_debug_cmd, debug_resolutions) =
                    ctx.interpolate_with_tracking(&debug_cmd);
                self.log_variable_resolutions(&debug_resolutions);
                debug_cmd = interpolated_debug_cmd;

                // Log the actual command being run
                self.user_interaction.display_info(&format!(
                    "Tests failed, running: {} (attempt {}/{})",
                    debug_cmd, attempt, debug_config.max_attempts
                ));

                // Execute the debug command
                let debug_result = self
                    .execute_claude_command(&debug_cmd, env, env_vars.clone())
                    .await?;

                // Note: commit verification for debug commands happens at a higher level
                // The debug_config.commit_required field indicates that the command
                // should create commits, which is enforced in the command template

                if !debug_result.success {
                    self.user_interaction
                        .display_error("Debug command failed, but continuing with retry");
                }

                // The temp_file will be dropped here, which is safe because the debug command
                // has already been executed and no longer needs the file
                drop(temp_file);

                // Continue to next attempt
            } else {
                // No on_failure configuration, return the failed result
                return Ok(test_result);
            }
        }
    }

    /// Execute foreach command with parallel iteration
    async fn execute_foreach_command(
        &self,
        foreach_config: crate::config::command::ForeachConfig,
        _env: &ExecutionEnvironment,
        _ctx: &WorkflowContext,
        _env_vars: &HashMap<String, String>,
    ) -> Result<StepResult> {
        execute_foreach_command(foreach_config).await
    }

    /// Execute goal-seeking command with iterative refinement
    async fn execute_goal_seek_command(
        &self,
        goal_seek_config: crate::cook::goal_seek::GoalSeekConfig,
        _env: &ExecutionEnvironment,
        _ctx: &WorkflowContext,
        _env_vars: &HashMap<String, String>,
    ) -> Result<StepResult> {
        execute_goal_seek_command(goal_seek_config).await
    }

    /// Execute handler command from registry
    async fn execute_handler_command(
        &self,
        handler_name: String,
        mut attributes: HashMap<String, AttributeValue>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        // Check if command registry is available
        let registry = self.command_registry.as_ref().ok_or_else(|| {
            anyhow!("Command registry not initialized. Call with_command_registry() first.")
        })?;

        // Create execution context for the handler
        let mut exec_context = ExecutionContext::new(env.working_dir.to_path_buf());
        exec_context.add_env_vars(env_vars);

        // Add session information if available
        if let Some(session_id) = ctx.variables.get("SESSION_ID") {
            exec_context = exec_context.with_session_id(session_id.clone());
        }
        if let Some(iteration) = ctx.iteration_vars.get("ITERATION") {
            if let Ok(iter_num) = iteration.parse::<usize>() {
                exec_context = exec_context.with_iteration(iter_num);
            }
        }

        // Interpolate attribute values and track resolutions
        let mut all_resolutions = Vec::new();
        for (attr_name, value) in attributes.iter_mut() {
            if let AttributeValue::String(s) = value {
                let (interpolated, resolutions) = ctx.interpolate_with_tracking(s);
                if !resolutions.is_empty() {
                    tracing::debug!("   Handler attribute '{}' variables:", attr_name);
                    all_resolutions.extend(resolutions);
                }
                *s = interpolated;
            }
        }
        self.log_variable_resolutions(&all_resolutions);

        // Execute the handler
        let result = registry
            .execute(&handler_name, &exec_context, attributes)
            .await;

        // Convert CommandResult to StepResult
        Ok(StepResult {
            success: result.is_success(),
            exit_code: result.exit_code,
            stdout: result.stdout.unwrap_or_else(|| {
                result
                    .data
                    .as_ref()
                    .map(|d| serde_json::to_string_pretty(d).unwrap_or_default())
                    .unwrap_or_default()
            }),
            stderr: result
                .stderr
                .unwrap_or_else(|| result.error.unwrap_or_default()),
            json_log_location: None,
        })
    }

    /// Handle test mode execution
    pub(crate) fn handle_test_mode_execution(
        &self,
        step: &WorkflowStep,
        command_type: &CommandType,
    ) -> Result<StepResult> {
        let command_str = match command_type {
            CommandType::Claude(cmd) => format!("Claude command: {cmd}"),
            CommandType::Shell(cmd) => format!("Shell command: {cmd}"),
            CommandType::Test(test_cmd) => format!("Test command: {}", test_cmd.command),
            CommandType::Legacy(cmd) => format!("Legacy command: {cmd}"),
            CommandType::Handler { handler_name, .. } => format!("Handler command: {handler_name}"),
            CommandType::GoalSeek(config) => format!("Goal-seek command: {}", config.goal),
            CommandType::Foreach(config) => {
                let item_count = match &config.input {
                    crate::config::command::ForeachInput::List(items) => items.len(),
                    crate::config::command::ForeachInput::Command(_) => 0,
                };
                format!("Foreach command: {} items", item_count)
            }
            CommandType::WriteFile(config) => format!("Write file command: {}", config.path),
        };

        println!("[TEST MODE] Would execute {command_str}");

        // Check if we should simulate no changes
        let should_simulate_no_changes = match command_type {
            CommandType::Claude(cmd) | CommandType::Legacy(cmd) => {
                self.is_test_mode_no_changes_command(cmd)
            }
            CommandType::Shell(_) => false,
            CommandType::Test(_) => false,
            CommandType::Handler { .. } => false,
            CommandType::GoalSeek(_) => false,
            CommandType::WriteFile(_) => false,
            CommandType::Foreach(_) => false,
        };

        if should_simulate_no_changes {
            println!("[TEST MODE] Simulating no changes");
            // If this command requires commits but simulates no changes,
            // it should fail UNLESS commit validation is explicitly skipped
            let skip_validation =
                std::env::var("PRODIGY_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";
            if step.commit_required && !skip_validation {
                return Err(anyhow::anyhow!(
                    "No changes were committed by {}",
                    self.get_step_display_name(step)
                ));
            }
            return Ok(StepResult {
                success: true,
                exit_code: Some(0),
                stdout: "[TEST MODE] No changes made".to_string(),
                stderr: String::new(),
                json_log_location: None,
            });
        }

        Ok(StepResult {
            success: true,
            exit_code: Some(0),
            stdout: "[TEST MODE] Command executed successfully".to_string(),
            stderr: String::new(),
            json_log_location: None,
        })
    }

    /// Convert JSON value to AttributeValue (instance method)
    pub(super) fn json_to_attribute_value(&self, value: serde_json::Value) -> AttributeValue {
        WorkflowExecutor::json_to_attribute_value_static(value)
    }

    /// Convert JSON value to AttributeValue (static method)
    pub(super) fn json_to_attribute_value_static(value: serde_json::Value) -> AttributeValue {
        match value {
            serde_json::Value::String(s) => AttributeValue::String(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    AttributeValue::Number(i as f64)
                } else if let Some(f) = n.as_f64() {
                    AttributeValue::Number(f)
                } else {
                    AttributeValue::Number(0.0)
                }
            }
            serde_json::Value::Bool(b) => AttributeValue::Boolean(b),
            serde_json::Value::Array(arr) => AttributeValue::Array(
                arr.into_iter()
                    .map(WorkflowExecutor::json_to_attribute_value_static)
                    .collect(),
            ),
            serde_json::Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    map.insert(k, WorkflowExecutor::json_to_attribute_value_static(v));
                }
                AttributeValue::Object(map)
            }
            serde_json::Value::Null => AttributeValue::Null,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_execution_result_success() {
        let exec_result = ExecutionResult {
            success: true,
            exit_code: Some(0),
            stdout: "output".to_string(),
            stderr: String::new(),
            metadata: HashMap::new(),
        };

        let step_result = convert_execution_result(exec_result);
        assert!(step_result.success);
        assert_eq!(step_result.exit_code, Some(0));
        assert_eq!(step_result.stdout, "output");
    }

    #[test]
    fn test_convert_execution_result_failure() {
        let exec_result = ExecutionResult {
            success: false,
            exit_code: Some(1),
            stdout: String::new(),
            stderr: "error".to_string(),
            metadata: HashMap::new(),
        };

        let step_result = convert_execution_result(exec_result);
        assert!(!step_result.success);
        assert_eq!(step_result.exit_code, Some(1));
        assert_eq!(step_result.stderr, "error");
    }

    #[test]
    fn test_format_command_description_claude() {
        let cmd = CommandType::Claude("test command".to_string());
        assert_eq!(format_command_description(&cmd), "claude: test command");
    }

    #[test]
    fn test_format_command_description_shell() {
        let cmd = CommandType::Shell("ls -la".to_string());
        assert_eq!(format_command_description(&cmd), "shell: ls -la");
    }

    #[tokio::test]
    async fn test_execute_shell_command_success() {
        let result = execute_shell_command(
            "echo 'test'",
            std::path::Path::new("/tmp"),
            HashMap::new(),
            None,
        )
        .await;

        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.success);
        assert!(step_result.stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_execute_shell_command_failure() {
        let result =
            execute_shell_command("exit 1", std::path::Path::new("/tmp"), HashMap::new(), None)
                .await;

        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(!step_result.success);
        assert_eq!(step_result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn test_execute_shell_command_timeout() {
        let result = execute_shell_command(
            "sleep 10",
            std::path::Path::new("/tmp"),
            HashMap::new(),
            Some(1), // 1 second timeout
        )
        .await;

        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(!step_result.success);
        assert!(step_result.stderr.contains("timed out"));
    }

    // ============================================================================
    // Tests for execute_write_file_command
    // ============================================================================

    #[tokio::test]
    async fn test_write_file_text_success() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "test.txt".to_string(),
            content: "Hello, World!".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "0644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_ok());

        let step_result = result.unwrap();
        assert!(step_result.success);
        assert_eq!(step_result.exit_code, Some(0));
        assert!(step_result.stdout.contains("Wrote 13 bytes"));
        assert!(step_result.stdout.contains("test.txt"));
        assert!(step_result.stdout.contains("Text"));

        // Verify file contents
        let file_path = temp_dir.path().join("test.txt");
        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "Hello, World!");
    }

    #[tokio::test]
    async fn test_write_file_json_validation_and_formatting() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "data.json".to_string(),
            content: r#"{"name":"test","value":123}"#.to_string(),
            format: crate::config::command::WriteFileFormat::Json,
            mode: "0644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_ok());

        let step_result = result.unwrap();
        assert!(step_result.success);
        assert!(step_result.stdout.contains("Json"));

        // Verify file is pretty-printed JSON
        let file_path = temp_dir.path().join("data.json");
        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert!(contents.contains("\"name\": \"test\""));
        assert!(contents.contains("\"value\": 123"));
    }

    #[tokio::test]
    async fn test_write_file_yaml_validation_and_formatting() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "config.yml".to_string(),
            content: "name: test\nvalue: 123".to_string(),
            format: crate::config::command::WriteFileFormat::Yaml,
            mode: "0644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_ok());

        let step_result = result.unwrap();
        assert!(step_result.success);
        assert!(step_result.stdout.contains("Yaml"));

        // Verify file contents
        let file_path = temp_dir.path().join("config.yml");
        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert!(contents.contains("name: test"));
    }

    #[tokio::test]
    async fn test_write_file_create_dirs() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "subdir/nested/file.txt".to_string(),
            content: "test".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "0644".to_string(),
            create_dirs: true,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_ok());

        // Verify directories were created
        let file_path = temp_dir.path().join("subdir/nested/file.txt");
        assert!(file_path.exists());
        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(contents, "test");
    }

    #[tokio::test]
    async fn test_write_file_reject_path_traversal() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "../etc/passwd".to_string(),
            content: "malicious".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "0644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("parent directory traversal"));
    }

    #[tokio::test]
    async fn test_write_file_invalid_json() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "bad.json".to_string(),
            content: "{invalid json}".to_string(),
            format: crate::config::command::WriteFileFormat::Json,
            mode: "0644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid JSON"));
    }

    #[tokio::test]
    async fn test_write_file_invalid_yaml() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "bad.yml".to_string(),
            content: "invalid: [unclosed".to_string(),
            format: crate::config::command::WriteFileFormat::Yaml,
            mode: "0644".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid YAML"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_write_file_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "executable.sh".to_string(),
            content: "#!/bin/bash\necho test".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "0755".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_ok());

        // Verify permissions
        let file_path = temp_dir.path().join("executable.sh");
        let metadata = std::fs::metadata(&file_path).unwrap();
        let permissions = metadata.permissions();
        assert_eq!(permissions.mode() & 0o777, 0o755);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_write_file_invalid_mode() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = crate::config::command::WriteFileConfig {
            path: "test.txt".to_string(),
            content: "test".to_string(),
            format: crate::config::command::WriteFileFormat::Text,
            mode: "invalid".to_string(),
            create_dirs: false,
        };

        let result = execute_write_file_command(&config, temp_dir.path()).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid file mode"));
    }

    // Tests for execute_shell_with_retry
    mod shell_retry_tests {
        use super::*;
        use crate::abstractions::git::MockGitOperations;
        use crate::config::command::TestDebugConfig;
        use crate::cook::execution::ClaudeExecutor;
        use crate::cook::execution::ExecutionResult;
        use crate::cook::interaction::SpinnerHandle;
        use crate::cook::interaction::UserInteraction;
        use crate::cook::interaction::VerbosityLevel;
        use crate::cook::orchestrator::ExecutionEnvironment;
        use crate::cook::session::state::SessionState;
        use crate::cook::session::summary::SessionSummary;
        use crate::cook::session::SessionInfo;
        use crate::cook::session::{SessionManager, SessionUpdate};
        use crate::cook::workflow::WorkflowContext;
        use crate::testing::config::TestConfiguration;
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::path::{Path, PathBuf};
        use std::sync::{Arc, Mutex};
        use tempfile::TempDir;

        // Mock implementations
        pub struct MockClaudeExecutor {
            responses: Arc<Mutex<Vec<ExecutionResult>>>,
            #[allow(clippy::type_complexity)]
            calls: Arc<Mutex<Vec<(String, PathBuf, HashMap<String, String>)>>>,
        }

        impl MockClaudeExecutor {
            fn new() -> Self {
                Self {
                    responses: Arc::new(Mutex::new(Vec::new())),
                    calls: Arc::new(Mutex::new(Vec::new())),
                }
            }

            fn add_response(&self, response: ExecutionResult) {
                self.responses.lock().unwrap().push(response);
            }
        }

        #[async_trait]
        impl ClaudeExecutor for MockClaudeExecutor {
            async fn execute_claude_command(
                &self,
                command: &str,
                working_dir: &Path,
                env_vars: HashMap<String, String>,
            ) -> Result<ExecutionResult> {
                self.calls.lock().unwrap().push((
                    command.to_string(),
                    working_dir.to_path_buf(),
                    env_vars.clone(),
                ));

                self.responses
                    .lock()
                    .unwrap()
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
            }

            async fn check_claude_cli(&self) -> Result<bool> {
                Ok(true)
            }

            async fn get_claude_version(&self) -> Result<String> {
                Ok("mock-version-1.0.0".to_string())
            }
        }

        pub struct MockSessionManager {
            updates: Arc<Mutex<Vec<SessionUpdate>>>,
        }

        impl MockSessionManager {
            fn new() -> Self {
                Self {
                    updates: Arc::new(Mutex::new(Vec::new())),
                }
            }
        }

        #[async_trait]
        impl SessionManager for MockSessionManager {
            async fn update_session(&self, update: SessionUpdate) -> Result<()> {
                self.updates.lock().unwrap().push(update);
                Ok(())
            }

            async fn start_session(&self, _session_id: &str) -> Result<()> {
                Ok(())
            }

            async fn complete_session(&self) -> Result<SessionSummary> {
                Ok(SessionSummary {
                    iterations: 1,
                    files_changed: 0,
                })
            }

            fn get_state(&self) -> Result<SessionState> {
                Ok(SessionState::new(
                    "test-session".to_string(),
                    PathBuf::from("/tmp"),
                ))
            }

            async fn save_state(&self, _path: &Path) -> Result<()> {
                Ok(())
            }

            async fn load_state(&self, _path: &Path) -> Result<()> {
                Ok(())
            }

            async fn load_session(&self, _session_id: &str) -> Result<SessionState> {
                Ok(SessionState::new(
                    "test-session".to_string(),
                    PathBuf::from("/tmp"),
                ))
            }

            async fn save_checkpoint(&self, _state: &SessionState) -> Result<()> {
                Ok(())
            }

            async fn list_resumable(&self) -> Result<Vec<SessionInfo>> {
                Ok(vec![])
            }

            async fn get_last_interrupted(&self) -> Result<Option<String>> {
                Ok(None)
            }
        }

        struct MockSpinnerHandle;

        impl SpinnerHandle for MockSpinnerHandle {
            fn update_message(&mut self, _message: &str) {}
            fn success(&mut self, _message: &str) {}
            fn fail(&mut self, _message: &str) {}
        }

        pub struct MockUserInteraction {
            messages: Arc<Mutex<Vec<(String, String)>>>,
        }

        impl MockUserInteraction {
            fn new() -> Self {
                Self {
                    messages: Arc::new(Mutex::new(Vec::new())),
                }
            }
        }

        #[async_trait]
        impl UserInteraction for MockUserInteraction {
            fn display_info(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("info".to_string(), message.to_string()));
            }

            fn display_progress(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("progress".to_string(), message.to_string()));
            }

            fn display_success(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("success".to_string(), message.to_string()));
            }

            fn display_error(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("error".to_string(), message.to_string()));
            }

            fn display_warning(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("warning".to_string(), message.to_string()));
            }

            fn display_action(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("action".to_string(), message.to_string()));
            }

            fn display_metric(&self, label: &str, value: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("metric".to_string(), format!("{}: {}", label, value)));
            }

            fn display_status(&self, message: &str) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("status".to_string(), message.to_string()));
            }

            async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
                Ok(true)
            }

            async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
                Ok("test".to_string())
            }

            fn start_spinner(&self, _message: &str) -> Box<dyn SpinnerHandle> {
                Box::new(MockSpinnerHandle)
            }

            fn iteration_start(&self, current: u32, total: u32) {
                self.messages.lock().unwrap().push((
                    "iteration_start".to_string(),
                    format!("{}/{}", current, total),
                ));
            }

            fn iteration_end(&self, current: u32, duration: std::time::Duration, success: bool) {
                self.messages.lock().unwrap().push((
                    "iteration_end".to_string(),
                    format!("{} {:?} {}", current, duration, success),
                ));
            }

            fn step_start(&self, step: u32, total: u32, description: &str) {
                self.messages.lock().unwrap().push((
                    "step_start".to_string(),
                    format!("{}/{} {}", step, total, description),
                ));
            }

            fn step_end(&self, step: u32, success: bool) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("step_end".to_string(), format!("{} {}", step, success)));
            }

            fn command_output(
                &self,
                output: &str,
                _verbosity: crate::cook::interaction::VerbosityLevel,
            ) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("command_output".to_string(), output.to_string()));
            }

            fn debug_output(
                &self,
                message: &str,
                _min_verbosity: crate::cook::interaction::VerbosityLevel,
            ) {
                self.messages
                    .lock()
                    .unwrap()
                    .push(("debug".to_string(), message.to_string()));
            }

            fn verbosity(&self) -> VerbosityLevel {
                VerbosityLevel::Normal
            }
        }

        async fn create_test_executor() -> (
            WorkflowExecutor,
            Arc<MockClaudeExecutor>,
            Arc<MockUserInteraction>,
        ) {
            let claude_executor = Arc::new(MockClaudeExecutor::new());
            let session_manager = Arc::new(MockSessionManager::new());
            let user_interaction = Arc::new(MockUserInteraction::new());
            let git_operations = Arc::new(MockGitOperations::new());

            // Set up default git mock responses
            for _ in 0..20 {
                git_operations.add_success_response("abc123").await;
            }

            let test_config = Arc::new(TestConfiguration {
                test_mode: false,
                ..Default::default()
            });

            let executor = WorkflowExecutor::with_test_config_and_git(
                claude_executor.clone() as Arc<dyn ClaudeExecutor>,
                session_manager.clone() as Arc<dyn SessionManager>,
                user_interaction.clone() as Arc<dyn UserInteraction>,
                test_config,
                git_operations,
            );

            (executor, claude_executor, user_interaction)
        }

        #[tokio::test]
        async fn test_shell_retry_success_first_attempt() {
            let (executor, _, _) = create_test_executor().await;

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let mut ctx = WorkflowContext::default();
            let env_vars = HashMap::new();

            let result = executor
                .execute_shell_with_retry("echo 'success'", None, &env, &mut ctx, env_vars, None)
                .await
                .unwrap();

            assert!(result.success);
            assert_eq!(result.exit_code, Some(0));
            assert!(result.stdout.contains("success"));
        }

        #[tokio::test]
        async fn test_shell_retry_success_after_retry() {
            let (executor, claude_mock, _) = create_test_executor().await;

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let mut ctx = WorkflowContext::default();
            let env_vars = HashMap::new();

            // Configure debug command to succeed
            let mut metadata = HashMap::new();
            metadata.insert("test".to_string(), "value".to_string());
            claude_mock.add_response(ExecutionResult {
                success: true,
                exit_code: Some(0),
                stdout: "debug output".to_string(),
                stderr: String::new(),
                metadata,
            });

            let on_failure = TestDebugConfig {
                claude: "/debug".to_string(),
                max_attempts: 3,
                fail_workflow: false,
                commit_required: false,
            };

            // First attempt fails, second succeeds
            let result = executor
                .execute_shell_with_retry(
                    "test -f /nonexistent && echo 'found' || (test $SHELL_ATTEMPT -gt 1 && echo 'retry success')",
                    Some(&on_failure),
                    &env,
                    &mut ctx,
                    env_vars,
                    None,
                )
                .await
                .unwrap();

            assert!(result.success);
            assert!(result.stdout.contains("retry success"));
        }

        #[tokio::test]
        async fn test_shell_retry_max_attempts_fail_workflow() {
            let (executor, claude_mock, _) = create_test_executor().await;

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let mut ctx = WorkflowContext::default();
            let env_vars = HashMap::new();

            // Configure debug commands to succeed (but shell command will keep failing)
            for _ in 0..2 {
                let mut metadata = HashMap::new();
                metadata.insert("test".to_string(), "value".to_string());
                claude_mock.add_response(ExecutionResult {
                    success: true,
                    exit_code: Some(0),
                    stdout: "debug output".to_string(),
                    stderr: String::new(),
                    metadata,
                });
            }

            let on_failure = TestDebugConfig {
                claude: "/debug".to_string(),
                max_attempts: 2,
                fail_workflow: true,
                commit_required: false,
            };

            // Command that always fails
            let result = executor
                .execute_shell_with_retry(
                    "exit 1",
                    Some(&on_failure),
                    &env,
                    &mut ctx,
                    env_vars,
                    None,
                )
                .await;

            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("failed after 2 attempts"));
        }

        #[tokio::test]
        async fn test_shell_retry_max_attempts_no_fail_workflow() {
            let (executor, claude_mock, _) = create_test_executor().await;

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let mut ctx = WorkflowContext::default();
            let env_vars = HashMap::new();

            // Configure debug commands to succeed
            for _ in 0..2 {
                let mut metadata = HashMap::new();
                metadata.insert("test".to_string(), "value".to_string());
                claude_mock.add_response(ExecutionResult {
                    success: true,
                    exit_code: Some(0),
                    stdout: "debug output".to_string(),
                    stderr: String::new(),
                    metadata,
                });
            }

            let on_failure = TestDebugConfig {
                claude: "/debug".to_string(),
                max_attempts: 2,
                fail_workflow: false,
                commit_required: false,
            };

            // Command that always fails
            let result = executor
                .execute_shell_with_retry(
                    "exit 1",
                    Some(&on_failure),
                    &env,
                    &mut ctx,
                    env_vars,
                    None,
                )
                .await
                .unwrap();

            assert!(!result.success);
            assert_eq!(result.exit_code, Some(1));
        }

        #[tokio::test]
        async fn test_shell_retry_no_on_failure_config() {
            let (executor, _, _) = create_test_executor().await;

            let temp_dir = TempDir::new().unwrap();
            let env = ExecutionEnvironment {
                working_dir: Arc::new(temp_dir.path().to_path_buf()),
                project_dir: Arc::new(temp_dir.path().to_path_buf()),
                worktree_name: None,
                session_id: Arc::from("test"),
            };

            let mut ctx = WorkflowContext::default();
            let env_vars = HashMap::new();

            // Command that fails - should return immediately without retry
            let result = executor
                .execute_shell_with_retry("exit 1", None, &env, &mut ctx, env_vars, None)
                .await
                .unwrap();

            assert!(!result.success);
            assert_eq!(result.exit_code, Some(1));
        }

        // Unit tests for output management pure functions

        #[test]
        fn test_should_use_temp_file_small_output() {
            assert!(!WorkflowExecutor::should_use_temp_file(100, 100));
            assert!(!WorkflowExecutor::should_use_temp_file(5000, 4999));
        }

        #[test]
        fn test_should_use_temp_file_large_output() {
            assert!(WorkflowExecutor::should_use_temp_file(5000, 5001));
            assert!(WorkflowExecutor::should_use_temp_file(10000, 1));
            assert!(WorkflowExecutor::should_use_temp_file(20000, 20000));
        }

        #[test]
        fn test_should_use_temp_file_boundary() {
            assert!(!WorkflowExecutor::should_use_temp_file(5000, 5000));
            assert!(WorkflowExecutor::should_use_temp_file(5000, 5001));
            assert!(WorkflowExecutor::should_use_temp_file(10001, 0));
        }

        #[test]
        fn test_format_shell_output() {
            let stdout = "output line 1\noutput line 2";
            let stderr = "error line 1\nerror line 2";
            let result = WorkflowExecutor::format_shell_output(stdout, stderr);

            assert!(result.contains("=== STDOUT ==="));
            assert!(result.contains("=== STDERR ==="));
            assert!(result.contains("output line 1"));
            assert!(result.contains("error line 1"));
        }

        #[test]
        fn test_format_shell_output_empty_stderr() {
            let stdout = "output";
            let stderr = "";
            let result = WorkflowExecutor::format_shell_output(stdout, stderr);

            assert!(result.contains("=== STDOUT ==="));
            assert!(result.contains("=== STDERR ==="));
            assert!(result.contains("output"));
        }

        #[test]
        fn test_format_inline_output() {
            let stdout = "stdout content";
            let stderr = "stderr content";
            let result = WorkflowExecutor::format_inline_output(stdout, stderr);

            assert!(result.contains("STDOUT:"));
            assert!(result.contains("STDERR:"));
            assert!(result.contains("stdout content"));
            assert!(result.contains("stderr content"));
        }

        #[test]
        fn test_format_inline_output_with_newlines() {
            let stdout = "line1\nline2\nline3";
            let stderr = "err1\nerr2";
            let result = WorkflowExecutor::format_inline_output(stdout, stderr);

            assert!(result.contains("STDOUT:"));
            assert!(result.contains("line1\nline2\nline3"));
            assert!(result.contains("err1\nerr2"));
        }

        #[test]
        fn test_create_output_temp_file() {
            let stdout = "test stdout";
            let stderr = "test stderr";
            let result = WorkflowExecutor::create_output_temp_file(stdout, stderr);

            assert!(result.is_ok());
            let temp_file = result.unwrap();
            let content = std::fs::read_to_string(temp_file.path()).unwrap();

            assert!(content.contains("=== STDOUT ==="));
            assert!(content.contains("=== STDERR ==="));
            assert!(content.contains("test stdout"));
            assert!(content.contains("test stderr"));
        }

        #[test]
        fn test_create_output_temp_file_large_content() {
            let stdout = "x".repeat(100000);
            let stderr = "y".repeat(50000);
            let result = WorkflowExecutor::create_output_temp_file(&stdout, &stderr);

            assert!(result.is_ok());
            let temp_file = result.unwrap();
            let content = std::fs::read_to_string(temp_file.path()).unwrap();

            assert!(content.contains(&stdout));
            assert!(content.contains(&stderr));
        }

        #[test]
        fn test_create_output_temp_file_empty() {
            let result = WorkflowExecutor::create_output_temp_file("", "");
            assert!(result.is_ok());

            let temp_file = result.unwrap();
            let content = std::fs::read_to_string(temp_file.path()).unwrap();
            assert!(content.contains("=== STDOUT ==="));
            assert!(content.contains("=== STDERR ==="));
        }

        // Unit tests for context management pure functions

        #[test]
        fn test_build_shell_context_vars_basic() {
            let vars =
                WorkflowExecutor::build_shell_context_vars(2, Some(1), "test output".to_string());

            assert_eq!(vars.get("shell.attempt").unwrap(), "2");
            assert_eq!(vars.get("shell.exit_code").unwrap(), "1");
            assert_eq!(vars.get("shell.output").unwrap(), "test output");
            assert_eq!(vars.len(), 3);
        }

        #[test]
        fn test_build_shell_context_vars_no_exit_code() {
            let vars = WorkflowExecutor::build_shell_context_vars(1, None, "output".to_string());

            assert_eq!(vars.get("shell.attempt").unwrap(), "1");
            assert_eq!(vars.get("shell.exit_code").unwrap(), "-1");
            assert_eq!(vars.get("shell.output").unwrap(), "output");
        }

        #[test]
        fn test_build_shell_context_vars_zero_exit_code() {
            let vars =
                WorkflowExecutor::build_shell_context_vars(1, Some(0), "success".to_string());

            assert_eq!(vars.get("shell.exit_code").unwrap(), "0");
        }

        #[test]
        fn test_build_shell_context_vars_high_attempt() {
            let vars =
                WorkflowExecutor::build_shell_context_vars(999, Some(127), "output".to_string());

            assert_eq!(vars.get("shell.attempt").unwrap(), "999");
            assert_eq!(vars.get("shell.exit_code").unwrap(), "127");
        }

        #[test]
        fn test_build_shell_context_vars_multiline_output() {
            let output = "line1\nline2\nline3".to_string();
            let vars = WorkflowExecutor::build_shell_context_vars(1, Some(1), output.clone());

            assert_eq!(vars.get("shell.output").unwrap(), &output);
        }

        #[test]
        fn test_build_shell_context_vars_large_output() {
            let output = "x".repeat(100000);
            let vars = WorkflowExecutor::build_shell_context_vars(1, Some(1), output.clone());

            assert_eq!(vars.get("shell.output").unwrap(), &output);
        }

        #[test]
        fn test_build_shell_context_vars_empty_output() {
            let vars = WorkflowExecutor::build_shell_context_vars(1, Some(0), String::new());

            assert_eq!(vars.get("shell.output").unwrap(), "");
        }

        #[test]
        fn test_build_shell_context_vars_special_characters() {
            let output = "test\t\n\r$var ${var} `cmd`".to_string();
            let vars = WorkflowExecutor::build_shell_context_vars(1, Some(1), output.clone());

            assert_eq!(vars.get("shell.output").unwrap(), &output);
        }

        // Unit tests for retry logic pure functions

        #[test]
        fn test_has_reached_max_attempts_not_reached() {
            assert!(!WorkflowExecutor::has_reached_max_attempts(1, 3));
            assert!(!WorkflowExecutor::has_reached_max_attempts(2, 3));
        }

        #[test]
        fn test_has_reached_max_attempts_exactly_reached() {
            assert!(WorkflowExecutor::has_reached_max_attempts(3, 3));
        }

        #[test]
        fn test_has_reached_max_attempts_exceeded() {
            assert!(WorkflowExecutor::has_reached_max_attempts(4, 3));
            assert!(WorkflowExecutor::has_reached_max_attempts(10, 3));
        }

        #[test]
        fn test_has_reached_max_attempts_boundary() {
            assert!(!WorkflowExecutor::has_reached_max_attempts(0, 1));
            assert!(WorkflowExecutor::has_reached_max_attempts(1, 1));
            assert!(WorkflowExecutor::has_reached_max_attempts(2, 1));
        }

        #[test]
        fn test_has_reached_max_attempts_first_attempt() {
            assert!(WorkflowExecutor::has_reached_max_attempts(1, 1));
            assert!(!WorkflowExecutor::has_reached_max_attempts(1, 2));
        }

        #[test]
        fn test_should_fail_workflow_on_max_attempts_true() {
            let result = WorkflowExecutor::should_fail_workflow_on_max_attempts(true);
            assert!(result.is_err());
            assert_eq!(result.unwrap_err(), "fail_workflow is true");
        }

        #[test]
        fn test_should_fail_workflow_on_max_attempts_false() {
            let result = WorkflowExecutor::should_fail_workflow_on_max_attempts(false);
            assert!(result.is_ok());
        }
    }
}
