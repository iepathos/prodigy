//! Builder and configuration for WorkflowExecutor
//!
//! Handles construction, configuration, and initialization logic.

use super::super::checkpoint::{
    create_checkpoint_with_total_steps, CheckpointManager,
    CompletedStep as CheckpointCompletedStep, ResumeContext, RetryState,
};
use super::super::error_recovery::ErrorRecoveryState;
use super::super::normalized;
use super::super::normalized::NormalizedWorkflow;
use super::super::validation::OnIncompleteConfig;
use super::{
    CaptureOutput, ExtendedWorkflowConfig, SensitivePatternConfig, StepResult, WorkflowContext,
    WorkflowExecutor, WorkflowStep,
};
use crate::abstractions::git::RealGitOperations;
use crate::commands::CommandRegistry;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::retry_state::RetryStateManager;
use crate::cook::session::SessionManager;
use crate::testing::config::TestConfiguration;
use crate::unified_session::TimingTracker;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

impl WorkflowExecutor {
    // Constructor and builder methods

    /// Create a new workflow executor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            timing_tracker: TimingTracker::new(),
            test_config: None,
            command_registry: None,
            subprocess: crate::subprocess::SubprocessManager::production(),
            sensitive_config: SensitivePatternConfig::default(),
            completed_steps: Vec::new(),
            checkpoint_manager: None,
            workflow_id: None,
            checkpoint_completed_steps: Vec::new(),
            environment_manager: None,
            global_environment_config: None,
            current_workflow: None,
            current_step_index: None,
            git_operations: Arc::new(RealGitOperations::new()),
            resume_context: None,
            retry_state_manager: Arc::new(RetryStateManager::new()),
            workflow_path: None,
            dry_run: false,
            assumed_commits: Vec::new(),
            dry_run_commands: Vec::new(),
            dry_run_validations: Vec::new(),
            dry_run_potential_handlers: Vec::new(),
        }
    }

    /// Sets the command registry for modular command handlers
    pub async fn with_command_registry(mut self) -> Self {
        self.command_registry = Some(CommandRegistry::with_defaults().await);
        self
    }

    /// Set the resume context for handling interrupted workflows
    pub fn with_resume_context(mut self, context: ResumeContext) -> Self {
        // Restore retry state if available in checkpoint
        if let Some(ref checkpoint) = context.checkpoint {
            if let Some(retry_checkpoint_state) = checkpoint.retry_checkpoint_state.clone() {
                // Clone the retry state manager Arc to avoid borrowing issues
                let retry_manager = self.retry_state_manager.clone();

                // Spawn a task to restore retry state asynchronously
                tokio::spawn(async move {
                    if let Err(e) = retry_manager
                        .restore_from_checkpoint(&retry_checkpoint_state)
                        .await
                    {
                        tracing::warn!("Failed to restore retry state from checkpoint: {}", e);
                    } else {
                        tracing::info!("Successfully restored retry state from checkpoint");
                    }
                });
            }
        }

        self.resume_context = Some(context);
        self
    }

    /// Set the workflow file path (for checkpoint resume)
    pub fn with_workflow_path(mut self, path: PathBuf) -> Self {
        self.workflow_path = Some(path);
        self
    }

    /// Enable dry-run mode for preview without execution
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set the environment configuration for the workflow
    pub fn with_environment_config(
        mut self,
        config: crate::cook::environment::EnvironmentConfig,
    ) -> Result<Self> {
        // Initialize environment manager with current directory
        let current_dir = std::env::current_dir()?;
        self.environment_manager = Some(crate::cook::environment::EnvironmentManager::new(
            current_dir,
        )?);
        self.global_environment_config = Some(config);
        Ok(self)
    }

    /// Set the checkpoint manager for workflow resumption
    pub fn with_checkpoint_manager(
        mut self,
        manager: Arc<CheckpointManager>,
        workflow_id: String,
    ) -> Self {
        self.checkpoint_manager = Some(manager);
        self.workflow_id = Some(workflow_id);
        self
    }

    /// Configure sensitive pattern detection
    pub fn with_sensitive_patterns(mut self, config: SensitivePatternConfig) -> Self {
        self.sensitive_config = config;
        self
    }

    /// Create a new workflow executor with test configuration
    pub fn with_test_config(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        test_config: Arc<TestConfiguration>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            timing_tracker: TimingTracker::new(),
            test_config: Some(test_config),
            command_registry: None,
            subprocess: crate::subprocess::SubprocessManager::production(),
            sensitive_config: SensitivePatternConfig::default(),
            completed_steps: Vec::new(),
            checkpoint_manager: None,
            workflow_id: None,
            checkpoint_completed_steps: Vec::new(),
            environment_manager: None,
            global_environment_config: None,
            current_workflow: None,
            current_step_index: None,
            git_operations: Arc::new(RealGitOperations::new()),
            resume_context: None,
            retry_state_manager: Arc::new(RetryStateManager::new()),
            workflow_path: None,
            dry_run: false,
            assumed_commits: Vec::new(),
            dry_run_commands: Vec::new(),
            dry_run_validations: Vec::new(),
            dry_run_potential_handlers: Vec::new(),
        }
    }

    /// Create executor with test configuration and custom git operations
    #[cfg(test)]
    pub fn with_test_config_and_git(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        test_config: Arc<TestConfiguration>,
        git_operations: Arc<dyn crate::abstractions::git::GitOperations>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            timing_tracker: TimingTracker::new(),
            test_config: Some(test_config),
            command_registry: None,
            subprocess: crate::subprocess::SubprocessManager::production(),
            sensitive_config: SensitivePatternConfig::default(),
            completed_steps: Vec::new(),
            checkpoint_manager: None,
            workflow_id: None,
            checkpoint_completed_steps: Vec::new(),
            environment_manager: None,
            global_environment_config: None,
            current_workflow: None,
            current_step_index: None,
            git_operations,
            resume_context: None,
            retry_state_manager: Arc::new(RetryStateManager::new()),
            workflow_path: None,
            dry_run: false,
            assumed_commits: Vec::new(),
            dry_run_commands: Vec::new(),
            dry_run_validations: Vec::new(),
            dry_run_potential_handlers: Vec::new(),
        }
    }

    // Configuration helpers

    /// Create a validation handler from on_incomplete configuration
    pub(super) fn create_validation_handler(
        &self,
        on_incomplete: &OnIncompleteConfig,
        _ctx: &WorkflowContext,
    ) -> Option<WorkflowStep> {
        // Create a step based on the handler configuration
        if on_incomplete.claude.is_some() || on_incomplete.shell.is_some() {
            Some(WorkflowStep {
                name: None,
                claude: on_incomplete.claude.clone(),
                shell: on_incomplete.shell.clone(),
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
                capture_output: CaptureOutput::Disabled,
                on_failure: None,
                retry: None,
                on_success: None,
                on_exit_code: Default::default(),
                commit_required: on_incomplete.commit_required,
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
        } else {
            None
        }
    }

    /// Restore error recovery state from resume context
    pub(super) fn restore_error_recovery_state(
        &self,
        step_index: usize,
        workflow_context: &mut WorkflowContext,
    ) {
        if let Some(ref resume_ctx) = self.resume_context {
            if let Some(recovery_state_value) =
                resume_ctx.variable_state.get("__error_recovery_state")
            {
                if let Ok(error_recovery_state) =
                    serde_json::from_value::<ErrorRecoveryState>(recovery_state_value.clone())
                {
                    if !error_recovery_state.active_handlers.is_empty() {
                        tracing::info!(
                            "Restored {} error handlers for step {}",
                            error_recovery_state.active_handlers.len(),
                            step_index
                        );
                        for (key, value) in error_recovery_state.error_context {
                            workflow_context
                                .variables
                                .insert(format!("error.{}", key), value.to_string());
                        }
                    }
                }
            }
        }
    }

    // Checkpoint functions

    /// Save a checkpoint during step execution (e.g., during retries)
    pub(super) async fn save_retry_checkpoint(
        &self,
        workflow: &NormalizedWorkflow,
        current_step_index: usize,
        retry_state: Option<RetryState>,
        ctx: &WorkflowContext,
    ) {
        if let Some(ref checkpoint_manager) = self.checkpoint_manager {
            if let Some(ref workflow_id) = self.workflow_id {
                let workflow_hash = format!("{:?}", workflow.steps.len());

                // Create a checkpoint with current retry state
                let mut checkpoint_steps = self.checkpoint_completed_steps.clone();

                // Add or update the current step with retry state
                if let Some(retry_state) = retry_state {
                    let step_name = if current_step_index < workflow.steps.len() {
                        match &workflow.steps[current_step_index].command {
                            normalized::StepCommand::Claude(cmd) => format!("claude: {}", cmd),
                            normalized::StepCommand::Shell(cmd) => format!("shell: {}", cmd),
                            normalized::StepCommand::Test { command, .. } => {
                                format!("test: {}", command)
                            }
                            normalized::StepCommand::Simple(cmd) => cmd.to_string(),
                            _ => "complex command".to_string(),
                        }
                    } else {
                        "unknown step".to_string()
                    };

                    let retry_step = CheckpointCompletedStep {
                        step_index: current_step_index,
                        command: step_name,
                        success: false,
                        output: None,
                        captured_variables: HashMap::new(),
                        duration: Duration::from_secs(0),
                        completed_at: chrono::Utc::now(),
                        retry_state: Some(retry_state),
                    };

                    // Remove any existing entry for this step and add the new one
                    checkpoint_steps.retain(|s| s.step_index != current_step_index);
                    checkpoint_steps.push(retry_step);
                }

                let mut checkpoint = create_checkpoint_with_total_steps(
                    workflow_id.clone(),
                    workflow,
                    ctx,
                    checkpoint_steps,
                    current_step_index,
                    workflow_hash,
                    workflow.steps.len(),
                );

                // Set workflow path if available
                if let Some(ref path) = self.workflow_path {
                    checkpoint.workflow_path = Some(path.clone());
                }

                // Add retry state from RetryStateManager
                if let Ok(retry_checkpoint_state) =
                    self.retry_state_manager.create_checkpoint_state().await
                {
                    checkpoint.retry_checkpoint_state = Some(retry_checkpoint_state);
                }

                if let Err(e) = checkpoint_manager.save_checkpoint(&checkpoint).await {
                    tracing::warn!("Failed to save retry checkpoint: {}", e);
                } else {
                    tracing::debug!(
                        "Saved retry checkpoint at step {} attempt",
                        current_step_index
                    );
                }
            }
        }
    }

    // Dry-run display

    /// Display dry-run information before workflow execution
    pub fn display_dry_run_info(&self, workflow: &ExtendedWorkflowConfig) {
        if self.dry_run {
            println!("[DRY RUN] Workflow execution simulation mode");
            println!("[DRY RUN] No commands will be executed");
            if workflow.max_iterations > 1 {
                println!("[DRY RUN] Would run {} iterations", workflow.max_iterations);
            }
        }
    }

    /// Display dry-run summary at the end of execution
    pub fn display_dry_run_summary(&self) {
        if !self.dry_run {
            return;
        }

        println!("\n[DRY RUN] Summary:");
        println!("==================");

        // Show main commands
        println!(
            "Main commands that would execute: {}",
            self.dry_run_commands.len()
        );
        if !self.dry_run_commands.is_empty() {
            for cmd in &self.dry_run_commands {
                println!("  - {}", cmd);
            }
        }

        // Show validation commands
        if !self.dry_run_validations.is_empty() {
            println!(
                "\nValidation commands that would execute: {}",
                self.dry_run_validations.len()
            );
            for val in &self.dry_run_validations {
                println!("  - {}", val);
            }
        }

        // Show potential failure handlers
        if !self.dry_run_potential_handlers.is_empty() {
            println!("\nPotential failure handlers (if needed):");
            for handler in &self.dry_run_potential_handlers {
                println!("  - {}", handler);
            }
        }

        // Show assumed commits
        if !self.assumed_commits.is_empty() {
            println!("\nAssumed commits: {}", self.assumed_commits.len());
            for commit in &self.assumed_commits {
                println!("  - From: {}", commit);
            }
        }

        println!("\nNo actual commands executed or files changed.");
        println!("To run for real, remove the --dry-run flag.");
    }

    // Message builders

    /// Build error message for a failed step
    pub(super) fn build_step_error_message(step: &WorkflowStep, result: &StepResult) -> String {
        super::pure::build_step_error_message(step, result)
    }

    /// Generate a commit message from template or default (delegated to commit_handler module)
    pub fn generate_commit_message(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> String {
        super::commit_handler::generate_commit_message(
            step,
            context,
            &self.get_step_display_name(step),
        )
    }
}
