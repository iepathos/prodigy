//! Merge workflow orchestration for worktree sessions
//!
//! This module handles all merge workflow execution logic, providing a clean
//! separation between worktree management and merge operation orchestration.
//! It supports both Claude-assisted merges and custom merge workflows defined
//! in YAML configuration.
//!
//! # Architecture
//!
//! The `MergeOrchestrator` encapsulates:
//! - Custom merge workflow execution
//! - Claude-assisted merge operations
//! - Variable interpolation for merge contexts
//! - Checkpoint management for merge workflows
//! - Shell and Claude command execution within merge workflows
//!
//! # Design Principles
//!
//! - **Dependency Injection**: All dependencies passed explicitly
//! - **I/O at Edges**: Command execution handled here, validation separate
//! - **Pure Functions**: Variable interpolation delegates to utilities
//! - **Async Orchestration**: Complex workflows coordinated asynchronously

use crate::config::mapreduce::MergeWorkflow;
use crate::cook::execution::{ClaudeExecutor, ClaudeExecutorImpl};
use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::manager_utilities;
use super::manager_validation;
use super::WorktreeState;

/// Orchestrates merge workflow execution for worktree sessions
///
/// The `MergeOrchestrator` is responsible for executing merge operations,
/// whether using Claude-assisted merges or custom merge workflows defined
/// in YAML configuration. It handles variable interpolation, checkpoint
/// management, and command execution.
pub struct MergeOrchestrator {
    /// Subprocess manager for executing git and shell commands
    subprocess: SubprocessManager,
    /// Base directory for worktrees
    base_dir: PathBuf,
    /// Repository root path
    repo_path: PathBuf,
    /// Verbosity level for output (0=quiet, 1=verbose, 2+=debug)
    verbosity: u8,
    /// Optional custom merge workflow configuration
    custom_merge_workflow: Option<MergeWorkflow>,
    /// Workflow environment variables for interpolation
    workflow_env: HashMap<String, String>,
}

impl MergeOrchestrator {
    /// Create a new merge orchestrator
    ///
    /// # Arguments
    ///
    /// * `subprocess` - Subprocess manager for command execution
    /// * `base_dir` - Base directory for worktrees
    /// * `repo_path` - Repository root path
    /// * `verbosity` - Verbosity level for output
    /// * `custom_merge_workflow` - Optional custom merge workflow
    /// * `workflow_env` - Workflow environment variables
    pub fn new(
        subprocess: SubprocessManager,
        base_dir: PathBuf,
        repo_path: PathBuf,
        verbosity: u8,
        custom_merge_workflow: Option<MergeWorkflow>,
        workflow_env: HashMap<String, String>,
    ) -> Self {
        Self {
            subprocess,
            base_dir,
            repo_path,
            verbosity,
            custom_merge_workflow,
            workflow_env,
        }
    }

    /// Execute merge workflow - orchestrates merge execution
    ///
    /// This is the main entry point for merge operations. It delegates to
    /// either custom merge workflow execution or Claude-assisted merge based
    /// on configuration.
    ///
    /// # Arguments
    ///
    /// * `name` - Worktree session name
    /// * `worktree_branch` - Source branch to merge from
    /// * `target_branch` - Target branch to merge into
    /// * `state_loader` - Function to load worktree state
    ///
    /// # Returns
    ///
    /// Output from the merge operation
    pub async fn execute_merge_workflow<F>(
        &self,
        name: &str,
        worktree_branch: &str,
        target_branch: &str,
        state_loader: F,
    ) -> Result<String>
    where
        F: Fn(&str) -> Result<WorktreeState>,
    {
        match &self.custom_merge_workflow {
            Some(merge_workflow) => {
                println!(
                    "🔄 Executing custom merge workflow for '{name}' into '{target_branch}'..."
                );
                self.execute_custom_merge_workflow(
                    merge_workflow,
                    name,
                    worktree_branch,
                    target_branch,
                    state_loader,
                )
                .await
            }
            None => {
                println!("🔄 Merging worktree '{name}' into '{target_branch}' using Claude-assisted merge...");
                self.execute_claude_merge(name, worktree_branch, target_branch).await
            }
        }
    }

