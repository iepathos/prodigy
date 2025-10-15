//! Execution pipeline for orchestrating workflow execution
//!
//! This module contains the logic for executing workflows, managing session state,
//! and handling signal interrupts during execution.

use crate::abstractions::git::GitOperations;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::{CookConfig, ExecutionEnvironment};
use crate::cook::session::{SessionManager, SessionState, SessionStatus, SessionUpdate};
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreeStatus};
use anyhow::{anyhow, Context, Result};
use log::debug;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Execution pipeline for coordinating workflow execution
pub struct ExecutionPipeline {
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    #[allow(dead_code)]
    git_operations: Arc<dyn GitOperations>,
    subprocess: SubprocessManager,
    session_ops: super::session_ops::SessionOperations,
    workflow_executor: super::workflow_execution::WorkflowExecutor,
}

impl ExecutionPipeline {
    /// Create a new execution pipeline
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: SubprocessManager,
        session_ops: super::session_ops::SessionOperations,
        workflow_executor: super::workflow_execution::WorkflowExecutor,
    ) -> Self {
        Self {
            session_manager,
            user_interaction,
            git_operations,
            subprocess,
            session_ops,
            workflow_executor,
        }
    }

    /// Initialize session metadata with workflow hash and type
    pub async fn initialize_session_metadata(
        &self,
        session_id: &str,
        config: &CookConfig,
    ) -> Result<()> {
        debug!("About to start session");
        self.session_manager.start_session(session_id).await?;
        debug!("Session started successfully");
        self.user_interaction
            .display_info(&format!("Starting session: {}", session_id));
        debug!("Session message displayed");

        // Calculate and store workflow hash
        debug!("Calculating workflow hash");
        let workflow_hash =
            super::session_ops::SessionOperations::calculate_workflow_hash(&config.workflow);
        debug!("Workflow hash calculated: {}", workflow_hash);

        debug!("Classifying workflow type");
        let workflow_type = super::core::DefaultCookOrchestrator::classify_workflow_type(config);
        debug!("Workflow type classified: {:?}", workflow_type);

        // Update session with workflow metadata
        debug!("Updating session with workflow hash");
        debug!("About to call update_session");
        let result = self
            .session_manager
            .update_session(SessionUpdate::SetWorkflowHash(workflow_hash))
            .await;
        debug!("update_session call returned");
        result?;
        debug!("Workflow hash updated");

        debug!("Updating session with workflow type");
        self.session_manager
            .update_session(SessionUpdate::SetWorkflowType(workflow_type.into()))
            .await?;
        debug!("Workflow type updated");

        Ok(())
    }

    /// Setup signal handlers for graceful interruption
    pub fn setup_signal_handlers(
        &self,
        config: &CookConfig,
        session_id: &str,
        worktree_name: Option<Arc<str>>,
    ) -> Result<JoinHandle<()>> {
        log::debug!("Setting up signal handlers");

        // Get merge config from workflow or mapreduce config
        let merge_config = config.workflow.merge.clone().or_else(|| {
            config
                .mapreduce_config
                .as_ref()
                .and_then(|m| m.merge.clone())
        });

        // Get workflow environment variables
        let workflow_env = config.workflow.env.clone().unwrap_or_default();

        let worktree_manager = Arc::new(WorktreeManager::with_config(
            config.project_path.to_path_buf(),
            self.subprocess.clone(),
            config.command.verbosity,
            merge_config,
            workflow_env,
        )?);

        crate::cook::signal_handler::setup_interrupt_handlers(
            worktree_manager,
            session_id.to_string(),
        )?;

        log::debug!("Signal handlers set up successfully");

        let session_manager = self.session_manager.clone();
        let worktree_name = worktree_name.clone();
        let project_path = Arc::clone(&config.project_path);
        let subprocess = self.subprocess.clone();

        let interrupt_handler = tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            // Mark session as interrupted when Ctrl+C is pressed
            session_manager
                .update_session(SessionUpdate::MarkInterrupted)
                .await
                .ok();

            // Also update worktree state if using a worktree
            if let Some(ref name) = worktree_name {
                if let Ok(worktree_manager) =
                    WorktreeManager::new(project_path.to_path_buf(), subprocess.clone())
                {
                    let _ = worktree_manager.update_session_state(name.as_ref(), |state| {
                        state.status = WorktreeStatus::Interrupted;
                        state.interrupted_at = Some(chrono::Utc::now());
                        state.interruption_type =
                            Some(crate::worktree::InterruptionType::UserInterrupt);
                        state.resumable = true;
                    });
                }
            }
        });

        Ok(interrupt_handler)
    }

    /// Finalize session with appropriate status and messaging
    pub async fn finalize_session(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        execution_result: Result<(), anyhow::Error>,
        cleanup_fn: impl std::future::Future<Output = Result<()>>,
        display_health_fn: impl std::future::Future<Output = Result<()>>,
    ) -> Result<()> {
        match execution_result {
            Ok(_) => {
                self.session_manager
                    .update_session(SessionUpdate::UpdateStatus(SessionStatus::Completed))
                    .await?;
                self.user_interaction
                    .display_success("Cook session completed successfully!");
            }
            Err(e) => {
                // Check if session was interrupted
                let state = self
                    .session_manager
                    .get_state()
                    .context("Failed to get session state after cook error")?;
                if state.status == SessionStatus::Interrupted {
                    self.user_interaction.display_warning(&format!(
                        "\nSession interrupted. Resume with: prodigy run {} --resume {}",
                        config
                            .workflow
                            .commands
                            .first()
                            .map(|_| config.command.playbook.display().to_string())
                            .unwrap_or_else(|| "<workflow>".to_string()),
                        env.session_id
                    ));
                    // Save checkpoint for resume
                    let checkpoint_path =
                        env.working_dir.join(".prodigy").join("session_state.json");
                    self.session_manager.save_state(&checkpoint_path).await?;

                    // Also update worktree state if using a worktree
                    if let Some(ref name) = env.worktree_name {
                        // Get merge config from workflow or mapreduce config
                        let merge_config = config.workflow.merge.clone().or_else(|| {
                            config
                                .mapreduce_config
                                .as_ref()
                                .and_then(|m| m.merge.clone())
                        });

                        // Get workflow environment variables
                        let workflow_env = config.workflow.env.clone().unwrap_or_default();

                        let worktree_manager = WorktreeManager::with_config(
                            config.project_path.to_path_buf(),
                            self.subprocess.clone(),
                            config.command.verbosity,
                            merge_config,
                            workflow_env,
                        )?;
                        worktree_manager.update_session_state(name.as_ref(), |state| {
                            state.status = WorktreeStatus::Interrupted;
                            state.interrupted_at = Some(chrono::Utc::now());
                            state.interruption_type =
                                Some(crate::worktree::InterruptionType::Unknown);
                            state.resumable = true;
                        })?;
                    }
                } else {
                    self.session_manager
                        .update_session(SessionUpdate::UpdateStatus(SessionStatus::Failed))
                        .await?;
                    self.session_manager
                        .update_session(SessionUpdate::AddError(e.to_string()))
                        .await?;
                    self.user_interaction
                        .display_error(&format!("Session failed: {e}"));

                    // Display how to resume the session
                    let state = self
                        .session_manager
                        .get_state()
                        .context("Failed to get session state for resume info")?;
                    if state.workflow_state.is_some() {
                        self.user_interaction.display_info(&format!(
                            "\n💡 To resume from last checkpoint, run: prodigy resume {}",
                            env.session_id
                        ));
                    }
                }
                return Err(e);
            }
        }

        // Cleanup
        cleanup_fn.await?;

        // Complete session
        let summary = self.session_manager.complete_session().await?;

        // Don't display session stats in dry-run mode
        if !config.command.dry_run {
            self.user_interaction.display_info(&format!(
                "Session complete: {} iterations, {} files changed",
                summary.iterations, summary.files_changed
            ));
        }

        // Display health score if metrics flag is set
        if config.command.metrics {
            display_health_fn.await?;
        }

        Ok(())
    }

    /// Resume a workflow from a previously interrupted session
    pub async fn resume_workflow(
        &self,
        session_id: &str,
        mut config: CookConfig,
    ) -> Result<()> {
        // Try to load the session state from UnifiedSessionManager
        // If it doesn't exist, we'll fall back to loading from the worktree session file
        let state_result = self.session_manager.load_session(session_id).await;

        let state = match state_result {
            Ok(s) => s,
            Err(_) => {
                // Session not found in unified storage, try loading from worktree
                let home = directories::BaseDirs::new()
                    .ok_or_else(|| anyhow!("Could not determine home directory"))?
                    .home_dir()
                    .to_path_buf();

                let worktree_path = home
                    .join(".prodigy")
                    .join("worktrees")
                    .join(config.project_path.file_name().unwrap_or_default())
                    .join(session_id);

                let session_file = worktree_path.join(".prodigy").join("session_state.json");

                if !session_file.exists() {
                    return Err(anyhow!(
                        "Session not found: {}\nTried:\n  - Unified session storage\n  - Worktree session file: {}",
                        session_id,
                        session_file.display()
                    ));
                }

                // Load from worktree session file
                self.session_manager.load_state(&session_file).await?;
                self.session_manager.get_state()?
            }
        };

        // Validate the session is resumable
        if !state.is_resumable() {
            return Err(anyhow!(
                "Session {} is not resumable (status: {:?})",
                session_id,
                state.status
            ));
        }

        // Validate workflow hasn't changed
        if let Some(ref stored_hash) = state.workflow_hash {
            let current_hash =
                super::session_ops::SessionOperations::calculate_workflow_hash(&config.workflow);
            if current_hash != *stored_hash {
                return Err(anyhow!(
                    "Workflow has been modified since interruption. \
                     Use --force to override or start a new session."
                ));
            }
        }

        // Display resume information
        self.user_interaction.display_info(&format!(
            "🔄 Resuming session: {} from {}",
            session_id,
            state
                .get_resume_info()
                .unwrap_or_else(|| "unknown state".to_string())
        ));

        // Restore the environment
        let env = self.restore_environment(&state, &config).await?;

        // Update the session manager with the loaded state
        // Use the working directory from the restored environment
        let session_file = env.working_dir.join(".prodigy").join("session_state.json");
        self.session_manager.load_state(&session_file).await?;

        // Resume the workflow execution from the saved state
        if let Some(ref workflow_state) = state.workflow_state {
            // Update config with saved arguments
            config.command.args = workflow_state.input_args.clone();
            config.command.map = workflow_state.map_patterns.clone();

            // Restore execution context if available
            if let Some(ref exec_context) = state.execution_context {
                // This context would need to be passed to the workflow executor
                // For now, we'll just log that it was restored
                self.user_interaction.display_info(&format!(
                    "Restored {} variables and {} step outputs",
                    exec_context.variables.len(),
                    exec_context.step_outputs.len()
                ));
            }

            // Execute the workflow starting from the saved position
            let result = self
                .resume_workflow_execution(
                    &env,
                    &config,
                    workflow_state.current_iteration,
                    workflow_state.current_step,
                )
                .await;

            // Handle result
            match result {
                Ok(_) => {
                    self.session_manager
                        .update_session(SessionUpdate::UpdateStatus(SessionStatus::Completed))
                        .await?;
                    self.user_interaction
                        .display_success("Resumed session completed successfully!");
                }
                Err(e) => {
                    // Check if session was interrupted again
                    let current_state = self
                        .session_manager
                        .get_state()
                        .context("Failed to get session state after resume error")?;
                    if current_state.status == SessionStatus::Interrupted {
                        self.user_interaction.display_warning(&format!(
                            "\nSession interrupted again. Resume with: prodigy run {} --resume {}",
                            config.command.playbook.display(),
                            session_id
                        ));
                        // Save updated checkpoint
                        self.session_manager.save_state(&session_file).await?;
                    } else {
                        self.session_manager
                            .update_session(SessionUpdate::UpdateStatus(SessionStatus::Failed))
                            .await?;
                        self.session_manager
                            .update_session(SessionUpdate::AddError(e.to_string()))
                            .await?;
                        self.user_interaction
                            .display_error(&format!("Resumed session failed: {e}"));
                    }
                    return Err(e);
                }
            }

            // Cleanup - need to call the cleanup function from the orchestrator
            // For now, we'll just complete the session without cleanup
            // TODO: Pass cleanup function as a parameter or make it available

            // Complete session
            let summary = self.session_manager.complete_session().await?;

            // Don't display misleading session stats in dry-run mode
            if !config.command.dry_run {
                self.user_interaction.display_info(&format!(
                    "Session complete: {} iterations, {} files changed",
                    summary.iterations, summary.files_changed
                ));
            }
        } else {
            return Err(anyhow!(
                "Session {} has no workflow state to resume",
                session_id
            ));
        }

        Ok(())
    }

    /// Restore the execution environment from saved state
    async fn restore_environment(
        &self,
        state: &SessionState,
        config: &CookConfig,
    ) -> Result<ExecutionEnvironment> {
        self.session_ops.restore_environment(state, config).await
    }

    /// Resume workflow execution from a specific point
    async fn resume_workflow_execution(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        start_iteration: usize,
        start_step: usize,
    ) -> Result<()> {
        use super::core::WorkflowType;

        self.user_interaction.display_info(&format!(
            "Resuming from iteration {} step {}",
            start_iteration + 1,
            start_step + 1
        ));

        // Load existing completed steps from session state
        let existing_state = self
            .session_manager
            .get_state()
            .context("Failed to get session state before workflow execution")?;
        let completed_steps = existing_state
            .workflow_state
            .as_ref()
            .map(|ws| ws.completed_steps.clone())
            .unwrap_or_default();

        // Create workflow state for checkpointing
        let workflow_state = crate::cook::session::WorkflowState {
            current_iteration: start_iteration,
            current_step: start_step,
            completed_steps,
            workflow_path: config.command.playbook.clone(),
            input_args: config.command.args.clone(),
            map_patterns: config.command.map.clone(),
            using_worktree: true,
        };

        // Update session with workflow state
        self.session_manager
            .update_session(SessionUpdate::UpdateWorkflowState(workflow_state))
            .await?;

        // Determine workflow type and route to appropriate resume handler
        let workflow_type = super::core::DefaultCookOrchestrator::classify_workflow_type(config);

        // For MapReduce workflows, use specialized resume mechanism
        if workflow_type == WorkflowType::MapReduce {
            // Check if there's an existing MapReduce job to resume
            if let Some(_mapreduce_config) = &config.mapreduce_config {
                // MapReduce workflows need to be executed through the orchestrator
                // which has access to the MapReduce execution logic
                return Err(anyhow!(
                    "MapReduce resume requires orchestrator-level execution"
                ));
            }
        }

        // Execute the workflow based on type, but skip completed steps
        match workflow_type {
            WorkflowType::MapReduce => {
                // MapReduce workflows have their own resume mechanism
                Err(anyhow!(
                    "MapReduce workflow requires mapreduce configuration"
                ))
            }
            WorkflowType::StructuredWithOutputs => {
                self.workflow_executor
                    .execute_structured_workflow_from(env, config, start_iteration, start_step)
                    .await
            }
            WorkflowType::WithArguments => {
                self.workflow_executor
                    .execute_iterative_workflow_from(env, config, start_iteration, start_step)
                    .await
            }
            WorkflowType::Standard => {
                self.workflow_executor
                    .execute_standard_workflow_from(env, config, start_iteration, start_step)
                    .await
            }
        }
    }
}
