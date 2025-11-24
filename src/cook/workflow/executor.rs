//! Workflow executor with commit verification
//!
//! Executes workflow steps in sequence, verifies git commits when required,
//! and manages iteration logic for continuous improvement sessions.
//!
//! ## Module Organization
//!
//! The executor is organized into focused submodules:
//!
//! - [`data_structures`]: Core data types (WorkflowStep, ExtendedWorkflowConfig, etc.)
//! - [`pure`]: Pure functions for validation, formatting, and computation
//! - [`context`]: Workflow context and variable management
//! - [`validation`]: Validation logic and conditional execution
//! - [`commands`]: Command execution (shell, claude, test, etc.)
//! - [`step_executor`]: Step-level execution orchestration
//! - [`orchestration`]: High-level workflow orchestration
//! - [`builder`]: Workflow executor builder pattern
//! - [`types`]: Type definitions and utilities
//! - [`failure_handler`]: Failure handling and recovery
//!
//! This organization separates concerns and makes the codebase easier to maintain and test.

#[path = "executor/builder.rs"]
mod builder;
#[path = "executor/commands.rs"]
pub(crate) mod commands;
#[path = "executor/commit_handler.rs"]
mod commit_handler;
#[path = "executor/context.rs"]
mod context;
#[path = "executor/data_structures.rs"]
mod data_structures;
#[path = "executor/failure_handler.rs"]
mod failure_handler;
#[path = "executor/git_support.rs"]
mod git_support;
#[path = "executor/orchestration.rs"]
mod orchestration;
#[path = "executor/pure.rs"]
mod pure;
#[path = "executor/retry_logic.rs"]
mod retry_logic;
#[path = "executor/specialized_commands.rs"]
pub(crate) mod specialized_commands;
#[path = "executor/step_executor.rs"]
mod step_executor;
#[path = "executor/types.rs"]
mod types;
#[path = "executor/validation.rs"]
mod validation;
#[cfg(test)]
#[path = "executor/validation_tests.rs"]
mod validation_tests;

use crate::abstractions::git::GitOperations;
#[cfg(test)]
use crate::commands::AttributeValue;
use crate::commands::CommandRegistry;
use crate::cook::execution::interpolation::InterpolationContext;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::retry_state::RetryStateManager;
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::cook::workflow::checkpoint::{
    self, CheckpointManager, CompletedStep as CheckpointCompletedStep, ResumeContext,
};
use crate::cook::workflow::normalized;
use crate::cook::workflow::normalized::NormalizedWorkflow;
use crate::cook::workflow::on_failure::OnFailureConfig;
use crate::testing::config::TestConfiguration;
use crate::unified_session::{format_duration, TimingTracker};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

// Pre-compiled regexes for variable interpolation
static BRACED_VAR_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$\{([^}]+)\}").expect("Failed to compile braced variable regex"));

static UNBRACED_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("Failed to compile unbraced variable regex")
});

// Re-export pure types for internal use
use pure::IterationContinuation;

// Re-export core types from types and context modules
pub use context::WorkflowContext;
pub use types::{CaptureOutput, CommandType, StepResult, VariableResolution};

// Re-export data structures for backward compatibility
pub use data_structures::{
    ExtendedWorkflowConfig, HandlerStep, SensitivePatternConfig, WorkflowMode, WorkflowStep,
};

/// Executes workflow steps with commit verification
pub struct WorkflowExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    timing_tracker: TimingTracker,
    test_config: Option<Arc<TestConfiguration>>,
    command_registry: Option<CommandRegistry>,
    subprocess: crate::subprocess::SubprocessManager,
    sensitive_config: SensitivePatternConfig,
    /// Track completed steps for resume functionality
    completed_steps: Vec<crate::cook::session::StepResult>,
    /// Checkpoint manager for workflow resumption
    checkpoint_manager: Option<Arc<CheckpointManager>>,
    /// Workflow ID for checkpoint tracking
    workflow_id: Option<String>,
    /// Checkpoint completed steps (separate from session steps)
    checkpoint_completed_steps: Vec<CheckpointCompletedStep>,
    /// Environment manager for workflow execution
    environment_manager: Option<crate::cook::environment::EnvironmentManager>,
    /// Global environment configuration
    global_environment_config: Option<crate::cook::environment::EnvironmentConfig>,
    /// Current workflow being executed (for checkpoint context)
    current_workflow: Option<NormalizedWorkflow>,
    /// Current step index being executed (for checkpoint context)
    current_step_index: Option<usize>,
    /// Git operations abstraction for testing
    git_operations: Arc<dyn GitOperations>,
    /// Resume context for handling interrupted workflows with error recovery state
    resume_context: Option<ResumeContext>,
    /// Retry state manager for checkpoint persistence
    retry_state_manager: Arc<RetryStateManager>,
    /// Dry-run mode - preview commands without executing
    dry_run: bool,
    /// Track assumed commits during dry-run for validation
    assumed_commits: Vec<String>,
    /// Path to the workflow file being executed (for checkpoint resume)
    workflow_path: Option<PathBuf>,
    /// Track dry-run commands that would be executed
    dry_run_commands: Vec<String>,
    /// Track dry-run validation commands
    dry_run_validations: Vec<String>,
    /// Track potential failure handlers in dry-run
    dry_run_potential_handlers: Vec<String>,
    /// Positional arguments passed via --args (Spec 163)
    positional_args: Option<Vec<String>>,
}

impl WorkflowExecutor {
    /// Handle on_failure configuration with retry logic
    async fn handle_on_failure(
        &mut self,
        step: &WorkflowStep,
        mut result: StepResult,
        on_failure_config: &OnFailureConfig,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // 1. Inject error context
        let step_name = self.get_step_display_name(step);
        let error_vars = failure_handler::create_error_context_variables(
            &result.stderr,
            result.exit_code,
            &step_name,
        );
        for (key, value) in error_vars {
            ctx.variables.insert(key, value);
        }

        // 2. Execute handler (new or legacy)
        let handler_commands = on_failure_config.handler_commands();
        if !handler_commands.is_empty() {
            result = self
                .handle_new_style_failure(
                    step,
                    result,
                    on_failure_config,
                    &handler_commands,
                    env,
                    ctx,
                )
                .await?;
        } else if let Some(handler) = on_failure_config.handler() {
            result = self
                .handle_legacy_failure(step, result, on_failure_config, &handler, env, ctx)
                .await?;
        }

        // 3. Cleanup error context
        for key in failure_handler::get_error_context_keys() {
            ctx.variables.remove(key);
        }

        Ok(result)
    }

