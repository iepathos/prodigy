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
    self, create_checkpoint_with_total_steps, CheckpointManager,
    CompletedStep as CheckpointCompletedStep, ResumeContext,
};
use crate::cook::workflow::normalized;
use crate::cook::workflow::normalized::NormalizedWorkflow;
use crate::cook::workflow::on_failure::OnFailureConfig;
use crate::testing::config::TestConfiguration;
use crate::unified_session::{format_duration, TimingTracker};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
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
use pure::{ExecutionFlags, IterationContinuation};

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
        // Inject error context as variables
        let step_name = self.get_step_display_name(step);
        ctx.variables
            .insert("error.message".to_string(), result.stderr.clone());
        ctx.variables.insert(
            "error.exit_code".to_string(),
            result.exit_code.unwrap_or(-1).to_string(),
        );
        ctx.variables
            .insert("error.step".to_string(), step_name.clone());
        ctx.variables
            .insert("error.timestamp".to_string(), Utc::now().to_rfc3339());

        // Get handler commands
        let handler_commands = on_failure_config.handler_commands();

        if !handler_commands.is_empty() {
            let strategy = on_failure_config.strategy();
            self.user_interaction.display_info(&format!(
                "Executing on_failure handler ({:?} strategy)...",
                strategy
            ));

            let mut handler_success = true;
            let mut handler_outputs = Vec::new();

            // Execute each handler command
            for (idx, cmd) in handler_commands.iter().enumerate() {
                self.user_interaction.display_progress(&format!(
                    "Handler command {}/{}",
                    idx + 1,
                    handler_commands.len()
                ));

                // Create a WorkflowStep from the HandlerCommand
                let handler_step = WorkflowStep {
                    name: None,
                    shell: cmd.shell.clone(),
                    claude: cmd.claude.clone(),
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    write_file: None,
                    command: None,
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    auto_commit: false,
                    commit_config: None,
                    output_file: None,
                    timeout: on_failure_config.handler_timeout(),
                    capture_output: CaptureOutput::Disabled,
                    on_failure: None,
                    retry: None,
                    on_success: None,
                    on_exit_code: Default::default(),
                    commit_required: false,
                    working_dir: None,
                    env: Default::default(),
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                };

                // Execute the handler command
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
                        result = retry_result;
                        break;
                    }
                }
            }
        } else if let Some(handler) = on_failure_config.handler() {
            // Fallback to legacy handler for backward compatibility
            self.user_interaction
                .display_info("Executing on_failure handler...");
            let failure_result = Box::pin(self.execute_step(&handler, env, ctx)).await?;
            result = failure_handler::append_handler_output(
                result,
                std::slice::from_ref(&failure_result.stdout),
            );

            // Check if we should retry the original command
            if failure_handler::should_retry_after_handler(on_failure_config, result.success) {
                let max_retries = failure_handler::get_handler_max_retries(on_failure_config);
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
                        result = retry_result;
                        break;
                    }
                }
            }
        }

        // Clear error variables from context
        ctx.variables.remove("error.message");
        ctx.variables.remove("error.exit_code");
        ctx.variables.remove("error.step");
        ctx.variables.remove("error.timestamp");

        Ok(result)
    }

    /// Determine command type from a workflow step
    pub(crate) fn determine_command_type(&self, step: &WorkflowStep) -> Result<CommandType> {
        // Count how many command fields are specified
        let mut specified_count = 0;
        if step.claude.is_some() {
            specified_count += 1;
        }
        if step.shell.is_some() {
            specified_count += 1;
        }
        if step.test.is_some() {
            specified_count += 1;
        }
        if step.handler.is_some() {
            specified_count += 1;
        }
        if step.goal_seek.is_some() {
            specified_count += 1;
        }
        if step.foreach.is_some() {
            specified_count += 1;
        }
        if step.write_file.is_some() {
            specified_count += 1;
        }
        if step.name.is_some() || step.command.is_some() {
            specified_count += 1;
        }

        // Ensure only one command type is specified
        if specified_count > 1 {
            return Err(anyhow!(
                "Multiple command types specified. Use only one of: claude, shell, test, handler, goal_seek, foreach, write_file, or name/command"
            ));
        }

        if specified_count == 0 {
            return Err(anyhow!(
                "No command specified. Use one of: claude, shell, test, handler, goal_seek, foreach, write_file, or name/command"
            ));
        }

        // Return the appropriate command type
        if let Some(handler_step) = &step.handler {
            // Convert serde_json::Value to AttributeValue
            let mut attributes = HashMap::new();
            for (key, value) in &handler_step.attributes {
                attributes.insert(key.clone(), self.json_to_attribute_value(value.clone()));
            }
            Ok(CommandType::Handler {
                handler_name: handler_step.name.clone(),
                attributes,
            })
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
            // Legacy support - prepend / if not present
            let command = if name.starts_with('/') {
                name.clone()
            } else {
                format!("/{name}")
            };
            Ok(CommandType::Legacy(command))
        } else if let Some(command) = &step.command {
            Ok(CommandType::Legacy(command.clone()))
        } else {
            Err(anyhow!("No valid command found in step"))
        }
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
        let workflow_state = crate::cook::session::WorkflowState {
            current_iteration: iteration.saturating_sub(1), // Convert to 0-based index
            current_step: step_index + 1,                   // Next step to execute
            completed_steps: self.completed_steps.clone(),
            workflow_path: env.working_dir.join("workflow.yml"),
            input_args: Vec::new(),
            map_patterns: Vec::new(),
            using_worktree: true, // Always true since worktrees are mandatory (spec 109)
        };
        self.session_manager
            .update_session(SessionUpdate::UpdateWorkflowState(workflow_state))
            .await
    }

    /// Handle commit verification and auto-commit (delegated to commit_handler module)
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
            // No commits were created - check if auto-commit is enabled
            if step.auto_commit {
                // Try to create an auto-commit
                if let Ok(has_changes) = commit_handler.has_uncommitted_changes(working_dir).await {
                    if has_changes {
                        let message = self.generate_commit_message(step, workflow_context);
                        if let Err(e) = commit_handler
                            .create_auto_commit(working_dir, &message, step_display)
                            .await
                        {
                            tracing::warn!("Failed to create auto-commit: {}", e);
                            if step.commit_required {
                                self.handle_no_commits_error(step)?;
                            }
                        } else {
                            return Ok(true);
                        }
                    } else if step.commit_required {
                        self.handle_no_commits_error(step)?;
                    }
                } else if step.commit_required {
                    self.handle_no_commits_error(step)?;
                }
            } else if step.commit_required {
                self.handle_no_commits_error(step)?;
            }
            Ok(false)
        } else {
            // Track commit metadata if available
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
    /// Calculate effective max iterations for a workflow (delegated to pure module)
    fn calculate_effective_max_iterations(workflow: &ExtendedWorkflowConfig, dry_run: bool) -> u32 {
        pure::calculate_effective_max_iterations(workflow, dry_run)
    }

    /// Build iteration context variables (delegated to pure module)
    fn build_iteration_context(iteration: u32) -> HashMap<String, String> {
        pure::build_iteration_context(iteration)
    }

    /// Get summary of available variables for debugging (delegated to pure module)
    fn get_available_variable_summary(context: &InterpolationContext) -> String {
        pure::get_available_variable_summary(context)
    }

    /// Validate workflow configuration (delegated to pure module)
    fn validate_workflow_config(workflow: &ExtendedWorkflowConfig) -> Result<()> {
        pure::validate_workflow_config(workflow)
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
                                        workflow_path: env.working_dir.join("workflow.yml"),
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
        Self::validate_workflow_config(workflow)?;

        let workflow_start = Instant::now();
        let execution_flags = Self::determine_execution_flags();

        // Display dry-run mode message
        self.display_dry_run_info(workflow);

        // Calculate effective max iterations
        let effective_max_iterations =
            Self::calculate_effective_max_iterations(workflow, self.dry_run);

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

        // Note: workflow_context is passed in from execute() wrapper

        // Start workflow timing in session
        self.session_manager
            .update_session(SessionUpdate::StartWorkflow)
            .await?;

        while should_continue && iteration < effective_max_iterations {
            iteration += 1;

            // Clear completed steps at the start of each iteration
            // This ensures steps can be re-executed in subsequent iterations
            self.completed_steps.clear();

            // Update iteration context with pure function
            let iteration_vars = Self::build_iteration_context(iteration);
            workflow_context.iteration_vars.extend(iteration_vars);

            let iteration_msg =
                orchestration::format_iteration_progress(iteration, effective_max_iterations);
            self.user_interaction.display_progress(&iteration_msg);

            // Start iteration timing
            self.timing_tracker.start_iteration();

            // Update session (skip in dry-run mode to avoid misleading stats)
            if !self.dry_run {
                self.session_manager
                    .update_session(SessionUpdate::IncrementIteration)
                    .await?;
                self.session_manager
                    .update_session(SessionUpdate::StartIteration(iteration))
                    .await?;
            }

            // Execute workflow steps
            for (step_index, step) in workflow.steps.iter().enumerate() {
                // Check if we should skip this step (already completed in previous run)
                if Self::should_skip_step_execution(step_index, &self.completed_steps) {
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

                // Store current workflow context for checkpoint tracking
                // TODO: Convert workflow to NormalizedWorkflow for checkpoint tracking
                // self.current_workflow = Some(workflow.clone());
                self.current_step_index = Some(step_index);

                let step_display = self.get_step_display_name(step);
                let step_msg = orchestration::format_step_progress(
                    step_index,
                    workflow.steps.len(),
                    &step_display,
                );
                self.user_interaction.display_progress(&step_msg);

                // Get HEAD before command execution if we need to verify commits
                let head_before = if !execution_flags.skip_validation
                    && step.commit_required
                    && !execution_flags.test_mode
                {
                    Some(self.get_current_head(&env.working_dir).await?)
                } else {
                    None
                };

                // Start command timing
                self.timing_tracker.start_command(step_display.clone());
                let command_start = Instant::now();
                let step_started_at = chrono::Utc::now();

                // Execute the step
                // Note: No with_context() wrapper here - the error from execute_step
                // already contains detailed information from build_step_error_message
                let step_result = self.execute_step(step, env, workflow_context).await?;

                // Display subprocess output when verbose logging is enabled
                self.log_step_output(&step_result);

                // Complete command timing
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

                // Track the completed step with output
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

                // Also track for checkpoint system
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

                // Save checkpoint if manager is available
                if let Some(ref checkpoint_manager) = self.checkpoint_manager {
                    if let Some(ref workflow_id) = self.workflow_id {
                        // Create a normalized workflow for hashing (simplified)
                        let workflow_hash = orchestration::create_workflow_hash(
                            &workflow.name,
                            workflow.steps.len(),
                        );

                        // Build normalized workflow
                        let normalized_workflow = orchestration::create_normalized_workflow(
                            &workflow.name,
                            workflow_context,
                        );

                        // Build checkpoint
                        let mut checkpoint = create_checkpoint_with_total_steps(
                            workflow_id.clone(),
                            &normalized_workflow,
                            workflow_context,
                            self.checkpoint_completed_steps.clone(),
                            step_index + 1,
                            workflow_hash,
                            workflow.steps.len(), // Pass the actual total steps count
                        );

                        // Set workflow path if available
                        if let Some(ref path) = self.workflow_path {
                            checkpoint.workflow_path = Some(path.clone());
                        }

                        // Save checkpoint asynchronously
                        if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                            tracing::warn!("Failed to save checkpoint: {}", e);
                        }
                    }
                }

                // Save workflow state after step execution
                self.save_workflow_state(env, iteration as usize, step_index)
                    .await?;

                // Check for commits if required (skip in dry-run mode)
                if !self.dry_run {
                    if let Some(before) = head_before {
                        any_changes = self
                            .handle_commit_verification(
                                &env.working_dir,
                                &before,
                                step,
                                &step_display,
                                workflow_context,
                            )
                            .await?
                            || any_changes;
                    }
                }
            }

            // Determine if we should continue iterations using pure function
            let continuation = Self::determine_iteration_continuation(
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
                IterationContinuation::AskUser => {
                    // Check based on metrics or ask user
                    self.should_continue_iterations(env).await?
                }
            };

            // Complete iteration timing
            if let Some(iteration_duration) = self.timing_tracker.complete_iteration() {
                self.session_manager
                    .update_session(SessionUpdate::CompleteIteration)
                    .await?;

                // Display iteration timing
                self.user_interaction.display_success(&format!(
                    "Iteration {} completed in {}",
                    iteration,
                    format_duration(iteration_duration)
                ));
            }

            // Analysis between iterations removed in this version
            // if should_continue && workflow.analyze_between {
            //     self.user_interaction
            //         .display_progress("Running analysis between iterations...");
            //     let analysis = self
            //         .analysis_coordinator
            //         .analyze_project(&env.working_dir)
            //         .await?;
            //     self.analysis_coordinator
            //         .save_analysis(&env.working_dir, &analysis)
            //         .await?;
            // }
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

        let command_name = match &command_type {
            CommandType::Claude(cmd) | CommandType::Legacy(cmd) => cmd
                .trim_start_matches('/')
                .split_whitespace()
                .next()
                .unwrap_or(""),
            CommandType::Shell(cmd) => cmd,
            CommandType::Test(test_cmd) => &test_cmd.command,
            CommandType::Handler { handler_name, .. } => handler_name,
            CommandType::GoalSeek(config) => &config.goal,
            CommandType::Foreach(_) => "foreach",
            CommandType::WriteFile(config) => &config.path,
        };

        eprintln!("\nWorkflow stopped: No changes were committed by {step_display}");
        eprintln!("\nThe command executed successfully but did not create any git commits.");

        // Check if this is a command that might legitimately not create commits
        if matches!(
            command_name,
            "prodigy-lint" | "prodigy-code-review" | "prodigy-analyze"
        ) {
            eprintln!(
                "This may be expected if there were no {} to fix.",
                if command_name == "prodigy-lint" {
                    "linting issues"
                } else if command_name == "prodigy-code-review" {
                    "issues found"
                } else {
                    "changes needed"
                }
            );
            eprintln!("\nTo allow this command to proceed without commits, set commit_required: false in your workflow");
        } else {
            eprintln!("Possible reasons:");
            eprintln!("- The specification may already be implemented");
            eprintln!("- The command may have encountered an issue without reporting an error");
            eprintln!("- No changes were needed");
            eprintln!("\nTo investigate:");
            eprintln!("- Check if the spec is already implemented");
            eprintln!("- Review the command output above for any warnings");
            eprintln!("- Run 'git status' to check for uncommitted changes");
        }

        eprintln!(
            "\nAlternatively, run with PRODIGY_NO_COMMIT_VALIDATION=true to skip all validation."
        );

        Err(anyhow!("No commits created by {}", step_display))
    }

    /// Execute a MapReduce workflow
    async fn execute_mapreduce(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        use crate::cook::execution::setup_executor::SetupPhaseExecutor;
        use crate::cook::execution::{MapReduceExecutor, SetupPhase};

        let workflow_start = Instant::now();

        // Handle dry-run mode for MapReduce
        if self.dry_run {
            // Use the DryRunValidator to validate the workflow
            use crate::cook::execution::mapreduce::dry_run::{DryRunConfig, DryRunValidator};

            println!("[DRY RUN] MapReduce workflow execution simulation mode");
            println!("[DRY RUN] Validating workflow configuration...");

            // Create dry-run configuration
            let _dry_run_config = DryRunConfig {
                show_work_items: true,
                show_variables: true,
                show_resources: true,
                sample_size: Some(5),
            };

            // Create the validator
            let validator = DryRunValidator::new();

            // Validate the workflow
            let validation_result = validator
                .validate_workflow_phases(
                    workflow.setup_phase.clone(),
                    workflow
                        .map_phase
                        .as_ref()
                        .ok_or_else(|| anyhow!("MapReduce workflow requires map phase"))?
                        .clone(),
                    workflow.reduce_phase.clone(),
                )
                .await;

            match validation_result {
                Ok(report) => {
                    // Display the validation report
                    use crate::cook::execution::mapreduce::dry_run::OutputFormatter;
                    let formatter = OutputFormatter::new();
                    println!("{}", formatter.format_human(&report));

                    if report.errors.is_empty() {
                        println!(
                            "\n[DRY RUN] Validation successful! Workflow is ready to execute."
                        );
                    } else {
                        println!(
                            "\n[DRY RUN] Validation failed with {} error(s)",
                            report.errors.len()
                        );
                        return Err(anyhow!("Dry-run validation failed"));
                    }
                }
                Err(e) => {
                    println!("[DRY RUN] Validation failed: {}", e);
                    return Err(anyhow!("Dry-run validation failed: {}", e));
                }
            }

            // Don't actually execute in dry-run mode
            return Ok(());
        }

        // Don't duplicate the message - it's already shown by the orchestrator

        // SPEC 134: MapReduce executes in the parent worktree (already created by orchestrator)
        // No need to create an additional intermediate worktree. This ensures:
        // 1. Setup phase runs in parent worktree
        // 2. Agent worktrees branch from parent worktree
        // 3. Reduce phase runs in parent worktree
        // 4. User is prompted to merge parent worktree to original branch (via orchestrator cleanup)
        tracing::info!(
            "Executing MapReduce in parent worktree: {}",
            env.working_dir.display()
        );

        // Use the existing environment directly - it already points to parent worktree
        let worktree_env = env.clone();

        // SPEC 128: Create immutable environment context for worktree execution
        // This context explicitly specifies the worktree directory and prevents
        // hidden state mutations. All environment configuration is immutable after creation.
        use crate::cook::environment::EnvironmentContextBuilder;
        let _worktree_context = EnvironmentContextBuilder::new(env.working_dir.to_path_buf())
            .with_config(
                self.global_environment_config
                    .as_ref()
                    .unwrap_or(&crate::cook::environment::EnvironmentConfig::default()),
            )
            .context("Failed to create immutable environment context")?
            .build();

        // Note: The immutable context pattern is now available for future refactoring.
        // Currently, the executor still uses ExecutionEnvironment (worktree_env) which is
        // passed explicitly to all phase executors. This ensures working directory is
        // always explicit in function signatures rather than hidden in mutable state.

        let mut workflow_context = WorkflowContext::default();
        let mut generated_input_file: Option<String> = None;
        let mut _captured_variables = HashMap::new();

        // Populate workflow context with environment variables from global config
        if let Some(ref global_env_config) = self.global_environment_config {
            for (key, env_value) in &global_env_config.global_env {
                // Resolve the env value to a string
                if let crate::cook::environment::EnvValue::Static(value) = env_value {
                    workflow_context
                        .variables
                        .insert(key.clone(), value.clone());
                }
                // For Dynamic and Conditional values, we'd need to evaluate them here
                // For now, we only support Static values in MapReduce workflows
            }
        }

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
                // Instead of mutating EnvironmentManager's working directory (which caused bugs),
                // we create an immutable context that explicitly specifies the worktree directory.
                // The setup phase executor uses worktree_env (ExecutionEnvironment) which already
                // has the correct working directory set, making this mutation unnecessary.
                //
                // The previous code:
                //   env_manager.set_working_dir(worktree_path);  // Hidden mutation!
                // caused bugs because state changes weren't visible in function signatures.
                //
                // The new approach: Pass worktree_env explicitly to all executors
                // (already done via execute_with_file_detection parameter)

                let mut setup_executor = SetupPhaseExecutor::new(&setup_phase);

                // Execute setup phase with file detection
                // IMPORTANT: Use worktree_env here to ensure setup executes in the worktree
                let (captured, gen_file) = setup_executor
                    .execute_with_file_detection(
                        &setup_phase.commands,
                        self,
                        &worktree_env,
                        &mut workflow_context,
                    )
                    .await
                    .map_err(|e| anyhow!("Setup phase failed: {}", e))?;

                _captured_variables = captured;
                generated_input_file = gen_file;
            }

            self.user_interaction
                .display_success("Setup phase completed");
        }

        // Ensure we have map phase configuration
        let mut map_phase = workflow
            .map_phase
            .as_ref()
            .ok_or_else(|| anyhow!("MapReduce workflow requires map phase configuration"))?
            .clone();

        // Update map phase input if setup generated a work-items.json file
        if let Some(generated_file) = generated_input_file {
            map_phase.config.input = generated_file;
        }

        // Interpolate map phase input with environment variables
        let mut interpolated_input = map_phase.config.input.clone();
        for (key, value) in &workflow_context.variables {
            // Replace both ${VAR} and $VAR patterns
            interpolated_input = interpolated_input.replace(&format!("${{{}}}", key), value);
            interpolated_input = interpolated_input.replace(&format!("${}", key), value);
        }
        map_phase.config.input = interpolated_input;

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
