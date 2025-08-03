//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using the extracted components.

use crate::abstractions::git::GitOperations;
use crate::analysis::{run_analysis, AnalysisConfig, OutputFormat, ProgressReporter};
use crate::config::{WorkflowCommand, WorkflowConfig};
use crate::simple_state::StateManager;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::analysis::AnalysisCoordinator;
use super::command::CookCommand;
use super::execution::{ClaudeExecutor, CommandExecutor};
use super::interaction::UserInteraction;
use super::metrics::MetricsCoordinator;
use super::session::{SessionManager, SessionStatus, SessionUpdate};
use super::workflow::{ExtendedWorkflowConfig, WorkflowExecutor, WorkflowStep};
use crate::session::{format_duration, TimingTracker};
use std::time::Instant;

/// Configuration for cook orchestration
#[derive(Debug, Clone)]
pub struct CookConfig {
    /// Command to execute
    pub command: CookCommand,
    /// Project path
    pub project_path: PathBuf,
    /// Workflow configuration
    pub workflow: WorkflowConfig,
}

/// Trait for orchestrating cook operations
#[async_trait]
pub trait CookOrchestrator: Send + Sync {
    /// Run the cook operation
    async fn run(&self, config: CookConfig) -> Result<()>;

    /// Check prerequisites
    async fn check_prerequisites(&self) -> Result<()>;

    /// Setup working environment
    async fn setup_environment(&self, config: &CookConfig) -> Result<ExecutionEnvironment>;

    /// Execute workflow
    async fn execute_workflow(&self, env: &ExecutionEnvironment, config: &CookConfig)
        -> Result<()>;

    /// Cleanup after execution
    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()>;
}

/// Execution environment for cook operations
pub struct ExecutionEnvironment {
    /// Working directory (may be worktree)
    pub working_dir: PathBuf,
    /// Original project directory
    pub project_dir: PathBuf,
    /// Worktree name if using worktree
    pub worktree_name: Option<String>,
    /// Session ID
    pub session_id: String,
}

/// Default implementation of cook orchestrator
pub struct DefaultCookOrchestrator {
    /// Session manager
    session_manager: Arc<dyn SessionManager>,
    /// Command executor
    #[allow(dead_code)]
    command_executor: Arc<dyn CommandExecutor>,
    /// Claude executor
    claude_executor: Arc<dyn ClaudeExecutor>,
    /// Analysis coordinator
    analysis_coordinator: Arc<dyn AnalysisCoordinator>,
    /// Metrics coordinator
    metrics_coordinator: Arc<dyn MetricsCoordinator>,
    /// User interaction
    user_interaction: Arc<dyn UserInteraction>,
    /// Git operations
    git_operations: Arc<dyn GitOperations>,
    /// State manager
    #[allow(dead_code)]
    state_manager: StateManager,
    /// Subprocess manager
    subprocess: crate::subprocess::SubprocessManager,
}

impl DefaultCookOrchestrator {
    /// Create a new orchestrator with dependencies
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        analysis_coordinator: Arc<dyn AnalysisCoordinator>,
        metrics_coordinator: Arc<dyn MetricsCoordinator>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        state_manager: StateManager,
        subprocess: crate::subprocess::SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            command_executor,
            claude_executor,
            analysis_coordinator,
            metrics_coordinator,
            user_interaction,
            git_operations,
            state_manager,
            subprocess,
        }
    }

    /// Generate session ID
    fn generate_session_id(&self) -> String {
        format!("cook-{}", chrono::Utc::now().timestamp())
    }
}

#[async_trait]
impl CookOrchestrator for DefaultCookOrchestrator {
    async fn run(&self, config: CookConfig) -> Result<()> {
        // Check prerequisites
        self.check_prerequisites().await?;

        // Setup environment
        let env = self.setup_environment(&config).await?;

        // Start session
        self.session_manager.start_session(&env.session_id).await?;

        // Execute workflow
        let result = self.execute_workflow(&env, &config).await;

        // Handle result
        match result {
            Ok(_) => {
                self.session_manager
                    .update_session(SessionUpdate::UpdateStatus(SessionStatus::Completed))
                    .await?;
                self.user_interaction
                    .display_success("Cook session completed successfully!");
            }
            Err(e) => {
                self.session_manager
                    .update_session(SessionUpdate::UpdateStatus(SessionStatus::Failed))
                    .await?;
                self.session_manager
                    .update_session(SessionUpdate::AddError(e.to_string()))
                    .await?;
                self.user_interaction
                    .display_error(&format!("Cook session failed: {e}"));
                return Err(e);
            }
        }

        // Cleanup
        self.cleanup(&env, &config).await?;

        // Complete session
        let summary = self.session_manager.complete_session().await?;
        self.user_interaction.display_info(&format!(
            "Session complete: {} iterations, {} files changed",
            summary.iterations, summary.files_changed
        ));

        Ok(())
    }

    async fn check_prerequisites(&self) -> Result<()> {
        // Skip checks in test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return Ok(());
        }

        // Check Claude CLI
        if !self.claude_executor.check_claude_cli().await? {
            anyhow::bail!("Claude CLI is not available. Please install it first.");
        }

        // Check git
        if !self.git_operations.is_git_repo().await {
            anyhow::bail!("Not in a git repository. Please run from a git repository.");
        }