    /// Execute handler commands in sequence
    ///
    /// Returns (success, outputs) tuple indicating if all handlers succeeded and their outputs.
    async fn execute_handler_commands(
        &mut self,
        handler_commands: &[crate::cook::workflow::on_failure::HandlerCommand],
        timeout: Option<u64>,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<(bool, Vec<String>)> {
        let mut handler_success = true;
        let mut handler_outputs = Vec::new();

        for (idx, cmd) in handler_commands.iter().enumerate() {
            self.user_interaction.display_progress(&format!(
                "Handler command {}/{}",
                idx + 1,
                handler_commands.len()
            ));

            let handler_step = failure_handler::create_handler_step(cmd, timeout);

            match Box::pin(self.execute_step(&handler_step, env, ctx)).await {
                Ok(handler_result) => {
                    handler_outputs.push(handler_result.stdout.clone());
                    if !handler_result.success && !cmd.continue_on_error {
                        handler_success = false;
                        self.user_interaction
                            .display_error(&format!("Handler command {} failed", idx + 1));
                        break;
                    }
                }
                Err(e) => {
                    self.user_interaction.display_error(&format!(
                        "Handler command {} error: {}",
                        idx + 1,
                        e
                    ));
                    if !cmd.continue_on_error {
                        handler_success = false;
                        break;
                    }
                }
            }
        }

        Ok((handler_success, handler_outputs))
    }

    /// Handle failure with new-style handler commands
    async fn handle_new_style_failure(
        &mut self,
        step: &WorkflowStep,
        mut result: StepResult,
        on_failure_config: &OnFailureConfig,
        handler_commands: &[crate::cook::workflow::on_failure::HandlerCommand],
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        let strategy = on_failure_config.strategy();
        self.user_interaction.display_info(&format!(
            "Executing on_failure handler ({:?} strategy)...",
            strategy
        ));

        // Execute handler commands
        let (handler_success, handler_outputs) = self
            .execute_handler_commands(
                handler_commands,
                on_failure_config.handler_timeout(),
                env,
                ctx,
            )
            .await?;

        // Add handler output to result
        result = failure_handler::append_handler_output(result, &handler_outputs);

        // Create handler result for strategy determination
        let handler_result = failure_handler::FailureHandlerResult {
            success: handler_success,
            outputs: handler_outputs,
            recovered: false,
        };

        // Check if step should be marked as recovered
        if failure_handler::determine_recovery_strategy(&handler_result, strategy) {
            self.user_interaction
                .display_success("Step recovered through on_failure handler");
            result = failure_handler::mark_step_recovered(result);
        }

        // Check if handler failure should be fatal
        if failure_handler::is_handler_failure_fatal(handler_success, on_failure_config) {
            return Err(anyhow!("Handler failure is fatal"));
        }

        // Check if we should retry the original command
        if failure_handler::should_retry_after_handler(on_failure_config, result.success) {
            let max_retries = failure_handler::get_handler_max_retries(on_failure_config);
            if let Some(retry_result) = self
                .retry_original_command(step, max_retries, env, ctx)
                .await?
            {
                result = retry_result;
            }
        }

        Ok(result)
    }

    /// Handle failure with legacy handler
    async fn handle_legacy_failure(
        &mut self,
        step: &WorkflowStep,
        mut result: StepResult,
        on_failure_config: &OnFailureConfig,
        handler: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        self.user_interaction
            .display_info("Executing on_failure handler...");
        let failure_result = Box::pin(self.execute_step(handler, env, ctx)).await?;
        result = failure_handler::append_handler_output(
            result,
            std::slice::from_ref(&failure_result.stdout),
        );

        // Check if we should retry the original command
        if failure_handler::should_retry_after_handler(on_failure_config, result.success) {
            let max_retries = failure_handler::get_handler_max_retries(on_failure_config);
            if let Some(retry_result) = self
                .retry_original_command(step, max_retries, env, ctx)
                .await?
            {
                result = retry_result;
            }
        }

        Ok(result)
    }

    /// Retry the original command after handler execution
    ///
    /// Returns Some(StepResult) if retry succeeds, None if all retries fail.
    async fn retry_original_command(
        &mut self,
        step: &WorkflowStep,
        max_retries: u32,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<Option<StepResult>> {
        for retry in 1..=max_retries {
            self.user_interaction.display_info(&format!(
                "Retrying original command (attempt {}/{})",
                retry, max_retries
            ));

            // Create a copy of the step without on_failure to avoid recursion
            let mut retry_step = step.clone();
            retry_step.on_failure = None;

            let retry_result = Box::pin(self.execute_step(&retry_step, env, ctx)).await?;
            if retry_result.success {
                return Ok(Some(retry_result));
            }
        }
        Ok(None)
    }

    /// Determine command type from a workflow step
    pub(crate) fn determine_command_type(&self, step: &WorkflowStep) -> Result<CommandType> {
        // Use pure function to count and validate
        let count = pure::count_specified_commands(step);
        pure::validate_single_command_type(count)?;

        // Determine command type based on which field is set
        self.extract_command_type(step)
    }

    /// Extract the command type from a validated step
    fn extract_command_type(&self, step: &WorkflowStep) -> Result<CommandType> {
        if let Some(handler_step) = &step.handler {
            self.build_handler_command_type(handler_step)
        } else if let Some(claude_cmd) = &step.claude {
            Ok(CommandType::Claude(claude_cmd.clone()))
        } else if let Some(shell_cmd) = &step.shell {
            Ok(CommandType::Shell(shell_cmd.clone()))
        } else if let Some(test_cmd) = &step.test {
            Ok(CommandType::Test(test_cmd.clone()))
        } else if let Some(goal_seek_config) = &step.goal_seek {
            Ok(CommandType::GoalSeek(goal_seek_config.clone()))
        } else if let Some(foreach_config) = &step.foreach {
            Ok(CommandType::Foreach(foreach_config.clone()))
        } else if let Some(write_file_config) = &step.write_file {
            Ok(CommandType::WriteFile(write_file_config.clone()))
        } else if let Some(name) = &step.name {
            Ok(CommandType::Legacy(pure::normalize_legacy_command(name)))
        } else if let Some(command) = &step.command {
            Ok(CommandType::Legacy(command.clone()))
        } else {
            Err(anyhow!("No valid command found in step"))
        }
    }

    /// Build handler command type with converted attributes
    fn build_handler_command_type(
        &self,
        handler_step: &crate::cook::workflow::HandlerStep,
    ) -> Result<CommandType> {
        let mut attributes = HashMap::new();
        for (key, value) in &handler_step.attributes {
            attributes.insert(key.clone(), self.json_to_attribute_value(value.clone()));
        }
        Ok(CommandType::Handler {
            handler_name: handler_step.name.clone(),
            attributes,
        })
    }

    /// Get display name for a step
    pub(crate) fn get_step_display_name(&self, step: &WorkflowStep) -> String {
        if let Some(claude_cmd) = &step.claude {
            format!("claude: {claude_cmd}")
        } else if let Some(shell_cmd) = &step.shell {
            format!("shell: {shell_cmd}")
        } else if let Some(test_cmd) = &step.test {
            format!("test: {}", test_cmd.command)
        } else if let Some(handler_step) = &step.handler {
            format!("handler: {}", handler_step.name)
        } else if let Some(write_file_config) = &step.write_file {
            format!("write_file: {}", write_file_config.path)
        } else if let Some(name) = &step.name {
            name.clone()
        } else if let Some(command) = &step.command {
            command.clone()
        } else {
            "unnamed step".to_string()
        }
    }

    /// Save workflow state for checkpoint and session tracking
    async fn save_workflow_state(
        &mut self,
        env: &ExecutionEnvironment,
        iteration: usize,
        step_index: usize,
    ) -> Result<()> {
        let workflow_path = self
            .workflow_path
            .clone()
            .unwrap_or_else(|| env.working_dir.join("workflow.yml"));

        let workflow_state = pure::build_workflow_state(
            iteration,
            step_index,
            self.completed_steps.clone(),
            workflow_path,
        );

        self.session_manager
            .update_session(SessionUpdate::UpdateWorkflowState(workflow_state))
            .await
    }

    /// Handle commit verification and auto-commit
    ///
    /// Verifies that commits were created after a step execution. If no commits
    /// were created, determines the appropriate action based on step configuration:
    /// - Create auto-commit if changes exist and auto_commit is enabled
    /// - Fail if commit_required is true
    /// - Continue silently otherwise
    ///
    /// The function uses extracted pure logic for decision-making, reducing
    /// cognitive complexity and improving testability.
    ///
    /// Returns `Ok(true)` if commits were created or auto-committed.
    async fn handle_commit_verification(
        &mut self,
        working_dir: &std::path::Path,
        head_before: &str,
        step: &WorkflowStep,
        step_display: &str,
        workflow_context: &mut WorkflowContext,
    ) -> Result<bool> {
        let head_after = self.get_current_head(working_dir).await?;
        let commit_handler = commit_handler::CommitHandler::new(
            Arc::clone(&self.git_operations),
            Arc::clone(&self.user_interaction),
        );

        if head_after == head_before {
            // No commits were created - determine action
            let has_changes = commit_handler.has_uncommitted_changes(working_dir).await;
            let action = pure::determine_no_commit_action(step, has_changes);

            match action {
                pure::CommitVerificationAction::CreateAutoCommit => {
                    let message = self.generate_commit_message(step, workflow_context);
                    let commit_created = self
                        .execute_auto_commit(
                            &commit_handler,
                            working_dir,
                            &message,
                            step_display,
                            step,
                        )
                        .await?;
                    return Ok(commit_created);
                }
                pure::CommitVerificationAction::RequireCommitError => {
                    self.handle_no_commits_error(step)?;
                }
                pure::CommitVerificationAction::NoAction => {}
            }
            return Ok(false);
        }

        // Commits were created - verify and track
        let (_, commits) = commit_handler
            .verify_and_handle_commits(working_dir, head_before, &head_after, step_display)
            .await?;

        // Store commit info in context for later use
        workflow_context.variables.insert(
            "step.commits".to_string(),
            commits
                .iter()
                .map(|c| &c.hash)
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
        );

        Ok(true)
    }

    /// Execute auto-commit with error handling
    ///
    /// Attempts to create an auto-commit and handles failures appropriately.
    /// Returns Ok(true) if commit was created successfully.
    async fn execute_auto_commit(
        &mut self,
        commit_handler: &commit_handler::CommitHandler,
        working_dir: &std::path::Path,
        message: &str,
        step_display: &str,
        step: &WorkflowStep,
    ) -> Result<bool> {
        match commit_handler
            .create_auto_commit(working_dir, message, step_display)
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::warn!("Failed to create auto-commit: {}", e);
                if step.commit_required {
                    self.handle_no_commits_error(step)?;
                }
                Ok(false)
            }
        }
    }

