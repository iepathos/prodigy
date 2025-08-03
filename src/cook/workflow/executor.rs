//! Workflow executor with commit verification
//!
//! Executes workflow steps in sequence, verifies git commits when required,
//! and manages iteration logic for continuous improvement sessions.

use crate::cook::analysis::AnalysisCoordinator;
use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::metrics::MetricsCoordinator;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::{SessionManager, SessionUpdate};
use crate::session::{format_duration, TimingTracker};
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

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
    timing_tracker: TimingTracker,
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
            timing_tracker: TimingTracker::new(),
        }
    }

    /// Execute a workflow
    pub async fn execute(
        &mut self,
        workflow: &ExtendedWorkflowConfig,
        env: &ExecutionEnvironment,
    ) -> Result<()> {
        let workflow_start = Instant::now();

        self.user_interaction.display_info(&format!(
            "Executing workflow: {} (max {} iterations)",
            workflow.name, workflow.max_iterations
        ));

        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        let skip_validation =
            std::env::var("MMM_NO_COMMIT_VALIDATION").unwrap_or_default() == "true";

        if workflow.iterate {
            self.user_interaction
                .display_progress("Starting improvement loop");
        }

        let mut iteration = 0;
        let mut should_continue = true;

        // Start workflow timing in session
        self.session_manager
            .update_session(SessionUpdate::StartWorkflow)
            .await?;

        while should_continue && iteration < workflow.max_iterations {
            iteration += 1;
            self.user_interaction.display_progress(&format!(
                "Starting iteration {}/{}",
                iteration, workflow.max_iterations
            ));

            // Start iteration timing
            self.timing_tracker.start_iteration();

            // Update session
            self.session_manager
                .update_session(SessionUpdate::IncrementIteration)
                .await?;
            self.session_manager
                .update_session(SessionUpdate::StartIteration(iteration))
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

                // Start command timing
                self.timing_tracker.start_command(step.command.clone());
                let command_start = Instant::now();

                // Execute the step
                let step_result = self
                    .execute_step(step, env)
                    .await
                    .context(format!("Failed to execute step: {}", step.name))?;

                // Complete command timing
                let command_duration = command_start.elapsed();
                if let Some((cmd_name, _)) = self.timing_tracker.complete_command() {
                    self.session_manager
                        .update_session(SessionUpdate::RecordCommandTiming(
                            cmd_name,
                            command_duration,
                        ))
                        .await?;
                }

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

            // Complete iteration timing
            if let Some(iteration_duration) = self.timing_tracker.complete_iteration() {
                self.session_manager
                    .update_session(SessionUpdate::CompleteIteration)
                    .await?;

                // Display iteration timing
                self.user_interaction.display_info(&format!(
                    "✓ Iteration {} completed in {}",
                    iteration,
                    format_duration(iteration_duration)
                ));
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

        // Display total workflow timing
        let total_duration = workflow_start.elapsed();
        self.user_interaction.display_info(&format!(
            "\n📊 Total workflow time: {} across {} iteration{}",
            format_duration(total_duration),
            iteration,
            if iteration == 1 { "" } else { "s" }
        ));

        Ok(())
    }

    /// Execute a single workflow step
    async fn execute_step(
        &mut self,
        step: &WorkflowStep,
        env: &ExecutionEnvironment,
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

        Ok(true)
    }

    /// Get current git HEAD
    async fn get_current_head(&self, working_dir: &std::path::Path) -> Result<String> {
        // We need to run git commands in the correct working directory (especially for worktrees)
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(working_dir)
            .output()
            .await
            .context("Failed to execute git rev-parse HEAD")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get git HEAD: {}", stderr));
        }

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
            // Extract just the command name, ignoring arguments
            let command_name = command_name
                .split_whitespace()
                .next()
                .unwrap_or(command_name);
            return no_changes_cmds
                .split(',')
                .any(|cmd| cmd.trim() == command_name);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper function to test get_current_head directly without needing a full executor
    async fn test_get_current_head(working_dir: &std::path::Path) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(working_dir)
            .output()
            .await
            .context("Failed to execute git rev-parse HEAD")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get git HEAD: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[tokio::test]
    async fn test_get_current_head_in_regular_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to init git repo");

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to set git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to set git name");

        // Create initial commit
        std::fs::write(repo_path.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .expect("Failed to stage files");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(repo_path)
            .output()
            .expect("Failed to create commit");

        // Test get_current_head
        let head = test_get_current_head(repo_path).await.unwrap();
        assert!(!head.is_empty());
        assert_eq!(head.len(), 40); // SHA-1 hash is 40 characters
    }

    #[tokio::test]
    async fn test_get_current_head_in_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let main_repo = temp_dir.path().join("main");
        let worktree_path = temp_dir.path().join("worktree");

        // Create main repo
        std::fs::create_dir(&main_repo).unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to init git repo");

        // Configure git user
        std::process::Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to set git email");

        std::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to set git name");

        // Create initial commit in main repo
        std::fs::write(main_repo.join("test.txt"), "test content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to stage files");

        std::process::Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to create commit");

        // Create worktree
        std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                "test-branch",
            ])
            .current_dir(&main_repo)
            .output()
            .expect("Failed to create worktree");

        // Make a commit in the worktree
        std::fs::write(worktree_path.join("worktree.txt"), "worktree content").unwrap();
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to stage files in worktree");

        std::process::Command::new("git")
            .args(["commit", "-m", "Worktree commit"])
            .current_dir(&worktree_path)
            .output()
            .expect("Failed to create commit in worktree");

        // Test get_current_head in worktree
        let worktree_head = test_get_current_head(&worktree_path).await.unwrap();
        assert!(!worktree_head.is_empty());
        assert_eq!(worktree_head.len(), 40);

        // Get main repo head
        let main_head = test_get_current_head(&main_repo).await.unwrap();

        // Heads should be different
        assert_ne!(
            worktree_head, main_head,
            "Worktree HEAD should differ from main repo HEAD"
        );
    }

    #[tokio::test]
    async fn test_get_current_head_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let non_git_dir = temp_dir.path();

        // Test in non-git directory
        let result = test_get_current_head(non_git_dir).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to get git HEAD"));
    }

    #[tokio::test]
    async fn test_get_current_head_respects_working_directory() {
        // This test verifies that the git command runs in the correct directory
        let temp_dir = TempDir::new().unwrap();
        let repo1 = temp_dir.path().join("repo1");
        let repo2 = temp_dir.path().join("repo2");

        // Create two separate repos
        for (repo_path, commit_msg) in &[(&repo1, "Repo 1 commit"), (&repo2, "Repo 2 commit")] {
            std::fs::create_dir(repo_path).unwrap();
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to init git repo");

            std::process::Command::new("git")
                .args(["config", "user.email", "test@example.com"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to set git email");

            std::process::Command::new("git")
                .args(["config", "user.name", "Test User"])
                .current_dir(repo_path)
                .output()
                .expect("Failed to set git name");

            std::fs::write(
                repo_path.join("test.txt"),
                format!("content for {commit_msg}"),
            )
            .unwrap();
            std::process::Command::new("git")
                .args(["add", "."])
                .current_dir(repo_path)
                .output()
                .expect("Failed to stage files");

            std::process::Command::new("git")
                .args(["commit", "-m", commit_msg])
                .current_dir(repo_path)
                .output()
                .expect("Failed to create commit");
        }

        // Get heads from both repos
        let head1 = test_get_current_head(&repo1).await.unwrap();
        let head2 = test_get_current_head(&repo2).await.unwrap();

        // They should be different
        assert_ne!(
            head1, head2,
            "Different repos should have different HEAD commits"
        );
    }
}
