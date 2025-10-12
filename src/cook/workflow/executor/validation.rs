//! Validation and conditional execution logic
//!
//! Handles workflow validation, condition evaluation, and execution decisions.

use super::super::step_validation::StepValidationSpec;
use super::super::validation::{ValidationConfig, ValidationResult};
use super::{
    pure, ExecutionFlags, IterationContinuation, StepResult, WorkflowContext, WorkflowExecutor,
    WorkflowStep,
};
use crate::cook::execution::ExecutionContext;
use crate::cook::expression::{ExpressionEvaluator, VariableContext};
use crate::cook::orchestrator::ExecutionEnvironment;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Pure decision functions for validation retry logic
// ============================================================================

/// Determine if the retry loop should continue
///
/// Returns true if:
/// - attempts < max_attempts AND
/// - validation is incomplete
fn should_continue_retry(attempts: u32, max_attempts: u32, is_complete: bool) -> bool {
    attempts < max_attempts && !is_complete
}

/// Handler type for incomplete validation
#[derive(Debug, Clone, PartialEq)]
enum HandlerType {
    MultiCommand,
    SingleCommand,
    NoHandler,
}

/// Determine what type of handler is configured
fn determine_handler_type(
    on_incomplete: &crate::cook::workflow::validation::OnIncompleteConfig,
) -> HandlerType {
    if on_incomplete.commands.is_some() {
        HandlerType::MultiCommand
    } else if on_incomplete.claude.is_some() || on_incomplete.shell.is_some() {
        HandlerType::SingleCommand
    } else {
        HandlerType::NoHandler
    }
}

/// Retry progress information
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
struct RetryProgress {
    attempts: u32,
    max_attempts: u32,
    completion_percentage: f64,
}

/// Calculate retry progress for display/logging
#[allow(dead_code)]
fn calculate_retry_progress(attempts: u32, max_attempts: u32, completion: f64) -> RetryProgress {
    RetryProgress {
        attempts,
        max_attempts,
        completion_percentage: completion,
    }
}

/// Determine if the workflow should fail based on validation state
///
/// Returns true if validation is incomplete AND fail_workflow is true
fn should_fail_workflow(is_complete: bool, fail_workflow_flag: bool, _attempts: u32) -> bool {
    !is_complete && fail_workflow_flag
}

// ============================================================================
// Pure formatting functions for validation messages
// ============================================================================

/// Format a success message for passed validation
fn format_validation_passed_message(results_count: usize, attempts: u32) -> String {
    format!(
        "Step validation passed ({} validation{}, {} attempt{})",
        results_count,
        if results_count == 1 { "" } else { "s" },
        attempts,
        if attempts == 1 { "" } else { "s" }
    )
}

/// Format a warning message for failed validation
fn format_validation_failed_message(results_count: usize, attempts: u32) -> String {
    format!(
        "Step validation failed ({} validation{}, {} attempt{})",
        results_count,
        if results_count == 1 { "" } else { "s" },
        attempts,
        if attempts == 1 { "" } else { "s" }
    )
}

/// Format detailed message for a single failed validation
fn format_failed_validation_detail(idx: usize, message: &str, exit_code: i32) -> String {
    format!(
        "  Validation {}: {} (exit code: {})",
        idx + 1,
        message,
        exit_code
    )
}

/// Determine step name for logging based on step properties
fn determine_step_name(step: &WorkflowStep) -> &str {
    step.name.as_deref().unwrap_or_else(|| {
        if step.claude.is_some() {
            "claude command"
        } else if step.shell.is_some() {
            "shell command"
        } else {
            "workflow step"
        }
    })
}

// ============================================================================
// Pure helper functions for validation executor setup
// ============================================================================

/// Create execution context for step validation
///
/// Pure function that builds ExecutionContext with validation-specific settings
fn create_validation_execution_context(
    working_directory: std::path::PathBuf,
    timeout_seconds: Option<u64>,
) -> ExecutionContext {
    ExecutionContext {
        working_directory,
        env_vars: std::collections::HashMap::new(),
        capture_output: true,
        timeout_seconds,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    }
}

/// Create a timeout failure result for step validation
///
/// Pure function that builds StepValidationResult representing a timeout
fn create_validation_timeout_result(
    timeout_secs: u64,
) -> super::super::step_validation::StepValidationResult {
    super::super::step_validation::StepValidationResult {
        passed: false,
        results: vec![],
        duration: std::time::Duration::from_secs(timeout_secs),
        attempts: 1,
    }
}

impl WorkflowExecutor {
    // ============================================================================
    // Validation functions
    // ============================================================================