    /// Handle commit squashing if enabled in workflow (delegated to commit_handler module)
    async fn handle_commit_squashing(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) {
        let commit_handler = commit_handler::CommitHandler::new(
            Arc::clone(&self.git_operations),
            Arc::clone(&self.user_interaction),
        );
        commit_handler.handle_commit_squashing(workflow, env).await;
    }

    /// Determine execution flags from environment variables (delegated to pure module)
    /// Get summary of available variables for debugging (delegated to pure module)
    fn get_available_variable_summary(context: &InterpolationContext) -> String {
        pure::get_available_variable_summary(context)
    }

    /// Determine if a step should be skipped (delegated to pure module)
    /// Determine if workflow should continue based on state (delegated to pure module)
    /// Execute workflow with checkpoint-on-error recovery
    ///
    /// Wraps workflow execution to ensure checkpoints are saved on both success and error paths.
    /// This enables graceful degradation and resume capability even when workflows fail.
    pub async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Initialize workflow context early for checkpoint saving
        let mut workflow_context = self.init_workflow_context(env);

        // Execute workflow and capture result
        let execution_result = self
            .execute_internal(workflow, env, &mut workflow_context)
            .await;

        // Save checkpoint based on execution result (success or failure)
        if let Some(ref checkpoint_manager) = self.checkpoint_manager {
            if let Some(ref workflow_id) = self.workflow_id {
                let workflow_hash =
                    orchestration::create_workflow_hash(&workflow.name, workflow.steps.len());
                let normalized_workflow =
                    orchestration::create_normalized_workflow(&workflow.name, &workflow_context);

                let checkpoint_result = match &execution_result {
                    Ok(_) => {
                        // Success: save completion checkpoint
                        let current_step_index =
                            self.current_step_index.unwrap_or(workflow.steps.len());
                        let checkpoint_create_result = checkpoint::create_completion_checkpoint(
                            workflow_id.clone(),
                            &normalized_workflow,
                            &workflow_context,
                            self.checkpoint_completed_steps.clone(),
                            current_step_index,
                            workflow_hash,
                        )
                        .map(|mut cp| {
                            // Set workflow path if available
                            if let Some(ref path) = self.workflow_path {
                                cp.workflow_path = Some(path.clone());
                            }
                            cp
                        });

                        // I/O operation: save to disk
                        match checkpoint_create_result {
                            Ok(cp) => checkpoint_manager.save_checkpoint(&cp).await,
                            Err(e) => Err(e),
                        }
                    }
                    Err(error) => {
                        // Failure: save error recovery checkpoint
                        let failed_step_index = self.current_step_index.unwrap_or(0);
                        let checkpoint_create_result = checkpoint::create_error_checkpoint(
                            workflow_id.clone(),
                            &normalized_workflow,
                            &workflow_context,
                            self.checkpoint_completed_steps.clone(),
                            workflow_hash,
                            error,
                            failed_step_index,
                        )
                        .map(|mut cp| {
                            // Set workflow path if available
                            if let Some(ref path) = self.workflow_path {
                                cp.workflow_path = Some(path.clone());
                            }
                            cp
                        });

                        // I/O operation: save to disk
                        let checkpoint_save_result = match checkpoint_create_result {
                            Ok(cp) => {
                                // Save checkpoint to disk
                                let save_result = checkpoint_manager.save_checkpoint(&cp).await;

                                // FIX: Update SessionManager's workflow_state so is_resumable() works correctly
                                // This ensures the in-memory session state is synchronized with the disk checkpoint
                                if save_result.is_ok() {
                                    let workflow_state = crate::cook::session::WorkflowState {
                                        current_iteration: 0,
                                        current_step: failed_step_index,
                                        completed_steps: self.completed_steps.clone(),
                                        // Use actual workflow path from executor, not hardcoded "workflow.yml"
                                        workflow_path: self.workflow_path.clone().unwrap_or_else(
                                            || env.working_dir.join("workflow.yml"),
                                        ),
                                        input_args: Vec::new(),
                                        map_patterns: Vec::new(),
                                        using_worktree: true,
                                    };

                                    // Update session manager (ignore errors to not mask the original failure)
                                    if let Err(e) = self.session_manager
                                        .update_session(crate::cook::session::SessionUpdate::UpdateWorkflowState(workflow_state))
                                        .await
                                    {
                                        tracing::warn!(
                                            "Failed to update session workflow_state after error checkpoint: {}",
                                            e
                                        );
                                    }
                                }

                                save_result
                            }
                            Err(e) => Err(e),
                        };

                        checkpoint_save_result
                    }
                };

                // Log checkpoint errors but don't fail the workflow
                if let Err(checkpoint_err) = checkpoint_result {
                    tracing::error!(
                        "Failed to save checkpoint for workflow {}: {}",
                        workflow_id,
                        checkpoint_err
                    );
                }
            }
        }