    /// Execute Claude merge - I/O operation
    ///
    /// Executes a Claude-assisted merge using the `/prodigy-merge-worktree` command.
    ///
    /// # Arguments
    ///
    /// * `name` - Worktree session name
    /// * `worktree_branch` - Source branch to merge from
    /// * `target_branch` - Target branch to merge into
    ///
    /// # Returns
    ///
    /// Output from the Claude merge command
    async fn execute_claude_merge(&self, name: &str, worktree_branch: &str, target_branch: &str) -> Result<String> {
        let worktree_path = self.base_dir.join(name);

        if !worktree_path.exists() {
            anyhow::bail!("Worktree path does not exist: {}", worktree_path.display());
        }

        if self.verbosity >= 1 {
            eprintln!("Running claude /prodigy-merge-worktree with source: {worktree_branch}, target: {target_branch}");
            eprintln!("Working directory: {}", worktree_path.display());
        }

        let env_vars = self.build_claude_environment_variables();
        let claude_executor = self.create_claude_executor();

        let result = claude_executor
            .execute_claude_command(
                &format!("/prodigy-merge-worktree {worktree_branch} {target_branch}"),
                &worktree_path,
                env_vars,
            )
            .await
            .context("Failed to execute claude /prodigy-merge-worktree")?;

        manager_validation::validate_claude_result(&result)?;
        if self.verbosity == 0 {
            // Clean output - only show the final result message
            println!("{}", result.stdout);
        }
        Ok(result.stdout)
    }

    /// Execute a custom merge workflow
    ///
    /// Executes all commands in a custom merge workflow, handling variable
    /// interpolation, checkpointing, and error handling.
    ///
    /// # Arguments
    ///
    /// * `merge_workflow` - Custom merge workflow configuration
    /// * `worktree_name` - Worktree session name
    /// * `source_branch` - Source branch to merge from
    /// * `target_branch` - Target branch to merge into
    /// * `state_loader` - Function to load worktree state
    ///
    /// # Returns
    ///
    /// Concatenated output from all workflow commands
    async fn execute_custom_merge_workflow<F>(
        &self,
        merge_workflow: &MergeWorkflow,
        worktree_name: &str,
        source_branch: &str,
        target_branch: &str,
        state_loader: F,
    ) -> Result<String>
    where
        F: Fn(&str) -> Result<WorktreeState>,
    {
        let mut output = String::new();

        // Compute worktree path from session name
        let worktree_path = self.base_dir.join(worktree_name);

        // Verify worktree exists
        if !worktree_path.exists() {
            anyhow::bail!("Worktree path does not exist: {}", worktree_path.display());
        }

        // Initialize merge variables and checkpoint manager
        let (variables, _session_id) = self
            .init_merge_variables(worktree_name, source_branch, target_branch, state_loader)
            .await?;
        let checkpoint_manager = self.create_merge_checkpoint_manager()?;

        // Execute each command in the merge workflow
        let mut step_index = 0;
        for command in &merge_workflow.commands {
            match command {
                crate::cook::workflow::WorkflowStep {
                    shell: Some(shell_cmd),
                    ..
                } => {
                    let cmd_output = self
                        .execute_merge_shell_command(
                            shell_cmd,
                            &variables,
                            step_index,
                            merge_workflow.commands.len(),
                            &worktree_path,
                        )
                        .await?;
                    output.push_str(&cmd_output);

                    step_index += 1;
                    if let Err(e) = self
                        .save_merge_checkpoint(
                            &checkpoint_manager,
                            worktree_name,
                            step_index,
                            merge_workflow.commands.len(),
                            &variables,
                        )
                        .await
                    {
                        tracing::warn!("Failed to save merge workflow checkpoint: {}", e);
                    }
                }
                crate::cook::workflow::WorkflowStep {
                    claude: Some(claude_cmd),
                    ..
                } => {
                    let cmd_output = self
                        .execute_merge_claude_command(
                            claude_cmd,
                            &variables,
                            step_index,
                            merge_workflow.commands.len(),
                            &worktree_path,
                        )
                        .await?;
                    output.push_str(&cmd_output);

                    step_index += 1;
                    if let Err(e) = self
                        .save_merge_checkpoint(
                            &checkpoint_manager,
                            worktree_name,
                            step_index,
                            merge_workflow.commands.len(),
                            &variables,
                        )
                        .await
                    {
                        tracing::warn!("Failed to save merge workflow checkpoint: {}", e);
                    }
                }
                _ => {
                    // For other command types, just log them for now
                    let cmd_str = format!("{:?}", command);
                    let interpolated = self.interpolate_merge_variables(&cmd_str, &variables);
                    eprintln!(
                        "Skipping unsupported merge workflow command: {}",
                        interpolated
                    );
                    step_index += 1;
                }
            }
        }

        // Clean up the merge workflow checkpoint after successful completion
        let workflow_id = format!("merge-workflow-{}", worktree_name);
        if let Err(e) = checkpoint_manager.delete_checkpoint(&workflow_id).await {
            tracing::warn!(
                "Failed to delete merge workflow checkpoint for {}: {}",
                workflow_id,
                e
            );
        } else {
            tracing::debug!("Deleted merge workflow checkpoint for {}", workflow_id);
        }

        Ok(output)
    }

