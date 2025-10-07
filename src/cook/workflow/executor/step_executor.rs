//! Step execution pipeline
//!
//! Handles individual workflow step execution, tracking, and result processing.
//! This module contains the core execution logic for workflow steps including:
//! - Step execution pipeline (initialization, execution, post-processing, finalization)
//! - Execution tracking (git changes, commits, session updates)
//! - Result handling (output capture, file writing, validation)
//! - Retry logic (enhanced retry with backoff and jitter)

use super::{CaptureOutput, CommandType, HandlerStep, StepResult, WorkflowContext, WorkflowExecutor, WorkflowStep};
use crate::cook::commit_tracker::TrackedCommit;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionUpdate;
use crate::cook::workflow::normalized;
use crate::cook::workflow::variables;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

impl WorkflowExecutor {
    // ========================================================================
    // Step Execution Core
    // ========================================================================

    /// Execute a single workflow step (public for resume functionality)
    pub async fn execute_single_step(
        &mut self,
        step: &normalized::NormalizedStep,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Convert NormalizedStep to WorkflowStep for execution
        let workflow_step = self.normalized_to_workflow_step(step)?;

        // Create a minimal execution environment
        let env = ExecutionEnvironment {
            working_dir: Arc::new(std::env::current_dir()?),
            project_dir: Arc::new(std::env::current_dir()?),
            worktree_name: None,
            session_id: Arc::from("resume-session"),
        };

        // Execute the step
        self.execute_step_internal(&workflow_step, &env, context)
            .await
    }

    /// Convert NormalizedStep to WorkflowStep
    fn normalized_to_workflow_step(
        &self,
        step: &normalized::NormalizedStep,
    ) -> Result<WorkflowStep> {
        use normalized::StepCommand;

        let mut workflow_step = WorkflowStep {
            name: Some(step.id.to_string()),
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            command: None,
            handler: None,
            capture: None,
            capture_format: None,
            capture_streams: Default::default(),
            output_file: None,
            capture_output: CaptureOutput::Disabled,
            timeout: step.timeout.map(|d| d.as_secs()),
            working_dir: step.working_dir.as_ref().map(|p| (**p).to_path_buf()),
            env: step
                .env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            on_failure: step
                .handlers
                .on_failure
                .as_ref()
                .map(|config| (**config).clone()),
            retry: None,
            on_success: step
                .handlers
                .on_success
                .as_ref()
                .map(|s| Box::new((**s).clone())),
            on_exit_code: step
                .handlers
                .on_exit_code
                .iter()
                .map(|(code, s)| (*code, Box::new((**s).clone())))
                .collect(),
            commit_required: step.commit_required,
            auto_commit: false,
            commit_config: None,
            validate: step.validation.as_ref().map(|v| (**v).clone()),
            step_validate: None,
            skip_validation: false,
            validation_timeout: None,
            ignore_validation_failure: false,
            when: step.when.as_ref().map(|w| w.to_string()),
        };

        // Set command based on step type
        match &step.command {
            StepCommand::Claude(cmd) => {
                workflow_step.claude = Some(cmd.to_string());
            }
            StepCommand::Shell(cmd) => {
                workflow_step.shell = Some(cmd.to_string());
            }
            StepCommand::Test {
                command,
                on_failure,
            } => {
                workflow_step.test = Some(crate::config::command::TestCommand {
                    command: command.to_string(),
                    on_failure: on_failure.as_ref().map(|f| (**f).clone()),
                });
            }
            StepCommand::GoalSeek(config) => {
                workflow_step.goal_seek = Some((**config).clone());
            }
            StepCommand::Handler(handler) => {
                workflow_step.handler = Some(HandlerStep {
                    name: handler.name.to_string(),
                    attributes: Arc::try_unwrap(handler.attributes.clone())
                        .unwrap_or_else(|arc| (*arc).clone()),
                });
            }
            StepCommand::Simple(cmd) => {
                // For simple commands, use the legacy command field
                workflow_step.command = Some(cmd.to_string());
            }
            StepCommand::Foreach(config) => {
                workflow_step.foreach = Some((**config).clone());
            }
        }

        Ok(workflow_step)
    }

