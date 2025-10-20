//! Workflow resume executor
//!
//! Handles resuming interrupted workflows from checkpoints.

use crate::config::WorkflowConfig;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::checkpoint::{
    self, CheckpointManager, ResumeOptions, WorkflowCheckpoint,
};
use crate::cook::workflow::error_recovery::{
    on_failure_to_error_handler, RecoveryAction, ResumeError, ResumeErrorRecovery,
};
use crate::cook::workflow::executor::{
    WorkflowContext, WorkflowExecutor as WorkflowExecutorImpl, WorkflowStep,
};
use crate::cook::workflow::normalized::NormalizedWorkflow;
use crate::cook::workflow::progress::{ExecutionPhase, ProgressDisplay, SequentialProgressTracker};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

/// Result of resuming a workflow
#[derive(Debug)]
pub struct ResumeResult {
    /// Whether resume was successful
    pub success: bool,
    /// Total steps executed (including resumed)
    pub total_steps_executed: usize,
    /// Steps that were skipped (already completed)
    pub skipped_steps: usize,
    /// Steps executed in this resume
    pub new_steps_executed: usize,
    /// Final workflow context
    pub final_context: WorkflowContext,
}

/// Executor for resuming workflows from checkpoints
pub struct ResumeExecutor {
    /// Checkpoint manager for loading/saving
    checkpoint_manager: Arc<CheckpointManager>,
    /// Claude executor for commands
    claude_executor: Option<Arc<dyn ClaudeExecutor>>,
    /// Session manager
    session_manager: Option<Arc<dyn SessionManager>>,
    /// User interaction
    user_interaction: Option<Arc<dyn UserInteraction>>,
    /// Error recovery manager
    error_recovery: ResumeErrorRecovery,
}

impl ResumeExecutor {
    /// Create a new resume executor
    pub fn new(checkpoint_manager: Arc<CheckpointManager>) -> Self {
        Self {
            checkpoint_manager,
            claude_executor: None,
            session_manager: None,
            user_interaction: None,
            error_recovery: ResumeErrorRecovery::new(),
        }
    }

    /// Set the executors for workflow execution
    pub fn with_executors(
        mut self,
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        self.claude_executor = Some(claude_executor);
        self.session_manager = Some(session_manager);
        self.user_interaction = Some(user_interaction);
        self
    }

    /// Resume a workflow from checkpoint
    pub async fn resume(
        &mut self,
        workflow_id: &str,
        options: ResumeOptions,
    ) -> Result<ResumeResult> {
        info!("Resuming workflow {}", workflow_id);

        // Load checkpoint to get workflow path
        let checkpoint = self
            .checkpoint_manager
            .load_checkpoint(workflow_id)
            .await
            .context("Failed to load checkpoint")?;

        // Get workflow path from checkpoint or error if not available
        let workflow_path = checkpoint.workflow_path
            .clone()
            .ok_or_else(|| anyhow!(
                "Workflow path not stored in checkpoint. Please use resume_with_path() with explicit path."
            ))?;

        // Check if the workflow file exists
        if !workflow_path.exists() {
            return Err(anyhow!(
                "Workflow file not found at {:?}. The file may have been moved or deleted.",
                workflow_path
            ));
        }

        // Delegate to execute_from_checkpoint for full execution
        self.execute_from_checkpoint(workflow_id, &workflow_path, options)
            .await
    }

    /// Resume a workflow from checkpoint with explicit workflow path
    /// Use this when the checkpoint doesn't have the workflow path stored (legacy checkpoints)
    pub async fn resume_with_path(
        &mut self,
        workflow_id: &str,
        workflow_path: &PathBuf,
        options: ResumeOptions,
    ) -> Result<ResumeResult> {
        info!(
            "Resuming workflow {} with explicit path {:?}",
            workflow_id, workflow_path
        );

        // Verify the workflow file exists
        if !workflow_path.exists() {
            return Err(anyhow!("Workflow file not found at {:?}", workflow_path));
        }

        // Delegate to execute_from_checkpoint for full execution
        self.execute_from_checkpoint(workflow_id, workflow_path, options)
            .await
    }

