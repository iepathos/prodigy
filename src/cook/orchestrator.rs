//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using the extracted components.

use crate::abstractions::git::GitOperations;
use crate::config::workflow::WorkflowConfig;
use crate::simple_state::StateManager;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use super::analysis::AnalysisCoordinator;
use super::command::CookCommand;
use super::execution::{ClaudeExecutor, CommandExecutor};
use super::interaction::UserInteraction;
use super::metrics::MetricsCoordinator;
use super::session::{SessionManager, SessionStatus, SessionUpdate};
use super::workflow::{ExtendedWorkflowConfig, WorkflowExecutor, WorkflowStep};

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
    async fn cleanup(&self, env: &ExecutionEnvironment) -> Result<()>;
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
        self.cleanup(&env).await?;

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
            let session = worktree_manager
                .create_session()
                .await?;

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
                .display_info("🔄 Executing structured workflow with inputs/outputs");
            return self.execute_structured_workflow(env, config).await;
        }

        // Check if we're processing with --args or --map
        let has_args_or_map = !config.command.args.is_empty() || !config.command.map.is_empty();
        if has_args_or_map {
            self.user_interaction
                .display_info("🔄 Processing workflow with arguments or file patterns");
            return self.execute_workflow_with_args(env, config).await;
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
                let (command_str, commit_required) = match cmd {
                    WorkflowCommand::Simple(s) => (s.clone(), true),
                    WorkflowCommand::Structured(c) => (c.name.clone(), c.metadata.commit_required),
                    WorkflowCommand::SimpleObject(simple) => {
                        (simple.name.clone(), simple.commit_required.unwrap_or(true))
                    }
                };
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
            let analysis = self
                .analysis_coordinator
                .analyze_project(&env.working_dir)
                .await?;
            self.analysis_coordinator
                .save_analysis(&env.working_dir, &analysis)
                .await?;
        } else if config.command.skip_analysis {
            self.user_interaction
                .display_info("📋 Skipping project analysis (--skip-analysis flag)");
        }

        // Create workflow executor
        let executor = WorkflowExecutor::new(
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

    async fn cleanup(&self, env: &ExecutionEnvironment) -> Result<()> {
        // Save final state
        let state_path = env.project_dir.join(".mmm/state.json");
        self.session_manager.save_state(&state_path).await?;

        // Clean up worktree if needed
        if let Some(ref worktree_name) = env.worktree_name {
            // Skip user prompt in test mode
            let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
            let should_merge = if test_mode {
                // Default to not merging in test mode to avoid complications
                false
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
            }
        }

        Ok(())
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

        // Run initial analysis if needed
        if !config.command.skip_analysis {
            self.user_interaction
                .display_progress("Running initial analysis...");
            let analysis = self
                .analysis_coordinator
                .analyze_project(&env.working_dir)
                .await?;
            self.analysis_coordinator
                .save_analysis(&env.working_dir, &analysis)
                .await?;
        }

        // Track outputs from previous commands
        let mut command_outputs: HashMap<String, HashMap<String, String>> = HashMap::new();

        // Execute each command in sequence
        for (step_index, cmd) in config.workflow.commands.iter().enumerate() {
            let command = cmd.to_command();

            self.user_interaction.display_progress(&format!(
                "Executing step {}/{}: {}",
                step_index + 1,
                config.workflow.commands.len(),
                command.name
            ));

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
                        .find_files_matching_pattern(&output_decl.file_pattern, &env.working_dir)
                        .await;

                    match pattern_result {
                        Ok(file_path) => {
                            self.user_interaction
                                .display_success(&format!("✓ Found output file: {file_path}"));
                            cmd_output_map.insert(output_name.clone(), file_path);
                        }
                        Err(e) => {
                            self.user_interaction.display_error(&format!(
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

        Ok(())
    }

    /// Find files matching a glob pattern in recent git commits
    async fn find_files_matching_pattern(
        &self,
        pattern: &str,
        working_dir: &std::path::Path,
    ) -> Result<String> {
        use glob::glob;

        // First, try to find files matching the pattern in the current directory
        let full_pattern = working_dir.join(pattern);

        self.user_interaction.display_info(&format!(
            "🔎 Searching for files matching: {}",
            full_pattern.display()
        ));

        let mut found_files = Vec::new();

        // Use glob to find matching files
        if let Ok(entries) = glob(&full_pattern.to_string_lossy()) {
            for path in entries.flatten() {
                if path.is_file() {
                    found_files.push(path);
                }
            }
        }

        if found_files.is_empty() {
            return Err(anyhow!("No files found matching pattern: {}", pattern));
        }

        // Sort by modification time and take the most recent
        found_files.sort_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        let latest_file = found_files.into_iter().last().unwrap();
        Ok(latest_file.to_string_lossy().to_string())
    }

    /// Execute workflow with arguments from --args or --map
    async fn execute_workflow_with_args(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        use std::collections::HashMap;

        // Collect all inputs from --map patterns and --args
        let mut all_inputs = Vec::new();

        // First, process --map patterns
        for pattern in &config.command.map {
            self.user_interaction
                .display_info(&format!("🔍 Processing file pattern: {pattern}"));

            match glob::glob(pattern) {
                Ok(entries) => {
                    let mut pattern_matches = 0;
                    for path in entries.flatten() {
                        self.user_interaction
                            .display_info(&format!("✓ Found file: {}", path.display()));

                        // Extract spec ID or use the full path
                        let input = if let Some(stem) = path.file_stem() {
                            let filename = stem.to_string_lossy();
                            // Extract numeric prefix if present (e.g., "65-cook-refactor" -> "65")
                            if let Some(dash_pos) = filename.find('-') {
                                filename[..dash_pos].to_string()
                            } else {
                                filename.to_string()
                            }
                        } else {
                            path.to_string_lossy().to_string()
                        };

                        all_inputs.push(input);
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
        }

        // Add direct arguments from --args
        if !config.command.args.is_empty() {
            self.user_interaction.display_info(&format!(
                "📝 Adding {} direct arguments from --args",
                config.command.args.len()
            ));
            all_inputs.extend(config.command.args.clone());
        }

        if all_inputs.is_empty() {
            return Err(anyhow!("No inputs found from --map patterns or --args"));
        }

        self.user_interaction
            .display_info(&format!("📋 Total inputs to process: {}", all_inputs.len()));

        // Run initial analysis if needed
        if !config.command.skip_analysis {
            self.user_interaction
                .display_progress("Running initial analysis...");
            let analysis = self
                .analysis_coordinator
                .analyze_project(&env.working_dir)
                .await?;
            self.analysis_coordinator
                .save_analysis(&env.working_dir, &analysis)
                .await?;
        }

        // Process each input
        for (index, input) in all_inputs.iter().enumerate() {
            self.user_interaction.display_info(&format!(
                "\n🔄 Processing input {}/{}: {}",
                index + 1,
                all_inputs.len(),
                input
            ));

            // Build variables map for this input
            let mut variables = HashMap::new();
            variables.insert("ARG".to_string(), input.clone());
            variables.insert("INDEX".to_string(), (index + 1).to_string());
            variables.insert("TOTAL".to_string(), all_inputs.len().to_string());

            // Execute each command in the workflow
            for (step_index, cmd) in config.workflow.commands.iter().enumerate() {
                let command = cmd.to_command();

                self.user_interaction.display_progress(&format!(
                    "Executing step {}/{}: {}",
                    step_index + 1,
                    config.workflow.commands.len(),
                    command.name
                ));

                // Build the command with resolved arguments
                let mut cmd_parts = vec![format!("/{}", command.name)];
                let mut has_arg_reference = false;

                // Resolve arguments
                for arg in &command.args {
                    let resolved_arg = arg.resolve(&variables);
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

                let final_command = cmd_parts.join(" ");

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
                for (key, value) in &variables {
                    env_vars.insert(format!("MMM_VAR_{key}"), value.clone());
                }

                // Handle test mode
                let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
                let skip_validation =
                    std::env::var("MMM_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";

                // Get HEAD before command execution if we need to verify commits
                let head_before =
                    if !skip_validation && command.metadata.commit_required && !test_mode {
                        Some(self.get_current_head(&env.working_dir).await?)
                    } else {
                        None
                    };

                // Execute the command
                let result = self
                    .claude_executor
                    .execute_claude_command(&final_command, &env.working_dir, env_vars)
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
                        self.user_interaction.display_error(&format!(
                            "❌ Command '{}' failed for input '{}', continuing...",
                            command.name, input
                        ));
                    }
                } else {
                    // In test mode, check if this command requires commits and would have made no changes
                    if test_mode && command.metadata.commit_required && !skip_validation {
                        if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
                            if no_changes_cmds
                                .split(',')
                                .any(|cmd| cmd.trim() == command.name.trim_start_matches('/'))
                            {
                                // This command would not have made changes in real execution
                                return Err(anyhow!(
                                    "No changes were committed by {}",
                                    final_command
                                ));
                            }
                        }
                    }
                    // Check for commits if required
                    if let Some(before) = head_before {
                        let head_after = self.get_current_head(&env.working_dir).await?;
                        if head_after == before {
                            // No commits were created
                            return Err(anyhow!("No changes were committed by {}", final_command));
                        }
                    }

                    self.user_interaction.display_success(&format!(
                        "✓ Command '{}' succeeded for input '{}'",
                        command.name, input
                    ));
                }
            }
        }

        self.user_interaction.display_success(&format!(
            "🎉 Processed all {} inputs successfully!",
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
        std::env::remove_var("MMM_TEST_MODE");

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

        // Create test directory structure
        std::fs::create_dir_all(temp_dir.path().join("specs/temp")).unwrap();

        // Create test files
        std::fs::write(
            temp_dir.path().join("specs/temp/123-tech-debt-cleanup.md"),
            "test spec content",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("specs/temp/456-tech-debt-cleanup.md"),
            "newer spec content",
        )
        .unwrap();
        std::fs::write(
            temp_dir.path().join("specs/temp/other-file.md"),
            "should not match",
        )
        .unwrap();

        // Test pattern matching
        let result = orchestrator
            .find_files_matching_pattern("specs/temp/*-tech-debt-cleanup.md", temp_dir.path())
            .await;

        assert!(result.is_ok());
        let found_file = result.unwrap();
        assert!(found_file.contains("tech-debt-cleanup.md"));
        assert!(!found_file.contains("other-file.md"));
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
                    },
                ),
                // Command without args
                crate::config::command::WorkflowCommand::SimpleObject(
                    crate::config::command::SimpleCommand {
                        name: "mmm-lint".to_string(),
                        commit_required: Some(false),
                        args: None,
                    },
                ),
                // Command with literal args
                crate::config::command::WorkflowCommand::SimpleObject(
                    crate::config::command::SimpleCommand {
                        name: "mmm-check".to_string(),
                        commit_required: Some(false),
                        args: Some(vec!["--strict".to_string()]),
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
}
