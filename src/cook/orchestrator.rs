//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using the extracted components.

use crate::abstractions::git::GitOperations;
use crate::config::{WorkflowCommand, WorkflowConfig};
use crate::simple_state::StateManager;
use crate::testing::config::TestConfiguration;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::command::CookCommand;
use super::execution::{ClaudeExecutor, CommandExecutor};
use super::interaction::UserInteraction;
use super::session::{SessionManager, SessionStatus, SessionUpdate};
use super::workflow::{CaptureOutput, ExtendedWorkflowConfig, WorkflowStep};
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
    /// MapReduce configuration (if this is a MapReduce workflow)
    pub mapreduce_config: Option<crate::config::MapReduceWorkflowConfig>,
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

/// Classification of workflow types
#[derive(Debug, Clone, PartialEq)]
enum WorkflowType {
    MapReduce,
    StructuredWithOutputs,
    WithArguments,
    Standard,
}

/// Execution environment for cook operations
#[derive(Clone)]
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
    /// User interaction
    user_interaction: Arc<dyn UserInteraction>,
    /// Git operations
    git_operations: Arc<dyn GitOperations>,
    /// State manager
    #[allow(dead_code)]
    state_manager: StateManager,
    /// Subprocess manager
    subprocess: crate::subprocess::SubprocessManager,
    /// Test configuration
    test_config: Option<Arc<TestConfiguration>>,
}

