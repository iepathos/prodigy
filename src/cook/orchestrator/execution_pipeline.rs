//! Execution pipeline for orchestrating workflow execution
//!
//! This module contains the logic for executing workflows, managing session state,
//! and handling signal interrupts during execution.

use crate::abstractions::git::GitOperations;
use crate::cook::execution::claude::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::{CookConfig, ExecutionEnvironment};
use crate::cook::session::{SessionManager, SessionState, SessionStatus, SessionUpdate};
use crate::cook::workflow::ExtendedWorkflowConfig;
use crate::subprocess::SubprocessManager;
use crate::worktree::{WorktreeManager, WorktreeStatus};
use anyhow::{anyhow, Context, Result};
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Represents the outcome of a workflow execution
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // Used in Phase 5
enum ExecutionOutcome {
    Success,
    Interrupted,
    Failed(String),
}

/// Classify the execution result based on result and session status
#[allow(dead_code)] // Used in Phase 5
fn classify_execution_result(
    result: &Result<()>,
    session_status: SessionStatus,
) -> ExecutionOutcome {
    match result {
        Ok(_) => ExecutionOutcome::Success,
        Err(e) => {
            if session_status == SessionStatus::Interrupted {
                ExecutionOutcome::Interrupted
            } else {
                ExecutionOutcome::Failed(e.to_string())
            }
        }
    }
}

/// Determine if a checkpoint should be saved based on the outcome
#[allow(dead_code)] // Used in Phase 5
fn should_save_checkpoint(outcome: &ExecutionOutcome) -> bool {
    matches!(outcome, ExecutionOutcome::Interrupted)
}

/// Generate the resume message for the user
#[allow(dead_code)] // Used in Phase 5
fn determine_resume_message(
    session_id: &str,
    playbook_path: &str,
    outcome: &ExecutionOutcome,
) -> Option<String> {
    match outcome {
        ExecutionOutcome::Interrupted => Some(format!(
            "\nSession interrupted. Resume with: prodigy run {} --resume {}",
            playbook_path, session_id
        )),
        ExecutionOutcome::Failed(_) => Some(format!(
            "\n💡 To resume from last checkpoint, run: prodigy resume {}",
            session_id
        )),
        ExecutionOutcome::Success => None,
    }
}

