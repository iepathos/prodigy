//! Workflow executor with commit verification
//!
//! Executes workflow steps in sequence, verifies git commits when required,
//! and manages iteration logic for continuous improvement sessions.

use crate::cook::analysis::AnalysisCoordinator;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::git_ops::git_command;
use crate::cook::interaction::UserInteraction;
use crate::cook::metrics::MetricsCoordinator;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::{SessionManager, SessionUpdate};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A simple workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Step name
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Whether this command is expected to create commits
    #[serde(default = "default_commit_required")]
    pub commit_required: bool,
}

fn default_commit_required() -> bool {
    true
}

/// Extended workflow configuration
#[derive(Debug, Clone)]
pub struct ExtendedWorkflowConfig {
    /// Workflow name
    pub name: String,
    /// Steps to execute
    pub steps: Vec<WorkflowStep>,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Whether to iterate
    pub iterate: bool,
    /// Analyze before workflow
    pub analyze_before: bool,
    /// Analyze between iterations
    pub analyze_between: bool,
    /// Collect metrics
    pub collect_metrics: bool,
}

/// Executes workflow steps with commit verification
pub struct WorkflowExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    analysis_coordinator: Arc<dyn AnalysisCoordinator>,
    metrics_coordinator: Arc<dyn MetricsCoordinator>,
    user_interaction: Arc<dyn UserInteraction>,
}