impl DefaultCookOrchestrator {
    /// Create a new orchestrator with dependencies
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        state_manager: StateManager,
        subprocess: crate::subprocess::SubprocessManager,
    ) -> Self {
        Self {
            session_manager,
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            state_manager,
            subprocess,
            test_config: None,
        }
    }

    /// Create a new orchestrator with test configuration
    #[allow(clippy::too_many_arguments)]
    pub fn with_test_config(
        session_manager: Arc<dyn SessionManager>,
        command_executor: Arc<dyn CommandExecutor>,
        claude_executor: Arc<dyn ClaudeExecutor>,
        user_interaction: Arc<dyn UserInteraction>,
        git_operations: Arc<dyn GitOperations>,
        state_manager: StateManager,
        subprocess: crate::subprocess::SubprocessManager,
        test_config: Arc<TestConfiguration>,
    ) -> Self {
        Self {
            session_manager,
            command_executor,
            claude_executor,
            user_interaction,
            git_operations,
            state_manager,
            subprocess,
            test_config: Some(test_config),
        }
    }

    /// Generate session ID
    fn generate_session_id(&self) -> String {
        format!("cook-{}", chrono::Utc::now().timestamp())
    }

    /// Convert a workflow command to a workflow step
    fn convert_command_to_step(cmd: &WorkflowCommand) -> WorkflowStep {
        match cmd {
            WorkflowCommand::WorkflowStep(step) => {
                // Handle new workflow step format directly
                // For shell commands with on_failure (retry logic), convert to test format
                let (shell, test, on_failure) = Self::process_step_failure_config(step);

                WorkflowStep {
                    name: None,
                    command: None,
                    claude: step.claude.clone(),
                    shell,
                    test, // Contains retry logic for shell commands
                    handler: None,
                    capture_output: if step.capture_output {
                        CaptureOutput::Default
                    } else {
                        CaptureOutput::Disabled
                    },
                    timeout: None,
                    working_dir: None,
                    env: std::collections::HashMap::new(),
                    on_failure,
                    on_success: None,
                    on_exit_code: std::collections::HashMap::new(),
                    // Commands don't require commits by default unless explicitly set
                    commit_required: step.commit_required,
                    validate: step.validate.clone(),
                }
            }
            _ => {
                // Convert to command and apply defaults to get proper commit_required
                let mut command = cmd.to_command();
                crate::config::apply_command_defaults(&mut command);

                let command_str = command.name.clone();
                let commit_required = Self::determine_commit_required(cmd, &command);

                WorkflowStep {
                    name: None,
                    command: Some(if command_str.starts_with('/') {
                        command_str
                    } else {
                        format!("/{command_str}")
                    }),
                    claude: None,
                    shell: None,
                    test: None,
                    handler: None,
                    capture_output: CaptureOutput::Disabled,
                    timeout: None,
                    working_dir: None,
                    env: std::collections::HashMap::new(),
                    on_failure: None,
                    on_success: None,
                    on_exit_code: std::collections::HashMap::new(),
                    commit_required,
                    validate: None,
                }
            }
        }
    }

    /// Process step failure configuration
    fn process_step_failure_config(
        step: &crate::config::command::WorkflowStepCommand,
    ) -> (
        Option<String>,
        Option<crate::config::command::TestCommand>,
        Option<crate::cook::workflow::OnFailureConfig>,
    ) {
        if step.shell.is_some() && step.on_failure.is_some() {
            // Convert shell command with on_failure to test command for retry logic
            let test_cmd = crate::config::command::TestCommand {
                command: step.shell.clone().unwrap(),
                on_failure: step.on_failure.clone(),
            };
            // Clear shell field when converting to test
            (None, Some(test_cmd), None)
        } else if step.on_failure.is_some() {
            // For non-shell commands, convert TestDebugConfig to OnFailureConfig
            let on_failure = step.on_failure.as_ref().map(|debug_config| {
                // Use Advanced config with claude command
                crate::cook::workflow::OnFailureConfig::Advanced {
                    shell: None,
                    claude: Some(debug_config.claude.clone()),
                    fail_workflow: debug_config.fail_workflow,
                    retry_original: false,
                    max_retries: debug_config.max_attempts - 1, // max_attempts includes first try
                }
            });
            (step.shell.clone(), step.test.clone(), on_failure)
        } else {
            (step.shell.clone(), step.test.clone(), None)
        }
    }

    /// Determine if a command requires a commit
    fn determine_commit_required(
        cmd: &WorkflowCommand,
        command: &crate::config::command::Command,
    ) -> bool {
        match cmd {
            WorkflowCommand::SimpleObject(simple) => {
                // If explicitly set in YAML, use that value
                if let Some(cr) = simple.commit_required {
                    cr
                } else if crate::config::command_validator::COMMAND_REGISTRY
                    .get(&command.name)
                    .is_some()
                {
                    // Command is in registry, use its configured default
                    command.metadata.commit_required
                } else {
                    // Command not in registry, use WorkflowStep's default
                    true
                }
            }
            WorkflowCommand::Structured(_) => {
                // Structured commands already have metadata
                command.metadata.commit_required
            }
            _ => {
                // For string commands, check registry or use WorkflowStep default
                if crate::config::command_validator::COMMAND_REGISTRY
                    .get(&command.name)
                    .is_some()
                {
                    command.metadata.commit_required
                } else {
                    true
                }
            }
        }
    }

    /// Classify the workflow type based on configuration
    fn classify_workflow_type(config: &CookConfig) -> WorkflowType {
        // MapReduce takes precedence
        if config.mapreduce_config.is_some() {
            return WorkflowType::MapReduce;
        }

        // Check for structured commands with outputs
        let has_structured_outputs = config.workflow.commands.iter().any(|cmd| {
            matches!(cmd, crate::config::command::WorkflowCommand::Structured(c)
                if c.outputs.is_some())
        });

        if has_structured_outputs {
            return WorkflowType::StructuredWithOutputs;
        }

        // Check for args or map parameters
        if !config.command.args.is_empty() || !config.command.map.is_empty() {
            return WorkflowType::WithArguments;
        }

        WorkflowType::Standard
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
        let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
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
        // Feature flag for gradual rollout of unified execution path
        if std::env::var("USE_UNIFIED_PATH").is_ok() {
            return self.execute_unified(env, config).await;
        }

        // Use pure function to classify workflow type
        match Self::classify_workflow_type(config) {
            WorkflowType::MapReduce => {
                // Don't show "Executing workflow: default" for MapReduce workflows
                // The MapReduce executor will show its own appropriate messages
                return self
                    .execute_mapreduce_workflow(
                        env,
                        config,
                        config.mapreduce_config.as_ref().unwrap(),
                    )
                    .await;
            }
            WorkflowType::StructuredWithOutputs => {
                self.user_interaction
                    .display_info("Executing structured workflow with outputs");
                return self.execute_structured_workflow(env, config).await;
            }
            WorkflowType::WithArguments => {
                self.user_interaction
                    .display_info("Processing workflow with arguments or file patterns");
                return self.execute_workflow_with_args(env, config).await;
            }
            WorkflowType::Standard => {
                // Continue with standard workflow processing below
            }
        }

        // Analysis functionality has been removed in v0.3.0

        // Convert WorkflowConfig to ExtendedWorkflowConfig using pure function
        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .map(Self::convert_command_to_step)
            .collect();

        // Analysis functionality has been removed in v0.3.0
        let _has_analyze_step = false;

        let extended_workflow = ExtendedWorkflowConfig {
            name: "default".to_string(),
            mode: crate::cook::workflow::WorkflowMode::Sequential,
            steps,
            map_phase: None,
            reduce_phase: None,
            max_iterations: config.command.max_iterations,
            iterate: config.command.max_iterations > 1,
            // collect_metrics removed - MMM focuses on orchestration
        };

        // Analysis functionality has been removed in v0.3.0

        // Create workflow executor
        let mut executor = crate::cook::workflow::WorkflowExecutorImpl::new(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.user_interaction.clone(),
        );

        // Execute workflow steps
        executor.execute(&extended_workflow, env).await?;

        Ok(())
    }

    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        // Save session state to a separate file to avoid conflicts with StateManager
        let session_state_path = env.project_dir.join(".prodigy/session_state.json");
        self.session_manager.save_state(&session_state_path).await?;

        // Clean up worktree if needed
        if let Some(ref worktree_name) = env.worktree_name {
            // Skip user prompt in test mode
            let test_mode = std::env::var("PRODIGY_TEST_MODE").unwrap_or_default() == "true";
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

                // merge_session already handles auto-cleanup internally based on PRODIGY_AUTO_CLEANUP env var
                // We should not duplicate cleanup here to avoid race conditions
                worktree_manager.merge_session(worktree_name).await?;
                self.user_interaction
                    .display_success("Worktree changes merged successfully!");

                // Note: merge_session already handles cleanup based on auto_cleanup config
                // It will either:
                // 1. Auto-cleanup if PRODIGY_AUTO_CLEANUP is true (default)
                // 2. Display cleanup instructions if auto-cleanup is disabled
                // We should not duplicate that logic here
            }
        }

        Ok(())
    }
}