        // Return original execution result
        execution_result
    }

    /// Execute workflow steps in a single iteration
    async fn execute_workflow_iteration(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
        workflow_context: &mut WorkflowContext,
        execution_flags: &pure::ExecutionFlags,
    ) -> Result<bool> {
        let mut any_changes = false;

        for (step_index, step) in workflow.steps.iter().enumerate() {
            // Check if we should skip this step
            if pure::should_skip_step_execution(step_index, &self.completed_steps) {
                let skip_msg = orchestration::format_skip_step(
                    step_index,
                    workflow.steps.len(),
                    &self.get_step_display_name(step),
                );
                self.user_interaction.display_info(&skip_msg);
                continue;
            }

            // Restore error recovery state if needed
            self.restore_error_recovery_state(step_index, workflow_context);

            // Execute single step and update changes flag
            let step_had_commits = self
                .execute_step_with_tracking(
                    step,
                    step_index,
                    workflow,
                    env,
                    workflow_context,
                    execution_flags,
                )
                .await?;

            any_changes = step_had_commits || any_changes;
        }

        Ok(any_changes)
    }

    /// Execute a single workflow step with full tracking (internal helper)
    async fn execute_step_with_tracking(
        &mut self,
        step: &WorkflowStep,
        step_index: usize,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
        workflow_context: &mut WorkflowContext,
        execution_flags: &pure::ExecutionFlags,
    ) -> Result<bool> {
        self.current_step_index = Some(step_index);

        let step_display = self.get_step_display_name(step);
        let step_msg =
            orchestration::format_step_progress(step_index, workflow.steps.len(), &step_display);
        self.user_interaction.display_progress(&step_msg);

        // Get HEAD before command execution if needed
        let head_before = if !execution_flags.skip_validation
            && step.commit_required
            && !execution_flags.test_mode
        {
            Some(self.get_current_head(&env.working_dir).await?)
        } else {
            None
        };

        // Start timing
        self.timing_tracker.start_command(step_display.clone());
        let command_start = Instant::now();
        let step_started_at = chrono::Utc::now();

        // Execute the step
        let step_result = self.execute_step(step, env, workflow_context).await?;

        // Display output
        self.log_step_output(&step_result);

        // Complete timing
        let command_duration = command_start.elapsed();
        let step_completed_at = chrono::Utc::now();
        if let Some((cmd_name, _)) = self.timing_tracker.complete_command() {
            self.session_manager
                .update_session(SessionUpdate::RecordCommandTiming(
                    cmd_name.clone(),
                    command_duration,
                ))
                .await?;
        }

        // Track completed steps
        let completed_step = orchestration::build_session_step_result(
            step_index,
            step_display.clone(),
            step,
            &step_result,
            command_duration,
            step_started_at,
            step_completed_at,
        );
        self.completed_steps.push(completed_step.clone());

        let checkpoint_step = orchestration::build_checkpoint_step(
            step_index,
            step_display.clone(),
            step,
            &step_result,
            workflow_context,
            command_duration,
            step_completed_at,
        );
        self.checkpoint_completed_steps.push(checkpoint_step);

        // Save checkpoint if available
        self.save_step_checkpoint(workflow, workflow_context, step_index)
            .await;

        // Save workflow state
        self.save_workflow_state(env, 1, step_index).await?;

        // Check for commits if required
        let had_commits = if !self.dry_run {
            if let Some(before) = head_before {
                self.handle_commit_verification(
                    &env.working_dir,
                    &before,
                    step,
                    &step_display,
                    workflow_context,
                )
                .await?
            } else {
                false
            }
        } else {
            false
        };

        Ok(had_commits)
    }

    /// Save step checkpoint if manager is available
    async fn save_step_checkpoint(
        &self,
        workflow: &ExtendedWorkflowConfig,
        workflow_context: &WorkflowContext,
        step_index: usize,
    ) {
        if let Some(ref checkpoint_manager) = self.checkpoint_manager {
            if let Some(ref workflow_id) = self.workflow_id {
                let workflow_hash =
                    orchestration::create_workflow_hash(&workflow.name, workflow.steps.len());
                let normalized_workflow =
                    orchestration::create_normalized_workflow(&workflow.name, workflow_context);

                let mut checkpoint = checkpoint::create_checkpoint_with_total_steps(
                    workflow_id.clone(),
                    &normalized_workflow,
                    workflow_context,
                    self.checkpoint_completed_steps.clone(),
                    step_index + 1,
                    workflow_hash,
                    workflow.steps.len(),
                );

                if let Some(ref path) = self.workflow_path {
                    checkpoint.workflow_path = Some(path.clone());
                }

                if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                    tracing::warn!("Failed to save checkpoint: {}", e);
                }
            }
        }
    }

    /// Internal execution implementation (private)
    async fn execute_internal(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
        workflow_context: &mut WorkflowContext,
    ) -> Result<()> {
        // Handle MapReduce mode
        if workflow.mode == WorkflowMode::MapReduce {
            return self.execute_mapreduce(workflow, env).await;
        }

        // Validate workflow configuration
        pure::validate_workflow_config(workflow)?;

        let workflow_start = Instant::now();
        let execution_flags = pure::determine_execution_flags();

        // Display dry-run mode message
        self.display_dry_run_info(workflow);

        // Calculate effective max iterations
        let effective_max_iterations =
            pure::calculate_effective_max_iterations(workflow, self.dry_run);

        // Only show workflow info for non-empty workflows
        if !workflow.steps.is_empty() {
            let start_msg =
                orchestration::format_workflow_start(&workflow.name, effective_max_iterations);
            self.user_interaction.display_info(&start_msg);
        }

        if workflow.iterate {
            self.user_interaction
                .display_progress("Starting improvement loop");
        }

        let mut iteration = 0;
        let mut should_continue = true;
        let mut any_changes = false;

        // Clear completed steps at the start of a new workflow
        self.completed_steps.clear();

        // Start workflow timing in session
        self.session_manager
            .update_session(SessionUpdate::StartWorkflow)
            .await?;

        while should_continue && iteration < effective_max_iterations {
            iteration += 1;

            // Clear completed steps at the start of each iteration
            self.completed_steps.clear();

            // Update iteration context
            let iteration_vars = pure::build_iteration_context(iteration);
            workflow_context.iteration_vars.extend(iteration_vars);

            let iteration_msg =
                orchestration::format_iteration_progress(iteration, effective_max_iterations);
            self.user_interaction.display_progress(&iteration_msg);

            // Start iteration timing
            self.timing_tracker.start_iteration();

            // Update session (skip in dry-run mode)
            if !self.dry_run {
                self.session_manager
                    .update_session(SessionUpdate::IncrementIteration)
                    .await?;
                self.session_manager
                    .update_session(SessionUpdate::StartIteration(iteration))
                    .await?;
            }

            // Execute all workflow steps
            let iteration_had_changes = self
                .execute_workflow_iteration(workflow, env, workflow_context, &execution_flags)
                .await?;

            any_changes = iteration_had_changes || any_changes;

            // Determine continuation using pure function
            let continuation = pure::determine_iteration_continuation(
                workflow,
                iteration,
                effective_max_iterations,
                any_changes,
                &execution_flags,
                self.is_focus_tracking_test(),
                self.should_stop_early_in_test_mode(),
            );

            should_continue = match continuation {
                IterationContinuation::Stop(reason) => {
                    self.user_interaction
                        .display_info(&format!("Stopping: {}", reason));
                    false
                }
                IterationContinuation::Continue => true,
                IterationContinuation::ContinueToMax => iteration < effective_max_iterations,
                IterationContinuation::AskUser => self.should_continue_iterations(env).await?,
            };

            // Complete iteration timing
            if let Some(iteration_duration) = self.timing_tracker.complete_iteration() {
                self.session_manager
                    .update_session(SessionUpdate::CompleteIteration)
                    .await?;

                self.user_interaction.display_success(&format!(
                    "Iteration {} completed in {}",
                    iteration,
                    format_duration(iteration_duration)
                ));
            }
        }

        // Metrics collection removed in v0.3.0

        // Handle commit squashing if enabled
        if any_changes {
            self.handle_commit_squashing(workflow, env).await;
        }

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total workflow time",
            &format!(
                "{} across {} iteration{}",
                format_duration(total_duration),
                iteration,
                if iteration == 1 { "" } else { "s" }
            ),
        );

        // Display dry-run summary if applicable
        self.display_dry_run_summary();

        Ok(())
    }

    /// Prepare environment variables for step execution
    /// Safely format environment variable value for logging (delegated to pure module)
    fn format_env_var_for_logging(key: &str, value: &str) -> String {
        pure::format_env_var_for_logging(key, value)
    }

    /// Format variable value for logging (delegated to pure module)
    fn format_variable_for_logging(value: &str) -> String {
        pure::format_variable_for_logging(value)
    }

    /// Determine if commit is required and validate (delegated to pure module)
    #[allow(clippy::too_many_arguments)]
    fn validate_commit_requirement(
        step: &WorkflowStep,
        tracked_commits_empty: bool,
        head_before: &str,
        head_after: &str,
        dry_run: bool,
        step_name: &str,
        assumed_commits: &[String],
        json_log_location: Option<&str>,
    ) -> Result<()> {
        pure::validate_commit_requirement(
            step,
            tracked_commits_empty,
            head_before,
            head_after,
            dry_run,
            step_name,
            assumed_commits,
            json_log_location,
        )
    }

    /// Build step commit variables (delegated to pure module)
    fn build_commit_variables(
        tracked_commits: &[crate::cook::commit_tracker::TrackedCommit],
    ) -> Result<HashMap<String, String>> {
        pure::build_commit_variables(tracked_commits)
    }

    /// Determine if workflow should fail based on step result (delegated to pure module)
    /// Build error message for failed step (delegated to pure module)
    /// Set up environment context for step execution
    async fn setup_step_environment_context(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<(
        HashMap<String, String>,
        Option<PathBuf>,
        ExecutionEnvironment,
    )> {
        // Set up environment for this step
        let (env_vars, working_dir_override) =
            if let Some(ref mut env_manager) = self.environment_manager {
                // Use environment manager to set up step environment
                let env_context = env_manager
                    .setup_step_environment(
                        step,
                        self.global_environment_config.as_ref(),
                        &ctx.variables,
                    )
                    .await?;

                // Only override working directory if step explicitly set it
                // This respects ExecutionEnvironment (e.g., MapReduce worktrees)
                // while allowing legitimate per-step directory overrides
                let working_dir_override =
                    if step.working_dir.is_some() && env_context.working_dir != **env.working_dir {
                        Some(env_context.working_dir.clone())
                    } else {
                        None
                    };

                (env_context.env, working_dir_override)
            } else {
                // Fall back to traditional environment preparation
                let env_vars = self.prepare_env_vars(step, env, ctx);
                let working_dir_override = step.working_dir.clone();
                (env_vars, working_dir_override)
            };

        // Update execution environment if working directory is overridden
        let mut actual_env = env.clone();
        if let Some(ref dir) = working_dir_override {
            actual_env.working_dir = Arc::new(dir.clone());
            tracing::info!("Working directory overridden to: {}", dir.display());
        }

        // Log environment variables being set
        if !env_vars.is_empty() {
            tracing::debug!("Environment Variables:");
            for (key, value) in &env_vars {
                let display_value = Self::format_env_var_for_logging(key, value);
                tracing::debug!("  {} = {}", key, display_value);
            }
        }

        tracing::debug!(
            "Actual execution directory: {}",
            actual_env.working_dir.display()
        );

        Ok((env_vars, working_dir_override, actual_env))
    }

    /// Validate commit requirements and display dry-run information if applicable
    fn validate_and_display_commit_info(
        &self,
        step: &WorkflowStep,
        tracked_commits: &[crate::cook::commit_tracker::TrackedCommit],
        before_head: &str,
        after_head: &str,
        json_log_location: Option<&str>,
    ) -> Result<()> {
        let step_name = self.get_step_display_name(step);

        // Validate commit requirements using pure function
        Self::validate_commit_requirement(
            step,
            tracked_commits.is_empty(),
            before_head,
            after_head,
            self.dry_run,
            &step_name,
            &self.assumed_commits,
            json_log_location,
        )?;

        // Handle dry run commit assumption display
        if self.dry_run && tracked_commits.is_empty() && after_head == before_head {
            let command_desc = if let Some(ref cmd) = step.claude {
                format!("claude: {}", cmd)
            } else if let Some(ref cmd) = step.shell {
                format!("shell: {}", cmd)
            } else if let Some(ref cmd) = step.command {
                format!("command: {}", cmd)
            } else {
                step_name.clone()
            };

            if self
                .assumed_commits
                .iter()
                .any(|c| c.contains(&command_desc))
            {
                println!(
                    "[DRY RUN] Skipping commit validation - assumed commit from: {}",
                    step_name
                );
            }
        }

        Ok(())
    }

    /// Handle legacy capture_output feature (deprecated)
    fn handle_legacy_capture(
        &self,
        step: &WorkflowStep,
        command_type: &CommandType,
        result: &StepResult,
        ctx: &mut WorkflowContext,
    ) {
        if step.capture_output.is_enabled() {
            // Get the variable name for this output (custom or default)
            if let Some(var_name) = step.capture_output.get_variable_name(command_type) {
                // Store with the specified variable name
                ctx.captured_outputs.insert(var_name, result.stdout.clone());
            }

            // Also store as generic CAPTURED_OUTPUT for backward compatibility
            ctx.captured_outputs
                .insert("CAPTURED_OUTPUT".to_string(), result.stdout.clone());
        }
    }

    /// Get current git HEAD (delegated to git_support module)
    async fn get_current_head(&self, working_dir: &std::path::Path) -> Result<String> {
        let helper = git_support::GitOperationsHelper::new(Arc::clone(&self.git_operations));
        helper.get_current_head(working_dir).await
    }

    /// Handle the case where no commits were created when expected
    pub(crate) fn handle_no_commits_error(&self, step: &WorkflowStep) -> Result<()> {
        let step_display = self.get_step_display_name(step);
        let command_type = self.determine_command_type(step)?;
        let command_name = pure::extract_command_name(&command_type);

        let error_message = pure::build_no_commits_error_message(command_name, &step_display);
        eprint!("{}", error_message);

        Err(anyhow!("No commits created by {}", step_display))
    }

    /// Execute MapReduce setup phase if present
    ///
    /// Handles setup phase configuration, execution with file detection,
    /// and variable capture. Returns the generated input file path if any.
    async fn execute_mapreduce_setup_phase(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        worktree_env: &ExecutionEnvironment,
        workflow_context: &mut WorkflowContext,
    ) -> Result<(Option<String>, HashMap<String, String>)> {
        use crate::cook::execution::setup_executor::SetupPhaseExecutor;
        use crate::cook::execution::SetupPhase;

        let mut generated_input_file: Option<String> = None;
        let mut captured_variables = HashMap::new();

        // Execute setup phase if present
        if !workflow.steps.is_empty() || workflow.setup_phase.is_some() {
            self.user_interaction
                .display_progress("Running setup phase...");

            // Use provided setup_phase configuration or create a default one
            let setup_phase = if let Some(ref setup) = workflow.setup_phase {
                setup.clone()
            } else if !workflow.steps.is_empty() {
                // For backward compatibility, no timeout by default
                SetupPhase {
                    commands: workflow.steps.clone(),
                    timeout: None,                   // No timeout by default
                    capture_outputs: HashMap::new(), // No variables to capture by default
                }
            } else {
                // No setup phase
                SetupPhase {
                    commands: vec![],
                    timeout: None, // No timeout by default
                    capture_outputs: HashMap::new(),
                }
            };

            if !setup_phase.commands.is_empty() {
                // SPEC 128: Immutable Environment Context Pattern
                // The setup phase executor uses worktree_env (ExecutionEnvironment) which already
                // has the correct working directory set, making environment mutations unnecessary.
                // Pass worktree_env explicitly to all executors.

                let mut setup_executor = SetupPhaseExecutor::new(&setup_phase);

                // Execute setup phase with file detection
                // IMPORTANT: Use worktree_env here to ensure setup executes in the worktree
                let (captured, gen_file) = setup_executor
                    .execute_with_file_detection(
                        &setup_phase.commands,
                        self,
                        worktree_env,
                        workflow_context,
                    )
                    .await
                    .map_err(|e| anyhow!("Setup phase failed: {}", e))?;

                captured_variables = captured;
                generated_input_file = gen_file;
            }

            self.user_interaction
                .display_success("Setup phase completed");
        }

        Ok((generated_input_file, captured_variables))
    }

    /// Execute a MapReduce workflow
    ///
    /// High-level orchestration of MapReduce workflow execution:
    /// 1. Validate workflow in dry-run mode (if enabled)
    /// 2. Prepare execution environment and workflow context
    /// 3. Execute setup phase (if present)
    /// 4. Configure map phase with interpolated inputs
    /// 5. Create and execute MapReduce executor
    /// 6. Update session with results
    async fn execute_mapreduce(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        use crate::cook::execution::MapReduceExecutor;

        let workflow_start = Instant::now();

        // Handle dry-run mode for MapReduce
        if self.dry_run {
            orchestration::validate_mapreduce_dry_run(workflow).await?;
            return Ok(());
        }

        // Don't duplicate the message - it's already shown by the orchestrator

        // SPEC 134: MapReduce executes in the parent worktree (already created by orchestrator)
        tracing::info!(
            "Executing MapReduce in parent worktree: {}",
            env.working_dir.display()
        );

        // Prepare environment and workflow context with environment variables
        // SPEC 163: Pass positional args for automatic ARG_N injection
        let (worktree_env, mut workflow_context) = orchestration::prepare_mapreduce_environment(
            env,
            self.global_environment_config.as_ref(),
            self.positional_args.as_deref(),
        )?;

        // Execute setup phase if present, capturing output and generated files
        let (generated_input_file, _captured_variables) = self
            .execute_mapreduce_setup_phase(workflow, &worktree_env, &mut workflow_context)
            .await?;

        // Configure map phase with input interpolation and environment variables
        let map_phase =
            orchestration::configure_map_phase(workflow, generated_input_file, &workflow_context)?;

        // Create MapReduce executor
        // Use the parent worktree as the base for map phase agent worktrees
        // Convert VerbosityLevel to u8 for merge operation verbosity control
        let verbosity_u8 = match self.user_interaction.verbosity() {
            crate::cook::interaction::VerbosityLevel::Quiet => 0,
            crate::cook::interaction::VerbosityLevel::Normal => 0,
            crate::cook::interaction::VerbosityLevel::Verbose => 1,
            crate::cook::interaction::VerbosityLevel::Debug => 2,
            crate::cook::interaction::VerbosityLevel::Trace => 3,
        };

        // SPEC 134: Create WorktreeManager for agent worktrees using parent worktree as base
        use crate::worktree::WorktreeManager;
        let worktree_manager = Arc::new(WorktreeManager::new(
            env.working_dir.to_path_buf(),
            self.subprocess.clone(),
        )?);

        let mut mapreduce_executor = MapReduceExecutor::new_with_verbosity(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.user_interaction.clone(),
            worktree_manager,
            env.working_dir.to_path_buf(), // Use parent worktree path as base for agents
            verbosity_u8,
        )
        .await;

        // Start workflow timing in session
        self.session_manager
            .update_session(SessionUpdate::StartWorkflow)
            .await?;

        // Execute MapReduce workflow
        // Note: setup phase was already executed above, so we pass None to avoid duplicate execution
        // Use worktree_env for map and reduce phases to ensure all execution happens in the worktree
        let results = mapreduce_executor
            .execute_with_context(
                None, // Setup already executed above with proper environment variables
                map_phase,
                workflow.reduce_phase.clone(),
                worktree_env,
            )
            .await?;

        // Update session with results
        let successful_count = results
            .iter()
            .filter(|r| matches!(r.status, crate::cook::execution::AgentStatus::Success))
            .count();

        self.session_manager
            .update_session(SessionUpdate::AddFilesChanged(successful_count))
            .await?;

        // Metrics collection removed in v0.3.0

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total MapReduce workflow time",
            &format_duration(total_duration),
        );

        Ok(())
    }
}

