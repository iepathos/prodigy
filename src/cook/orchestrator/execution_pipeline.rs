//! Execution pipeline for orchestrating workflow execution
//!
//! This module contains the logic for executing workflows, managing session state,
//! and handling signal interrupts during execution.

use crate::abstractions::git::GitOperations;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::{CookConfig, ExecutionEnvironment};
use crate::cook::session::{SessionManager, SessionStatus, SessionUpdate};
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreeStatus};
use anyhow::{Context, Result};
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
}

impl ExecutionPipeline {
    /// Create a new execution pipeline
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            user_interaction,
            git_operations,
            subprocess,
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
}