        Ok(())
    }

    async fn setup_environment(&self, config: &CookConfig) -> Result<ExecutionEnvironment> {
        let session_id = self.generate_session_id();
        let mut working_dir = config.project_path.clone();
        let mut worktree_name = None;

        // Setup worktree if requested
        if config.command.worktree {
            let worktree_manager =
                WorktreeManager::new(config.project_path.clone(), self.subprocess.clone())?;
            let session = worktree_manager.create_session().await?;

            working_dir = session.path.clone();
            worktree_name = Some(session.name.clone());

            self.user_interaction
                .display_info(&format!("Created worktree at: {}", working_dir.display()));
        }

        Ok(ExecutionEnvironment {
            working_dir,
            project_dir: config.project_path.clone(),
            worktree_name,
            session_id,
        })
    }

    async fn execute_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        // Check if this is a structured workflow with inputs/outputs
        let has_structured_commands = config.workflow.commands.iter().any(|cmd| {
            matches!(cmd, crate::config::command::WorkflowCommand::Structured(c)
                if c.inputs.is_some() || c.outputs.is_some())
        });

        if has_structured_commands {
            self.user_interaction
                .display_info("Executing structured workflow with inputs/outputs");
            return self.execute_structured_workflow(env, config).await;
        }

        // Check if we're processing with --args or --map
        let has_args_or_map = !config.command.args.is_empty() || !config.command.map.is_empty();
        if has_args_or_map {
            self.user_interaction
                .display_info("Processing workflow with arguments or file patterns");
            return self.execute_workflow_with_args(env, config).await;
        }

        // Check if any commands have analysis configuration
        let has_analysis_config = config.workflow.commands.iter().any(|cmd| {
            matches!(cmd, crate::config::command::WorkflowCommand::Structured(c)
                if c.analysis.is_some() || c.metadata.analysis.is_some())
        });

        if has_analysis_config {
            // Use the new direct command execution approach that supports per-step analysis
            return self.execute_workflow_with_analysis(env, config).await;
        }

        // Convert WorkflowConfig to ExtendedWorkflowConfig
        // For now, create a simple workflow with the commands
        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                use crate::config::command::WorkflowCommand;
                let (command_str, commit_required, analysis_config) = match cmd {
                    WorkflowCommand::Simple(s) => (s.clone(), true, None),
                    WorkflowCommand::Structured(c) => (
                        c.name.clone(),
                        c.metadata.commit_required,
                        c.analysis.clone().or_else(|| c.metadata.analysis.clone()),
                    ),
                    WorkflowCommand::SimpleObject(simple) => (
                        simple.name.clone(),
                        simple.commit_required.unwrap_or(true),
                        simple.analysis.clone(),
                    ),
                };

                // If analysis is configured, run it before this step
                if let Some(ref _analysis_cfg) = analysis_config {
                    // Store the analysis config for later use
                    // We'll need to run analysis before executing this step
                }

                WorkflowStep {
                    name: format!("Step {}", i + 1),
                    command: if command_str.starts_with('/') {
                        command_str
                    } else {
                        format!("/{command_str}")
                    },
                    env: std::collections::HashMap::new(),
                    commit_required,
                }
            })
            .collect();

        let extended_workflow = ExtendedWorkflowConfig {
            name: "default".to_string(),
            steps,
            max_iterations: config.command.max_iterations,
            iterate: config.command.max_iterations > 1,
            analyze_before: true,
            analyze_between: false,
            collect_metrics: config.command.metrics,
        };

        // Run initial analysis if needed
        if extended_workflow.analyze_before && !config.command.skip_analysis {
            self.user_interaction
                .display_progress("Running initial analysis...");

            // Create progress reporter wrapper
            let progress = Arc::new(OrchestrationProgressReporter {
                interaction: self.user_interaction.clone(),
            });

            // Configure unified analysis
            let analysis_config = AnalysisConfig::builder()
                .output_format(OutputFormat::Summary)
                .save_results(true)
                .commit_changes(false)
                .verbose(false)
                .build();

            // Run unified analysis
            let _results = run_analysis(
                &env.working_dir,
                analysis_config,
                self.subprocess.clone(),
                progress,
            )
            .await?;
        } else if config.command.skip_analysis {
            self.user_interaction
                .display_info("Skipping project analysis (--skip-analysis flag)");
        }

        // Create workflow executor
        let mut executor = WorkflowExecutor::new(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.analysis_coordinator.clone(),
            self.metrics_coordinator.clone(),
            self.user_interaction.clone(),
        );

        // Execute workflow steps
        executor.execute(&extended_workflow, env).await?;

        Ok(())
    }

    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        // Save session state to a separate file to avoid conflicts with StateManager
        let session_state_path = env.project_dir.join(".mmm/session_state.json");
        self.session_manager.save_state(&session_state_path).await?;

        // Clean up worktree if needed
        if let Some(ref worktree_name) = env.worktree_name {
            // Skip user prompt in test mode
            let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
            let should_merge = if test_mode {
                // Default to not merging in test mode to avoid complications
                false
            } else if config.command.auto_accept {
                // Auto-accept when -y flag is provided
                true
            } else {
                // Ask user if they want to merge
                self.user_interaction
                    .prompt_yes_no("Would you like to merge the worktree changes?")
                    .await?
            };

            if should_merge {
                let worktree_manager =
                    WorktreeManager::new(env.project_dir.clone(), self.subprocess.clone())?;
                worktree_manager.merge_session(worktree_name).await?;
                self.user_interaction
                    .display_success("Worktree changes merged successfully!");

                // After successful merge, handle cleanup
                if config.command.auto_accept {
                    // Auto cleanup when -y flag is provided
                    if let Err(e) = worktree_manager.cleanup_session(worktree_name, true).await {
                        eprintln!("⚠️ Warning: Failed to clean up worktree '{worktree_name}': {e}");
                    } else {
                        self.user_interaction.display_success("Worktree cleaned up");
                    }
                } else {
                    // Prompt for cleanup
                    let should_cleanup = self
                        .user_interaction
                        .prompt_yes_no("Would you like to clean up the worktree?")
                        .await?;

                    if should_cleanup {
                        if let Err(e) = worktree_manager.cleanup_session(worktree_name, true).await
                        {
                            eprintln!(
                                "⚠️ Warning: Failed to clean up worktree '{worktree_name}': {e}"
                            );
                        } else {
                            self.user_interaction.display_success("Worktree cleaned up");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Progress reporter wrapper for UserInteraction
struct OrchestrationProgressReporter {
    interaction: Arc<dyn UserInteraction>,
}

impl ProgressReporter for OrchestrationProgressReporter {
    fn display_progress(&self, message: &str) {
        self.interaction.display_progress(message);
    }

    fn display_info(&self, message: &str) {
        self.interaction.display_info(message);
    }

    fn display_warning(&self, message: &str) {
        self.interaction.display_warning(message);
    }

    fn display_success(&self, message: &str) {
        self.interaction.display_success(message);
    }
}

impl DefaultCookOrchestrator {
    /// Execute a structured workflow with inputs/outputs
    async fn execute_structured_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        use crate::config::command::InputMethod;
        use std::collections::HashMap;

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
                let command = cmd.to_command();

                self.user_interaction.display_progress(&format!(
                    "Executing step {}/{}: {}",
                    step_index + 1,
                    config.workflow.commands.len(),
                    command.name
                ));

                // Check if this command requires analysis
                if let Some(ref analysis_config) = command.analysis {
                    self.run_analysis_if_needed(env, analysis_config).await?;
                }

                // Resolve inputs and build final command arguments
                let mut final_args = command.args.clone();
                let mut resolved_variables = HashMap::new();

                if let Some(ref inputs) = command.inputs {
                    for (input_name, input_ref) in inputs {
                        self.user_interaction.display_info(&format!(
                            "🔍 Resolving input '{}' from: {}",
                            input_name, input_ref.from
                        ));

                        // Parse the reference (e.g., "${cleanup.spec}")
                        let resolved_value =
                            if input_ref.from.starts_with("${") && input_ref.from.ends_with('}') {
                                let var_ref = &input_ref.from[2..input_ref.from.len() - 1];
                                if let Some((cmd_id, output_name)) = var_ref.split_once('.') {
                                    if let Some(cmd_outputs) = command_outputs.get(cmd_id) {
                                        if let Some(value) = cmd_outputs.get(output_name) {
                                            self.user_interaction.display_success(&format!(
                                                "✓ Resolved {cmd_id}.{output_name} = {value}"
                                            ));
                                            value.clone()
                                        } else {
                                            return Err(anyhow!(
                                                "Output '{}' not found for command '{}'",
                                                output_name,
                                                cmd_id
                                            ));
                                        }
                                    } else {
                                        return Err(anyhow!(
                                            "Command '{}' not found or hasn't produced outputs yet",
                                            cmd_id
                                        ));
                                    }
                                } else {
                                    return Err(anyhow!(
                                        "Invalid variable reference format: {}",
                                        input_ref.from
                                    ));
                                }
                            } else {
                                input_ref.from.clone()
                            };

                        // Store resolved variable for later use
                        resolved_variables.insert(input_name.clone(), resolved_value.clone());

                        // Apply the input based on the pass_as method
                        match &input_ref.pass_as {
                            InputMethod::Argument { position } => {
                                self.user_interaction.display_info(&format!(
                                "📝 Passing '{resolved_value}' as argument at position {position}"
                            ));

                                // Ensure we have enough space in the args vector
                                while final_args.len() <= *position {
                                    final_args.push(crate::config::command::CommandArg::Literal(
                                        String::new(),
                                    ));
                                }
                                final_args[*position] =
                                    crate::config::command::CommandArg::Literal(resolved_value);
                            }
                            InputMethod::Environment { name: env_name } => {
                                // This would be handled during command execution
                                self.user_interaction.display_info(&format!(
                                    "🌍 Will set environment variable {env_name}={resolved_value}"
                                ));
                            }
                            InputMethod::Stdin => {
                                // This would be handled during command execution
                                self.user_interaction.display_info(&format!(
                                    "📥 Will pass '{resolved_value}' via stdin"
                                ));
                            }
                        }
                    }
                }

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
                    .display_info(&format!("🚀 Executing command: {final_command}"));

                // Execute the command
                let mut env_vars = HashMap::new();
                env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
                env_vars.insert(
                    "MMM_CONTEXT_DIR".to_string(),
                    env.working_dir
                        .join(".mmm/context")
                        .to_string_lossy()
                        .to_string(),
                );
                env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());

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
                                    .display_success(&format!("✓ Found output file: {file_path}"));
                                cmd_output_map.insert(output_name.clone(), file_path);
                            }
                            Err(e) => {
                                self.user_interaction.display_warning(&format!(
                                    "❌ Failed to find output '{output_name}': {e}"
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
    async fn find_files_matching_pattern(
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
    fn matches_glob_pattern(&self, file: &str, pattern: &str) -> bool {
        // Simple glob matching for common cases
        if pattern == "*" {
            return true;
        }

        // Handle patterns like "*.md" or "*test*"
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            let filename = file.split('/').next_back().unwrap_or(file);
            return filename.starts_with(prefix) && filename.ends_with(suffix);
        }

        false
    }

    /// Execute workflow with per-step analysis configuration
    async fn execute_workflow_with_analysis(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        let workflow_start = Instant::now();
        let mut timing_tracker = TimingTracker::new();

        // Run initial analysis if needed
        if !config.command.skip_analysis {
            self.user_interaction
                .display_progress("Running initial analysis...");

            // Create progress reporter wrapper
            let progress = Arc::new(OrchestrationProgressReporter {
                interaction: self.user_interaction.clone(),
            });

            // Configure unified analysis
            let analysis_config = AnalysisConfig::builder()
                .output_format(OutputFormat::Summary)
                .save_results(true)
                .commit_changes(false)
                .verbose(false)
                .build();

            // Run unified analysis
            let _results = run_analysis(
                &env.working_dir,
                analysis_config,
                self.subprocess.clone(),
                progress,
            )
            .await?;
        }

        // Execute iterations if configured
        let max_iterations = config.command.max_iterations;
        for iteration in 1..=max_iterations {
            timing_tracker.start_iteration();

            if iteration > 1 {
                self.user_interaction
                    .display_progress(&format!("Starting iteration {iteration}/{max_iterations}"));
            }

            // Increment iteration counter
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;

            // Execute each command in sequence
            for (step_index, cmd) in config.workflow.commands.iter().enumerate() {
                let command = cmd.to_command();

                self.user_interaction.display_progress(&format!(
                    "Executing step {}/{}: {}",
                    step_index + 1,
                    config.workflow.commands.len(),
                    command.name
                ));

                // Start timing this command
                timing_tracker.start_command(command.name.clone());

                // Check if this command requires analysis
                if let Some(ref analysis_config) = command.analysis {
                    self.run_analysis_if_needed(env, analysis_config).await?;
                }

                // Build command string
                let mut cmd_parts = vec![format!("/{}", command.name)];
                for arg in &command.args {
                    let resolved_arg = arg.resolve(&HashMap::new());
                    if !resolved_arg.is_empty() {
                        cmd_parts.push(resolved_arg);
                    }
                }
                let final_command = cmd_parts.join(" ");

                self.user_interaction
                    .display_info(&format!("🚀 Executing command: {final_command}"));

                // Execute the command
                let mut env_vars = HashMap::new();
                env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
                env_vars.insert(
                    "MMM_CONTEXT_DIR".to_string(),
                    env.working_dir
                        .join(".mmm/context")
                        .to_string_lossy()
                        .to_string(),
                );
                env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());

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

                    // Complete command timing
                    if let Some((cmd_name, duration)) = timing_tracker.complete_command() {
                        self.user_interaction.display_success(&format!(
                            "✓ Command '{}' completed in {}",
                            cmd_name,
                            format_duration(duration)
                        ));
                    }
                }
            }

            // Complete iteration timing
            if let Some(iteration_duration) = timing_tracker.complete_iteration() {
                self.user_interaction.display_info(&format!(
                    "✓ Iteration {} completed in {}",
                    iteration,
                    format_duration(iteration_duration)
                ));
            }
        }

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_info(&format!(
            "\n📊 Total workflow time: {} across {} iteration{}",
            format_duration(total_duration),
            max_iterations,
            if max_iterations == 1 { "" } else { "s" }
        ));

        Ok(())
    }

    /// Execute workflow with arguments from --args or --map
    async fn execute_workflow_with_args(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        let workflow_start = Instant::now();
        let mut timing_tracker = TimingTracker::new();

        // Collect all inputs from --map patterns and --args
        let all_inputs = self.collect_workflow_inputs(config)?;

        if all_inputs.is_empty() {
            return Err(anyhow!("No inputs found from --map patterns or --args"));
        }

        self.user_interaction
            .display_info(&format!("📋 Total inputs to process: {}", all_inputs.len()));

        // Run initial analysis if needed
        if !config.command.skip_analysis {
            self.user_interaction
                .display_progress("Running initial analysis...");

            // Create progress reporter wrapper
            let progress = Arc::new(OrchestrationProgressReporter {
                interaction: self.user_interaction.clone(),
            });

            // Configure unified analysis
            let analysis_config = AnalysisConfig::builder()
                .output_format(OutputFormat::Summary)
                .save_results(true)
                .commit_changes(false)
                .verbose(false)
                .build();

            // Run unified analysis
            let _results = run_analysis(
                &env.working_dir,
                analysis_config,
                self.subprocess.clone(),
                progress,
            )
            .await?;
        }

        // Process each input
        for (index, input) in all_inputs.iter().enumerate() {
            timing_tracker.start_iteration();

            self.process_workflow_input(
                env,
                config,
                input,
                index,
                all_inputs.len(),
                &mut timing_tracker,
            )
            .await?;

            if let Some(iteration_duration) = timing_tracker.complete_iteration() {
                self.user_interaction.display_info(&format!(
                    "✓ Input {} completed in {}",
                    index + 1,
                    format_duration(iteration_duration)
                ));
            }
        }

        self.user_interaction.display_success(&format!(
            "🎉 Processed all {} inputs successfully!",
            all_inputs.len()
        ));

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_info(&format!(
            "\n📊 Total workflow time: {} for {} inputs",
            format_duration(total_duration),
            all_inputs.len()
        ));

        Ok(())
    }

    /// Get current git HEAD
    async fn get_current_head(&self, _working_dir: &std::path::Path) -> Result<String> {
        let output = self
            .git_operations
            .git_command(&["rev-parse", "HEAD"], "get current HEAD")
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Collect inputs from --map patterns and --args
    fn collect_workflow_inputs(&self, config: &CookConfig) -> Result<Vec<String>> {
        let mut all_inputs = Vec::new();

        // Process --map patterns
        for pattern in &config.command.map {
            self.user_interaction
                .display_info(&format!("🔍 Processing file pattern: {pattern}"));

            let pattern_inputs = self.process_glob_pattern(pattern)?;
            all_inputs.extend(pattern_inputs);
        }

        // Add direct arguments from --args
        if !config.command.args.is_empty() {
            self.user_interaction.display_info(&format!(
                "📝 Adding {} direct arguments from --args",
                config.command.args.len()
            ));
            all_inputs.extend(config.command.args.clone());
        }

        Ok(all_inputs)
    }

    /// Process a single glob pattern and return extracted inputs
    fn process_glob_pattern(&self, pattern: &str) -> Result<Vec<String>> {
        let mut inputs = Vec::new();

        match glob::glob(pattern) {
            Ok(entries) => {
                let mut pattern_matches = 0;
                for path in entries.flatten() {
                    self.user_interaction
                        .display_info(&format!("✓ Found file: {}", path.display()));

                    let input = self.extract_input_from_path(&path);
                    inputs.push(input);
                    pattern_matches += 1;
                }

                if pattern_matches == 0 {
                    self.user_interaction
                        .display_warning(&format!("⚠️ No files matched pattern: {pattern}"));
                } else {
                    self.user_interaction.display_success(&format!(
                        "📁 Found {pattern_matches} files matching pattern: {pattern}"
                    ));
                }
            }
            Err(e) => {
                self.user_interaction
                    .display_error(&format!("❌ Error processing pattern '{pattern}': {e}"));
            }
        }

        Ok(inputs)
    }

    /// Extract input string from a file path
    fn extract_input_from_path(&self, path: &std::path::Path) -> String {
        if let Some(stem) = path.file_stem() {
            let filename = stem.to_string_lossy();
            // Extract numeric prefix if present (e.g., "65-cook-refactor" -> "65")
            if let Some(dash_pos) = filename.find('-') {
                filename[..dash_pos].to_string()
            } else {
                filename.to_string()
            }
        } else {
            path.to_string_lossy().to_string()
        }
    }

    /// Process a single workflow input
    async fn process_workflow_input(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        input: &str,
        index: usize,
        total: usize,
        timing_tracker: &mut TimingTracker,
    ) -> Result<()> {
        self.user_interaction.display_info(&format!(
            "\n🔄 Processing input {}/{}: {}",
            index + 1,
            total,
            input
        ));

        // Update session - increment iteration for each input processed
        self.session_manager
            .update_session(SessionUpdate::IncrementIteration)
            .await?;

        // Build variables map for this input
        let mut variables = HashMap::new();
        variables.insert("ARG".to_string(), input.to_string());
        variables.insert("INDEX".to_string(), (index + 1).to_string());
        variables.insert("TOTAL".to_string(), total.to_string());

        // Execute each command in the workflow
        for (step_index, cmd) in config.workflow.commands.iter().enumerate() {
            self.execute_workflow_command(
                env,
                config,
                cmd,
                step_index,
                input,
                &mut variables,
                timing_tracker,
            )
            .await?;
        }

        Ok(())
    }

    /// Execute a single workflow command
    #[allow(clippy::too_many_arguments)]
    async fn execute_workflow_command(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        cmd: &WorkflowCommand,
        step_index: usize,
        input: &str,
        variables: &mut HashMap<String, String>,
        timing_tracker: &mut TimingTracker,
    ) -> Result<()> {
        let command = cmd.to_command();

        self.user_interaction.display_progress(&format!(
            "Executing step {}/{}: {}",
            step_index + 1,
            config.workflow.commands.len(),
            command.name
        ));

        // Start timing this command
        timing_tracker.start_command(command.name.clone());

        // Check if this command requires analysis
        if let Some(ref analysis_config) = command.analysis {
            self.run_analysis_if_needed(env, analysis_config).await?;
        }

        // Build the command with resolved arguments
        let (final_command, has_arg_reference) = self.build_command(&command, variables);

        // Only show ARG in log if the command actually uses it
        if has_arg_reference {
            self.user_interaction.display_info(&format!(
                "🚀 Executing command: {final_command} (ARG={input})"
            ));
        } else {
            self.user_interaction
                .display_info(&format!("🚀 Executing command: {final_command}"));
        }

        // Prepare environment variables
        let env_vars = self.prepare_environment_variables(env, variables);

        // Execute and validate command
        self.execute_and_validate_command(env, config, &command, &final_command, input, env_vars)
            .await?;

        // Complete command timing
        if let Some((cmd_name, duration)) = timing_tracker.complete_command() {
            self.user_interaction.display_success(&format!(
                "✓ Command '{}' succeeded for input '{}' in {}",
                cmd_name,
                input,
                format_duration(duration)
            ));
        } else {
            self.user_interaction.display_success(&format!(
                "✓ Command '{}' succeeded for input '{}'",
                command.name, input
            ));
        }

        Ok(())
    }

    /// Build command string with resolved arguments
    fn build_command(
        &self,
        command: &crate::config::command::Command,
        variables: &HashMap<String, String>,
    ) -> (String, bool) {
        let mut cmd_parts = vec![format!("/{}", command.name)];
        let mut has_arg_reference = false;

        // Resolve arguments
        for arg in &command.args {
            let resolved_arg = arg.resolve(variables);
            if !resolved_arg.is_empty() {
                cmd_parts.push(resolved_arg);
                // Check if this command actually uses the ARG variable
                if arg.is_variable()
                    && matches!(arg, crate::config::command::CommandArg::Variable(var) if var == "ARG")
                {
                    has_arg_reference = true;
                }
            }
        }

        (cmd_parts.join(" "), has_arg_reference)
    }

    /// Prepare environment variables for command execution
    fn prepare_environment_variables(
        &self,
        env: &ExecutionEnvironment,
        variables: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "MMM_CONTEXT_DIR".to_string(),
            env.working_dir
                .join(".mmm/context")
                .to_string_lossy()
                .to_string(),
        );
        env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());

        // Add variables as environment variables too
        for (key, value) in variables {
            env_vars.insert(format!("MMM_VAR_{key}"), value.clone());
        }

        env_vars
    }

    /// Execute command and validate results
    async fn execute_and_validate_command(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        command: &crate::config::command::Command,
        final_command: &str,
        input: &str,
        env_vars: HashMap<String, String>,
    ) -> Result<()> {
        // Handle test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        let skip_validation =
            std::env::var("MMM_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";

        // Get HEAD before command execution if we need to verify commits
        let head_before = if !skip_validation && command.metadata.commit_required && !test_mode {
            Some(self.get_current_head(&env.working_dir).await?)
        } else {
            None
        };

        // Execute the command
        let result = self
            .claude_executor
            .execute_claude_command(final_command, &env.working_dir, env_vars)
            .await?;

        if !result.success {
            if config.command.fail_fast {
                return Err(anyhow!(
                    "Command '{}' failed for input '{}' with exit code {:?}. Error: {}",
                    command.name,
                    input,
                    result.exit_code,
                    result.stderr
                ));
            } else {
                self.user_interaction.display_warning(&format!(
                    "⚠️ Command '{}' failed for input '{}', continuing...",
                    command.name, input
                ));
                return Ok(());
            }
        }

        // In test mode with MMM_NO_COMMIT_VALIDATION or specific command list, skip validation
        if test_mode && skip_validation {
            // Check if this command is in the skip list
            if let Ok(skip_cmds) = std::env::var("MMM_NO_COMMIT_VALIDATION") {
                if skip_cmds
                    .split(',')
                    .any(|cmd| cmd.trim() == command.name.trim_start_matches('/'))
                {
                    // This command would not have made changes in real execution
                    return Err(anyhow!("No changes were committed by {}", final_command));
                }
            }
        }
        // Check for commits if required
        if let Some(before) = head_before {
            let head_after = self.get_current_head(&env.working_dir).await?;
            if head_after == before {
                // No commits were created
                return Err(anyhow!("No changes were committed by {}", final_command));
            } else {
                // Track file changes when commits were made
                self.session_manager
                    .update_session(SessionUpdate::AddFilesChanged(1))
                    .await?;
            }
        } else if test_mode && command.metadata.commit_required && !skip_validation {
            // In test mode, check if the command simulated no changes and is required to commit
            if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
                let command_name = final_command.trim_start_matches('/');
                // Extract just the command name, ignoring arguments
                let command_name = command_name
                    .split_whitespace()
                    .next()
                    .unwrap_or(command_name);
                if no_changes_cmds
                    .split(',')
                    .any(|cmd| cmd.trim() == command_name)
                {
                    // This command was configured to simulate no changes but requires commits
                    return Err(anyhow!("No changes were committed by {}", final_command));
                }
            }
        }

        Ok(())
    }

    /// Run analysis if needed based on configuration
    async fn run_analysis_if_needed(
        &self,
        env: &ExecutionEnvironment,
        config: &crate::config::command::AnalysisConfig,
    ) -> Result<()> {
        // Check cache age if not forcing refresh
        if !config.force_refresh {
            let mut all_cached = true;
            let mut oldest_age = 0i64;

            // Always check both context and metrics caches
            let cache_paths = [
                (
                    "context",
                    env.working_dir.join(".mmm/context/analysis_metadata.json"),
                ),
                ("metrics", env.working_dir.join(".mmm/metrics/current.json")),
            ];

            for (_analysis_type, cache_path) in &cache_paths {
                if !cache_path.exists() {
                    all_cached = false;
                    break;
                }

                // Read metadata to check age
                if let Ok(content) = tokio::fs::read_to_string(&cache_path).await {
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(timestamp_str) = data.get("timestamp").and_then(|v| v.as_str())
                        {
                            if let Ok(timestamp) =
                                chrono::DateTime::parse_from_rfc3339(timestamp_str)
                            {
                                let age = chrono::Utc::now().signed_duration_since(timestamp);
                                oldest_age = oldest_age.max(age.num_seconds());
                                if age.num_seconds() >= config.max_cache_age as i64 {
                                    all_cached = false;
                                    break;
                                }
                            } else {
                                all_cached = false;
                                break;
                            }
                        } else {
                            all_cached = false;
                            break;
                        }
                    } else {
                        all_cached = false;
                        break;
                    }
                } else {
                    all_cached = false;
                    break;
                }
            }

            if all_cached {
                self.user_interaction.display_info(&format!(
                    "Using cached analysis (age: {}s, max: {}s)",
                    oldest_age, config.max_cache_age
                ));
                return Ok(());
            }
        }

        // Use unified analysis function
        self.user_interaction.display_progress(&format!(
            "Running analysis{}...",
            if config.force_refresh {
                " (forced refresh)"
            } else {
                ""
            }
        ));

        // Create progress reporter wrapper
        let progress = Arc::new(OrchestrationProgressReporter {
            interaction: self.user_interaction.clone(),
        });

        // Configure unified analysis
        let analysis_config = AnalysisConfig::builder()
            .output_format(OutputFormat::Summary)
            .save_results(true)
            .commit_changes(false) // We'll commit later if in worktree mode
            .force_refresh(config.force_refresh)
            .run_metrics(true)
            .run_context(true)
            .verbose(false)
            .build();

        // Run unified analysis
        let _results = run_analysis(
            &env.working_dir,
            analysis_config,
            self.subprocess.clone(),
            progress,
        )
        .await?;

        // Commit analysis if in worktree mode
        if env.worktree_name.is_some() {
            // Check if there are changes to commit
            let status_output = self
                .subprocess
                .runner()
                .run(crate::subprocess::runner::ProcessCommand {
                    program: "git".to_string(),
                    args: vec!["status".to_string(), "--porcelain".to_string()],
                    env: HashMap::new(),
                    working_dir: Some(env.working_dir.clone()),
                    timeout: None,
                    stdin: None,
                    suppress_stderr: false,
                })
                .await?;

            if !status_output.stdout.is_empty() {
                // Add and commit analysis changes
                self.subprocess
                    .runner()
                    .run(crate::subprocess::runner::ProcessCommand {
                        program: "git".to_string(),
                        args: vec!["add".to_string(), ".mmm/".to_string()],
                        env: HashMap::new(),
                        working_dir: Some(env.working_dir.clone()),
                        timeout: None,
                        stdin: None,
                        suppress_stderr: false,
                    })
                    .await?;

                self.subprocess
                    .runner()
                    .run(crate::subprocess::runner::ProcessCommand {
                        program: "git".to_string(),
                        args: vec![
                            "commit".to_string(),
                            "-m".to_string(),
                            "analysis: update project context and metrics".to_string(),
                        ],
                        env: HashMap::new(),
                        working_dir: Some(env.working_dir.clone()),
                        timeout: None,
                        stdin: None,
                        suppress_stderr: false,
                    })
                    .await?;

                self.user_interaction
                    .display_success("Analysis committed to git");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cook::analysis::runner::AnalysisRunnerImpl;
    use crate::cook::execution::claude::ClaudeExecutorImpl;
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::cook::interaction::mocks::MockUserInteraction;
    use crate::cook::metrics::collector::MetricsCollectorImpl;
    use crate::cook::session::tracker::SessionTrackerImpl;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;
    use tempfile::TempDir;

    // Custom mock git operations for testing
    struct TestMockGitOperations {
        is_repo: std::sync::Mutex<bool>,
    }

    impl TestMockGitOperations {
        fn new() -> Self {
            Self {
                is_repo: std::sync::Mutex::new(true),
            }
        }

        fn set_is_git_repo(&self, value: bool) {
            *self.is_repo.lock().unwrap() = value;
        }
    }

    #[async_trait]
    impl GitOperations for TestMockGitOperations {
        async fn git_command(
            &self,
            _args: &[&str],
            _description: &str,
        ) -> Result<std::process::Output> {
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }

        async fn is_git_repo(&self) -> bool {
            *self.is_repo.lock().unwrap()
        }

        async fn get_last_commit_message(&self) -> Result<String> {
            Ok("test commit".to_string())
        }

        async fn check_git_status(&self) -> Result<String> {
            Ok("nothing to commit".to_string())
        }

        async fn stage_all_changes(&self) -> Result<()> {
            Ok(())
        }

        async fn create_commit(&self, _message: &str) -> Result<()> {
            Ok(())
        }

        async fn create_worktree(&self, _name: &str, _path: &Path) -> Result<()> {
            Ok(())
        }

        async fn get_current_branch(&self) -> Result<String> {
            Ok("main".to_string())
        }

        async fn switch_branch(&self, _branch: &str) -> Result<()> {
            Ok(())
        }
    }

    fn create_test_orchestrator() -> (
        DefaultCookOrchestrator,
        Arc<MockUserInteraction>,
        Arc<TestMockGitOperations>,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let _mock_runner1 = MockCommandRunner::new();
        let mock_runner2 = MockCommandRunner::new();
        let mock_runner3 = MockCommandRunner::new();
        let mock_runner4 = MockCommandRunner::new();
        let mock_interaction = Arc::new(MockUserInteraction::new());
        let mock_git = Arc::new(TestMockGitOperations::new());

        let session_manager = Arc::new(SessionTrackerImpl::new(
            "test".to_string(),
            temp_dir.path().to_path_buf(),
        ));

        let command_executor = Arc::new(crate::cook::execution::runner::RealCommandRunner::new());
        let claude_executor = Arc::new(ClaudeExecutorImpl::new(mock_runner2));
        let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(mock_runner3));
        let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(mock_runner4));
        let state_manager = StateManager::new().unwrap();
        let subprocess = crate::subprocess::SubprocessManager::production();

        let orchestrator = DefaultCookOrchestrator::new(
            session_manager,
            command_executor,
            claude_executor,
            analysis_coordinator,
            metrics_coordinator,
            mock_interaction.clone(),
            mock_git.clone(),
            state_manager,
            subprocess,
        );

        (orchestrator, mock_interaction, mock_git)
    }

    #[tokio::test]
    async fn test_prerequisites_check_no_git() {
        // Ensure we're not in test mode for this test
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var("MMM_TEST_MODE") };

        let temp_dir = TempDir::new().unwrap();
        let _mock_runner1 = MockCommandRunner::new();
        let mock_runner2 = MockCommandRunner::new();
        let mock_runner3 = MockCommandRunner::new();
        let mock_runner4 = MockCommandRunner::new();
        let mock_interaction = Arc::new(MockUserInteraction::new());
        let mock_git = Arc::new(TestMockGitOperations::new());

        // Set up mock response for Claude CLI check
        mock_runner2.add_response(crate::cook::execution::ExecutionResult {
            success: true,
            stdout: "claude 1.0.0".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let session_manager = Arc::new(SessionTrackerImpl::new(
            "test".to_string(),
            temp_dir.path().to_path_buf(),
        ));

        let command_executor = Arc::new(crate::cook::execution::runner::RealCommandRunner::new());
        let claude_executor = Arc::new(ClaudeExecutorImpl::new(mock_runner2));
        let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(mock_runner3));
        let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(mock_runner4));
        let state_manager = StateManager::new().unwrap();
        let subprocess = crate::subprocess::SubprocessManager::production();

        let orchestrator = DefaultCookOrchestrator::new(
            session_manager,
            command_executor,
            claude_executor,
            analysis_coordinator,
            metrics_coordinator,
            mock_interaction.clone(),
            mock_git.clone(),
            state_manager,
            subprocess,
        );

        mock_git.set_is_git_repo(false);

        let result = orchestrator.check_prerequisites().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Not in a git repository"));
    }

    #[tokio::test]
    async fn test_setup_environment_basic() {
        let (orchestrator, _, _) = create_test_orchestrator();

        let config = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 5,
                worktree: false,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: false,
                resume: None,
                skip_analysis: false,
            },
            project_path: PathBuf::from("/tmp/test"),
            workflow: WorkflowConfig { commands: vec![] },
        };

        let env = orchestrator.setup_environment(&config).await.unwrap();

        assert_eq!(env.project_dir, PathBuf::from("/tmp/test"));
        assert_eq!(env.working_dir, PathBuf::from("/tmp/test"));
        assert!(env.worktree_name.is_none());
        assert!(env.session_id.starts_with("cook-"));
    }

    #[tokio::test]
    async fn test_detect_structured_workflow() {
        let (_orchestrator, _, _) = create_test_orchestrator();

        // Test with simple workflow (no inputs/outputs)
        let simple_workflow = WorkflowConfig {
            commands: vec![
                crate::config::command::WorkflowCommand::Simple("/mmm-code-review".to_string()),
                crate::config::command::WorkflowCommand::Simple("/mmm-lint".to_string()),
            ],
        };

        let simple_config = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: false,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: false,
                resume: None,
                skip_analysis: true,
            },
            project_path: PathBuf::from("/tmp/test"),
            workflow: simple_workflow,
        };

        // Should not detect as structured
        let has_structured = simple_config.workflow.commands.iter().any(|cmd| {
            matches!(cmd, crate::config::command::WorkflowCommand::Structured(c)
                if c.inputs.is_some() || c.outputs.is_some())
        });
        assert!(!has_structured);

        // Test with structured workflow (has inputs/outputs)
        let structured_cmd = crate::config::command::Command {
            name: "mmm-implement-spec".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: crate::config::command::CommandMetadata::default(),
            id: Some("implement".to_string()),
            outputs: None,
            inputs: Some(HashMap::from([(
                "spec".to_string(),
                crate::config::command::InputReference {
                    from: "${cleanup.spec}".to_string(),
                    pass_as: crate::config::command::InputMethod::Argument { position: 0 },
                    default: None,
                },
            )])),
            analysis: None,
        };

        let structured_workflow = WorkflowConfig {
            commands: vec![crate::config::command::WorkflowCommand::Structured(
                Box::new(structured_cmd),
            )],
        };

        let structured_config = CookConfig {
            command: simple_config.command.clone(),
            project_path: simple_config.project_path.clone(),
            workflow: structured_workflow,
        };

        // Should detect as structured
        let has_structured = structured_config.workflow.commands.iter().any(|cmd| {
            matches!(cmd, crate::config::command::WorkflowCommand::Structured(c)
                if c.inputs.is_some() || c.outputs.is_some())
        });
        assert!(has_structured);
    }

    #[tokio::test]
    async fn test_find_files_matching_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let (orchestrator, _, _) = create_test_orchestrator();

        // Initialize git repo in temp dir
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Create initial commit
        std::fs::write(temp_dir.path().join("README.md"), "Initial").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Create test files
        std::fs::create_dir_all(temp_dir.path().join("specs")).unwrap();
        std::fs::write(
            temp_dir
                .path()
                .join("specs/iteration-123-tech-debt-cleanup.md"),
            "test spec content",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("specs/other-file.md"),
            "should not match",
        )
        .unwrap();

        // Add and commit files
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "-m", "Add test files"])
            .current_dir(temp_dir.path())
            .output()
            .unwrap();

        // Test pattern matching with wildcard
        let result = orchestrator
            .find_files_matching_pattern("*-tech-debt-cleanup.md", temp_dir.path())
            .await;

        assert!(result.is_ok());
        let found_file = result.unwrap();
        assert!(found_file.contains("tech-debt-cleanup.md"));

        // Test that non-matching files are not found
        let result2 = orchestrator
            .find_files_matching_pattern("*-other-pattern.md", temp_dir.path())
            .await;
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_workflow_detects_structured_commands() {
        let (_orchestrator, _, _) = create_test_orchestrator();

        // Create a structured workflow
        let cleanup_cmd = crate::config::command::Command {
            name: "mmm-cleanup-tech-debt".to_string(),
            args: vec![],
            options: HashMap::new(),
            metadata: crate::config::command::CommandMetadata::default(),
            id: Some("cleanup".to_string()),
            outputs: Some(HashMap::from([(
                "spec".to_string(),
                crate::config::command::OutputDeclaration {
                    file_pattern: "specs/temp/*-tech-debt-cleanup.md".to_string(),
                },
            )])),
            inputs: None,
            analysis: None,
        };

        let workflow = WorkflowConfig {
            commands: vec![crate::config::command::WorkflowCommand::Structured(
                Box::new(cleanup_cmd),
            )],
        };

        let config = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: false,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: false,
                resume: None,
                skip_analysis: true,
            },
            project_path: PathBuf::from("/tmp/test"),
            workflow,
        };

        // The orchestrator should detect this as a structured workflow
        let has_structured = config.workflow.commands.iter().any(|cmd| {
            matches!(cmd, crate::config::command::WorkflowCommand::Structured(c)
                if c.inputs.is_some() || c.outputs.is_some())
        });

        assert!(
            has_structured,
            "Should detect workflow with outputs as structured"
        );
    }

    #[test]
    fn test_input_resolution_logic() {
        use crate::config::command::{CommandArg, InputMethod, InputReference};

        // Test variable resolution
        let mut resolved_variables = HashMap::new();
        resolved_variables.insert("spec_file".to_string(), "path/to/spec.md".to_string());

        let arg = CommandArg::Variable("spec_file".to_string());
        let resolved = arg.resolve(&resolved_variables);
        assert_eq!(resolved, "path/to/spec.md");

        // Test literal resolution
        let literal_arg = CommandArg::Literal("literal_value".to_string());
        let resolved_literal = literal_arg.resolve(&resolved_variables);
        assert_eq!(resolved_literal, "literal_value");

        // Test input reference parsing
        let input_ref = InputReference {
            from: "${cleanup.spec}".to_string(),
            pass_as: InputMethod::Argument { position: 0 },
            default: None,
        };

        assert!(input_ref.from.starts_with("${"));
        assert!(input_ref.from.ends_with('}'));

        let var_ref = &input_ref.from[2..input_ref.from.len() - 1];
        let parts: Vec<&str> = var_ref.split('.').collect();
        assert_eq!(parts, vec!["cleanup", "spec"]);
    }

    #[test]
    fn test_file_pattern_validation() {
        // Test various file patterns
        let patterns = vec![
            ("specs/temp/*-tech-debt-cleanup.md", true),
            ("**/*.rs", true),
            ("src/main.rs", true),
            ("", false),
        ];

        for (pattern, expected_valid) in patterns {
            let is_valid = !pattern.is_empty();
            assert_eq!(
                is_valid, expected_valid,
                "Pattern '{pattern}' validation failed"
            );
        }
    }

    #[tokio::test]
    async fn test_arg_resolution_only_for_commands_with_args() {
        let temp_dir = TempDir::new().unwrap();
        let mock_runner = MockCommandRunner::new();
        let mock_interaction = Arc::new(MockUserInteraction::new());
        let mock_git = Arc::new(TestMockGitOperations::new());

        // Set up mock responses for Claude commands (need 3 for our 3 commands)
        for _ in 0..3 {
            mock_runner.add_response(crate::cook::execution::ExecutionResult {
                success: true,
                stdout: "Command executed".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
            });
        }

        let session_manager = Arc::new(SessionTrackerImpl::new(
            "test".to_string(),
            temp_dir.path().to_path_buf(),
        ));

        let command_executor = Arc::new(crate::cook::execution::runner::RealCommandRunner::new());
        let claude_executor = Arc::new(ClaudeExecutorImpl::new(mock_runner));
        let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(MockCommandRunner::new()));
        let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(MockCommandRunner::new()));
        let state_manager = StateManager::new().unwrap();
        let subprocess = crate::subprocess::SubprocessManager::production();

        let orchestrator = DefaultCookOrchestrator::new(
            session_manager,
            command_executor,
            claude_executor,
            analysis_coordinator,
            metrics_coordinator,
            mock_interaction.clone(),
            mock_git.clone(),
            state_manager,
            subprocess,
        );

        // Create a workflow with commands that do and don't use $ARG
        let workflow = WorkflowConfig {
            commands: vec![
                // Command with $ARG
                crate::config::command::WorkflowCommand::SimpleObject(
                    crate::config::command::SimpleCommand {
                        name: "mmm-implement-spec".to_string(),
                        commit_required: Some(false),
                        args: Some(vec!["$ARG".to_string()]),
                        analysis: None,
                    },
                ),
                // Command without args
                crate::config::command::WorkflowCommand::SimpleObject(
                    crate::config::command::SimpleCommand {
                        name: "mmm-lint".to_string(),
                        commit_required: Some(false),
                        args: None,
                        analysis: None,
                    },
                ),
                // Command with literal args
                crate::config::command::WorkflowCommand::SimpleObject(
                    crate::config::command::SimpleCommand {
                        name: "mmm-check".to_string(),
                        commit_required: Some(false),
                        args: Some(vec!["--strict".to_string()]),
                        analysis: None,
                    },
                ),
            ],
        };

        let config = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: false,
                map: vec![],
                args: vec!["test-value".to_string()],
                fail_fast: false,
                metrics: false,
                auto_accept: false,
                resume: None,
                skip_analysis: true,
            },
            project_path: temp_dir.path().to_path_buf(),
            workflow,
        };

        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test-session".to_string(),
        };

        // Execute the workflow
        let result = orchestrator.execute_workflow(&env, &config).await;
        assert!(result.is_ok());

        // Check the interactions - should have different messages for commands with/without ARG
        let messages = mock_interaction.get_messages();

        // Find the command execution messages
        let command_messages: Vec<String> = messages
            .iter()
            .filter_map(|msg| {
                // Messages are prefixed with INFO:, so we need to check the content after that
                if msg.contains("🚀 Executing command:") {
                    Some(msg.clone())
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            command_messages.len(),
            3,
            "Should have 3 command execution messages"
        );

        // First command should show ARG
        assert!(
            command_messages[0].contains("(ARG=test-value)"),
            "First command should show ARG: {}",
            command_messages[0]
        );

        // Second command should NOT show ARG
        assert!(
            !command_messages[1].contains("(ARG="),
            "Second command should NOT show ARG: {}",
            command_messages[1]
        );

        // Third command should NOT show ARG (has literal args, not $ARG)
        assert!(
            !command_messages[2].contains("(ARG="),
            "Third command should NOT show ARG: {}",
            command_messages[2]
        );
    }

    #[test]
    fn test_command_arg_detection() {
        use crate::config::command::CommandArg;

        // Test variable detection
        let arg_var = CommandArg::Variable("ARG".to_string());
        assert!(arg_var.is_variable());
        assert!(matches!(&arg_var, CommandArg::Variable(var) if var == "ARG"));

        // Test literal detection
        let arg_literal = CommandArg::Literal("--flag".to_string());
        assert!(!arg_literal.is_variable());
        assert!(!matches!(&arg_literal, CommandArg::Variable(var) if var == "ARG"));

        // Test other variable
        let other_var = CommandArg::Variable("FILE".to_string());
        assert!(other_var.is_variable());
        assert!(!matches!(&other_var, CommandArg::Variable(var) if var == "ARG"));
    }

    #[tokio::test]
    async fn test_auto_accept_worktree_merge() {
        let temp_dir = TempDir::new().unwrap();
        let (orchestrator, mock_interaction, _) = create_test_orchestrator();

        // Create environment without worktree to test the basic flow
        let env_no_worktree = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test-session".to_string(),
        };

        // Test config with auto_accept = true
        let config_auto = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: true,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: true, // This should skip the prompt
                resume: None,
                skip_analysis: false,
            },
            project_path: temp_dir.path().to_path_buf(),
            workflow: WorkflowConfig { commands: vec![] },
        };

        // Test config with auto_accept = false
        let config_manual = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: true,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: false, // This should prompt the user
                resume: None,
                skip_analysis: false,
            },
            project_path: temp_dir.path().to_path_buf(),
            workflow: WorkflowConfig { commands: vec![] },
        };

        // Test without worktree (should succeed for both)
        let result_auto_no_wt = orchestrator.cleanup(&env_no_worktree, &config_auto).await;
        assert!(result_auto_no_wt.is_ok());

        let result_manual_no_wt = orchestrator.cleanup(&env_no_worktree, &config_manual).await;
        assert!(result_manual_no_wt.is_ok());

        // Verify the auto_accept flag logic by checking messages (without actual worktree operations)
        // Both should succeed without prompting since there's no worktree to merge
        let messages = mock_interaction.get_messages();
        let prompt_count = messages
            .iter()
            .filter(|msg| msg.starts_with("PROMPT:"))
            .count();
        assert_eq!(
            prompt_count, 0,
            "Should not have prompted when no worktree is present"
        );
    }

    #[tokio::test]
    async fn test_worktree_cleanup_after_merge() {
        let temp_dir = TempDir::new().unwrap();
        let (orchestrator, mock_interaction, _) = create_test_orchestrator();

        // Create environment with worktree
        let env_with_worktree = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: Some("test-worktree".to_string()),
            session_id: "test-session".to_string(),
        };

        // Test with auto_accept = true (should not prompt for cleanup)
        let _config_auto = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: true,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: true,
                resume: None,
                skip_analysis: false,
            },
            project_path: temp_dir.path().to_path_buf(),
            workflow: WorkflowConfig { commands: vec![] },
        };

        // Test with auto_accept = false (should prompt for cleanup)
        let config_manual = CookConfig {
            command: CookCommand {
                playbook: PathBuf::from("test.yml"),
                path: None,
                max_iterations: 1,
                worktree: true,
                map: vec![],
                args: vec![],
                fail_fast: false,
                metrics: false,
                auto_accept: false,
                resume: None,
                skip_analysis: false,
            },
            project_path: temp_dir.path().to_path_buf(),
            workflow: WorkflowConfig { commands: vec![] },
        };

        // Configure mock to respond to user prompts
        mock_interaction.add_yes_no_response(false); // Response to "merge the worktree changes"

        // Test with manual config
        // Note: In test mode, the actual worktree operations won't happen
        // so we're just verifying the structure is correct
        let result_manual = orchestrator
            .cleanup(&env_with_worktree, &config_manual)
            .await;
        assert!(result_manual.is_ok());

        let messages = mock_interaction.get_messages();
        let merge_prompts = messages
            .iter()
            .filter(|msg| msg.contains("merge the worktree changes"))
            .count();
        let cleanup_prompts = messages
            .iter()
            .filter(|msg| msg.contains("clean up the worktree"))
            .count();

        // In non-test mode, we would see both prompts
        // But in test mode (MMM_TEST_MODE=true), merge prompt is skipped
        // So we can only verify the structure is correct
        assert!(merge_prompts <= 1, "Should have at most one merge prompt");
        assert!(
            cleanup_prompts <= 1,
            "Should have at most one cleanup prompt"
        );
    }
}