// Implement the WorkflowExecutor trait
#[async_trait::async_trait]
impl super::traits::StepExecutor for WorkflowExecutor {
    async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Call the existing execute_step method
        self.execute_step(step, env, context).await
    }
}

#[async_trait::async_trait]
impl super::traits::WorkflowExecutor for WorkflowExecutor {
    async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Call the existing execute method
        self.execute(workflow, env).await
    }
}

/// Adapter to allow StepValidationExecutor to use WorkflowExecutor for command execution
struct StepValidationCommandExecutor {
    workflow_executor: *mut WorkflowExecutor,
    env: ExecutionEnvironment,
    ctx: WorkflowContext,
}

unsafe impl Send for StepValidationCommandExecutor {}
unsafe impl Sync for StepValidationCommandExecutor {}

#[async_trait::async_trait]
impl crate::cook::execution::CommandExecutor for StepValidationCommandExecutor {
    async fn execute(
        &self,
        command_type: &str,
        args: &[String],
        _context: crate::cook::execution::ExecutionContext,
    ) -> Result<crate::cook::execution::ExecutionResult> {
        // Safety: We ensure the workflow executor pointer is valid during validation
        let executor = unsafe { &mut *self.workflow_executor };

        // Create a workflow step for the validation command
        let step = match command_type {
            "claude" => WorkflowStep {
                claude: Some(args.join(" ")),
                ..Default::default()
            },
            "shell" => WorkflowStep {
                shell: Some(args.join(" ")),
                ..Default::default()
            },
            _ => {
                return Err(anyhow!(
                    "Unsupported validation command type: {}",
                    command_type
                ));
            }
        };

        // Execute the step
        let mut ctx_clone = self.ctx.clone();
        let result = executor
            .execute_step(&step, &self.env, &mut ctx_clone)
            .await?;

        Ok(crate::cook::execution::ExecutionResult {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            success: result.success,
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper function to test get_current_head directly without needing a full executor
    #[cfg(test)]
    async fn test_get_current_head(working_dir: &std::path::Path) -> Result<String> {
        use crate::abstractions::git::RealGitOperations;
        use anyhow::Context;
        let git_ops = RealGitOperations::new();
        let output = git_ops
            .git_command_in_dir(&["rev-parse", "HEAD"], "get HEAD", working_dir)
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[test]
    fn test_variable_interpolation_with_tracking() {
        let mut ctx = WorkflowContext::default();
        ctx.variables.insert("ARG".to_string(), "98".to_string());
        ctx.variables
            .insert("USER".to_string(), "alice".to_string());

        let template = "Running command with $ARG and ${USER}";
        let (result, resolutions) = ctx.interpolate_with_tracking(template);

        assert_eq!(result, "Running command with 98 and alice");
        assert_eq!(resolutions.len(), 2);

        // Check resolutions - order may vary due to HashMap iteration
        let arg_resolution = resolutions.iter().find(|r| r.name == "ARG").unwrap();
        assert_eq!(arg_resolution.raw_expression, "$ARG");
        assert_eq!(arg_resolution.resolved_value, "98");

        let user_resolution = resolutions.iter().find(|r| r.name == "USER").unwrap();
        assert_eq!(user_resolution.raw_expression, "${USER}");
        assert_eq!(user_resolution.resolved_value, "alice");
    }

    #[test]
    fn test_variable_interpolation_with_validation_results() {
        let mut ctx = WorkflowContext::default();

        // Add a validation result
        let validation = crate::cook::workflow::validation::ValidationResult {
            completion_percentage: 95.5,
            status: crate::cook::workflow::validation::ValidationStatus::Incomplete,
            implemented: vec![],
            missing: vec!["test coverage".to_string(), "documentation".to_string()],
            gaps: Default::default(),
            raw_output: None,
        };
        ctx.validation_results
            .insert("validation".to_string(), validation);

        let template = "Completion: ${validation.completion}%, missing: ${validation.missing}";
        let (result, resolutions) = ctx.interpolate_with_tracking(template);

        assert_eq!(
            result,
            "Completion: 95.5%, missing: test coverage, documentation"
        );
        assert_eq!(resolutions.len(), 2);
        assert_eq!(resolutions[0].name, "validation.completion");
        assert_eq!(resolutions[0].resolved_value, "95.5");
        assert_eq!(resolutions[1].name, "validation.missing");
        assert_eq!(
            resolutions[1].resolved_value,
            "test coverage, documentation"
        );
    }

    #[test]
    fn test_variable_interpolation_no_variables() {
        let ctx = WorkflowContext::default();
        let template = "No variables here";
        let (result, resolutions) = ctx.interpolate_with_tracking(template);

        assert_eq!(result, "No variables here");
        assert_eq!(resolutions.len(), 0);
    }

    // Minimal mock implementations for tests
    #[cfg(test)]
    pub(crate) mod test_mocks {
        use super::*;
        use crate::cook::execution::{ClaudeExecutor, ExecutionResult};
        use crate::cook::interaction::VerbosityLevel;
        use crate::cook::interaction::{SpinnerHandle, UserInteraction};
        use crate::cook::session::{
            SessionInfo, SessionManager, SessionState, SessionSummary, SessionUpdate,
        };
        use async_trait::async_trait;
        use std::collections::HashMap;
        use std::path::Path;

        pub struct MockClaudeExecutor;

        impl MockClaudeExecutor {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl ClaudeExecutor for MockClaudeExecutor {
            async fn execute_claude_command(
                &self,
                _command: &str,
                _project_path: &Path,
                _env_vars: HashMap<String, String>,
            ) -> Result<ExecutionResult> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn check_claude_cli(&self) -> Result<bool> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn get_claude_version(&self) -> Result<String> {
                unreachable!("Not used in format_variable_value tests")
            }
        }

        pub struct MockSessionManager;

        impl MockSessionManager {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl SessionManager for MockSessionManager {
            async fn start_session(&self, _session_id: &str) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn update_session(&self, _update: SessionUpdate) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn complete_session(&self) -> Result<SessionSummary> {
                unreachable!("Not used in format_variable_value tests")
            }

            fn get_state(&self) -> Result<SessionState> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn save_state(&self, _path: &Path) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn load_state(&self, _path: &Path) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn load_session(&self, _session_id: &str) -> Result<SessionState> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn save_checkpoint(&self, _state: &SessionState) -> Result<()> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn list_resumable(&self) -> Result<Vec<SessionInfo>> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn get_last_interrupted(&self) -> Result<Option<String>> {
                unreachable!("Not used in format_variable_value tests")
            }
        }

        pub struct MockUserInteraction;

        impl MockUserInteraction {
            pub fn new() -> Self {
                Self
            }
        }

        #[async_trait]
        impl UserInteraction for MockUserInteraction {
            async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
                unreachable!("Not used in format_variable_value tests")
            }

            async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
                unreachable!("Not used in format_variable_value tests")
            }

            fn display_info(&self, _message: &str) {}
            fn display_warning(&self, _message: &str) {}
            fn display_error(&self, _message: &str) {}
            fn display_success(&self, _message: &str) {}
            fn display_progress(&self, _message: &str) {}
            fn start_spinner(&self, _message: &str) -> Box<dyn SpinnerHandle> {
                struct NoOpSpinner;
                impl SpinnerHandle for NoOpSpinner {
                    fn update_message(&mut self, _message: &str) {}
                    fn success(&mut self, _message: &str) {}
                    fn fail(&mut self, _message: &str) {}
                }
                Box::new(NoOpSpinner)
            }
            fn display_action(&self, _message: &str) {}
            fn display_metric(&self, _label: &str, _value: &str) {}
            fn display_status(&self, _message: &str) {}
            fn iteration_start(&self, _current: u32, _total: u32) {}
            fn iteration_end(&self, _current: u32, _duration: std::time::Duration, _success: bool) {
            }
            fn step_start(&self, _step: u32, _total: u32, _description: &str) {}
            fn step_end(&self, _step: u32, _success: bool) {}
            fn command_output(&self, _output: &str, _verbosity: VerbosityLevel) {}
            fn debug_output(&self, _message: &str, _min_verbosity: VerbosityLevel) {}
            fn verbosity(&self) -> VerbosityLevel {
                VerbosityLevel::Normal
            }
        }
    }

    #[test]
    fn test_format_variable_value_short_string() {
        let executor = create_test_executor();

        let value = "simple value";
        let formatted = executor.format_variable_value(value);
        assert_eq!(formatted, "\"simple value\"");
    }

    #[test]
    fn test_format_variable_value_json_array() {
        let executor = create_test_executor();

        let value = r#"["item1", "item2", "item3"]"#;
        let formatted = executor.format_variable_value(value);
        assert_eq!(formatted, r#"["item1","item2","item3"]"#);
    }

    // Test helper function for creating WorkflowExecutor with mocks
    fn create_test_executor() -> WorkflowExecutor {
        use test_mocks::{MockClaudeExecutor, MockSessionManager, MockUserInteraction};

        WorkflowExecutor::new(
            Arc::new(MockClaudeExecutor::new()),
            Arc::new(MockSessionManager::new()),
            Arc::new(MockUserInteraction::new()),
        )
    }

    #[test]
    fn test_format_variable_value_large_array() {
        let executor = create_test_executor();

        // Create a large array
        let items: Vec<String> = (0..100).map(|i| format!("\"item{}\"", i)).collect();
        let value = format!("[{}]", items.join(","));
        let formatted = executor.format_variable_value(&value);
        assert!(formatted.contains("...100 items..."));
    }

    #[test]
    fn test_format_variable_value_json_object() {
        let executor = create_test_executor();

        let value = r#"{"name": "test", "value": 42}"#;
        let formatted = executor.format_variable_value(value);
        // Should be pretty-printed
        assert!(formatted.contains("name"));
        assert!(formatted.contains("test"));
        assert!(formatted.contains("value"));
        assert!(formatted.contains("42"));
    }

    #[test]
    fn test_format_variable_value_truncated() {
        let executor = create_test_executor();

        let value = "a".repeat(300);
        let formatted = executor.format_variable_value(&value);
        assert!(formatted.contains("...\" (showing first 200 chars)"));
        assert!(formatted.starts_with("\""));
    }

    #[test]
    fn test_json_to_attribute_value_static_string() {
        let json = serde_json::json!("hello world");
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::String("hello world".to_string()));
    }

    #[test]
    fn test_json_to_attribute_value_static_integer() {
        let json = serde_json::json!(42);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(42.0));
    }

    #[test]
    fn test_json_to_attribute_value_static_float() {
        let json = serde_json::json!(123.456);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(123.456));
    }

    #[test]
    fn test_json_to_attribute_value_static_boolean_true() {
        let json = serde_json::json!(true);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Boolean(true));
    }

    #[test]
    fn test_json_to_attribute_value_static_boolean_false() {
        let json = serde_json::json!(false);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Boolean(false));
    }