    /// Initialize variables for merge workflow execution
    ///
    /// Collects all merge-specific variables including worktree name, branches,
    /// session ID, git information, and workflow environment variables.
    ///
    /// # Arguments
    ///
    /// * `worktree_name` - Worktree session name
    /// * `source_branch` - Source branch to merge from
    /// * `target_branch` - Target branch to merge into
    /// * `state_loader` - Function to load worktree state
    ///
    /// # Returns
    ///
    /// Tuple of (variables map, session_id)
    async fn init_merge_variables<F>(
        &self,
        worktree_name: &str,
        source_branch: &str,
        target_branch: &str,
        state_loader: F,
    ) -> Result<(HashMap<String, String>, String)>
    where
        F: Fn(&str) -> Result<WorktreeState>,
    {
        let mut session_id = String::new();
        if let Ok(state) = state_loader(worktree_name) {
            session_id = state.session_id;
        }

        let mut variables = HashMap::new();
        variables.insert("merge.worktree".to_string(), worktree_name.to_string());
        variables.insert("merge.source_branch".to_string(), source_branch.to_string());
        variables.insert("merge.target_branch".to_string(), target_branch.to_string());
        variables.insert("merge.session_id".to_string(), session_id.clone());

        // Get git information for merge workflow
        let worktree_path = self.base_dir.join(worktree_name);
        if worktree_path.exists() {
            use crate::cook::execution::mapreduce::resources::git_operations::{
                GitOperationsConfig, GitOperationsService,
            };

            let config = GitOperationsConfig {
                max_commits: 100, // Limit to recent commits for merge context
                max_files: 500,   // Reasonable limit for modified files
                ..Default::default()
            };
            let mut git_service = GitOperationsService::new(config);

            match git_service
                .get_merge_git_info(&worktree_path, target_branch)
                .await
            {
                Ok(git_info) => {
                    // Serialize commits as JSON for use in workflows
                    if let Ok(commits_json) = serde_json::to_string(&git_info.commits) {
                        variables.insert("merge.commits".to_string(), commits_json);
                        variables.insert(
                            "merge.commit_count".to_string(),
                            git_info.commits.len().to_string(),
                        );
                    }

                    // Serialize modified files as JSON
                    if let Ok(files_json) = serde_json::to_string(&git_info.modified_files) {
                        variables.insert("merge.modified_files".to_string(), files_json);
                        variables.insert(
                            "merge.file_count".to_string(),
                            git_info.modified_files.len().to_string(),
                        );
                    }

                    // Add simple list of file paths for easy reference
                    let file_paths: Vec<String> = git_info
                        .modified_files
                        .iter()
                        .map(|f| f.path.to_string_lossy().to_string())
                        .collect();
                    variables.insert("merge.file_list".to_string(), file_paths.join(", "));

                    // Add simple list of commit IDs
                    let commit_ids: Vec<String> = git_info
                        .commits
                        .iter()
                        .map(|c| c.short_id.clone())
                        .collect();
                    variables.insert("merge.commit_ids".to_string(), commit_ids.join(", "));

                    tracing::debug!(
                        "Added git information to merge variables: {} commits, {} files",
                        git_info.commits.len(),
                        git_info.modified_files.len()
                    );
                }
                Err(e) => {
                    // Log warning but continue - merge can proceed without git info
                    tracing::warn!("Failed to get git information for merge variables: {}", e);
                    variables.insert("merge.commits".to_string(), "[]".to_string());
                    variables.insert("merge.modified_files".to_string(), "[]".to_string());
                    variables.insert("merge.commit_count".to_string(), "0".to_string());
                    variables.insert("merge.file_count".to_string(), "0".to_string());
                    variables.insert("merge.file_list".to_string(), String::new());
                    variables.insert("merge.commit_ids".to_string(), String::new());
                }
            }
        } else {
            // Worktree doesn't exist yet, set empty values
            variables.insert("merge.commits".to_string(), "[]".to_string());
            variables.insert("merge.modified_files".to_string(), "[]".to_string());
            variables.insert("merge.commit_count".to_string(), "0".to_string());
            variables.insert("merge.file_count".to_string(), "0".to_string());
            variables.insert("merge.file_list".to_string(), String::new());
            variables.insert("merge.commit_ids".to_string(), String::new());
        }

        // Include workflow environment variables for interpolation in merge workflow commands
        for (key, value) in &self.workflow_env {
            variables.insert(key.clone(), value.clone());
        }

        Ok((variables, session_id))
    }