impl WorkflowExecutor {
    /// Create a new workflow executor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        analysis_coordinator: Arc<dyn AnalysisCoordinator>,
        metrics_coordinator: Arc<dyn MetricsCoordinator>,
        user_interaction: Arc<dyn UserInteraction>,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            analysis_coordinator,
            metrics_coordinator,
            user_interaction,
        }
    }

    /// Execute a workflow
    pub async fn execute(
        &self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        self.user_interaction.display_info(&format!(
            "Executing workflow: {} (max {} iterations)",
            workflow.name, workflow.max_iterations
        ));

        if let Some(ref focus) = env.focus {
            self.user_interaction
                .display_info(&format!("🎯 Focus: {focus}"));
        }

        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        let skip_validation =
            std::env::var("MMM_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";

        if workflow.iterate {
            self.user_interaction
                .display_progress("Starting improvement loop");
        }

        let mut iteration = 0;
        let mut should_continue = true;

        while should_continue && iteration < workflow.max_iterations {
            iteration += 1;
            self.user_interaction.display_progress(&format!(
                "Starting iteration {}/{}",
                iteration, workflow.max_iterations
            ));

            // Update session
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;

            // Execute workflow steps
            let mut any_changes = false;
            for (step_index, step) in workflow.steps.iter().enumerate() {
                self.user_interaction.display_progress(&format!(
                    "Executing step {}/{}: {}",
                    step_index + 1,
                    workflow.steps.len(),
                    step.command
                ));

                // Get HEAD before command execution if we need to verify commits
                let head_before = if !skip_validation && step.commit_required && !test_mode {
                    Some(self.get_current_head(&env.working_dir).await?)
                } else {
                    None
                };

                // Execute the step
                let step_result = self
                    .execute_step(step, env, iteration == 1 && step_index == 0, &env.focus)
                    .await
                    .context(format!("Failed to execute step: {}", step.name))?;

                // Check for commits if required
                if let Some(before) = head_before {
                    let head_after = self.get_current_head(&env.working_dir).await?;
                    if head_after == before {
                        // No commits were created
                        self.handle_no_commits_error(step)?;
                    } else {
                        any_changes = true;
                        self.user_interaction
                            .display_success(&format!("✓ {} created commits", step.name));
                    }
                } else {
                    // In test mode or when commit_required is false
                    if step_result {
                        any_changes = true;
                    } else if test_mode && step.commit_required && !skip_validation {
                        // In test mode, if no changes were made and commits were required, fail
                        self.handle_no_commits_error(step)?;
                    }
                }
            }

            // Check if we should continue
            if workflow.iterate {
                if !any_changes {
                    self.user_interaction
                        .display_info("No changes were made - stopping early");
                    should_continue = false;
                } else if self.is_focus_tracking_test() {
                    // In focus tracking test, continue for all iterations
                    should_continue = iteration < workflow.max_iterations;
                } else if test_mode {
                    // In test mode, check for early termination
                    should_continue = !self.should_stop_early_in_test_mode();
                } else {
                    // Check based on metrics or ask user
                    should_continue = self.should_continue_iterations(env).await?;
                }
            } else {
                // Single iteration workflow
                should_continue = false;
            }

            // Run analysis between iterations if configured
            if should_continue && workflow.analyze_between {
                self.user_interaction
                    .display_progress("Running analysis between iterations...");
                let analysis = self
                    .analysis_coordinator
                    .analyze_project(&env.working_dir)
                    .await?;
                self.analysis_coordinator
                    .save_analysis(&env.working_dir, &analysis)
                    .await?;
            }
        }

        // Collect final metrics if enabled
        if workflow.collect_metrics {
            self.collect_and_report_metrics(env).await?;
        }

        Ok(())
    }

    /// Execute a single workflow step
    async fn execute_step(
        &self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
        is_first_step: bool,
        focus: &Option<String>,
    ) -> Result<bool> {
        // Prepare environment variables
        let mut env_vars = HashMap::new();

        // Add MMM context variables
        env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());
        env_vars.insert(
            "MMM_CONTEXT_DIR".to_string(),
            env.working_dir
                .join(".mmm/context")
                .to_string_lossy()
                .to_string(),
        );

        // Add focus for first step only
        if is_first_step {
            if let Some(ref focus_value) = focus {
                env_vars.insert("MMM_FOCUS".to_string(), focus_value.clone());
            }
        }

        env_vars.insert("MMM_AUTOMATION".to_string(), "true".to_string());

        // Add step-specific environment variables
        for (key, value) in &step.env {
            env_vars.insert(key.clone(), value.clone());
        }

        // Handle test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return self.handle_test_mode_execution(step);
        }

        // Execute the command
        let result = self
            .claude_executor
            .execute_claude_command(&step.command, &env.working_dir, env_vars)
            .await?;

        if !result.success {
            anyhow::bail!(
                "Step '{}' failed with exit code {:?}. Error: {}",
                step.name,
                result.exit_code,
                result.stderr
            );
        }

        // Count files changed
        self.session_manager
            .update_session(SessionUpdate::AddFilesChanged(1))
            .await?;

        Ok(true)
    }

    /// Handle test mode execution
    fn handle_test_mode_execution(&self, step: &WorkflowStep) -> Result<bool> {
        println!("[TEST MODE] Would execute Claude command: {}", step.command);

        // Check if we should simulate no changes
        if self.is_test_mode_no_changes_command(&step.command) {
            println!("[TEST MODE] Simulating no changes for: {}", step.command);
            return Ok(false);
        }

        // Track focus for mmm-code-review if requested
        if step.command == "/mmm-code-review" {
            if let Ok(track_file) = std::env::var("MMM_TRACK_FOCUS") {
                if let Ok(focus) = std::env::var("MMM_FOCUS") {
                    self.write_focus_to_file(&track_file, &focus);
                }
            }
        }

        Ok(true)
    }

    /// Get current git HEAD
    async fn get_current_head(&self, _working_dir: &std::path::Path) -> Result<String> {
        let output = git_command(&["rev-parse", "HEAD"], "get current HEAD")
            .await
            .context("Failed to get git HEAD")?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Handle the case where no commits were created when expected
    fn handle_no_commits_error(&self, step: &WorkflowStep) -> Result<()> {
        let command_name = step.command.trim_start_matches('/');

        eprintln!(
            "\n❌ Workflow stopped: No changes were committed by {}",
            step.command
        );
        eprintln!("\nThe command executed successfully but did not create any git commits.");

        // Check if this is a command that might legitimately not create commits
        if matches!(command_name, "mmm-lint" | "mmm-code-review" | "mmm-analyze") {
            eprintln!(
                "This may be expected if there were no {} to fix.",
                if command_name == "mmm-lint" {
                    "linting issues"
                } else if command_name == "mmm-code-review" {
                    "issues found"
                } else {
                    "changes needed"
                }
            );
            eprintln!("\nTo allow this command to proceed without commits, set commit_required: false in your workflow");
        } else {
            eprintln!("Possible reasons:");
            eprintln!("- The specification may already be implemented");
            eprintln!("- The command may have encountered an issue without reporting an error");
            eprintln!("- No changes were needed");
            eprintln!("\nTo investigate:");
            eprintln!("- Check if the spec is already implemented");
            eprintln!("- Review the command output above for any warnings");
            eprintln!("- Run 'git status' to check for uncommitted changes");
        }

        eprintln!(
            "\nAlternatively, run with MMM_NO_COMMIT_VALIDATION=true to skip all validation."
        );

        Err(anyhow!("No commits created by command {}", step.command))
    }

    /// Check if we should continue iterations
    async fn should_continue_iterations(&self, env: &ExecutionEnvironment) -> Result<bool> {
        // Try to use metrics
        if let Ok(metrics) = self.metrics_coordinator.collect_all(&env.working_dir).await {
            // Simple heuristic: stop if no lint warnings
            if metrics.lint_warnings == 0 {
                self.user_interaction
                    .display_success("No lint warnings remaining, stopping iterations");
                return Ok(false);
            }
        }

        // Ask user
        self.user_interaction
            .prompt_yes_no("Continue with another iteration?")
            .await
    }

    /// Collect and report final metrics
    async fn collect_and_report_metrics(&self, env: &ExecutionEnvironment) -> Result<()> {
        self.user_interaction
            .display_progress("Collecting final metrics...");
        let metrics = self
            .metrics_coordinator
            .collect_all(&env.working_dir)
            .await?;
        self.metrics_coordinator
            .store_metrics(&env.working_dir, &metrics)
            .await?;

        // Generate report
        let history = self
            .metrics_coordinator
            .load_history(&env.working_dir)
            .await?;
        let report = self
            .metrics_coordinator
            .generate_report(&metrics, &history)
            .await?;
        self.user_interaction.display_info(&report);

        Ok(())
    }

    /// Check if we should stop early in test mode
    fn should_stop_early_in_test_mode(&self) -> bool {
        // Check if we're configured to simulate no changes
        std::env::var("MMM_TEST_NO_CHANGES_COMMANDS")
            .unwrap_or_default()
            .split(',')
            .any(|cmd| cmd.trim() == "mmm-code-review" || cmd.trim() == "mmm-lint")
    }

    /// Check if this is the focus tracking test
    fn is_focus_tracking_test(&self) -> bool {
        std::env::var("MMM_TRACK_FOCUS").is_ok()
    }

    /// Check if this is a test mode command that should simulate no changes
    fn is_test_mode_no_changes_command(&self, command: &str) -> bool {
        if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
            let command_name = command.trim_start_matches('/');
            return no_changes_cmds
                .split(',')
                .any(|cmd| cmd.trim() == command_name);
        }
        false
    }

    /// Write focus to tracking file
    fn write_focus_to_file(&self, track_file: &str, focus_str: &str) {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(track_file)
        {
            let _ = writeln!(file, "iteration: focus={focus_str}");
        }
    }
}