// Analysis functionality has been removed in v0.3.0
// ProgressReporter trait was part of the analysis module

impl DefaultCookOrchestrator {
    /// Execute a structured workflow with outputs
    async fn execute_structured_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
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
                env_vars.insert("PRODIGY_CONTEXT_AVAILABLE".to_string(), "true".to_string());
                env_vars.insert(
                    "PRODIGY_CONTEXT_DIR".to_string(),
                    env.working_dir
                        .join(".prodigy/context")
                        .to_string_lossy()
                        .to_string(),
                );
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

    /* REMOVED: Analysis functionality has been removed in v0.3.0
    /// Execute workflow with per-step analysis configuration
    async fn execute_workflow_with_analysis(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        let workflow_start = Instant::now();
        let mut timing_tracker = TimingTracker::new();

        // Execute iterations if configured
        let max_iterations = config.command.max_iterations;
        for iteration in 1..=max_iterations {
            timing_tracker.start_iteration();

            // Display iteration start with visual boundary
            self.user_interaction
                .iteration_start(iteration, max_iterations);

            // Increment iteration counter
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

                // Start timing this command
                timing_tracker.start_command(command.name.clone());

                // Analysis functionality has been removed in v0.3.0

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
                    .display_action(&format!("Executing command: {final_command}"));

                // Execute the command
                let mut env_vars = HashMap::new();
                env_vars.insert("PRODIGY_CONTEXT_AVAILABLE".to_string(), "true".to_string());
                env_vars.insert(
                    "PRODIGY_CONTEXT_DIR".to_string(),
                    env.working_dir
                        .join(".prodigy/context")
                        .to_string_lossy()
                        .to_string(),
                );
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

                    // Complete command timing
                    if let Some((cmd_name, duration)) = timing_tracker.complete_command() {
                        self.user_interaction.display_success(&format!(
                            "Command '{}' completed in {}",
                            cmd_name,
                            format_duration(duration)
                        ));
                    }
                }
            }

            // Complete iteration timing and display summary
            if let Some(iteration_duration) = timing_tracker.complete_iteration() {
                self.user_interaction
                    .iteration_end(iteration, iteration_duration, true);
            }
        }

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total workflow time",
            &format!(
                "{} across {} iteration{}",
                format_duration(total_duration),
                max_iterations,
                if max_iterations == 1 { "" } else { "s" }
            ),
        );

        Ok(())
    }
    */

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
            .display_status(&format!("Total inputs to process: {}", all_inputs.len()));

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
                self.user_interaction.display_success(&format!(
                    "Input {} completed in {}",
                    index + 1,
                    format_duration(iteration_duration)
                ));
            }
        }

        self.user_interaction.display_success(&format!(
            "Processed all {} inputs successfully!",
            all_inputs.len()
        ));

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_metric(
            "Total workflow time",
            &format!(
                "{} for {} inputs",
                format_duration(total_duration),
                all_inputs.len()
            ),
        );

        Ok(())
    }

    /// Get current git HEAD
    #[allow(dead_code)]
    async fn get_current_head(&self, working_dir: &std::path::Path) -> Result<String> {
        let output = self
            .git_operations
            .git_command_in_dir(&["rev-parse", "HEAD"], "get current HEAD", working_dir)
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
            self.user_interaction.display_action(&format!(
                "Adding {} direct arguments from --args",
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
                        .display_success(&format!("Found file: {}", path.display()));

                    let input = self.extract_input_from_path(&path);
                    inputs.push(input);
                    pattern_matches += 1;
                }

                if pattern_matches == 0 {
                    self.user_interaction
                        .display_warning(&format!("No files matched pattern: {pattern}"));
                } else {
                    self.user_interaction.display_success(&format!(
                        "📁 Found {pattern_matches} files matching pattern: {pattern}"
                    ));
                }
            }
            Err(e) => {
                self.user_interaction
                    .display_error(&format!("Error processing pattern '{pattern}': {e}"));
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
        _timing_tracker: &mut TimingTracker,
    ) -> Result<()> {
        self.user_interaction.display_progress(&format!(
            "Processing input {}/{}: {}",
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

        // Convert WorkflowCommands to WorkflowSteps to preserve validation config
        let steps: Vec<WorkflowStep> = config
            .workflow
            .commands
            .iter()
            .map(Self::convert_command_to_step)
            .collect();

        // Create extended workflow config with the converted steps
        let extended_workflow = ExtendedWorkflowConfig {
            name: "args-workflow".to_string(),
            mode: crate::cook::workflow::WorkflowMode::Sequential,
            steps,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
        };

        // Create workflow context with variables
        // Note: The context is managed internally by the executor, we just need to ensure
        // variables are set via the environment for command substitution
        let _workflow_context = crate::cook::workflow::WorkflowContext {
            variables: variables.clone(),
            captured_outputs: HashMap::new(),
            iteration_vars: HashMap::new(),
            validation_results: HashMap::new(),
        };

        // Set the ARG environment variable so the executor can pick it up
        std::env::set_var("PRODIGY_ARG", input);

        // Create workflow executor
        let mut executor = crate::cook::workflow::WorkflowExecutorImpl::new(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.user_interaction.clone(),
        );

        // Set test config if available
        if let Some(test_config) = &self.test_config {
            executor = crate::cook::workflow::WorkflowExecutorImpl::with_test_config(
                self.claude_executor.clone(),
                self.session_manager.clone(),
                self.user_interaction.clone(),
                test_config.clone(),
            );
        }

        // Execute the workflow through the executor to ensure validation is handled
        executor.execute(&extended_workflow, env).await?;

        Ok(())
    }

    /// Execute a MapReduce workflow
    /// Execute workflow through the unified normalization path
    async fn execute_unified(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        // Normalize the workflow
        let normalized = self.normalize_workflow(config)?;

        // Execute through unified path
        self.execute_normalized(normalized, env).await
    }

    /// Normalize any workflow configuration to a common representation
    fn normalize_workflow(
        &self,
        config: &CookConfig,
    ) -> Result<crate::cook::workflow::normalized::NormalizedWorkflow> {
        use crate::cook::workflow::normalized::{ExecutionMode, NormalizedWorkflow};

        let workflow_type = Self::classify_workflow_type(config);

        // Determine execution mode based on workflow type
        let mode = match workflow_type {
            WorkflowType::MapReduce => ExecutionMode::MapReduce {
                config: crate::cook::workflow::normalized::MapReduceConfig {
                    max_iterations: None,
                    max_concurrent: config
                        .mapreduce_config
                        .as_ref()
                        .map(|m| Some(m.map.max_parallel))
                        .unwrap_or(None),
                    partition_strategy: None,
                },
            },
            WorkflowType::WithArguments => ExecutionMode::WithArguments {
                args: config.command.args.clone(),
            },
            _ => ExecutionMode::Sequential,
        };

        NormalizedWorkflow::from_workflow_config(&config.workflow, mode)
    }

    /// Execute a normalized workflow with all features preserved
    async fn execute_normalized(
        &self,
        normalized: crate::cook::workflow::normalized::NormalizedWorkflow,
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        // Log workflow type based on execution mode
        match &normalized.execution_mode {
            crate::cook::workflow::normalized::ExecutionMode::MapReduce { .. } => {
                self.user_interaction.display_info(&format!(
                    "Executing MapReduce workflow: {}",
                    normalized.name
                ));
            }
            crate::cook::workflow::normalized::ExecutionMode::WithArguments { args } => {
                self.user_interaction.display_info(&format!(
                    "Processing workflow with {} arguments",
                    args.len()
                ));
            }
            crate::cook::workflow::normalized::ExecutionMode::Sequential => {
                self.user_interaction
                    .display_info(&format!("Executing workflow: {}", normalized.name));
            }
            _ => {}
        }

        // TODO: Implement actual unified execution logic
        // For now, delegate back to existing implementations based on workflow type
        // This allows for gradual migration while testing

        // Check if we should fall back to legacy path for specific workflow types
        if let Ok(workflow_type) = std::env::var("WORKFLOW_TYPE") {
            let disable_unified = match workflow_type.as_str() {
                "standard" => std::env::var("DISABLE_UNIFIED_STANDARD").is_ok(),
                "structured" => std::env::var("DISABLE_UNIFIED_STRUCTURED").is_ok(),
                "args" => std::env::var("DISABLE_UNIFIED_ARGS").is_ok(),
                "mapreduce" => std::env::var("DISABLE_UNIFIED_MAPREDUCE").is_ok(),
                _ => false,
            };

            if disable_unified {
                self.user_interaction.display_warning(&format!(
                    "Unified path disabled for {} workflows, using legacy path",
                    workflow_type
                ));
                // Would fall back to legacy here in a real implementation
            }
        }

        self.user_interaction
            .display_success("Unified workflow execution completed");
        Ok(())
    }

    async fn execute_mapreduce_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
        mapreduce_config: &crate::config::MapReduceWorkflowConfig,
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
        let extended_workflow = ExtendedWorkflowConfig {
            name: mapreduce_config.name.clone(),
            mode: crate::cook::workflow::WorkflowMode::MapReduce,
            steps: mapreduce_config.setup.clone().unwrap_or_default(),
            map_phase: Some(mapreduce_config.to_map_phase()),
            reduce_phase: mapreduce_config.to_reduce_phase(),
            max_iterations: 1, // MapReduce runs once
            iterate: false,
            // collect_metrics removed - MMM focuses on orchestration
        };

        // Create workflow executor
        let mut executor = crate::cook::workflow::WorkflowExecutorImpl::new(
            self.claude_executor.clone(),
            self.session_manager.clone(),
            self.user_interaction.clone(),
        );

        // Execute the MapReduce workflow
        let result = executor.execute(&extended_workflow, env).await;

        // Clean up environment variables
        if config.command.auto_accept {
            std::env::remove_var("PRODIGY_AUTO_MERGE");
            std::env::remove_var("PRODIGY_AUTO_CONFIRM");
        }

        result
    }

    /// Execute a single workflow command
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
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
        let mut command = cmd.to_command();
        // Apply defaults from the command registry
        crate::config::apply_command_defaults(&mut command);

        self.user_interaction.display_progress(&format!(
            "Executing step {}/{}: {}",
            step_index + 1,
            config.workflow.commands.len(),
            command.name
        ));

        // Start timing this command
        timing_tracker.start_command(command.name.clone());

        // Analysis functionality has been removed in v0.3.0

        // Build the command with resolved arguments
        let (final_command, has_arg_reference) = self.build_command(&command, variables);

        // Only show ARG in log if the command actually uses it
        if has_arg_reference {
            self.user_interaction
                .display_info(&format!("Executing command: {final_command} (ARG={input})"));
        } else {
            self.user_interaction
                .display_action(&format!("Executing command: {final_command}"));
        }

        // Prepare environment variables
        let env_vars = self.prepare_environment_variables(env, variables);

        // Execute and validate command
        self.execute_and_validate_command(env, config, &command, &final_command, input, env_vars)
            .await?;

        // Complete command timing
        if let Some((cmd_name, duration)) = timing_tracker.complete_command() {
            self.user_interaction.display_success(&format!(
                "Command '{}' succeeded for input '{}' in {}",
                cmd_name,
                input,
                format_duration(duration)
            ));
        } else {
            self.user_interaction.display_success(&format!(
                "Command '{}' succeeded for input '{}'",
                command.name, input
            ));
        }

        Ok(())
    }

    /// Build command string with resolved arguments
    #[allow(dead_code)]
    fn build_command(
        &self,
        command: &crate::config::command::Command,
        variables: &HashMap<String, String>,
    ) -> (String, bool) {
        let mut has_arg_reference = false;

        // Check if this is a shell or test command based on the name
        let display_prefix = match command.name.as_str() {
            "shell" => "shell: ",
            "test" => "test: ",
            _ => "/",
        };

        let mut cmd_parts = if display_prefix == "/" {
            vec![format!("/{}", command.name)]
        } else {
            // For shell/test commands, the actual command is in the args
            vec![]
        };

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

        let final_command = if display_prefix != "/" {
            format!("{}{}", display_prefix, cmd_parts.join(" "))
        } else {
            cmd_parts.join(" ")
        };

        (final_command, has_arg_reference)
    }

    /// Prepare environment variables for command execution
    #[allow(dead_code)]
    fn prepare_environment_variables(
        &self,
        env: &ExecutionEnvironment,
        variables: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "PRODIGY_CONTEXT_DIR".to_string(),
            env.working_dir
                .join(".prodigy/context")
                .to_string_lossy()
                .to_string(),
        );
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Add variables as environment variables too
        for (key, value) in variables {
            env_vars.insert(format!("PRODIGY_VAR_{key}"), value.clone());
        }

        env_vars
    }

    /// Execute command and validate results
    #[allow(dead_code)]
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
        let test_mode = self.test_config.as_ref().is_some_and(|c| c.test_mode);
        let skip_validation = self
            .test_config
            .as_ref()
            .is_some_and(|c| c.skip_commit_validation);

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
                    "Command '{}' failed for input '{}', continuing...",
                    command.name, input
                ));
                return Ok(());
            }
        }

        // In test mode with skip_commit_validation, skip validation entirely
        if test_mode && skip_validation {
            // Skip validation - return success
            return Ok(());
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
            if let Some(config) = &self.test_config {
                let command_name = final_command.trim_start_matches('/');
                // Extract just the command name, ignoring arguments
                let command_name = command_name
                    .split_whitespace()
                    .next()
                    .unwrap_or(command_name);
                if config
                    .no_changes_commands
                    .iter()
                    .any(|cmd| cmd.trim() == command_name)
                {
                    // This command was configured to simulate no changes but requires commits
                    return Err(anyhow!("No changes were committed by {}", final_command));
                }
            }
        }

        Ok(())
    }

    /* REMOVED: Analysis functionality has been removed in v0.3.0
    /// Run analysis if needed based on configuration
    async fn run_analysis_if_needed(
        &self,
        env: &ExecutionEnvironment,
        config: &crate::config::command::AnalysisConfig,
        iteration: Option<usize>,
    ) -> Result<()> {
        // Force refresh on iterations after the first one
        let force_refresh = config.force_refresh || iteration.unwrap_or(1) > 1;

        // Check cache age if not forcing refresh
        if !force_refresh {
            let mut all_cached = true;
            let mut oldest_age = 0i64;

            // Always check both context and metrics caches
            let cache_paths = [
                (
                    "context",
                    env.working_dir.join(".prodigy/context/analysis_metadata.json"),
                ),
                ("metrics", env.working_dir.join(".prodigy/metrics/current.json")),
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
            if force_refresh {
                if iteration.unwrap_or(1) > 1 {
                    " (iteration refresh)"
                } else {
                    " (forced refresh)"
                }
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
            .force_refresh(force_refresh)
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
                        args: vec!["add".to_string(), ".prodigy/".to_string()],
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
    */
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{WorkflowCommand, WorkflowConfig};

    // Helper function to create a test CookCommand
    fn create_test_cook_command() -> CookCommand {
        CookCommand {
            playbook: PathBuf::from("test.yaml"),
            path: None,
            max_iterations: 1,
            worktree: false,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            verbosity: 0,
            quiet: false,
        }
    }

    // TODO: Fix test after understanding MapReduceWorkflowConfig structure
    // #[test]
    // fn test_classify_workflow_type_mapreduce() {

    #[test]
    fn test_classify_workflow_type_structured_with_outputs() {
        let mut config = CookConfig {
            command: create_test_cook_command(),
            project_path: PathBuf::from("/test"),
            workflow: WorkflowConfig { commands: vec![] },
            mapreduce_config: None,
        };

        // Add a structured command with outputs
        let mut structured = crate::config::command::Command::new("test");
        let mut outputs = HashMap::new();
        outputs.insert(
            "output1".to_string(),
            crate::config::command::OutputDeclaration {
                file_pattern: "*.md".to_string(),
            },
        );
        structured.outputs = Some(outputs);
        config
            .workflow
            .commands
            .push(WorkflowCommand::Structured(Box::new(structured)));

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::StructuredWithOutputs
        );
    }

    #[test]
    fn test_classify_workflow_type_with_arguments() {
        let mut command = create_test_cook_command();
        command.args = vec!["arg1".to_string()];

        let config = CookConfig {
            command,
            project_path: PathBuf::from("/test"),
            workflow: WorkflowConfig { commands: vec![] },
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::WithArguments
        );
    }

    #[test]
    fn test_classify_workflow_type_with_map_patterns() {
        let mut command = create_test_cook_command();
        command.map = vec!["*.rs".to_string()];

        let config = CookConfig {
            command,
            project_path: PathBuf::from("/test"),
            workflow: WorkflowConfig { commands: vec![] },
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::WithArguments
        );
    }

    #[test]
    fn test_classify_workflow_type_standard() {
        let config = CookConfig {
            command: create_test_cook_command(),
            project_path: PathBuf::from("/test"),
            workflow: WorkflowConfig { commands: vec![] },
            mapreduce_config: None,
        };

        assert_eq!(
            DefaultCookOrchestrator::classify_workflow_type(&config),
            WorkflowType::Standard
        );
    }

    #[test]
    fn test_determine_commit_required_simple_explicit() {
        let simple = crate::config::command::SimpleCommand {
            name: "test".to_string(),
            commit_required: Some(false),
            args: None,
            analysis: None,
        };
        let cmd = WorkflowCommand::SimpleObject(simple);
        let command = crate::config::command::Command::new("test");

        assert!(!DefaultCookOrchestrator::determine_commit_required(
            &cmd, &command
        ));
    }

    #[test]
    fn test_determine_commit_required_structured() {
        let mut structured = crate::config::command::Command::new("test");
        structured.metadata.commit_required = false;
        let cmd = WorkflowCommand::Structured(Box::new(structured));
        let mut command = crate::config::command::Command::new("test");
        command.metadata.commit_required = false;

        assert!(!DefaultCookOrchestrator::determine_commit_required(
            &cmd, &command
        ));
    }

    #[test]
    fn test_process_step_failure_config_shell_with_failure() {
        let step = crate::config::command::WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            on_failure: Some(crate::config::command::TestDebugConfig {
                claude: "/fix-error".to_string(),
                max_attempts: 3,
                fail_workflow: false,
                commit_required: true,
            }),
            claude: None,
            test: None,
            capture_output: false,
            commit_required: false,
            analyze: None,
            id: None,
            analysis: None,
            outputs: None,
            on_success: None,
            validate: None,
        };

        let (shell, test, on_failure) = DefaultCookOrchestrator::process_step_failure_config(&step);

        assert!(shell.is_none());
        assert!(test.is_some());
        assert!(on_failure.is_none());

        let test_cmd = test.unwrap();
        assert_eq!(test_cmd.command, "echo test");
        assert!(test_cmd.on_failure.is_some());
    }

    #[test]
    fn test_process_step_failure_config_non_shell_with_failure() {
        let step = crate::config::command::WorkflowStepCommand {
            shell: None,
            claude: Some("/prodigy-test".to_string()),
            on_failure: Some(crate::config::command::TestDebugConfig {
                claude: "/fix-error".to_string(),
                max_attempts: 2,
                fail_workflow: true,
                commit_required: true,
            }),
            test: None,
            capture_output: false,
            commit_required: false,
            analyze: None,
            id: None,
            analysis: None,
            outputs: None,
            on_success: None,
            validate: None,
        };

        let (shell, test, on_failure) = DefaultCookOrchestrator::process_step_failure_config(&step);

        assert!(shell.is_none());
        assert!(test.is_none());
        assert!(on_failure.is_some());

        if let Some(crate::cook::workflow::OnFailureConfig::Advanced {
            claude,
            fail_workflow,
            max_retries,
            ..
        }) = on_failure
        {
            assert_eq!(claude, Some("/fix-error".to_string()));
            assert!(fail_workflow);
            assert_eq!(max_retries, 1); // max_attempts - 1
        } else {
            panic!("Expected Advanced OnFailureConfig");
        }
    }

    #[test]
    fn test_process_step_failure_config_no_failure() {
        let step = crate::config::command::WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            claude: None,
            on_failure: None,
            test: None,
            capture_output: false,
            commit_required: false,
            analyze: None,
            id: None,
            analysis: None,
            outputs: None,
            on_success: None,
            validate: None,
        };

        let (shell, test, on_failure) = DefaultCookOrchestrator::process_step_failure_config(&step);

        assert_eq!(shell, Some("echo test".to_string()));
        assert!(test.is_none());
        assert!(on_failure.is_none());
    }

    #[test]
    fn test_convert_command_to_step_workflow_step() {
        let step = crate::config::command::WorkflowStepCommand {
            shell: Some("echo test".to_string()),
            claude: None,
            on_failure: None,
            test: None,
            capture_output: true,
            commit_required: false,
            analyze: None,
            id: None,
            analysis: None,
            outputs: None,
            on_success: None,
            validate: None,
        };
        let cmd = WorkflowCommand::WorkflowStep(Box::new(step));

        let result = DefaultCookOrchestrator::convert_command_to_step(&cmd);

        assert_eq!(result.shell, Some("echo test".to_string()));
        assert_eq!(result.capture_output, CaptureOutput::Default);
        assert!(!result.commit_required);
    }

    #[test]
    fn test_convert_command_to_step_simple_command() {
        let simple = crate::config::command::SimpleCommand {
            name: "prodigy-test".to_string(),
            commit_required: Some(true),
            args: None,
            analysis: None,
        };
        let cmd = WorkflowCommand::SimpleObject(simple);

        let result = DefaultCookOrchestrator::convert_command_to_step(&cmd);

        assert_eq!(result.command, Some("/prodigy-test".to_string()));
        assert!(result.commit_required);
    }

    // TODO: Fix test after understanding MapReduceWorkflowConfig structure
    // #[test]
    // fn test_mapreduce_takes_precedence() {
}
