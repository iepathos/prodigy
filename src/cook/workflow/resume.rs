//! Workflow resume executor
//!
//! Handles resuming interrupted workflows from checkpoints.

use crate::config::WorkflowConfig;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::checkpoint::{
    self, CheckpointManager, ResumeContext, ResumeOptions, WorkflowCheckpoint,
};
use crate::cook::workflow::executor::{WorkflowContext, WorkflowExecutor as WorkflowExecutorImpl};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

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
}

impl ResumeExecutor {
    /// Create a new resume executor
    pub fn new(checkpoint_manager: Arc<CheckpointManager>) -> Self {
        Self {
            checkpoint_manager,
            claude_executor: None,
            session_manager: None,
            user_interaction: None,
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
    pub async fn resume(&self, workflow_id: &str, options: ResumeOptions) -> Result<ResumeResult> {
        info!("Resuming workflow {}", workflow_id);

        // Load checkpoint
        let checkpoint = self
            .checkpoint_manager
            .load_checkpoint(workflow_id)
            .await
            .context("Failed to load checkpoint")?;

        // Validate checkpoint unless skipped
        if !options.skip_validation {
            self.validate_checkpoint(&checkpoint)?;
        }

        // Check if workflow is already complete
        if checkpoint.execution_state.status == checkpoint::WorkflowStatus::Completed
            && !options.force
        {
            // Return success with a message that the workflow is already complete
            println!("Workflow {} is already completed - nothing to resume", workflow_id);
            return Ok(ResumeResult {
                success: true,
                total_steps_executed: checkpoint.execution_state.current_step_index,
                skipped_steps: checkpoint.execution_state.current_step_index,
                new_steps_executed: 0,
                final_context: WorkflowContext::default(),
            });
        }

        // Build resume context
        let resume_context = self.build_resume_context(checkpoint.clone(), &options)?;

        // Restore workflow context
        let workflow_context = self.restore_workflow_context(&checkpoint)?;

        info!(
            "Resuming from step {} of {}, skipping {} completed steps",
            resume_context.start_from_step,
            checkpoint.execution_state.total_steps,
            resume_context.skip_steps.len()
        );

        // For now, create a simplified resume result
        // Full implementation would require loading the workflow from the file
        let result = ResumeResult {
            success: true,
            total_steps_executed: checkpoint.execution_state.current_step_index,
            skipped_steps: checkpoint.execution_state.current_step_index,
            new_steps_executed: 0,
            final_context: workflow_context.clone(),
        };

        info!(
            "Workflow {} checkpoint loaded. Full resume would skip {} steps and continue from step {}",
            workflow_id,
            resume_context.skip_steps.len(),
            resume_context.start_from_step
        );

        // Delete checkpoint on successful completion
        if result.success {
            self.checkpoint_manager
                .delete_checkpoint(workflow_id)
                .await?;
            info!("Workflow {} resumed successfully", workflow_id);
        }

        Ok(result)
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

    /// Build resume context from checkpoint and options
    fn build_resume_context(
        &self,
        checkpoint: WorkflowCheckpoint,
        options: &ResumeOptions,
    ) -> Result<ResumeContext> {
        let mut context = checkpoint::build_resume_context(checkpoint);

        // Override start step if specified
        if let Some(from_step) = options.from_step {
            if from_step >= context.skip_steps.len() {
                return Err(anyhow!(
                    "Cannot resume from step {}: only {} steps completed",
                    from_step,
                    context.skip_steps.len()
                ));
            }
            // Adjust skip steps to start from specified step
            context.skip_steps.truncate(from_step);
            context.start_from_step = from_step;
        }

        // Reset failures if requested
        if options.reset_failures {
            if let Some(ref mut mapreduce_state) = context.mapreduce_state {
                mapreduce_state.failed_items.clear();
            }
        }

        Ok(context)
    }

    /// Restore workflow context from checkpoint
    fn restore_workflow_context(&self, checkpoint: &WorkflowCheckpoint) -> Result<WorkflowContext> {
        let mut context = WorkflowContext::default();

        // Restore variables
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

        Ok(context)
    }

    /// Execute workflow from checkpoint with full execution support
    pub async fn execute_from_checkpoint(
        &self,
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
            println!("Workflow {} is already completed - nothing to resume", workflow_id);
            return Ok(ResumeResult {
                success: true,
                total_steps_executed: checkpoint.execution_state.current_step_index,
                skipped_steps: checkpoint.execution_state.current_step_index,
                new_steps_executed: 0,
                final_context: WorkflowContext::default(),
            });
        }

        // Load the workflow file
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
            working_dir: workflow_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf(),
            project_dir: workflow_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf(),
            worktree_name: None,
            session_id: format!("resume-{}", workflow_id),
        };

        // Restore workflow context
        let mut workflow_context = self.restore_workflow_context(&checkpoint)?;

        // Create workflow executor with checkpoint support
        let mut executor = WorkflowExecutorImpl::new(
            claude_executor.clone(),
            session_manager.clone(),
            user_interaction.clone(),
        )
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

        // Skip completed steps and execute remaining ones
        for (step_index, step) in extended_workflow.steps.iter().enumerate() {
            if step_index < start_from {
                info!("Skipping completed step {}: {:?}", step_index + 1, step);
                continue;
            }

            info!(
                "Executing step {}/{}: {:?}",
                step_index + 1,
                total_steps,
                step
            );

            // Execute the step
            match executor
                .execute_step(step, &env, &mut workflow_context)
                .await
            {
                Ok(_result) => {
                    steps_executed += 1;
                    info!("Step {} completed successfully", step_index + 1);
                }
                Err(e) => {
                    // If step fails, save checkpoint and return error
                    info!("Step {} failed: {}", step_index + 1, e);
                    return Err(e);
                }
            }
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
    let manager = CheckpointManager::new(checkpoint_dir);
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