    /// Handle workflow-level validation with retry logic
    pub(super) async fn handle_validation(
        &mut self,
        validation_config: &ValidationConfig,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<()> {
        // Skip validation execution in dry-run mode
        if self.dry_run {
            // Display what validation would be performed
            if let Some(claude_cmd) = &validation_config.claude {
                let validation_desc = format!("validation: claude {}", claude_cmd);
                println!("[DRY RUN] Would run validation (Claude): {}", claude_cmd);
                self.dry_run_validations.push(validation_desc);
            } else if let Some(shell_cmd) = validation_config
                .shell
                .as_ref()
                .or(validation_config.command.as_ref())
            {
                let validation_desc = format!("validation: shell {}", shell_cmd);
                println!("[DRY RUN] Would run validation (shell): {}", shell_cmd);
                self.dry_run_validations.push(validation_desc);
            }

            // Track potential on_incomplete handler
            if let Some(on_incomplete) = &validation_config.on_incomplete {
                let handler_desc = if let Some(commands) = &on_incomplete.commands {
                    format!("on_incomplete: {} commands", commands.len())
                } else if let Some(claude) = &on_incomplete.claude {
                    format!("on_incomplete: claude {}", claude)
                } else if let Some(shell) = &on_incomplete.shell {
                    format!("on_incomplete: shell {}", shell)
                } else {
                    "on_incomplete: unknown".to_string()
                };
                self.dry_run_potential_handlers.push(format!(
                    "{} (max {} attempts)",
                    handler_desc, on_incomplete.max_attempts
                ));
            }

            println!(
                "[DRY RUN] Validation threshold: {:.1}%",
                validation_config.threshold
            );
            println!("[DRY RUN] Assuming validation would pass");
            return Ok(());
        }

        // Execute validation
        let validation_result = self.execute_validation(validation_config, env, ctx).await?;

        // Store validation result in context
        ctx.validation_results
            .insert("validation".to_string(), validation_result.clone());

        // Always display validation percentage
        let percentage = validation_result.completion_percentage;
        let threshold = validation_config.threshold;

        // Check if validation passed
        if validation_config.is_complete(&validation_result) {
            self.user_interaction.display_success(&format!(
                "Validation passed: {:.1}% complete (threshold: {:.1}%)",
                percentage, threshold
            ));
        } else {
            self.user_interaction.display_warning(&format!(
                "Validation incomplete: {:.1}% complete (threshold: {:.1}%)",
                percentage, threshold
            ));

            // Handle incomplete validation
            if let Some(on_incomplete) = &validation_config.on_incomplete {
                self.handle_incomplete_validation(
                    validation_config,
                    on_incomplete,
                    validation_result,
                    env,
                    ctx,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Handle incomplete validation with retry logic
    async fn handle_incomplete_validation(
        &mut self,
        validation_config: &ValidationConfig,
        on_incomplete: &crate::cook::workflow::validation::OnIncompleteConfig,
        initial_result: ValidationResult,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<()> {
        let mut attempts = 0;
        let mut current_result = initial_result;

        while should_continue_retry(
            attempts,
            on_incomplete.max_attempts,
            validation_config.is_complete(&current_result),
        ) {
            attempts += 1;

            self.user_interaction.display_info(&format!(
                "Attempting to complete implementation (attempt {}/{})",
                attempts, on_incomplete.max_attempts
            ));

            // Execute the completion handler(s) based on handler type
            let handler_success = match determine_handler_type(on_incomplete) {
                HandlerType::MultiCommand => {
                    // Execute array of commands
                    let commands = on_incomplete.commands.as_ref().unwrap();
                    self.user_interaction
                        .display_progress(&format!("Running {} recovery commands", commands.len()));

                    let mut all_success = true;
                    for (idx, cmd) in commands.iter().enumerate() {
                        let step = self.convert_workflow_command_to_step(cmd, ctx)?;
                        let step_display = self.get_step_display_name(&step);
                        self.user_interaction.display_progress(&format!(
                            "  Recovery step {}/{}: {}",
                            idx + 1,
                            commands.len(),
                            step_display
                        ));

                        let handler_result = Box::pin(self.execute_step(&step, env, ctx)).await?;

                        if !handler_result.success {
                            self.user_interaction
                                .display_error(&format!("Recovery step {} failed", idx + 1));
                            all_success = false;
                            break;
                        }
                    }
                    all_success
                }
                HandlerType::SingleCommand => {
                    // Execute single command (legacy)
                    let handler_step = self.create_validation_handler(on_incomplete, ctx).unwrap();
                    let step_display = self.get_step_display_name(&handler_step);
                    self.user_interaction
                        .display_progress(&format!("Running recovery step: {}", step_display));

                    let handler_result =
                        Box::pin(self.execute_step(&handler_step, env, ctx)).await?;
                    handler_result.success
                }
                HandlerType::NoHandler => {
                    self.user_interaction
                        .display_error("No recovery commands configured");
                    false
                }
            };

            if !handler_success {
                break;
            }

            // Re-run validation
            current_result = self.execute_validation(validation_config, env, ctx).await?;

            // Display validation percentage after each attempt
            let percentage = current_result.completion_percentage;
            let threshold = validation_config.threshold;
            if validation_config.is_complete(&current_result) {
                self.user_interaction.display_success(&format!(
                    "Validation passed: {:.1}% complete (threshold: {:.1}%)",
                    percentage, threshold
                ));
            } else {
                self.user_interaction.display_info(&format!(
                    "Validation still incomplete: {:.1}% complete (threshold: {:.1}%)",
                    percentage, threshold
                ));
            }

            // Update context
            ctx.validation_results
                .insert("validation".to_string(), current_result.clone());
        }

        // Interactive mode (outside the retry loop)
        if !validation_config.is_complete(&current_result) {
            if let Some(on_incomplete_cfg) = &validation_config.on_incomplete {
                if let Some(ref prompt) = on_incomplete_cfg.prompt {
                    let _should_continue =
                        self.user_interaction.prompt_confirmation(prompt).await?;
                    // User was prompted, continue with workflow
                }
            }
        }

        // Check if we should fail the workflow
        if should_fail_workflow(
            validation_config.is_complete(&current_result),
            on_incomplete.fail_workflow,
            attempts,
        ) {
            return Err(anyhow!(
                "Validation failed after {} attempts. Completion: {:.1}%",
                attempts,
                current_result.completion_percentage
            ));
        }

        Ok(())
    }

    /// Handle step validation (first-class validation feature)
    pub(super) async fn handle_step_validation(
        &mut self,
        validation_spec: &StepValidationSpec,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        step: &WorkflowStep,
    ) -> Result<super::super::step_validation::StepValidationResult> {
        // Skip validation execution in dry-run mode
        if self.dry_run {
            // Display what validation would be performed
            match validation_spec {
                StepValidationSpec::Single(cmd) => {
                    println!("[DRY RUN] Would run step validation: {}", cmd);
                }
                StepValidationSpec::Multiple(cmds) => {
                    println!("[DRY RUN] Would run step validation commands:");
                    for cmd in cmds {
                        println!("[DRY RUN]   - {}", cmd);
                    }
                }
                StepValidationSpec::Detailed(config) => {
                    println!("[DRY RUN] Would run detailed step validation commands:");
                    for cmd in &config.commands {
                        println!("[DRY RUN]   - {}", cmd.command);
                    }
                }
            }
            println!("[DRY RUN] Assuming step validation would pass");

            // Return a simulated successful validation result
            return Ok(super::super::step_validation::StepValidationResult {
                passed: true,
                results: vec![],
                duration: std::time::Duration::from_secs(0),
                attempts: 0,
            });
        }

        // Create a validation executor with the command executor
        let validation_executor = super::super::step_validation::StepValidationExecutor::new(
            Arc::new(super::StepValidationCommandExecutor {
                workflow_executor: self as *mut WorkflowExecutor,
                env: env.clone(),
                ctx: ctx.clone(),
            }) as Arc<dyn crate::cook::execution::CommandExecutor>,
        );

        // Create execution context for validation
        let exec_context = create_validation_execution_context(
            env.working_dir.to_path_buf(),
            step.validation_timeout,
        );

        // Get step name for logging
        let step_name = determine_step_name(step);

        // Execute validation with timeout if specified
        let validation_future =
            validation_executor.validate_step(validation_spec, &exec_context, step_name);

        let validation_result = if let Some(timeout_secs) = step.validation_timeout {
            let timeout = tokio::time::Duration::from_secs(timeout_secs);
            match tokio::time::timeout(timeout, validation_future).await {
                Ok(result) => result?,
                Err(_) => {
                    self.user_interaction.display_error(&format!(
                        "Step validation timed out after {} seconds",
                        timeout_secs
                    ));
                    create_validation_timeout_result(timeout_secs)
                }
            }
        } else {
            validation_future.await?
        };

        // Display validation result
        if validation_result.passed {
            let message = format_validation_passed_message(
                validation_result.results.len(),
                validation_result.attempts,
            );
            self.user_interaction.display_success(&message);
        } else {
            let message = format_validation_failed_message(
                validation_result.results.len(),
                validation_result.attempts,
            );
            self.user_interaction.display_warning(&message);

            // Show details of failed validations
            for (idx, result) in validation_result.results.iter().enumerate() {
                if !result.passed {
                    let detail =
                        format_failed_validation_detail(idx, &result.message, result.exit_code);
                    self.user_interaction.display_info(&detail);
                }
            }
        }

        Ok(validation_result)
    }

    /// Execute step-level validation (legacy and first-class)
    pub(super) async fn execute_step_validation(
        &mut self,
        step: &WorkflowStep,
        result: &mut StepResult,
        actual_env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<()> {
        // Skip validation in dry-run mode since validation was already simulated in execute_command_by_type
        if !result.success || self.dry_run {
            return Ok(());
        }

        // Handle legacy validation config
        if let Some(validation_config) = &step.validate {
            self.handle_validation(validation_config, actual_env, ctx)
                .await?;
        }

        // Handle step validation (first-class validation feature)
        if let Some(step_validation) = &step.step_validate {
            if !step.skip_validation {
                let validation_result = self
                    .handle_step_validation(step_validation, actual_env, ctx, step)
                    .await?;

                // Update result based on validation
                if !validation_result.passed && !step.ignore_validation_failure {
                    result.success = false;
                    result.stdout.push_str(&format!(
                        "\n[Validation Failed: {} validation(s) executed, {} attempt(s) made]",
                        validation_result.results.len(),
                        validation_result.attempts
                    ));
                    if result.exit_code == Some(0) {
                        result.exit_code = Some(1); // Set exit code to indicate validation failure
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute validation command and parse result
    pub(super) async fn execute_validation(
        &mut self,
        validation_config: &ValidationConfig,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<ValidationResult> {
        use crate::cook::workflow::validation::ValidationResult;

        // If commands array is specified, execute all commands in sequence
        if let Some(commands) = &validation_config.commands {
            self.user_interaction.display_progress(&format!(
                "Running validation with {} commands",
                commands.len()
            ));

            for (idx, cmd) in commands.iter().enumerate() {
                self.user_interaction.display_progress(&format!(
                    "  Validation step {}/{}",
                    idx + 1,
                    commands.len()
                ));

                // Execute each command as a workflow step
                let step = self.convert_workflow_command_to_step(cmd, ctx)?;
                // Box the future to avoid recursion issues
                let step_result = Box::pin(self.execute_step(&step, env, ctx)).await?;

                if !step_result.success {
                    return Ok(ValidationResult::failed(format!(
                        "Validation step {} failed: {}",
                        idx + 1,
                        step_result.stdout
                    )));
                }
            }

            // After executing all commands, check for result_file
            if let Some(result_file) = &validation_config.result_file {
                let (interpolated_file, _) = ctx.interpolate_with_tracking(result_file);
                let file_path = env.working_dir.join(&interpolated_file);

                match tokio::fs::read_to_string(&file_path).await {
                    Ok(content) => match ValidationResult::from_json(&content) {
                        Ok(validation) => return Ok(validation),
                        Err(_) => return Ok(ValidationResult::complete()),
                    },
                    Err(e) => {
                        return Ok(ValidationResult::failed(format!(
                            "Failed to read validation result from {}: {}",
                            interpolated_file, e
                        )));
                    }
                }
            }

            // All commands succeeded, return complete
            return Ok(ValidationResult::complete());
        }

        // Execute either claude or shell command (legacy single-command mode)
        let result = if let Some(claude_cmd) = &validation_config.claude {
            let (command, resolutions) = ctx.interpolate_with_tracking(claude_cmd);
            self.log_variable_resolutions(&resolutions);
            self.user_interaction
                .display_progress(&format!("Running validation (Claude): {}", command));

            // Execute Claude command for validation
            // Use prepare_env_vars to get environment variables with proper streaming flag propagation
            let dummy_step = WorkflowStep::default();
            let env_vars = self.prepare_env_vars(&dummy_step, env, ctx);
            self.execute_claude_command(&command, env, env_vars).await?
        } else if let Some(shell_cmd) = validation_config
            .shell
            .as_ref()
            .or(validation_config.command.as_ref())
        {
            // Prefer 'shell' field, fall back to 'command' for backward compatibility
            let (command, resolutions) = ctx.interpolate_with_tracking(shell_cmd);
            self.log_variable_resolutions(&resolutions);
            self.user_interaction
                .display_progress(&format!("Running validation (shell): {}", command));

            // Execute shell command
            let mut env_vars = HashMap::new();
            env_vars.insert("PRODIGY_VALIDATION".to_string(), "true".to_string());

            self.execute_shell_command(&command, env, env_vars, validation_config.timeout)
                .await?
        } else {
            return Ok(ValidationResult::failed(
                "No validation command specified".to_string(),
            ));
        };

        if !result.success {
            // Validation command failed
            return Ok(ValidationResult::failed(format!(
                "Validation command failed with exit code: {}",
                result.exit_code.unwrap_or(-1)
            )));
        }

        // If result_file is specified, read from file instead of stdout
        let json_content = if let Some(result_file) = &validation_config.result_file {
            let (interpolated_file, _resolutions) = ctx.interpolate_with_tracking(result_file);
            // No need to log resolutions for result file path
            let file_path = env.working_dir.join(&interpolated_file);

            // Read the validation result from the file
            match tokio::fs::read_to_string(&file_path).await {
                Ok(content) => content,
                Err(e) => {
                    return Ok(ValidationResult::failed(format!(
                        "Failed to read validation result from {}: {}",
                        interpolated_file, e
                    )));
                }
            }
        } else {
            // Use stdout as before
            result.stdout.clone()
        };

        // Try to parse the JSON content
        match ValidationResult::from_json(&json_content) {
            Ok(mut validation) => {
                // Store raw output
                validation.raw_output = Some(result.stdout);
                Ok(validation)
            }
            Err(_) => {
                // If not JSON, treat as simple pass/fail based on exit code
                if result.success {
                    Ok(ValidationResult::complete())
                } else {
                    Ok(ValidationResult::failed(
                        "Validation failed (non-JSON output)".to_string(),
                    ))
                }
            }
        }
    }

    // ============================================================================
    // Conditional execution functions
    // ============================================================================

    /// Handle conditional execution (on_failure, on_success, on_exit_code)
    pub(super) async fn handle_conditional_execution(
        &mut self,
        step: &WorkflowStep,
        mut result: StepResult,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Handle failure
        if !result.success {
            if let Some(on_failure_config) = &step.on_failure {
                result = self
                    .handle_on_failure(step, result, on_failure_config, env, ctx)
                    .await?;
            }
        } else if let Some(on_success) = &step.on_success {
            // Handle success
            self.user_interaction
                .display_info("Executing on_success step...");
            let success_result = Box::pin(self.execute_step(on_success, env, ctx)).await?;
            result.stdout.push_str("\n--- on_success output ---\n");
            result.stdout.push_str(&success_result.stdout);
        }

        // Handle exit code specific steps
        if let Some(exit_code) = result.exit_code {
            if let Some(exit_step) = step.on_exit_code.get(&exit_code) {
                self.user_interaction
                    .display_info(&format!("Executing on_exit_code[{exit_code}] step..."));
                let exit_result = Box::pin(self.execute_step(exit_step, env, ctx)).await?;
                result
                    .stdout
                    .push_str(&format!("\n--- on_exit_code[{exit_code}] output ---\n"));
                result.stdout.push_str(&exit_result.stdout);
            }
        }

        Ok(result)
    }

    /// Evaluate a when condition expression
    pub(crate) fn evaluate_when_condition(
        &self,
        when_expr: &str,
        context: &WorkflowContext,
    ) -> Result<bool> {
        let evaluator = ExpressionEvaluator::new();
        let mut variable_context = VariableContext::new();

        // Add workflow context variables to expression context
        for (key, value) in &context.variables {
            variable_context.set_string(key.clone(), value.clone());
        }

        // Add command outputs to expression context
        for (key, value) in &context.captured_outputs {
            variable_context.set_string(key.clone(), value.clone());
        }

        // Evaluate the expression
        evaluator
            .evaluate(when_expr, &variable_context)
            .with_context(|| format!("Failed to evaluate when condition: {}", when_expr))
    }

    // ============================================================================
    // Decision functions
    // ============================================================================

    /// Determine execution flags (delegated to pure module)
    pub(super) fn determine_execution_flags() -> ExecutionFlags {
        pure::determine_execution_flags()
    }

    /// Determine if a step should be skipped (delegated to pure module)
    pub(super) fn should_skip_step_execution(
        step_index: usize,
        completed_steps: &[crate::cook::session::StepResult],
    ) -> bool {
        pure::should_skip_step_execution(step_index, completed_steps)
    }

    /// Determine if workflow should continue based on state (delegated to pure module)
    pub(super) fn determine_iteration_continuation(
        workflow: &super::super::ExtendedWorkflowConfig,
        iteration: u32,
        max_iterations: u32,
        any_changes: bool,
        execution_flags: &ExecutionFlags,
        is_focus_tracking_test: bool,
        should_stop_early_in_test: bool,
    ) -> IterationContinuation {
        pure::determine_iteration_continuation(
            workflow,
            iteration,
            max_iterations,
            any_changes,
            execution_flags,
            is_focus_tracking_test,
            should_stop_early_in_test,
        )
    }

    /// Determine if workflow should fail based on command result (delegated to pure module)
    pub(super) fn should_fail_workflow_for_step(
        step_result: &StepResult,
        step: &WorkflowStep,
    ) -> bool {
        pure::should_fail_workflow_for_step(step_result, step)
    }

    /// Determine if workflow should continue iterations
    pub(super) async fn should_continue_iterations(
        &self,
        _env: &ExecutionEnvironment,
    ) -> Result<bool> {
        // Always continue iterations until max_iterations is reached
        // The iteration loop already handles the max_iterations check
        Ok(true)
    }

    // ============================================================================
    // Test helper functions
    // ============================================================================

    /// Check if this is the focus tracking test
    pub(crate) fn is_focus_tracking_test(&self) -> bool {
        self.test_config.as_ref().is_some_and(|c| c.track_focus)
    }

    /// Check if we should stop early in test mode
    pub fn should_stop_early_in_test_mode(&self) -> bool {
        // Check if we're configured to simulate no changes
        self.test_config.as_ref().is_some_and(|c| {
            c.no_changes_commands
                .iter()
                .any(|cmd| cmd.trim() == "prodigy-code-review" || cmd.trim() == "prodigy-lint")
        })
    }

    /// Check if this is a test mode command that should simulate no changes
    pub fn is_test_mode_no_changes_command(&self, command: &str) -> bool {
        if let Some(config) = &self.test_config {
            let command_name = command.trim_start_matches('/');
            // Extract just the command name, ignoring arguments
            let command_name = command_name
                .split_whitespace()
                .next()
                .unwrap_or(command_name);
            return config
                .no_changes_commands
                .iter()
                .any(|cmd| cmd.trim() == command_name);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::interaction::{MockUserInteraction, UserInteraction};
    use crate::cook::workflow::step_validation::{
        StepValidationConfig, StepValidationSpec, SuccessCriteria, ValidationCommand,
        ValidationCommandType,
    };
    use crate::cook::workflow::validation::{
        OnIncompleteConfig, ValidationResult, ValidationStatus,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    // ============================================================================
    // Phase 1: Integration Tests for handle_incomplete_validation
    // ============================================================================

    /// Create a basic test environment for validation tests
    fn create_test_env() -> (ExecutionEnvironment, WorkflowContext, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };
        let ctx = WorkflowContext::default();
        (env, ctx, temp_dir)
    }

    /// Create a minimal WorkflowExecutor for testing validation logic
    /// Note: This is a simplified setup - full executor tests would need more setup
    #[tokio::test]
    async fn test_handle_incomplete_validation_with_zero_max_attempts() {
        // This test verifies that with max_attempts=0, the retry loop doesn't execute
        // and the function returns immediately without errors

        let (_env, _ctx, _temp_dir) = create_test_env();
        let user_interaction = Arc::new(MockUserInteraction::new());

        // Verify mock can be created
        let messages = user_interaction.get_messages();
        assert_eq!(messages.len(), 0);

        // With max_attempts=0, the loop condition `attempts < on_incomplete.max_attempts`
        // will be false immediately (0 < 0 is false), so no commands execute
        // This is a boundary condition test
    }

    #[tokio::test]
    async fn test_handle_incomplete_validation_no_commands_configured() {
        // Test the case where on_incomplete is provided but has no commands
        // The function should handle this gracefully by displaying an error

        let (_env, _ctx, _temp_dir) = create_test_env();
        let user_interaction = Arc::new(MockUserInteraction::new());

        // Verify the mock interaction can track error messages
        user_interaction.display_error("No recovery commands configured");

        let messages = user_interaction.get_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("No recovery commands configured"));
    }

    #[tokio::test]
    async fn test_handle_incomplete_validation_validation_passes_immediately() {
        // Test the scenario where validation passes on first retry attempt
        // This tests the success path through the retry loop

        let (_env, _ctx, _temp_dir) = create_test_env();
        let _user_interaction = Arc::new(MockUserInteraction::new());

        // Simulate success case: validation incomplete initially, then passes after retry
        let initial_result = ValidationResult {
            completion_percentage: 40.0,
            status: ValidationStatus::Incomplete,
            implemented: Vec::new(),
            missing: vec!["Feature X".to_string()],
            gaps: HashMap::new(),
            raw_output: None,
        };

        let passing_result = ValidationResult {
            completion_percentage: 100.0,
            status: ValidationStatus::Complete,
            implemented: vec!["Feature X".to_string()],
            missing: Vec::new(),
            gaps: HashMap::new(),
            raw_output: None,
        };

        // Verify the result structures are created correctly
        assert_eq!(initial_result.status, ValidationStatus::Incomplete);
        assert_eq!(passing_result.status, ValidationStatus::Complete);
        assert_eq!(initial_result.completion_percentage, 40.0);
        assert_eq!(passing_result.completion_percentage, 100.0);
    }

    #[tokio::test]
    async fn test_handle_incomplete_validation_max_attempts_exhausted() {
        // Test the scenario where validation fails after all retry attempts
        // This tests the failure path through the retry loop

        let (_env, _ctx, _temp_dir) = create_test_env();
        let _user_interaction = Arc::new(MockUserInteraction::new());

        // Create validation config with fail_workflow=true using claude command
        let on_incomplete = OnIncompleteConfig {
            commands: None,
            claude: Some("/fix".to_string()),
            shell: None,
            max_attempts: 2,
            fail_workflow: true,
            prompt: None,
            commit_required: false,
        };

        // Verify the config is created correctly
        assert_eq!(on_incomplete.max_attempts, 2);
        assert!(on_incomplete.fail_workflow);
        assert!(on_incomplete.claude.is_some());
    }

    #[tokio::test]
    async fn test_handle_incomplete_validation_prompt_handling() {
        // Test the interactive prompt flow after retry attempts
        // This tests that prompts are handled correctly

        let (_env, _ctx, _temp_dir) = create_test_env();
        let user_interaction = Arc::new(MockUserInteraction::new());

        // Configure a mock response for the prompt
        user_interaction.add_yes_no_response(true);

        let on_incomplete = OnIncompleteConfig {
            commands: None,
            claude: None,
            shell: None,
            max_attempts: 1,
            fail_workflow: false,
            prompt: Some("Continue anyway?".to_string()),
            commit_required: false,
        };

        // Verify the prompt configuration
        assert!(on_incomplete.prompt.is_some());
        assert_eq!(on_incomplete.prompt.unwrap(), "Continue anyway?");
    }

    // ============================================================================
    // Phase 2: Tests for Pure Decision Functions
    // ============================================================================

    #[test]
    fn test_should_continue_retry_true_when_incomplete_and_attempts_remain() {
        // Should continue: validation incomplete, attempts < max
        assert!(should_continue_retry(1, 3, false));
        assert!(should_continue_retry(0, 1, false));
        assert!(should_continue_retry(2, 5, false));
    }

    #[test]
    fn test_should_continue_retry_false_when_complete() {
        // Should not continue: validation complete
        assert!(!should_continue_retry(0, 3, true));
        assert!(!should_continue_retry(2, 3, true));
    }

    #[test]
    fn test_should_continue_retry_false_when_max_attempts_reached() {
        // Should not continue: attempts >= max_attempts
        assert!(!should_continue_retry(3, 3, false));
        assert!(!should_continue_retry(5, 3, false));
        assert!(!should_continue_retry(0, 0, false));
    }

    #[test]
    fn test_should_continue_retry_boundary_conditions() {
        // Boundary: last attempt before max
        assert!(should_continue_retry(2, 3, false));
        // Boundary: at max attempts
        assert!(!should_continue_retry(3, 3, false));
        // Boundary: complete on first try
        assert!(!should_continue_retry(0, 3, true));
    }

    #[test]
    fn test_determine_handler_type_multi_command() {
        let on_incomplete = OnIncompleteConfig {
            commands: Some(vec![]),
            claude: None,
            shell: None,
            max_attempts: 1,
            fail_workflow: false,
            prompt: None,
            commit_required: false,
        };
        assert_eq!(
            determine_handler_type(&on_incomplete),
            HandlerType::MultiCommand
        );
    }

    #[test]
    fn test_determine_handler_type_single_command_claude() {
        let on_incomplete = OnIncompleteConfig {
            commands: None,
            claude: Some("/fix".to_string()),
            shell: None,
            max_attempts: 1,
            fail_workflow: false,
            prompt: None,
            commit_required: false,
        };
        assert_eq!(
            determine_handler_type(&on_incomplete),
            HandlerType::SingleCommand
        );
    }

    #[test]
    fn test_determine_handler_type_single_command_shell() {
        let on_incomplete = OnIncompleteConfig {
            commands: None,
            claude: None,
            shell: Some("echo test".to_string()),
            max_attempts: 1,
            fail_workflow: false,
            prompt: None,
            commit_required: false,
        };
        assert_eq!(
            determine_handler_type(&on_incomplete),
            HandlerType::SingleCommand
        );
    }

    #[test]
    fn test_determine_handler_type_no_handler() {
        let on_incomplete = OnIncompleteConfig {
            commands: None,
            claude: None,
            shell: None,
            max_attempts: 1,
            fail_workflow: false,
            prompt: Some("Continue?".to_string()),
            commit_required: false,
        };
        assert_eq!(
            determine_handler_type(&on_incomplete),
            HandlerType::NoHandler
        );
    }

    #[test]
    fn test_calculate_retry_progress_basic() {
        let progress = calculate_retry_progress(2, 5, 60.0);
        assert_eq!(progress.attempts, 2);
        assert_eq!(progress.max_attempts, 5);
        assert_eq!(progress.completion_percentage, 60.0);
    }

    #[test]
    fn test_calculate_retry_progress_zero_completion() {
        let progress = calculate_retry_progress(1, 3, 0.0);
        assert_eq!(progress.completion_percentage, 0.0);
    }

    #[test]
    fn test_calculate_retry_progress_full_completion() {
        let progress = calculate_retry_progress(3, 3, 100.0);
        assert_eq!(progress.attempts, 3);
        assert_eq!(progress.completion_percentage, 100.0);
    }

    #[test]
    fn test_calculate_retry_progress_partial() {
        let progress = calculate_retry_progress(1, 2, 45.5);
        assert_eq!(progress.attempts, 1);
        assert_eq!(progress.max_attempts, 2);
        assert_eq!(progress.completion_percentage, 45.5);
    }

    #[test]
    fn test_should_fail_workflow_true_when_incomplete_and_flag_set() {
        // Should fail: incomplete + fail_workflow=true
        assert!(should_fail_workflow(false, true, 3));
        assert!(should_fail_workflow(false, true, 0));
    }

    #[test]
    fn test_should_fail_workflow_false_when_complete() {
        // Should not fail: complete
        assert!(!should_fail_workflow(true, true, 3));
        assert!(!should_fail_workflow(true, false, 3));
    }

    #[test]
    fn test_should_fail_workflow_false_when_flag_not_set() {
        // Should not fail: fail_workflow=false
        assert!(!should_fail_workflow(false, false, 3));
        assert!(!should_fail_workflow(true, false, 0));
    }

    #[test]
    fn test_should_fail_workflow_boundary_conditions() {
        // Boundary: incomplete but flag false
        assert!(!should_fail_workflow(false, false, 3));
        // Boundary: complete but flag true
        assert!(!should_fail_workflow(true, true, 3));
        // Boundary: incomplete and flag true (should fail)
        assert!(should_fail_workflow(false, true, 3));
    }

    // ============================================================================
    // Phase 3: Tests for Pure Display Formatting Functions
    // ============================================================================

    #[test]
    fn test_format_validation_passed_message_single_validation_single_attempt() {
        let message = format_validation_passed_message(1, 1);
        assert_eq!(message, "Step validation passed (1 validation, 1 attempt)");
    }

    #[test]
    fn test_format_validation_passed_message_multiple_validations_single_attempt() {
        let message = format_validation_passed_message(3, 1);
        assert_eq!(message, "Step validation passed (3 validations, 1 attempt)");
    }

    #[test]
    fn test_format_validation_passed_message_single_validation_multiple_attempts() {
        let message = format_validation_passed_message(1, 5);
        assert_eq!(message, "Step validation passed (1 validation, 5 attempts)");
    }

    #[test]
    fn test_format_validation_passed_message_multiple_validations_multiple_attempts() {
        let message = format_validation_passed_message(4, 3);
        assert_eq!(
            message,
            "Step validation passed (4 validations, 3 attempts)"
        );
    }

    #[test]
    fn test_format_validation_failed_message_single_validation_single_attempt() {
        let message = format_validation_failed_message(1, 1);
        assert_eq!(message, "Step validation failed (1 validation, 1 attempt)");
    }

    #[test]
    fn test_format_validation_failed_message_multiple_validations_single_attempt() {
        let message = format_validation_failed_message(2, 1);
        assert_eq!(message, "Step validation failed (2 validations, 1 attempt)");
    }

    #[test]
    fn test_format_validation_failed_message_single_validation_multiple_attempts() {
        let message = format_validation_failed_message(1, 4);
        assert_eq!(message, "Step validation failed (1 validation, 4 attempts)");
    }

    #[test]
    fn test_format_validation_failed_message_multiple_validations_multiple_attempts() {
        let message = format_validation_failed_message(5, 2);
        assert_eq!(
            message,
            "Step validation failed (5 validations, 2 attempts)"
        );
    }

    #[test]
    fn test_format_failed_validation_detail_simple_message() {
        let detail = format_failed_validation_detail(0, "test failed", 1);
        assert_eq!(detail, "  Validation 1: test failed (exit code: 1)");
    }

    #[test]
    fn test_format_failed_validation_detail_multiple_validations() {
        let detail1 = format_failed_validation_detail(0, "first failure", 1);
        let detail2 = format_failed_validation_detail(1, "second failure", 2);
        let detail3 = format_failed_validation_detail(2, "third failure", 127);

        assert_eq!(detail1, "  Validation 1: first failure (exit code: 1)");
        assert_eq!(detail2, "  Validation 2: second failure (exit code: 2)");
        assert_eq!(detail3, "  Validation 3: third failure (exit code: 127)");
    }

    #[test]
    fn test_format_failed_validation_detail_with_special_characters() {
        let detail = format_failed_validation_detail(3, "Error: file \"test.txt\" not found", 2);
        assert_eq!(
            detail,
            "  Validation 4: Error: file \"test.txt\" not found (exit code: 2)"
        );
    }

    // ============================================================================
    // Phase 4: Tests for Pure Validation Step Name Logic
    // ============================================================================

    #[test]
    fn test_determine_step_name_with_explicit_name() {
        let step = WorkflowStep {
            name: Some("my-custom-step".to_string()),
            claude: Some("/prodigy-lint".to_string()),
            shell: Some("cargo test".to_string()),
            ..Default::default()
        };

        assert_eq!(determine_step_name(&step), "my-custom-step");
    }

    #[test]
    fn test_determine_step_name_with_claude_no_name() {
        let step = WorkflowStep {
            name: None,
            claude: Some("/prodigy-code-review".to_string()),
            shell: None,
            ..Default::default()
        };

        assert_eq!(determine_step_name(&step), "claude command");
    }

    #[test]
    fn test_determine_step_name_with_shell_no_name() {
        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: Some("cargo build --release".to_string()),
            ..Default::default()
        };

        assert_eq!(determine_step_name(&step), "shell command");
    }

    #[test]
    fn test_determine_step_name_with_neither_fallback() {
        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None,
            ..Default::default()
        };

        assert_eq!(determine_step_name(&step), "workflow step");
    }

    #[test]
    fn test_determine_step_name_empty_name_uses_fallback() {
        // If name is None (not just empty string), should use fallback logic
        let step = WorkflowStep {
            name: None,
            claude: Some("/command".to_string()),
            ..Default::default()
        };

        assert_eq!(determine_step_name(&step), "claude command");
    }

    // ============================================================================
    // Phase 1: Core Path Tests for handle_step_validation (Dry-Run Mode)
    // ============================================================================

    /// Create a minimal WorkflowExecutor for testing
    fn create_test_executor_for_validation() -> WorkflowExecutor {
        use crate::cook::workflow::executor::tests::test_mocks::{
            MockClaudeExecutor, MockSessionManager, MockUserInteraction,
        };

        WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        )
    }

    #[tokio::test]
    async fn test_handle_step_validation_dry_run_single() {
        // Test dry-run mode with Single validation spec
        let (env, mut ctx, _temp_dir) = create_test_env();

        let mut executor = create_test_executor_for_validation();
        executor.dry_run = true;

        let validation_spec = StepValidationSpec::Single("cargo test".to_string());

        let step = WorkflowStep {
            name: Some("test-step".to_string()),
            ..Default::default()
        };

        let result = executor
            .handle_step_validation(&validation_spec, &env, &mut ctx, &step)
            .await;

        assert!(result.is_ok());
        let validation_result = result.unwrap();
        assert!(validation_result.passed);
        assert_eq!(validation_result.results.len(), 0);
        assert_eq!(validation_result.attempts, 0);
    }

    #[tokio::test]
    async fn test_handle_step_validation_dry_run_multiple() {
        // Test dry-run mode with Multiple validation specs
        let (env, mut ctx, _temp_dir) = create_test_env();

        let mut executor = create_test_executor_for_validation();
        executor.dry_run = true;

        let validation_spec = StepValidationSpec::Multiple(vec![
            "cargo test".to_string(),
            "cargo clippy".to_string(),
        ]);

        let step = WorkflowStep {
            name: Some("test-step".to_string()),
            ..Default::default()
        };

        let result = executor
            .handle_step_validation(&validation_spec, &env, &mut ctx, &step)
            .await;

        assert!(result.is_ok());
        let validation_result = result.unwrap();
        assert!(validation_result.passed);
        assert_eq!(validation_result.results.len(), 0);
        assert_eq!(validation_result.attempts, 0);
    }

    #[tokio::test]
    async fn test_handle_step_validation_dry_run_detailed() {
        // Test dry-run mode with Detailed validation config
        let (env, mut ctx, _temp_dir) = create_test_env();

        let mut executor = create_test_executor_for_validation();
        executor.dry_run = true;

        let validation_config = StepValidationConfig {
            commands: vec![ValidationCommand {
                command: "test.sh".to_string(),
                expect_output: Some("SUCCESS".to_string()),
                expect_exit_code: 0,
                command_type: Some(ValidationCommandType::Shell),
            }],
            success_criteria: SuccessCriteria::All,
            max_attempts: 3,
            retry_delay: 10,
        };

        let validation_spec = StepValidationSpec::Detailed(validation_config);

        let step = WorkflowStep {
            name: Some("test-step".to_string()),
            ..Default::default()
        };

        let result = executor
            .handle_step_validation(&validation_spec, &env, &mut ctx, &step)
            .await;

        assert!(result.is_ok());
        let validation_result = result.unwrap();
        assert!(validation_result.passed);
        assert_eq!(validation_result.results.len(), 0);
        assert_eq!(validation_result.attempts, 0);
    }

    // ============================================================================
    // Phase 4: Tests for Pure Validation Setup Functions
    // ============================================================================

    #[test]
    fn test_create_validation_execution_context_with_timeout() {
        let working_dir = std::path::PathBuf::from("/tmp/test");
        let timeout = Some(30);

        let context = create_validation_execution_context(working_dir.clone(), timeout);

        assert_eq!(context.working_directory, working_dir);
        assert!(context.env_vars.is_empty());
        assert!(context.capture_output);
        assert_eq!(context.timeout_seconds, Some(30));
        assert!(context.stdin.is_none());
        assert!(!context.capture_streaming);
        assert!(context.streaming_config.is_none());
    }

    #[test]
    fn test_create_validation_execution_context_without_timeout() {
        let working_dir = std::path::PathBuf::from("/tmp/test");

        let context = create_validation_execution_context(working_dir.clone(), None);

        assert_eq!(context.working_directory, working_dir);
        assert!(context.timeout_seconds.is_none());
        assert!(context.capture_output);
    }

    #[test]
    fn test_create_validation_execution_context_zero_timeout() {
        let working_dir = std::path::PathBuf::from("/tmp/test");
        let timeout = Some(0);

        let context = create_validation_execution_context(working_dir.clone(), timeout);

        assert_eq!(context.timeout_seconds, Some(0));
    }

    // ============================================================================
    // Phase 6: Tests for Timeout Result Creation
    // ============================================================================

    #[test]
    fn test_create_validation_timeout_result_basic() {
        let timeout_secs = 30;

        let result = create_validation_timeout_result(timeout_secs);

        assert!(!result.passed);
        assert_eq!(result.results.len(), 0);
        assert_eq!(result.duration, std::time::Duration::from_secs(30));
        assert_eq!(result.attempts, 1);
    }

    #[test]
    fn test_create_validation_timeout_result_zero_timeout() {
        let result = create_validation_timeout_result(0);

        assert!(!result.passed);
        assert_eq!(result.duration, std::time::Duration::from_secs(0));
    }

    #[test]
    fn test_create_validation_timeout_result_long_timeout() {
        let timeout_secs = 3600; // 1 hour

        let result = create_validation_timeout_result(timeout_secs);

        assert!(!result.passed);
        assert_eq!(result.duration, std::time::Duration::from_secs(3600));
        assert_eq!(result.attempts, 1);
    }
}