    /// Execute a shell command in the merge workflow
    ///
    /// # Arguments
    ///
    /// * `shell_cmd` - Shell command to execute
    /// * `variables` - Variables for interpolation
    /// * `step_index` - Current step index (0-based)
    /// * `total_steps` - Total number of steps in workflow
    /// * `worktree_path` - Working directory for command execution
    ///
    /// # Returns
    ///
    /// Standard output from the command
    async fn execute_merge_shell_command(
        &self,
        shell_cmd: &str,
        variables: &HashMap<String, String>,
        step_index: usize,
        total_steps: usize,
        worktree_path: &Path,
    ) -> Result<String> {
        let shell_cmd_interpolated = self.interpolate_merge_variables(shell_cmd, variables);

        let step_name = format!("shell: {}", shell_cmd_interpolated);
        println!(
            "🔄 Executing step {}/{}: {}",
            step_index + 1,
            total_steps,
            step_name
        );

        self.log_execution_context(&step_name, variables, worktree_path);

        tracing::info!("Executing shell command: {}", shell_cmd_interpolated);
        tracing::info!("Working directory: {}", worktree_path.display());

        let shell_command = ProcessCommandBuilder::new("sh")
            .current_dir(worktree_path)
            .args(["-c", &shell_cmd_interpolated])
            .build();

        let result = self.subprocess.runner().run(shell_command).await?;
        if !result.status.success() {
            anyhow::bail!(
                "Merge workflow shell command failed: {}",
                shell_cmd_interpolated
            );
        }
        if !result.stdout.is_empty() {
            println!("{}", result.stdout.trim());
        }
        Ok(result.stdout)
    }