    /// Internal execute_step method that doesn't modify self
    async fn execute_step_internal(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        context: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // Check conditional execution (when clause)
        if let Some(when_expr) = &step.when {
            let should_execute = self.evaluate_when_condition(when_expr, context)?;
            if !should_execute {
                tracing::info!("Skipping step due to when condition: {}", when_expr);
                return Ok(StepResult {
                    success: true,
                    exit_code: Some(0),
                    stdout: "Skipped due to when condition".to_string(),
                    stderr: String::new(),
                });
            }
        }

        // Determine command type
        let command_type = self.determine_command_type(step)?;

        // Prepare environment variables
        let env_vars = self.prepare_env_vars(step, env, context);

        // Execute the command based on its type
        self.execute_command_by_type(&command_type, step, env, context, env_vars)
            .await
    }

    /// Execute a single workflow step
    pub async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
    ) -> Result<StepResult> {
        // === PHASE 1: Initialization ===
        // Get step name for logging
        let step_name = self.get_step_display_name(step);

        // Log execution context (progress display, tracing)
        self.log_step_execution_context(&step_name, env, ctx);

        // Initialize git tracking for commit monitoring
        let (commit_tracker, before_head) = self.initialize_step_tracking(env, ctx).await?;

        // Determine command type (claude, shell, etc.)
        let command_type = self.determine_command_type(step)?;

        // Set up environment variables and working directory
        let (env_vars, _working_dir_override, actual_env) =
            self.setup_step_environment_context(step, env, ctx).await?;

        // Early return for test mode (no actual execution)
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return self.handle_test_mode_execution(step, &command_type);
        }

        // === PHASE 2: Execution ===
        // Execute command with retry support if configured
        let mut result = self
            .execute_with_retry_if_configured(step, &command_type, &actual_env, ctx, env_vars)
            .await?;

        // === PHASE 3: Post-Execution Processing ===
        // Track commits created during execution and create auto-commit if needed
        let tracked_commits = self
            .track_and_commit_changes(step, &commit_tracker, &before_head, ctx)
            .await?;

        // Validate commit requirements
        let after_head = commit_tracker.get_current_head().await?;
        self.validate_and_display_commit_info(step, &tracked_commits, &before_head, &after_head)?;

        // Capture output to variables and files
        self.capture_step_output(step, &result, ctx).await?;
        self.write_output_to_file(step, &result, &actual_env)?;
        self.handle_legacy_capture(step, &command_type, &result, ctx);

        // Execute validation checks if configured
        self.execute_step_validation(step, &mut result, &actual_env, ctx)
            .await?;

        // Handle conditional execution (on_failure, on_success handlers)
        result = self
            .handle_conditional_execution(step, result, &actual_env, ctx)
            .await?;

        // === PHASE 4: Finalization ===
        // Determine if step failure should fail the workflow
        let result = self.finalize_step_result(step, result)?;

        // Update session state with git changes
        self.track_and_update_session(ctx).await?;

        Ok(result)
    }

    /// Convert WorkflowCommand to WorkflowStep for execution
    pub(super) fn convert_workflow_command_to_step(
        &self,
        cmd: &crate::config::WorkflowCommand,
        _ctx: &WorkflowContext,
    ) -> Result<WorkflowStep> {
        use crate::config::WorkflowCommand;

        match cmd {
            WorkflowCommand::WorkflowStep(step) => {
                // Convert WorkflowStepCommand to WorkflowStep
                Ok(WorkflowStep {
                    name: None,
                    claude: step.claude.clone(),
                    shell: step.shell.clone(),
                    test: step.test.clone(),
                    goal_seek: step.goal_seek.clone(),
                    foreach: step.foreach.clone(),
                    command: None,
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: step.output_file.clone().map(std::path::PathBuf::from),
                    timeout: step.timeout,
                    capture_output: if step.capture_output.is_some() {
                        CaptureOutput::Variable("validation_output".to_string())
                    } else {
                        CaptureOutput::Disabled
                    },
                    on_failure: None,
                    retry: None,
                    on_success: None, // on_success conversion not supported to avoid recursion
                    on_exit_code: Default::default(),
                    commit_required: step.commit_required,
                    auto_commit: false,
                    commit_config: None,
                    working_dir: None,
                    env: Default::default(),
                    validate: step.validate.clone(),
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: step.when.clone(),
                })
            }
            WorkflowCommand::Simple(cmd_str) => {
                // Simple string command
                Ok(WorkflowStep {
                    name: None,
                    claude: None,
                    shell: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    command: Some(cmd_str.clone()),
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    timeout: None,
                    capture_output: CaptureOutput::Disabled,
                    on_failure: None,
                    retry: None,
                    on_success: None,
                    on_exit_code: Default::default(),
                    commit_required: false,
                    auto_commit: false,
                    commit_config: None,
                    working_dir: None,
                    env: Default::default(),
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                })
            }
            WorkflowCommand::Structured(cmd) => {
                // Structured Command -> WorkflowStep conversion
                Ok(WorkflowStep {
                    name: Some(cmd.name.clone()),
                    claude: None,
                    shell: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    command: Some(cmd.name.clone()),
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    timeout: cmd.metadata.timeout,
                    capture_output: CaptureOutput::Disabled,
                    on_failure: None,
                    retry: None, // Retry not supported in this conversion path
                    on_success: None,
                    on_exit_code: Default::default(),
                    commit_required: cmd.metadata.commit_required,
                    auto_commit: false,
                    commit_config: None,
                    working_dir: None,
                    env: Default::default(),
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                })
            }
            WorkflowCommand::SimpleObject(simple) => {
                // SimpleCommand -> WorkflowStep conversion
                Ok(WorkflowStep {
                    name: Some(simple.name.clone()),
                    claude: None,
                    shell: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    command: Some(simple.name.clone()),
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    timeout: None,
                    capture_output: CaptureOutput::Disabled,
                    on_failure: None,
                    retry: None,
                    on_success: None,
                    on_exit_code: Default::default(),
                    commit_required: simple.commit_required.unwrap_or(false),
                    auto_commit: false,
                    commit_config: None,
                    working_dir: None,
                    env: Default::default(),
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                })
            }
        }
    }

    // ========================================================================
    // Execution Tracking
    // ========================================================================

    /// Initialize git tracking for this step
    async fn initialize_step_tracking(
        &self,
        env: &ExecutionEnvironment,
        ctx: &WorkflowContext,
    ) -> Result<(crate::cook::commit_tracker::CommitTracker, String)> {
        // Track git changes - begin step
        let step_id = format!("step_{}", self.completed_steps.len());
        if let Some(ref git_tracker) = ctx.git_tracker {
            if let Ok(mut tracker) = git_tracker.lock() {
                let _ = tracker.begin_step(&step_id);
            }
        }

        // Initialize CommitTracker for this step using the executor's git operations (enables mocking)
        let git_ops = self.git_operations.clone();
        let working_dir = env.working_dir.to_path_buf();
        let mut commit_tracker =
            crate::cook::commit_tracker::CommitTracker::new(git_ops, working_dir);
        commit_tracker.initialize().await?;

        // Get the HEAD before step execution
        let before_head = commit_tracker.get_current_head().await?;

        Ok((commit_tracker, before_head))
    }

    /// Track commits and create auto-commit if needed
    async fn track_and_commit_changes(
        &self,
        step: &WorkflowStep,
        commit_tracker: &crate::cook::commit_tracker::CommitTracker,
        before_head: &str,
        ctx: &mut WorkflowContext,
    ) -> Result<Vec<TrackedCommit>> {
        // Track commits created during step execution
        let after_head = commit_tracker.get_current_head().await?;
        let step_name = self.get_step_display_name(step);
        let mut tracked_commits = commit_tracker
            .track_step_commits(&step_name, before_head, &after_head)
            .await?;

        // Create auto-commit if configured and changes exist
        if step.auto_commit && commit_tracker.has_changes().await? {
            let message_template = step
                .commit_config
                .as_ref()
                .and_then(|c| c.message_template.as_deref());
            let auto_commit = commit_tracker
                .create_auto_commit(
                    &step_name,
                    message_template,
                    &ctx.variables,
                    step.commit_config.as_ref(),
                )
                .await?;

            // Add auto-commit to tracked commits
            tracked_commits.push(auto_commit);
        }

        // Populate commit variables in context if we have commits
        let commit_vars = Self::build_commit_variables(&tracked_commits)?;
        ctx.variables.extend(commit_vars);

        Ok(tracked_commits)
    }

    /// Track git changes and update session state
    async fn track_and_update_session(&mut self, ctx: &WorkflowContext) -> Result<()> {
        // Track git changes - complete step
        let files_changed_count = if let Some(ref git_tracker) = ctx.git_tracker {
            if let Ok(mut tracker) = git_tracker.lock() {
                if let Ok(changes) = tracker.complete_step() {
                    // Log changes for debugging
                    tracing::debug!(
                        "Step git changes: {} added, {} modified, {} deleted, {} commits",
                        changes.files_added.len(),
                        changes.files_modified.len(),
                        changes.files_deleted.len(),
                        changes.commits.len()
                    );

                    // Count actual files changed
                    let count = changes.files_changed().len();
                    if count > 0 {
                        count
                    } else {
                        1
                    }
                } else {
                    // Fallback to counting 1 file changed as before
                    1
                }
            } else {
                // Fallback if we can't get lock
                1
            }
        } else {
            // No git tracker, use original behavior
            1
        };

        // Update session with file count (moved outside of lock scope)
        self.session_manager
            .update_session(SessionUpdate::AddFilesChanged(files_changed_count))
            .await?;

        Ok(())
    }

    /// Log step execution context for debugging and progress tracking
    fn log_step_execution_context(
        &self,
        step_name: &str,
        env: &ExecutionEnvironment,
        ctx: &WorkflowContext,
    ) {
        // Log verbose execution context at DEBUG level
        tracing::debug!("=== Step Execution Context ===");
        tracing::debug!("Step: {}", step_name);
        tracing::debug!("Working Directory: {}", env.working_dir.display());
        tracing::debug!("Project Directory: {}", env.project_dir.display());
        if let Some(ref worktree) = env.worktree_name {
            tracing::debug!("Worktree: {}", worktree);
        }
        tracing::debug!("Session ID: {}", env.session_id);

        // Log variables if any
        if !ctx.variables.is_empty() {
            tracing::debug!("Variables:");
            for (key, value) in &ctx.variables {
                let display_value = Self::format_variable_for_logging(value);
                tracing::debug!("  {} = {}", key, display_value);
            }
        }

        // Log captured outputs if any
        if !ctx.captured_outputs.is_empty() {
            tracing::debug!("Captured Outputs:");
            for (key, value) in &ctx.captured_outputs {
                let display_value = Self::format_variable_for_logging(value);
                tracing::debug!("  {} = {}", key, display_value);
            }
        }
    }

    /// Log step output for debugging
    pub(super) fn log_step_output(&self, step_result: &StepResult) {
        if tracing::enabled!(tracing::Level::DEBUG) {
            if !step_result.stdout.is_empty() {
                let stdout_lines: Vec<&str> = step_result.stdout.lines().collect();
                if stdout_lines.len() <= 20 || tracing::enabled!(tracing::Level::TRACE) {
                    tracing::debug!("Command stdout:\n{}", step_result.stdout);
                } else {
                    let preview: String = stdout_lines
                        .iter()
                        .take(10)
                        .chain(std::iter::once(&"... [output truncated] ..."))
                        .chain(stdout_lines.iter().rev().take(5).rev())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n");
                    tracing::debug!("Command stdout (abbreviated):\n{}", preview);
                }
            }

            if !step_result.stderr.is_empty() {
                let stderr_lines: Vec<&str> = step_result.stderr.lines().collect();
                if stderr_lines.len() <= 20 || tracing::enabled!(tracing::Level::TRACE) {
                    tracing::debug!("Command stderr:\n{}", step_result.stderr);
                } else {
                    let preview: String = stderr_lines
                        .iter()
                        .take(10)
                        .chain(std::iter::once(&"... [output truncated] ..."))
                        .chain(stderr_lines.iter().rev().take(5).rev())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("\n");
                    tracing::debug!("Command stderr (abbreviated):\n{}", preview);
                }
            }
        }
    }

    // ========================================================================
    // Result Handling
    // ========================================================================

    /// Capture command output to variable store if configured
    async fn capture_step_output(
        &self,
        step: &WorkflowStep,
        result: &StepResult,
        ctx: &mut WorkflowContext,
    ) -> Result<()> {
        if let Some(capture_name) = &step.capture {
            let command_result = variables::CommandResult {
                stdout: Some(result.stdout.clone()),
                stderr: Some(result.stderr.clone()),
                exit_code: result.exit_code.unwrap_or(-1),
                success: result.success,
                duration: std::time::Duration::from_secs(0), // TODO: Track actual duration
            };

            let capture_format = step.capture_format.unwrap_or_default();
            let capture_streams = &step.capture_streams;

            ctx.variable_store
                .capture_command_result(
                    capture_name,
                    command_result,
                    capture_format,
                    capture_streams,
                )
                .await
                .map_err(|e| anyhow!("Failed to capture command result: {}", e))?;

            // Also update captured_outputs for backward compatibility
            ctx.captured_outputs
                .insert(capture_name.clone(), result.stdout.clone());
        }

        Ok(())
    }

    /// Write command output to file if configured
    fn write_output_to_file(
        &self,
        step: &WorkflowStep,
        result: &StepResult,
        actual_env: &ExecutionEnvironment,
    ) -> Result<()> {
        if let Some(output_file) = &step.output_file {
            use std::fs;

            let output_path = if output_file.is_absolute() {
                output_file.clone()
            } else {
                actual_env.working_dir.join(output_file)
            };

            // Create parent directory if needed
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create output directory: {}", e))?;
            }

            // Write output to file
            fs::write(&output_path, &result.stdout)
                .map_err(|e| anyhow!("Failed to write output to file {:?}: {}", output_path, e))?;
        }

        Ok(())
    }

    /// Finalize step result by handling workflow failure logic
    fn finalize_step_result(
        &self,
        step: &WorkflowStep,
        mut result: StepResult,
    ) -> Result<StepResult> {
        // Check if we should fail the workflow based on the result using pure function
        let should_fail = Self::should_fail_workflow_for_step(&result, step);

        if should_fail {
            let error_msg = Self::build_step_error_message(step, &result);

            // Log full error details for debugging
            tracing::error!(
                "Step failed - Command: {}, Exit code: {:?}, Stderr length: {} bytes",
                self.get_step_display_name(step),
                result.exit_code,
                result.stderr.len()
            );
            if !result.stderr.is_empty() {
                tracing::error!("Step stderr: {}", result.stderr);
            }

            anyhow::bail!(error_msg);
        }

        // If the command failed but we're not failing the workflow (should_fail is false),
        // we need to modify the result to indicate success so the workflow continues
        if !result.success && !should_fail {
            result.success = true;
            result.stdout.push_str(
                "\n[Note: Command failed but workflow continues due to on_failure configuration]",
            );
        }

        Ok(result)
    }

    // ========================================================================
    // Retry Logic
    // ========================================================================

    /// Execute command with retry logic if configured
    async fn execute_with_retry_if_configured(
        &mut self,
        step: &WorkflowStep,
        command_type: &CommandType,
        actual_env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        if let Some(retry_config) = &step.retry {
            // Use enhanced retry executor
            let step_name = self.get_step_display_name(step);
            self.execute_with_enhanced_retry(
                retry_config.clone(),
                &step_name,
                command_type,
                step,
                actual_env,
                ctx,
                env_vars,
            )
            .await
        } else {
            // Execute without enhanced retry
            self.execute_command_by_type(command_type, step, actual_env, ctx, env_vars)
                .await
        }
    }

    /// Execute command with enhanced retry logic
    #[allow(clippy::too_many_arguments)]
    async fn execute_with_enhanced_retry(
        &mut self,
        retry_config: crate::cook::retry_v2::RetryConfig,
        step_name: &str,
        command_type: &CommandType,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        ctx: &mut WorkflowContext,
        env_vars: HashMap<String, String>,
    ) -> Result<StepResult> {
        let command_id = step_name.to_string();
        let mut retry_ctx =
            super::failure_handler::RetryContext::new(command_id.clone(), retry_config.attempts);

        // Manual retry loop since we can't clone self
        loop {
            retry_ctx.next_attempt();

            // Check if we've exhausted retries
            if !retry_ctx.should_continue() {
                let error_msg = super::failure_handler::build_retry_exhausted_message(
                    step_name,
                    retry_config.attempts,
                    retry_ctx.last_error.as_deref(),
                );
                return Err(anyhow::anyhow!(error_msg));
            }

            // Calculate delay if this is a retry
            if !retry_ctx.is_first_attempt() {
                let delay =
                    super::failure_handler::calculate_retry_delay(&retry_config, retry_ctx.attempt - 1);
                let jittered_delay =
                    super::failure_handler::apply_jitter(delay, retry_config.jitter_factor);

                let retry_msg = super::failure_handler::format_retry_message(
                    step_name,
                    retry_ctx.attempt,
                    retry_config.attempts,
                    jittered_delay,
                );
                self.user_interaction.display_info(&retry_msg);

                tokio::time::sleep(jittered_delay).await;
            }

            // Execute the command
            let attempt_start = std::time::Instant::now();
            match self
                .execute_command_by_type(command_type, step, env, ctx, env_vars.clone())
                .await
            {
                Ok(result) => {
                    if !retry_ctx.is_first_attempt() {
                        let success_msg = super::failure_handler::format_retry_success_message(
                            step_name,
                            retry_ctx.attempt,
                        );
                        self.user_interaction.display_info(&success_msg);

                        // Record successful retry attempt
                        let retry_attempt = super::failure_handler::create_retry_attempt(
                            retry_ctx.attempt,
                            attempt_start.elapsed(),
                            true,
                            None,
                            super::failure_handler::calculate_retry_delay(
                                &retry_config,
                                retry_ctx.attempt - 1,
                            ),
                            result.exit_code,
                        );
                        let _ = self
                            .retry_state_manager
                            .update_retry_state(&command_id, retry_attempt, &retry_config)
                            .await;
                    }
                    return Ok(result);
                }
                Err(err) => {
                    let error_str = err.to_string();

                    // Record failed retry attempt
                    let retry_attempt = super::failure_handler::create_retry_attempt(
                        retry_ctx.attempt,
                        attempt_start.elapsed(),
                        false,
                        Some(error_str.clone()),
                        if !retry_ctx.is_first_attempt() {
                            super::failure_handler::calculate_retry_delay(
                                &retry_config,
                                retry_ctx.attempt - 1,
                            )
                        } else {
                            Duration::from_secs(0)
                        },
                        None,
                    );
                    let _ = self
                        .retry_state_manager
                        .update_retry_state(&command_id, retry_attempt, &retry_config)
                        .await;

                    // Check if we should retry this error
                    if !super::failure_handler::should_attempt_retry(&retry_ctx, &error_str, &retry_config)
                    {
                        return Err(err);
                    }

                    let failure_msg = super::failure_handler::format_retry_failure_message(
                        retry_ctx.attempt,
                        retry_config.attempts,
                        &error_str,
                    );
                    self.user_interaction.display_warning(&failure_msg);

                    retry_ctx.record_error(error_str);
                }
            }
        }
    }
}