    #[test]
    fn test_json_to_attribute_value_static_null() {
        let json = serde_json::json!(null);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Null);
    }

    #[test]
    fn test_json_to_attribute_value_static_array() {
        let json = serde_json::json!([1, "two", true, null]);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(
            result,
            AttributeValue::Array(vec![
                AttributeValue::Number(1.0),
                AttributeValue::String("two".to_string()),
                AttributeValue::Boolean(true),
                AttributeValue::Null,
            ])
        );
    }

    #[test]
    fn test_json_to_attribute_value_static_nested_array() {
        let json = serde_json::json!([[1, 2], [3, 4]]);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(
            result,
            AttributeValue::Array(vec![
                AttributeValue::Array(vec![
                    AttributeValue::Number(1.0),
                    AttributeValue::Number(2.0),
                ]),
                AttributeValue::Array(vec![
                    AttributeValue::Number(3.0),
                    AttributeValue::Number(4.0),
                ]),
            ])
        );
    }

    #[test]
    fn test_json_to_attribute_value_static_object() {
        let json = serde_json::json!({
            "name": "test",
            "count": 42,
            "active": true,
            "data": null
        });
        let result = WorkflowExecutor::json_to_attribute_value_static(json);

        if let AttributeValue::Object(map) = result {
            assert_eq!(
                map.get("name"),
                Some(&AttributeValue::String("test".to_string()))
            );
            assert_eq!(map.get("count"), Some(&AttributeValue::Number(42.0)));
            assert_eq!(map.get("active"), Some(&AttributeValue::Boolean(true)));
            assert_eq!(map.get("data"), Some(&AttributeValue::Null));
            assert_eq!(map.len(), 4);
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_json_to_attribute_value_static_nested_object() {
        let json = serde_json::json!({
            "user": {
                "name": "Alice",
                "age": 30
            },
            "settings": {
                "theme": "dark",
                "notifications": true
            }
        });
        let result = WorkflowExecutor::json_to_attribute_value_static(json);

        if let AttributeValue::Object(map) = result {
            // Check user object
            if let Some(AttributeValue::Object(user_map)) = map.get("user") {
                assert_eq!(
                    user_map.get("name"),
                    Some(&AttributeValue::String("Alice".to_string()))
                );
                assert_eq!(user_map.get("age"), Some(&AttributeValue::Number(30.0)));
            } else {
                panic!("Expected user to be an Object");
            }

            // Check settings object
            if let Some(AttributeValue::Object(settings_map)) = map.get("settings") {
                assert_eq!(
                    settings_map.get("theme"),
                    Some(&AttributeValue::String("dark".to_string()))
                );
                assert_eq!(
                    settings_map.get("notifications"),
                    Some(&AttributeValue::Boolean(true))
                );
            } else {
                panic!("Expected settings to be an Object");
            }
        } else {
            panic!("Expected Object variant");
        }
    }

    #[test]
    fn test_json_to_attribute_value_static_large_numbers() {
        // Test large integer
        let json = serde_json::json!(9007199254740991i64); // Max safe integer in JavaScript
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(9007199254740991.0));

        // Test negative integer
        let json = serde_json::json!(-42);
        let result = WorkflowExecutor::json_to_attribute_value_static(json);
        assert_eq!(result, AttributeValue::Number(-42.0));
    }

    #[tokio::test]
    async fn test_get_current_head_in_regular_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to init git repo");

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to set git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to set git name");

        // Create initial commit
        std::fs::write(repo_path.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("Failed to stage files");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to create commit");

        // Test get_current_head
        let head = test_get_current_head(repo_path).await.unwrap();
        assert!(!head.is_empty());
        assert_eq!(head.len(), 40); // SHA-1 hash is 40 characters
    }

    #[tokio::test]
    async fn test_get_current_head_in_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let main_repo = temp_dir.path().join("main");
        let worktree_path = temp_dir.path().join("worktree");

        // Create main repo
        std::fs::create_dir(&main_repo).unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to init git repo");

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to set git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to set git name");

        // Create initial commit in main repo
        std::fs::write(main_repo.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to stage files");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to create commit");

        // Create worktree
        std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                "test-branch",
            ])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to create worktree");

        // Make a commit in the worktree
        std::fs::write(worktree_path.join("worktree.txt"), "worktree content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to stage files in worktree");

        std::process::Command::new("git")
            .args(["commit", "-m", "Worktree commit"])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to create commit in worktree");

        // Test get_current_head in worktree
        let worktree_head = test_get_current_head(&worktree_path).await.unwrap();
        assert!(!worktree_head.is_empty());
        assert_eq!(worktree_head.len(), 40);

        // Get main repo head
        let main_head = test_get_current_head(&main_repo).await.unwrap();

        // Heads should be different
        assert_ne!(
            worktree_head, main_head,
            "Worktree HEAD should differ from main repo HEAD"
        );
    }

    #[tokio::test]
    async fn test_get_current_head_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let non_git_dir = temp_dir.path();

        // Test in non-git directory
        let result = test_get_current_head(non_git_dir).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to get git HEAD"));
    }

    #[tokio::test]
    async fn test_get_current_head_respects_working_directory() {
        // This test verifies that the git command runs in the correct directory
        let temp_dir = TempDir::new().unwrap();
        let repo1 = temp_dir.path().join("repo1");
        let repo2 = temp_dir.path().join("repo2");

        // Create two separate repos
        for (repo_path, commit_msg) in &[(&repo1, "Repo 1 commit"), (&repo2, "Repo 2 commit")] {
            std::fs::create_dir(repo_path).unwrap();
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to init git repo");

            std::process::Command::new("git")
                .args(["config", "user.email", "test@example.com"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to set git email");

            std::process::Command::new("git")
                .args(["config", "user.name", "Test User"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to set git name");

            std::fs::write(
                repo_path.join("test.txt"),
                format!("content for {commit_msg}"),
            )
            .unwrap();
            std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(repo_path)
                .output()
                .expect("Failed to stage files");

            std::process::Command::new("git")
                .args(["commit", "-m", commit_msg])
                .current_dir(repo_path)
                .output()
                .expect("Failed to create commit");
        }

        // Get heads from both repos
        let head1 = test_get_current_head(&repo1).await.unwrap();
        let head2 = test_get_current_head(&repo2).await.unwrap();

        // They should be different
        assert_ne!(
            head1, head2,
            "Different repos should have different HEAD commits"
        );
    }
}

#[cfg(test)]
#[path = "handle_commit_verification_tests.rs"]
mod handle_commit_verification_tests;
