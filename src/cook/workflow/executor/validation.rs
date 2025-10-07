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

        while attempts < on_incomplete.max_attempts
            && !validation_config.is_complete(&current_result)
        {
            attempts += 1;

            self.user_interaction.display_info(&format!(
                "Attempting to complete implementation (attempt {}/{})",
                attempts, on_incomplete.max_attempts
            ));

            // Execute the completion handler(s)
            let handler_success = if let Some(commands) = &on_incomplete.commands {
                // Execute array of commands
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
            } else if let Some(handler_step) = self.create_validation_handler(on_incomplete, ctx) {
                // Execute single command (legacy)
                let step_display = self.get_step_display_name(&handler_step);
                self.user_interaction
                    .display_progress(&format!("Running recovery step: {}", step_display));

                let handler_result = Box::pin(self.execute_step(&handler_step, env, ctx)).await?;
                handler_result.success
            } else {
                self.user_interaction
                    .display_error("No recovery commands configured");
                false
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
        if !validation_config.is_complete(&current_result) && on_incomplete.fail_workflow {
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
        let exec_context = ExecutionContext {
            working_directory: env.working_dir.to_path_buf(),
            env_vars: std::collections::HashMap::new(),
            capture_output: true,
            timeout_seconds: step.validation_timeout,
            stdin: None,
            capture_streaming: false,
            streaming_config: None,
        };

        // Get step name for logging
        let step_name = step.name.as_deref().unwrap_or_else(|| {
            if step.claude.is_some() {
                "claude command"
            } else if step.shell.is_some() {
                "shell command"
            } else {
                "workflow step"
            }
        });

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
                    super::super::step_validation::StepValidationResult {
                        passed: false,
                        results: vec![],
                        duration: std::time::Duration::from_secs(timeout_secs),
                        attempts: 1,
                    }
                }
            }
        } else {
            validation_future.await?
        };

        // Display validation result
        if validation_result.passed {
            self.user_interaction.display_success(&format!(
                "Step validation passed ({} validation{}, {} attempt{})",
                validation_result.results.len(),
                if validation_result.results.len() == 1 {
                    ""
                } else {
                    "s"
                },
                validation_result.attempts,
                if validation_result.attempts == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        } else {
            self.user_interaction.display_warning(&format!(
                "Step validation failed ({} validation{}, {} attempt{})",
                validation_result.results.len(),
                if validation_result.results.len() == 1 {
                    ""
                } else {
                    "s"
                },
                validation_result.attempts,
                if validation_result.attempts == 1 {
                    ""
                } else {
                    "s"
                }
            ));

            // Show details of failed validations
            for (idx, result) in validation_result.results.iter().enumerate() {
                if !result.passed {
                    self.user_interaction.display_info(&format!(
                        "  Validation {}: {} (exit code: {})",
                        idx + 1,
                        result.message,
                        result.exit_code
                    ));
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
            let mut env_vars = HashMap::new();
            // Enable streaming for validation commands
            if std::env::var("PRODIGY_CLAUDE_STREAMING").unwrap_or_else(|_| "true".to_string())
                == "true"
            {
                env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
            }
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
