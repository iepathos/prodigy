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
    StepResult {
        success: result.success,
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
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
    }
}

// ============================================================================
// WorkflowExecutor Command Execution Methods
// ============================================================================

impl WorkflowExecutor {
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
            return Ok(StepResult {
                success: true,
                stdout: format!("[dry-run] {}", command_desc),
                stderr: String::new(),
                exit_code: Some(0),
            });
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
        use std::fs;
        use tempfile::NamedTempFile;

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
                if attempt >= debug_config.max_attempts {
                    self.user_interaction.display_error(&format!(
                        "Shell command failed after {} attempts",
                        debug_config.max_attempts
                    ));

                    if debug_config.fail_workflow {
                        return Err(anyhow!(
                            "Shell command failed after {} attempts and fail_workflow is true",
                            debug_config.max_attempts
                        ));
                    } else {
                        // Return the last result
                        return Ok(shell_result);
                    }
                }

                // Save shell output to a temp file if it's too large
                let temp_file = if shell_result.stdout.len() + shell_result.stderr.len() > 10000 {
                    // Create a temporary file for large outputs
                    let temp_file = NamedTempFile::new()?;
                    let combined_output = format!(
                        "=== STDOUT ===\n{}\n\n=== STDERR ===\n{}",
                        shell_result.stdout, shell_result.stderr
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

                // Add shell-specific variables to context
                ctx.variables
                    .insert("shell.attempt".to_string(), attempt.to_string());
                ctx.variables.insert(
                    "shell.exit_code".to_string(),
                    shell_result.exit_code.unwrap_or(-1).to_string(),
                );

                if let Some(output_file) = output_path {
                    ctx.variables
                        .insert("shell.output".to_string(), output_file);
                } else {
                    // For smaller outputs, pass directly
                    let combined_output = format!(
                        "STDOUT:\n{}\n\nSTDERR:\n{}",
                        shell_result.stdout, shell_result.stderr
                    );
                    ctx.variables
                        .insert("shell.output".to_string(), combined_output);
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
            });
        }

        Ok(StepResult {
            success: true,
            exit_code: Some(0),
            stdout: "[TEST MODE] Command executed successfully".to_string(),
            stderr: String::new(),
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
}