    /// Execute a Claude command in the merge workflow
    ///
    /// # Arguments
    ///
    /// * `claude_cmd` - Claude command to execute
    /// * `variables` - Variables for interpolation
    /// * `step_index` - Current step index (0-based)
    /// * `total_steps` - Total number of steps in workflow
    /// * `worktree_path` - Working directory for command execution
    ///
    /// # Returns
    ///
    /// Standard output from the command
    async fn execute_merge_claude_command(
        &self,
        claude_cmd: &str,
        variables: &HashMap<String, String>,
        step_index: usize,
        total_steps: usize,
        worktree_path: &Path,
    ) -> Result<String> {
        let claude_cmd_interpolated = self.interpolate_merge_variables(claude_cmd, variables);

        let step_name = format!("claude: {}", claude_cmd_interpolated);
        println!(
            "🔄 Executing step {}/{}: {}",
            step_index + 1,
            total_steps,
            step_name
        );

        self.log_execution_context(&step_name, variables, worktree_path);

        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Explicitly set console output based on verbosity unless overridden by environment
        let console_output_override = std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").ok();
        if let Some(override_value) = console_output_override {
            // Environment variable takes precedence
            env_vars.insert("PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(), override_value);
        } else {
            // Default: only show console output when verbosity >= 1
            env_vars.insert(
                "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
                (self.verbosity >= 1).to_string(),
            );
        }

        self.log_claude_execution_details(&env_vars);

        use crate::cook::execution::runner::RealCommandRunner;
        let command_runner = RealCommandRunner::new();
        let claude_executor =
            ClaudeExecutorImpl::new(command_runner).with_verbosity(self.verbosity);

        let result = claude_executor
            .execute_claude_command(&claude_cmd_interpolated, worktree_path, env_vars)
            .await?;

        if !result.success {
            anyhow::bail!(
                "Merge workflow Claude command failed: {}",
                claude_cmd_interpolated
            );
        }
        Ok(result.stdout)
    }

    /// Interpolate merge-specific variables in a string
    ///
    /// Delegates to the utilities module for pure string interpolation.
    ///
    /// # Arguments
    ///
    /// * `input` - String with variable placeholders
    /// * `variables` - Variables to interpolate
    ///
    /// # Returns
    ///
    /// String with variables replaced
    fn interpolate_merge_variables(
        &self,
        input: &str,
        variables: &HashMap<String, String>,
    ) -> String {
        manager_utilities::interpolate_variables(input, variables)
    }

    /// Log execution context for debugging
    ///
    /// Logs detailed execution context including step name, working directory,
    /// variables, and environment settings.
    ///
    /// # Arguments
    ///
    /// * `step_name` - Name of the step being executed
    /// * `variables` - Current variable state
    /// * `worktree_path` - Working directory path
    fn log_execution_context(
        &self,
        step_name: &str,
        variables: &HashMap<String, String>,
        worktree_path: &Path,
    ) {
        tracing::debug!("=== Step Execution Context ===");
        tracing::debug!("Step: {}", step_name);
        tracing::debug!("Working Directory: {}", worktree_path.display());
        tracing::debug!("Worktree Path: {}", worktree_path.display());
        tracing::debug!("Project Directory: {}", self.repo_path.display());
        tracing::debug!(
            "Variables:\n{}",
            manager_utilities::format_variables_for_log(variables, "  ")
        );
        tracing::debug!("Environment Variables:");
        tracing::debug!("  PRODIGY_AUTOMATION = true");
        if self.verbosity >= 1 {
            tracing::debug!("  PRODIGY_CLAUDE_STREAMING = true");
        }
        tracing::debug!("Actual execution directory: {}", worktree_path.display());
    }

    /// Log Claude-specific execution details
    ///
    /// # Arguments
    ///
    /// * `env_vars` - Environment variables for the Claude command
    fn log_claude_execution_details(&self, env_vars: &HashMap<String, String>) {
        tracing::debug!("Environment Variables:");
        for (key, value) in env_vars {
            tracing::debug!("  {} = {}", key, value);
        }
        tracing::debug!("Actual execution directory: {}", self.repo_path.display());

        tracing::debug!(
            "Claude execution mode: streaming={}, env_var={:?}",
            self.verbosity >= 1,
            env_vars.get("PRODIGY_CLAUDE_STREAMING")
        );
        if self.verbosity >= 1 {
            tracing::debug!("Using streaming mode for Claude command");
        } else {
            tracing::debug!("Using print mode for Claude command");
        }
    }

    /// Save checkpoint after a successful step
    ///
    /// # Arguments
    ///
    /// * `checkpoint_manager` - Checkpoint manager instance
    /// * `worktree_name` - Worktree session name
    /// * `step_index` - Current step index
    /// * `total_steps` - Total number of steps
    /// * `variables` - Current variable state
    async fn save_merge_checkpoint(
        &self,
        checkpoint_manager: &crate::cook::workflow::checkpoint::CheckpointManager,
        worktree_name: &str,
        step_index: usize,
        total_steps: usize,
        variables: &HashMap<String, String>,
    ) -> Result<()> {
        let checkpoint = crate::cook::workflow::checkpoint::WorkflowCheckpoint {
            workflow_id: format!("merge-workflow-{}", worktree_name),
            execution_state: crate::cook::workflow::checkpoint::ExecutionState {
                current_step_index: step_index,
                total_steps,
                status: crate::cook::workflow::checkpoint::WorkflowStatus::Running,
                start_time: chrono::Utc::now(),
                last_checkpoint: chrono::Utc::now(),
                current_iteration: Some(1),
                total_iterations: Some(1),
            },
            completed_steps: vec![],
            variable_state: variables
                .clone()
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
            mapreduce_state: None,
            timestamp: chrono::Utc::now(),
            version: 1,
            workflow_hash: format!("merge-{}", worktree_name),
            total_steps,
            workflow_name: Some(format!("merge-workflow-{}", worktree_name)),
            workflow_path: None,
            error_recovery_state: None,
            retry_checkpoint_state: None,
            variable_checkpoint_state: None,
        };
        checkpoint_manager.save_checkpoint(&checkpoint).await?;
        tracing::info!("Saved checkpoint for merge workflow at step {}", step_index);
        Ok(())
    }

    /// Build Claude environment variables for automation
    ///
    /// # Returns
    ///
    /// HashMap of environment variables for Claude execution
    fn build_claude_environment_variables(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Explicitly set console output based on verbosity unless overridden by environment
        let console_output_override = std::env::var("PRODIGY_CLAUDE_CONSOLE_OUTPUT").ok();
        if let Some(override_value) = console_output_override {
            // Environment variable takes precedence
            env_vars.insert("PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(), override_value);
        } else {
            // Default: only show console output when verbosity >= 1
            env_vars.insert(
                "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
                (self.verbosity >= 1).to_string(),
            );
        }

        env_vars
    }

    /// Create a Claude executor instance
    ///
    /// # Returns
    ///
    /// Claude executor configured with current verbosity
    fn create_claude_executor(
        &self,
    ) -> ClaudeExecutorImpl<crate::cook::execution::runner::RealCommandRunner> {
        use crate::cook::execution::runner::RealCommandRunner;
        let command_runner = RealCommandRunner::new();
        ClaudeExecutorImpl::new(command_runner).with_verbosity(self.verbosity)
    }

    /// Create a checkpoint manager for merge operations
    ///
    /// # Returns
    ///
    /// Checkpoint manager instance for merge workflow checkpoints
    fn create_merge_checkpoint_manager(
        &self,
    ) -> Result<crate::cook::workflow::checkpoint::CheckpointManager> {
        use crate::storage::{extract_repo_name, GlobalStorage};
        use std::fs;

        let storage = GlobalStorage::new()
            .map_err(|e| anyhow::anyhow!("Failed to create global storage: {}", e))?;

        let repo_name = extract_repo_name(&self.repo_path)
            .map_err(|e| anyhow::anyhow!("Failed to extract repository name: {}", e))?;

        // Create checkpoint directory synchronously
        let checkpoint_dir = storage
            .base_dir()
            .join("state")
            .join(&repo_name)
            .join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).context("Failed to create checkpoint directory")?;

        use crate::cook::workflow::checkpoint_path::CheckpointStorage;

        #[allow(deprecated)]
        let manager = crate::cook::workflow::checkpoint::CheckpointManager::with_storage(
            CheckpointStorage::Local(checkpoint_dir),
        );
        Ok(manager)
    }
}