    /// Validate checkpoint integrity and compatibility
    fn validate_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
        // Check checkpoint version compatibility
        if checkpoint.version > checkpoint::CHECKPOINT_VERSION {
            return Err(anyhow!(
                "Checkpoint version {} is not supported",
                checkpoint.version
            ));
        }

        // Validate execution state consistency
        if checkpoint.execution_state.current_step_index > checkpoint.execution_state.total_steps {
            return Err(anyhow!("Invalid checkpoint: step index out of bounds"));
        }

        // Validate completed steps match current index
        if checkpoint.completed_steps.len() > checkpoint.execution_state.current_step_index {
            return Err(anyhow!(
                "Checkpoint inconsistency: completed steps exceed current index"
            ));
        }

        Ok(())
    }

    /// Restore workflow context from checkpoint
    pub fn restore_workflow_context(
        &self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<WorkflowContext> {
        use crate::cook::workflow::variable_checkpoint::VariableResumeManager;

        let mut context = WorkflowContext::default();
        let manager = VariableResumeManager::new();

        // Attempt to restore from enhanced checkpoint state, with automatic migration from legacy
        match self.restore_variables_unified(&manager, &mut context, checkpoint) {
            Ok(()) => {
                info!("Variables restored successfully from checkpoint");
            }
            Err(e) => {
                warn!("Failed to restore variables from checkpoint: {}", e);
                // If unified restoration fails, try to recover with minimal state
                context = WorkflowContext::default();
            }
        }

        Ok(context)
    }

    /// Unified variable restoration that handles both enhanced and legacy checkpoints
    fn restore_variables_unified(
        &self,
        manager: &crate::cook::workflow::variable_checkpoint::VariableResumeManager,
        context: &mut WorkflowContext,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<()> {
        // Try enhanced checkpoint state first
        if let Some(var_checkpoint_state) = &checkpoint.variable_checkpoint_state {
            info!("Restoring from enhanced checkpoint format");

            // Restore variables from enhanced checkpoint
            let (mut variables, captured_outputs, iteration_vars) = manager
                .restore_from_checkpoint(var_checkpoint_state)
                .context("Failed to restore from enhanced checkpoint")?;

            context.variables = variables.clone();
            context.captured_outputs = captured_outputs;
            context.iteration_vars = iteration_vars;

            // Validate environment compatibility
            if let Ok(compatibility) =
                manager.validate_environment(&var_checkpoint_state.environment_snapshot)
            {
                if !compatibility.is_compatible {
                    warn!("Environment has changed since checkpoint creation");
                    if !compatibility.missing_variables.is_empty() {
                        warn!(
                            "Missing environment variables: {:?}",
                            compatibility.missing_variables.keys().collect::<Vec<_>>()
                        );
                    }
                    if !compatibility.changed_variables.is_empty() {
                        warn!(
                            "Changed environment variables: {:?}",
                            compatibility.changed_variables.keys().collect::<Vec<_>>()
                        );
                    }
                }
            }

            // Restore/recalculate MapReduce variables if applicable
            self.restore_mapreduce_variables(manager, &mut variables, context, checkpoint)?;

            info!(
                "Restored {} variables from enhanced checkpoint",
                context.variables.len()
            );
        } else {
            // Migrate from legacy format
            info!("Migrating from legacy checkpoint format");
            self.migrate_from_legacy_checkpoint(context, checkpoint)?;
        }

        Ok(())
    }

    /// Restore MapReduce variables from checkpoint state
    fn restore_mapreduce_variables(
        &self,
        manager: &crate::cook::workflow::variable_checkpoint::VariableResumeManager,
        variables: &mut HashMap<String, String>,
        context: &mut WorkflowContext,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<()> {
        if let Some(ref mapreduce_state) = checkpoint.mapreduce_state {
            let total_items = mapreduce_state.total_items;
            let successful_items = mapreduce_state.completed_items.len();
            let failed_items = mapreduce_state.failed_items.len();

            // Recalculate MapReduce aggregate variables
            let mapreduce_vars = manager.recalculate_mapreduce_variables(
                total_items,
                successful_items,
                failed_items,
            );

            // Merge MapReduce variables into context
            for (key, value) in mapreduce_vars {
                variables.insert(key.clone(), value.clone());
                context.variables.insert(key, value);
            }

            // Also restore any saved aggregate variables to ensure consistency
            for (key, value) in &mapreduce_state.aggregate_variables {
                context.variables.insert(key.clone(), value.clone());
            }

            info!(
                "Restored MapReduce variables: total={}, successful={}, failed={}",
                total_items, successful_items, failed_items
            );
        }

        Ok(())
    }

    /// Migrate variables from legacy checkpoint format
    fn migrate_from_legacy_checkpoint(
        &self,
        context: &mut WorkflowContext,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<()> {
        // Restore variables from legacy format
        for (key, value) in &checkpoint.variable_state {
            match value {
                Value::String(s) => {
                    context.variables.insert(key.clone(), s.clone());
                }
                Value::Number(n) => {
                    context.variables.insert(key.clone(), n.to_string());
                }
                Value::Bool(b) => {
                    context.variables.insert(key.clone(), b.to_string());
                }
                _ => {
                    // For complex values, store as JSON
                    context
                        .variables
                        .insert(key.clone(), serde_json::to_string(value)?);
                }
            }
        }

        // Restore captured outputs from completed steps
        for step in &checkpoint.completed_steps {
            if let Some(ref output) = step.output {
                context
                    .captured_outputs
                    .insert(format!("step_{}", step.step_index), output.clone());

                // Also restore step-specific variables
                for (var_key, var_value) in &step.captured_variables {
                    context.variables.insert(var_key.clone(), var_value.clone());
                }
            }
        }

        info!(
            "Migrated {} variables from legacy checkpoint format",
            context.variables.len()
        );

        Ok(())
    }

    /// Execute workflow from checkpoint with full execution support
    pub async fn execute_from_checkpoint(
        &mut self,
        workflow_id: &str,
        workflow_path: &PathBuf,
        options: ResumeOptions,
    ) -> Result<ResumeResult> {
        // Ensure we have executors
        let claude_executor = self
            .claude_executor
            .as_ref()
            .ok_or_else(|| anyhow!("Claude executor not configured for resume"))?;
        let session_manager = self
            .session_manager
            .as_ref()
            .ok_or_else(|| anyhow!("Session manager not configured for resume"))?;
        let user_interaction = self
            .user_interaction
            .as_ref()
            .ok_or_else(|| anyhow!("User interaction not configured for resume"))?;

        info!("Executing workflow {} from checkpoint", workflow_id);

        // Load checkpoint
        let checkpoint = self
            .checkpoint_manager
            .load_checkpoint(workflow_id)
            .await
            .context("Failed to load checkpoint")?;

        // Validate checkpoint
        if !options.skip_validation {
            self.validate_checkpoint(&checkpoint)?;
        }

        // Check if workflow is already complete
        if checkpoint.execution_state.status == checkpoint::WorkflowStatus::Completed
            && !options.force
        {
            println!(
                "Workflow {} is already completed - nothing to resume",
                workflow_id
            );
            return Ok(ResumeResult {
                success: true,
                total_steps_executed: checkpoint.execution_state.current_step_index,
                skipped_steps: checkpoint.execution_state.current_step_index,
                new_steps_executed: 0,
                final_context: WorkflowContext::default(),
            });
        }

        // Create progress tracker for resume
        let total_steps = checkpoint.total_steps;
        let skipped_steps = checkpoint.completed_steps.len();
        let current_iteration = checkpoint.execution_state.current_iteration.unwrap_or(1);
        let max_iterations = checkpoint.execution_state.total_iterations.unwrap_or(1);

        let mut progress_tracker = SequentialProgressTracker::for_resume(
            workflow_id.to_string(),
            checkpoint
                .workflow_name
                .clone()
                .unwrap_or_else(|| workflow_id.to_string()),
            total_steps,
            max_iterations,
            skipped_steps,
            current_iteration,
        );

        // Create progress display
        let mut progress_display = ProgressDisplay::new();

        // Set initial phase
        progress_tracker
            .update_phase(ExecutionPhase::LoadingCheckpoint)
            .await;
        progress_display.force_update(&format!("Loading checkpoint for workflow {}", workflow_id));

        // Load the workflow file
        progress_tracker
            .update_phase(ExecutionPhase::RestoringState)
            .await;
        progress_display.force_update("Loading workflow file and restoring state...");

        let workflow_content = tokio::fs::read_to_string(workflow_path)
            .await
            .context("Failed to read workflow file")?;

        // Parse workflow based on type
        let workflow_config: WorkflowConfig = if workflow_path.extension().and_then(|s| s.to_str())
            == Some("yml")
            || workflow_path.extension().and_then(|s| s.to_str()) == Some("yaml")
        {
            serde_yaml::from_str(&workflow_content)?
        } else if workflow_path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&workflow_content)?
        } else {
            return Err(anyhow!("Unsupported workflow file format"));
        };

        // Convert workflow commands to steps
        let steps = workflow_config
            .commands
            .into_iter()
            .map(|cmd| {
                let mut step = crate::cook::workflow::executor::WorkflowStep {
                    name: None,
                    claude: None,
                    shell: None,
                    test: None,
                    goal_seek: None,
                    foreach: None,
                    write_file: None,
                    command: None,
                    handler: None,
                    capture: None,
                    capture_format: None,
                    capture_streams: Default::default(),
                    output_file: None,
                    capture_output: crate::cook::workflow::executor::CaptureOutput::Disabled,
                    timeout: None,
                    working_dir: None,
                    env: std::collections::HashMap::new(),
                    on_failure: None,
                    retry: None,
                    on_success: None,
                    on_exit_code: std::collections::HashMap::new(),
                    auto_commit: false,
                    commit_config: None,
                    commit_required: false,
                    validate: None,
                    step_validate: None,
                    skip_validation: false,
                    validation_timeout: None,
                    ignore_validation_failure: false,
                    when: None,
                };

                // Parse command based on enum variant
                match cmd {
                    crate::config::WorkflowCommand::Simple(cmd_str) => {
                        if cmd_str.starts_with("claude:") {
                            step.claude =
                                Some(cmd_str.strip_prefix("claude:").unwrap().trim().to_string());
                        } else if cmd_str.starts_with("shell:") {
                            step.shell =
                                Some(cmd_str.strip_prefix("shell:").unwrap().trim().to_string());
                        } else if !cmd_str.contains(':') {
                            // Default to shell if no prefix
                            step.shell = Some(cmd_str);
                        } else {
                            // Treat as legacy command
                            step.command = Some(cmd_str);
                        }
                    }
                    crate::config::WorkflowCommand::WorkflowStep(wf_step) => {
                        step.claude = wf_step.claude;
                        step.shell = wf_step.shell;
                        // Convert TestDebugConfig to OnFailureConfig
                        if let Some(test_debug) = wf_step.on_failure {
                            // Create a HandlerCommand from the TestDebugConfig
                            let handler_cmd = crate::cook::workflow::on_failure::HandlerCommand {
                                claude: Some(test_debug.claude),
                                shell: None,
                                continue_on_error: false,
                            };

                            step.on_failure = Some(crate::cook::workflow::on_failure::OnFailureConfig::Detailed(
                                crate::cook::workflow::on_failure::FailureHandlerConfig {
                                    commands: vec![handler_cmd],
                                    strategy: crate::cook::workflow::on_failure::HandlerStrategy::default(),
                                    timeout: None,
                                    capture: std::collections::HashMap::new(),
                                    fail_workflow: test_debug.fail_workflow,
                                    handler_failure_fatal: false,
                                }
                            ));
                        }
                        // Copy other fields if they exist
                    }
                    _ => {
                        // For other variants, try to convert to a command string
                        step.command = Some(format!("{:?}", cmd));
                    }
                }

                step
            })
            .collect();

        // Convert to extended workflow config
        let extended_workflow = crate::cook::workflow::executor::ExtendedWorkflowConfig {
            name: checkpoint
                .workflow_name
                .clone()
                .unwrap_or_else(|| "resumed".to_string()),
            steps,
            mode: crate::cook::workflow::executor::WorkflowMode::Sequential,
            max_iterations: 1,
            iterate: false,
            setup_phase: None,    // Not a MapReduce workflow
            map_phase: None,      // Not a MapReduce workflow
            reduce_phase: None,   // Not a MapReduce workflow
            retry_defaults: None, // Would need to be loaded from checkpoint
            environment: None,    // Would need to be loaded from checkpoint
        };

        // Create execution environment
        let env = ExecutionEnvironment {
            working_dir: Arc::new(
                workflow_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf(),
            ),
            project_dir: Arc::new(
                workflow_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .to_path_buf(),
            ),
            worktree_name: None,
            session_id: Arc::from(format!("resume-{}", workflow_id)),
        };

        // Restore workflow context
        let mut workflow_context = self.restore_workflow_context(&checkpoint)?;

        // Create workflow executor with checkpoint support
        let mut executor = WorkflowExecutorImpl::new(
            claude_executor.clone(),
            session_manager.clone(),
            user_interaction.clone(),
        )
        .with_workflow_path(workflow_path.clone())
        .with_checkpoint_manager(self.checkpoint_manager.clone(), workflow_id.to_string());

        // Execute remaining steps
        let start_from = checkpoint.execution_state.current_step_index;
        let total_steps = extended_workflow.steps.len();
        let mut steps_executed = 0;

        info!(
            "Resuming execution from step {} of {}",
            start_from + 1,
            total_steps
        );

        // Update progress to executing phase
        progress_tracker
            .update_phase(ExecutionPhase::ExecutingSteps)
            .await;
        progress_display.force_update(&format!(
            "Resuming from step {}/{}. {} steps already completed.",
            start_from + 1,
            total_steps,
            skipped_steps
        ));

        // Set progress callback to display updates
        progress_tracker.set_callback(move |update| {
            if let Some(ref msg) = update.message {
                println!("📊 {}", msg);
            }
        });

        // Start iteration tracking if needed
        if current_iteration > 0 {
            progress_tracker.start_iteration(current_iteration).await;
        }

        // Skip completed steps and execute remaining ones
        for (step_index, step) in extended_workflow.steps.iter().enumerate() {
            if step_index < start_from {
                info!("Skipping completed step {}: {:?}", step_index + 1, step);
                progress_tracker
                    .skip_step(step_index, "Already completed from checkpoint".to_string())
                    .await;
                continue;
            }

            // Get step name for progress display
            let step_name = get_step_name(step);

            info!(
                "Executing step {}/{}: {}",
                step_index + 1,
                total_steps,
                step_name
            );

            // Start step in progress tracker
            progress_tracker
                .start_step(step_index, step_name.clone())
                .await;

            // Update progress display
            let progress_msg = progress_tracker.format_progress().await;
            progress_display.update(&progress_msg);

            // Execute the step with error recovery
            let step_start = std::time::Instant::now();
            match executor
                .execute_step(step, &env, &mut workflow_context)
                .await
            {
                Ok(_result) => {
                    steps_executed += 1;
                    let duration = step_start.elapsed();
                    info!("Step {} completed successfully", step_index + 1);

                    // Update progress tracker
                    progress_tracker.complete_step(step_index, duration).await;
                }
                Err(e) => {
                    warn!("Step {} failed: {}", step_index + 1, e);

                    // Update progress tracker for failure
                    progress_tracker.fail_step(step_index, e.to_string()).await;

                    // Check if step has error handler
                    if let Some(ref on_failure) = step.on_failure {
                        info!("Executing error handler for step {}", step_index + 1);

                        // Convert OnFailureConfig to ErrorHandler
                        if let Some(handler) = on_failure_to_error_handler(on_failure, step_index) {
                            // Execute error handler with resume context
                            match self
                                .error_recovery
                                .execute_error_handler_with_resume_context(
                                    &handler,
                                    &e.to_string(),
                                    &mut workflow_context,
                                )
                                .await
                            {
                                Ok(true) => {
                                    info!("Error handler succeeded, continuing workflow");
                                    steps_executed += 1;
                                    continue; // Continue to next step
                                }
                                Ok(false) => {
                                    warn!("Error handler failed for step {}", step_index + 1);
                                    if !on_failure.should_fail_workflow() {
                                        warn!("Continuing despite handler failure");
                                        continue;
                                    }
                                }
                                Err(handler_err) => {
                                    error!("Error executing handler: {}", handler_err);
                                }
                            }
                        }
                    }

                    // No handler or handler failed - attempt recovery
                    let recovery_action = self
                        .error_recovery
                        .handle_resume_error(&ResumeError::Other(anyhow!("{}", e)), &checkpoint)
                        .await?;

                    match recovery_action {
                        RecoveryAction::Retry { delay, .. } => {
                            warn!("Retrying step {} after {:?}", step_index + 1, delay);
                            tokio::time::sleep(delay).await;
                            // Retry the current step
                            match executor
                                .execute_step(step, &env, &mut workflow_context)
                                .await
                            {
                                Ok(_) => {
                                    info!("Retry succeeded for step {}", step_index + 1);
                                    steps_executed += 1;
                                    continue;
                                }
                                Err(retry_err) => {
                                    error!(
                                        "Retry failed for step {}: {}",
                                        step_index + 1,
                                        retry_err
                                    );
                                    return Err(retry_err);
                                }
                            }
                        }
                        RecoveryAction::Continue => {
                            warn!("Continuing despite error in step {}", step_index + 1);
                            continue;
                        }
                        RecoveryAction::SafeAbort { cleanup_actions } => {
                            error!("Aborting workflow due to unrecoverable error");

                            // Execute cleanup actions if any
                            if !cleanup_actions.is_empty() {
                                info!(
                                    "Executing {} cleanup actions before abort",
                                    cleanup_actions.len()
                                );
                                for action in cleanup_actions {
                                    // Create a WorkflowStep from the handler command
                                    let cleanup_step = WorkflowStep {
                                        name: Some("cleanup".to_string()),
                                        shell: action.shell.clone(),
                                        claude: action.claude.clone(),
                                        test: None,
                                        goal_seek: None,
                                        foreach: None,
                                        write_file: None,
                                        command: None,
                                        handler: None,
                                        capture: None,
                                        capture_format: None,
                                        capture_streams: Default::default(),
                                        output_file: None,
                                        timeout: None,
                                        capture_output:
                                            crate::cook::workflow::executor::CaptureOutput::Disabled,
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
                                    };

                                    // Execute the cleanup step
                                    let _ = executor
                                        .execute_step(&cleanup_step, &env, &mut workflow_context)
                                        .await;
                                }
                            }

                            return Err(e);
                        }
                        RecoveryAction::Fallback { alternative_path } => {
                            warn!("Attempting fallback path: {}", alternative_path);
                            // Load alternative workflow configuration from file
                            let alt_path = std::path::Path::new(&alternative_path);
                            let content =
                                tokio::fs::read_to_string(&alt_path).await.map_err(|err| {
                                    anyhow!("Failed to read fallback workflow file: {}", err)
                                })?;

                            let alt_config: WorkflowConfig = serde_yaml::from_str(&content)
                                .map_err(|err| {
                                    anyhow!("Failed to parse fallback workflow: {}", err)
                                })?;

                            // Convert to normalized workflow
                            let _alt_workflow = NormalizedWorkflow::from_workflow_config(
                                &alt_config,
                                crate::cook::workflow::normalized::ExecutionMode::Sequential,
                            )?;

                            // Execute the fallback workflow from the current step
                            info!("Executing fallback workflow from step {}", step_index);
                            // Note: This would need more implementation to properly merge contexts
                            // For now, we'll just continue with the current workflow
                            warn!("Fallback workflow execution not fully implemented, continuing with current workflow");
                            continue;
                        }
                        RecoveryAction::PartialResume { from_step } => {
                            warn!("Performing partial resume from step {}", from_step);
                            // Jump to the specified step
                            if from_step < step_index {
                                // We've already passed this step, continue normally
                                continue;
                            } else if from_step > step_index {
                                // Skip ahead to the specified step
                                for i in step_index..from_step {
                                    progress_tracker
                                        .skip_step(i, "Skipping to recovery point".to_string())
                                        .await;
                                }
                                continue;
                            }
                            // If from_step == step_index, just continue normally
                            continue;
                        }
                        RecoveryAction::RequestIntervention { message } => {
                            error!("Manual intervention required: {}", message);

                            // Save checkpoint with intervention request
                            self.checkpoint_manager
                                .save_intervention_request(workflow_id, &message)
                                .await?;

                            // Return error with intervention message
                            return Err(anyhow!(
                                "Workflow suspended for manual intervention: {}. Resume with 'prodigy resume {}' after resolving.",
                                message,
                                workflow_id
                            ));
                        }
                    }
                }
            }
        }

        // Update progress to completed
        progress_tracker
            .update_phase(ExecutionPhase::Completed)
            .await;
        let final_progress = progress_tracker.format_progress().await;
        progress_display.force_update(&final_progress);

        // Show final summary
        let total_duration = progress_tracker.start_time.elapsed();
        println!("\n✅ Workflow Resume Complete!");
        println!("   Total steps: {}", total_steps);
        println!("   Steps skipped (already completed): {}", start_from);
        println!("   Steps executed in this session: {}", steps_executed);
        println!("   Total duration: {:.2}s", total_duration.as_secs_f64());
        if steps_executed > 0 {
            let avg_step_time = total_duration.as_secs_f64() / steps_executed as f64;
            println!("   Average step time: {:.2}s", avg_step_time);
        }

        // Delete checkpoint on successful completion
        self.checkpoint_manager
            .delete_checkpoint(workflow_id)
            .await?;

        info!(
            "Workflow {} completed successfully. Executed {} new steps.",
            workflow_id, steps_executed
        );

        Ok(ResumeResult {
            success: true,
            total_steps_executed: total_steps,
            skipped_steps: start_from,
            new_steps_executed: steps_executed,
            final_context: workflow_context,
        })
    }
}

/// List all resumable workflows
pub async fn list_resumable_workflows(checkpoint_dir: PathBuf) -> Result<Vec<ResumableWorkflow>> {
    use crate::cook::workflow::checkpoint_path::CheckpointStorage;

    #[allow(deprecated)]
    let manager = CheckpointManager::with_storage(CheckpointStorage::Local(checkpoint_dir));
    let workflow_ids = manager.list_checkpoints().await?;

    let mut resumable = Vec::new();
    for workflow_id in workflow_ids {
        if let Ok(checkpoint) = manager.load_checkpoint(&workflow_id).await {
            resumable.push(ResumableWorkflow {
                workflow_id,
                status: format!("{:?}", checkpoint.execution_state.status),
                progress: format!(
                    "{}/{}",
                    checkpoint.execution_state.current_step_index,
                    checkpoint.execution_state.total_steps
                ),
                last_checkpoint: checkpoint.timestamp,
                can_resume: checkpoint.execution_state.status
                    != checkpoint::WorkflowStatus::Completed,
            });
        }
    }

    Ok(resumable)
}

/// Information about a resumable workflow
#[derive(Debug)]
pub struct ResumableWorkflow {
    pub workflow_id: String,
    pub status: String,
    pub progress: String,
    pub last_checkpoint: chrono::DateTime<chrono::Utc>,
    pub can_resume: bool,
}

/// Helper function to get a display name for a workflow step
fn get_step_name(step: &crate::cook::workflow::executor::WorkflowStep) -> String {
    if let Some(ref name) = step.name {
        name.clone()
    } else if let Some(ref claude) = step.claude {
        format!("claude: {}", claude)
    } else if let Some(ref shell) = step.shell {
        format!("shell: {}", shell)
    } else if let Some(ref cmd) = step.command {
        cmd.clone()
    } else {
        "unnamed step".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio;

    #[tokio::test]
    async fn test_resume_with_workflow_path_in_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let checkpoint_dir = temp_dir.path().join("checkpoints");
        tokio::fs::create_dir_all(&checkpoint_dir).await.unwrap();

        // Create a test workflow file
        let workflow_path = temp_dir.path().join("test.yml");
        tokio::fs::write(&workflow_path, "name: test\nsteps:\n  - shell: echo test")
            .await
            .unwrap();

        // Create a checkpoint with workflow path
        let checkpoint = WorkflowCheckpoint {
            workflow_id: "test-workflow".to_string(),
            workflow_path: Some(workflow_path.clone()),
            execution_state: checkpoint::ExecutionState {
                current_step_index: 0,
                total_steps: 1,
                status: checkpoint::WorkflowStatus::Interrupted,
                start_time: chrono::Utc::now(),
                last_checkpoint: chrono::Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: Vec::new(),
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: chrono::Utc::now(),
            variable_checkpoint_state: None,
            version: checkpoint::CHECKPOINT_VERSION,
            workflow_hash: "test-hash".to_string(),
            total_steps: 1,
            workflow_name: Some("test".to_string()),
            error_recovery_state: None,
            retry_checkpoint_state: None,
        };

        // Save checkpoint
        #[allow(deprecated)]
        let checkpoint_manager = Arc::new(CheckpointManager::new(checkpoint_dir));
        checkpoint_manager
            .save_checkpoint(&checkpoint)
            .await
            .unwrap();

        // Create resume executor
        let mut executor = ResumeExecutor::new(checkpoint_manager.clone());

        // Test resume - should succeed with workflow path from checkpoint
        let options = ResumeOptions::default();
        let result = executor.resume("test-workflow", options).await;

        // Should fail because we don't have executors set up, but it should get past the workflow path check
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("executor not configured")
                || error_msg.contains("not configured for resume")
        );
    }

    #[tokio::test]
    async fn test_resume_without_workflow_path_in_checkpoint() {
        let temp_dir = TempDir::new().unwrap();
        let checkpoint_dir = temp_dir.path().join("checkpoints");
        tokio::fs::create_dir_all(&checkpoint_dir).await.unwrap();

        // Create a checkpoint WITHOUT workflow path (legacy checkpoint)
        let checkpoint = WorkflowCheckpoint {
            workflow_id: "legacy-workflow".to_string(),
            workflow_path: None, // No workflow path stored
            execution_state: checkpoint::ExecutionState {
                current_step_index: 0,
                total_steps: 1,
                status: checkpoint::WorkflowStatus::Interrupted,
                start_time: chrono::Utc::now(),
                last_checkpoint: chrono::Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: Vec::new(),
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: chrono::Utc::now(),
            variable_checkpoint_state: None,
            version: checkpoint::CHECKPOINT_VERSION,
            workflow_hash: "test-hash".to_string(),
            total_steps: 1,
            workflow_name: Some("test".to_string()),
            error_recovery_state: None,
            retry_checkpoint_state: None,
        };

        // Save checkpoint
        #[allow(deprecated)]
        let checkpoint_manager = Arc::new(CheckpointManager::new(checkpoint_dir));
        checkpoint_manager
            .save_checkpoint(&checkpoint)
            .await
            .unwrap();

        // Create resume executor
        let mut executor = ResumeExecutor::new(checkpoint_manager.clone());

        // Test resume without path - should fail with helpful error
        let options = ResumeOptions::default();
        let result = executor.resume("legacy-workflow", options).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("resume_with_path"));
    }

    #[tokio::test]
    async fn test_resume_with_path_explicit() {
        let temp_dir = TempDir::new().unwrap();
        let checkpoint_dir = temp_dir.path().join("checkpoints");
        tokio::fs::create_dir_all(&checkpoint_dir).await.unwrap();

        // Create a test workflow file
        let workflow_path = temp_dir.path().join("test.yml");
        tokio::fs::write(&workflow_path, "name: test\nsteps:\n  - shell: echo test")
            .await
            .unwrap();

        // Create a checkpoint WITHOUT workflow path
        let checkpoint = WorkflowCheckpoint {
            workflow_id: "explicit-path-workflow".to_string(),
            workflow_path: None,
            execution_state: checkpoint::ExecutionState {
                current_step_index: 0,
                total_steps: 1,
                status: checkpoint::WorkflowStatus::Interrupted,
                start_time: chrono::Utc::now(),
                last_checkpoint: chrono::Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: Vec::new(),
            variable_state: HashMap::new(),
            mapreduce_state: None,
            timestamp: chrono::Utc::now(),
            variable_checkpoint_state: None,
            version: checkpoint::CHECKPOINT_VERSION,
            workflow_hash: "test-hash".to_string(),
            total_steps: 1,
            workflow_name: Some("test".to_string()),
            error_recovery_state: None,
            retry_checkpoint_state: None,
        };

        // Save checkpoint
        #[allow(deprecated)]
        let checkpoint_manager = Arc::new(CheckpointManager::new(checkpoint_dir));
        checkpoint_manager
            .save_checkpoint(&checkpoint)
            .await
            .unwrap();

        // Create resume executor
        let mut executor = ResumeExecutor::new(checkpoint_manager.clone());

        // Test resume_with_path - should work with explicit path
        let options = ResumeOptions::default();
        let result = executor
            .resume_with_path("explicit-path-workflow", &workflow_path, options)
            .await;

        // Should fail because we don't have executors set up, but it should get past the workflow path check
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("executor not configured")
                || error_msg.contains("not configured for resume")
        );
    }
}