/// Execution pipeline for coordinating workflow execution
pub struct ExecutionPipeline {
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    claude_executor: Arc<dyn ClaudeExecutor>,
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
        claude_executor: Arc<dyn ClaudeExecutor>,
        git_operations: Arc<dyn GitOperations>,
        subprocess: SubprocessManager,
        session_ops: super::session_ops::SessionOperations,
        workflow_executor: super::workflow_execution::WorkflowExecutor,
    ) -> Self {
        Self {
            session_manager,
            user_interaction,
            claude_executor,
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

    /// Create a WorktreeManager from the config
    fn create_worktree_manager(&self, config: &CookConfig) -> Result<WorktreeManager> {
        // Get merge config from workflow or mapreduce config
        let merge_config = config.workflow.merge.clone().or_else(|| {
            config
                .mapreduce_config
                .as_ref()
                .and_then(|m| m.merge.clone())
        });

        // Get workflow environment variables
        let workflow_env = config.workflow.env.clone().unwrap_or_default();

        WorktreeManager::with_config(
            config.project_path.to_path_buf(),
            self.subprocess.clone(),
            config.command.verbosity,
            merge_config,
            workflow_env,
        )
    }

    /// Update worktree state to mark as interrupted
    fn update_worktree_interrupted_state(
        worktree_manager: &WorktreeManager,
        worktree_name: &str,
    ) -> Result<()> {
        worktree_manager.update_session_state(worktree_name, |state| {
            state.status = WorktreeStatus::Interrupted;
            state.interrupted_at = Some(chrono::Utc::now());
            state.interruption_type = Some(crate::worktree::InterruptionType::Unknown);
            state.resumable = true;
        })
    }

    /// Setup signal handlers for graceful interruption
    pub fn setup_signal_handlers(
        &self,
        config: &CookConfig,
        session_id: &str,
        worktree_name: Option<Arc<str>>,
    ) -> Result<JoinHandle<()>> {
        log::debug!("Setting up signal handlers");

        let worktree_manager = Arc::new(self.create_worktree_manager(config)?);

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
                        let worktree_manager = self.create_worktree_manager(config)?;
                        Self::update_worktree_interrupted_state(&worktree_manager, name.as_ref())?;
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

    /// Validate that a session is in a resumable state
    ///
    /// Checks if the session status allows for resumption.
    fn validate_session_resumable(&self, session_id: &str, state: &SessionState) -> Result<()> {
        if !state.is_resumable() {
            return Err(anyhow!(
                "Session {} is not resumable (status: {:?})",
                session_id,
                state.status
            ));
        }
        Ok(())
    }

    /// Validate that the workflow hasn't been modified since the session was interrupted
    ///
    /// Compares the stored workflow hash with the current workflow hash.
    fn validate_workflow_unchanged(&self, state: &SessionState, config: &CookConfig) -> Result<()> {
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
        Ok(())
    }

    /// Restore config from saved workflow state
    ///
    /// Updates the config with saved arguments and map patterns from the workflow state.
    fn restore_config_from_workflow_state(
        &self,
        config: &mut CookConfig,
        workflow_state: &super::super::session::WorkflowState,
    ) {
        config.command.args = workflow_state.input_args.clone();
        config.command.map = workflow_state.map_patterns.clone();
    }

    /// Display session completion summary
    ///
    /// Shows session statistics unless in dry-run mode.
    fn display_session_completion(
        &self,
        summary: &super::super::session::SessionSummary,
        is_dry_run: bool,
    ) {
        if !is_dry_run {
            self.user_interaction.display_info(&format!(
                "Session complete: {} iterations, {} files changed",
                summary.iterations, summary.files_changed
            ));
        }
    }

    /// Handle the result of a resumed workflow execution
    ///
    /// Processes success, interruption, and failure cases appropriately.
    async fn handle_resume_result(
        &self,
        result: Result<()>,
        session_file: &std::path::Path,
        session_id: &str,
        playbook_path: &std::path::Path,
    ) -> Result<()> {
        match result {
            Ok(_) => {
                self.session_manager
                    .update_session(SessionUpdate::UpdateStatus(SessionStatus::Completed))
                    .await?;
                self.user_interaction
                    .display_success("Resumed session completed successfully!");
                Ok(())
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
                        playbook_path.display(),
                        session_id
                    ));
                    // Save updated checkpoint
                    self.session_manager.save_state(session_file).await?;
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
                Err(e)
            }
        }
    }

    /// Load session state with fallback to worktree session file
    ///
    /// This function attempts to load the session state from UnifiedSessionManager first.
    /// If not found, it falls back to loading from the worktree session file.
    async fn load_session_with_fallback(
        &self,
        session_id: &str,
        config: &CookConfig,
    ) -> Result<SessionState> {
        // Try to load the session state from UnifiedSessionManager
        let state_result = self.session_manager.load_session(session_id).await;

        match state_result {
            Ok(s) => Ok(s),
            Err(_) => {
                // Session not found in unified storage, try loading from worktree
                // The config.project_path should already be the worktree path when resuming
                let session_file = config
                    .project_path
                    .join(".prodigy")
                    .join("session_state.json");

                if !session_file.exists() {
                    return Err(anyhow!(
                        "Session not found: {}\nTried:\n  - Unified session storage\n  - Worktree session file: {}",
                        session_id,
                        session_file.display()
                    ));
                }

                // Load from worktree session file
                self.session_manager.load_state(&session_file).await?;
                self.session_manager.get_state()
            }
        }
    }

    /// Resume a workflow from a previously interrupted session
    pub async fn resume_workflow(&self, session_id: &str, mut config: CookConfig) -> Result<()> {
        // Load session state with fallback to worktree
        let state = self.load_session_with_fallback(session_id, &config).await?;

        // Validate the session is resumable
        self.validate_session_resumable(session_id, &state)?;

        // Validate workflow hasn't changed
        self.validate_workflow_unchanged(&state, &config)?;

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

        // Transition session status to InProgress (from Failed, Interrupted, etc.)
        self.session_manager
            .update_session(SessionUpdate::UpdateStatus(SessionStatus::InProgress))
            .await?;

        // Resume the workflow execution from the saved state
        if let Some(ref workflow_state) = state.workflow_state {
            // Restore config from saved workflow state
            self.restore_config_from_workflow_state(&mut config, workflow_state);

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

            // Handle the result (success, interruption, or failure)
            self.handle_resume_result(result, &session_file, session_id, &config.command.playbook)
                .await?;

            // Cleanup - need to call the cleanup function from the orchestrator
            // For now, we'll just complete the session without cleanup
            // TODO: Pass cleanup function as a parameter or make it available

            // Complete session and display summary
            let summary = self.session_manager.complete_session().await?;
            self.display_session_completion(&summary, config.command.dry_run);
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

    /// Execute a structured workflow with outputs
    pub async fn execute_structured_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        // Analysis will be run per-command as needed based on their configuration

        // Track outputs from previous commands
        let mut command_outputs: HashMap<String, HashMap<String, String>> = HashMap::new();

        // Execute iterations if configured
        let max_iterations = config.command.max_iterations;
        for iteration in 1..=max_iterations {
            if iteration > 1 {
                self.user_interaction
                    .display_progress(&format!("Starting iteration {iteration}/{max_iterations}"));
            }

            // Increment iteration counter once per iteration, not per command
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;

            // Execute each command in sequence
            for (step_index, cmd) in config.workflow.commands.iter().enumerate() {
                let mut command = cmd.to_command();
                // Apply defaults from the command registry
                crate::config::apply_command_defaults(&mut command);

                // Display step start with description
                let step_description = format!(
                    "{}: {}",
                    command.name,
                    command
                        .args
                        .iter()
                        .map(|a| a.resolve(&HashMap::new()))
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                self.user_interaction.step_start(
                    (step_index + 1) as u32,
                    config.workflow.commands.len() as u32,
                    &step_description,
                );

                // Analysis functionality has been removed in v0.3.0

                // Resolve variables from command outputs for use in variable expansion
                let mut resolved_variables = HashMap::new();

                // Collect all available outputs as variables
                for (cmd_id, outputs) in &command_outputs {
                    for (output_name, value) in outputs {
                        let var_name = format!("{cmd_id}.{output_name}");
                        resolved_variables.insert(var_name, value.clone());
                    }
                }

                // The command args already contain variable references that will be
                // expanded by the command parser
                let final_args = command.args.clone();

                // Build final command string with resolved arguments
                let mut cmd_parts = vec![format!("/{}", command.name)];
                for arg in &final_args {
                    let resolved_arg = arg.resolve(&resolved_variables);
                    if !resolved_arg.is_empty() {
                        cmd_parts.push(resolved_arg);
                    }
                }
                let final_command = cmd_parts.join(" ");

                self.user_interaction
                    .display_action(&format!("Executing command: {final_command}"));

                // Execute the command
                let mut env_vars = HashMap::new();
                env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

                let result = self
                    .claude_executor
                    .execute_claude_command(&final_command, &env.working_dir, env_vars)
                    .await?;

                if !result.success {
                    anyhow::bail!(
                        "Command '{}' failed with exit code {:?}. Error: {}",
                        command.name,
                        result.exit_code,
                        result.stderr
                    );
                } else {
                    // Track file changes when command succeeds
                    self.session_manager
                        .update_session(SessionUpdate::AddFilesChanged(1))
                        .await?;
                }

                // Handle outputs if specified
                if let Some(ref outputs) = command.outputs {
                    let mut cmd_output_map = HashMap::new();

                    for (output_name, output_decl) in outputs {
                        self.user_interaction.display_info(&format!(
                            "🔍 Looking for output '{}' with pattern: {}",
                            output_name, output_decl.file_pattern
                        ));

                        // Find files matching the pattern in git commits
                        let pattern_result = self
                            .find_files_matching_pattern(
                                &output_decl.file_pattern,
                                &env.working_dir,
                            )
                            .await;

                        match pattern_result {
                            Ok(file_path) => {
                                self.user_interaction
                                    .display_success(&format!("Found output file: {file_path}"));
                                cmd_output_map.insert(output_name.clone(), file_path);
                            }
                            Err(e) => {
                                self.user_interaction.display_warning(&format!(
                                    "Failed to find output '{output_name}': {e}"
                                ));
                                return Err(e);
                            }
                        }
                    }

                    // Store outputs for this command
                    if let Some(ref id) = command.id {
                        command_outputs.insert(id.clone(), cmd_output_map);
                        self.user_interaction
                            .display_success(&format!("💾 Stored outputs for command '{id}'"));
                    }
                }
            }

            // Check if we should continue iterations
            if iteration < max_iterations {
                // Could add logic here to check if improvements were made
                // For now, continue with all iterations as requested
            }
        }

        Ok(())
    }

    /// Find files matching a pattern in the last git commit
    pub async fn find_files_matching_pattern(
        &self,
        pattern: &str,
        working_dir: &std::path::Path,
    ) -> Result<String> {
        use tokio::process::Command;

        self.user_interaction.display_info(&format!(
            "🔎 Searching for files matching '{pattern}' in last commit"
        ));

        // Get list of files changed in the last commit
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD~1", "HEAD"])
            .current_dir(working_dir)
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow!(
                "Failed to get git diff: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let files = String::from_utf8(output.stdout)?;

        // Check each file in the diff against the pattern
        for file in files.lines() {
            let file = file.trim();
            if file.is_empty() {
                continue;
            }

            // Match based on pattern type
            let matches = if let Some(suffix) = pattern.strip_prefix('*') {
                // Wildcard pattern - match suffix
                file.ends_with(suffix)
            } else if pattern.contains('*') {
                // Glob-style pattern
                self.matches_glob_pattern(file, pattern)
            } else {
                // Simple substring match - just check if filename contains pattern
                file.split('/')
                    .next_back()
                    .unwrap_or(file)
                    .contains(pattern)
            };

            if matches {
                let full_path = working_dir.join(file);
                return Ok(full_path.to_string_lossy().to_string());
            }
        }

        Err(anyhow!(
            "No files found matching pattern '{}' in last commit",
            pattern
        ))
    }

    /// Helper to match glob-style patterns
    pub fn matches_glob_pattern(&self, file: &str, pattern: &str) -> bool {
        super::workflow_classifier::matches_glob_pattern(file, pattern)
    }

    /// Execute a MapReduce workflow with a pre-configured executor
    pub async fn execute_mapreduce_workflow_with_executor(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        mapreduce_config: &crate::config::MapReduceWorkflowConfig,
        mut executor: crate::cook::workflow::WorkflowExecutorImpl,
    ) -> Result<()> {
        // Display MapReduce-specific message
        self.user_interaction.display_info(&format!(
            "Executing MapReduce workflow: {}",
            mapreduce_config.name
        ));

        // Set environment variables for MapReduce execution
        // This ensures auto-merge works when -y flag is provided
        if config.command.auto_accept {
            std::env::set_var("PRODIGY_AUTO_MERGE", "true");
            std::env::set_var("PRODIGY_AUTO_CONFIRM", "true");
        }

        // Convert MapReduce config to ExtendedWorkflowConfig
        // Extract setup commands if they exist
        let setup_steps = mapreduce_config
            .setup
            .as_ref()
            .map(|setup| setup.commands.clone())
            .unwrap_or_default();

        let extended_workflow = ExtendedWorkflowConfig {
            name: mapreduce_config.name.clone(),
            mode: crate::cook::workflow::WorkflowMode::MapReduce,
            steps: setup_steps,
            setup_phase: mapreduce_config.to_setup_phase().context(
                "Failed to resolve setup phase configuration. Check that environment variables are properly defined."
            )?,
            map_phase: Some(mapreduce_config.to_map_phase().context(
                "Failed to resolve MapReduce configuration. Check that environment variables are properly defined."
            )?),
            reduce_phase: mapreduce_config.to_reduce_phase(),
            max_iterations: 1, // MapReduce runs once
            iterate: false,
            retry_defaults: None,
            environment: None,
            // collect_metrics removed - MMM focuses on orchestration
        };

        // Set global environment configuration if present in MapReduce workflow
        if mapreduce_config.env.is_some()
            || mapreduce_config.secrets.is_some()
            || mapreduce_config.env_files.is_some()
            || mapreduce_config.profiles.is_some()
        {
            let global_env_config = crate::cook::environment::EnvironmentConfig {
                global_env: mapreduce_config
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    crate::cook::environment::EnvValue::Static(v.clone()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                secrets: mapreduce_config.secrets.clone().unwrap_or_default(),
                env_files: mapreduce_config.env_files.clone().unwrap_or_default(),
                inherit: true,
                profiles: mapreduce_config.profiles.clone().unwrap_or_default(),
                active_profile: None,
            };
            executor = executor.with_environment_config(global_env_config)?;
        }
        // Also check standard workflow env (for backward compatibility with workflows that use both)
        else if config.workflow.env.is_some()
            || config.workflow.secrets.is_some()
            || config.workflow.env_files.is_some()
            || config.workflow.profiles.is_some()
        {
            let global_env_config = crate::cook::environment::EnvironmentConfig {
                global_env: config
                    .workflow
                    .env
                    .as_ref()
                    .map(|env| {
                        env.iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    crate::cook::environment::EnvValue::Static(v.clone()),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                secrets: config.workflow.secrets.clone().unwrap_or_default(),
                env_files: config.workflow.env_files.clone().unwrap_or_default(),
                inherit: true,
                profiles: config.workflow.profiles.clone().unwrap_or_default(),
                active_profile: None,
            };
            executor = executor.with_environment_config(global_env_config)?;
        }

        // Execute the MapReduce workflow
        let result = executor.execute(&extended_workflow, env).await;

        // Clean up environment variables
        if config.command.auto_accept {
            std::env::remove_var("PRODIGY_AUTO_MERGE");
            std::env::remove_var("PRODIGY_AUTO_CONFIRM");
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_execution_result_success() {
        let result: Result<()> = Ok(());
        let outcome = classify_execution_result(&result, SessionStatus::InProgress);
        assert_eq!(outcome, ExecutionOutcome::Success);
    }

    #[test]
    fn test_classify_execution_result_interrupted() {
        let result: Result<()> = Err(anyhow!("interrupted"));
        let outcome = classify_execution_result(&result, SessionStatus::Interrupted);
        assert_eq!(outcome, ExecutionOutcome::Interrupted);
    }

    #[test]
    fn test_classify_execution_result_failed() {
        let result: Result<()> = Err(anyhow!("test error"));
        let outcome = classify_execution_result(&result, SessionStatus::InProgress);
        match outcome {
            ExecutionOutcome::Failed(msg) => assert!(msg.contains("test error")),
            _ => panic!("Expected Failed outcome"),
        }
    }

    #[test]
    fn test_should_save_checkpoint_on_interrupted() {
        let outcome = ExecutionOutcome::Interrupted;
        assert!(should_save_checkpoint(&outcome));
    }

    #[test]
    fn test_should_not_save_checkpoint_on_success() {
        let outcome = ExecutionOutcome::Success;
        assert!(!should_save_checkpoint(&outcome));
    }

    #[test]
    fn test_should_not_save_checkpoint_on_failed() {
        let outcome = ExecutionOutcome::Failed("error".to_string());
        assert!(!should_save_checkpoint(&outcome));
    }

    #[test]
    fn test_determine_resume_message_interrupted() {
        let outcome = ExecutionOutcome::Interrupted;
        let message = determine_resume_message("session-123", "workflow.yml", &outcome);
        assert!(message.is_some());
        assert!(message.unwrap().contains("prodigy run workflow.yml --resume session-123"));
    }

    #[test]
    fn test_determine_resume_message_failed() {
        let outcome = ExecutionOutcome::Failed("error".to_string());
        let message = determine_resume_message("session-123", "workflow.yml", &outcome);
        assert!(message.is_some());
        assert!(message.unwrap().contains("prodigy resume session-123"));
    }

    #[test]
    fn test_determine_resume_message_success() {
        let outcome = ExecutionOutcome::Success;
        let message = determine_resume_message("session-123", "workflow.yml", &outcome);
        assert!(message.is_none());
    }
}
