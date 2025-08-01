//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using the extracted components.

use crate::abstractions::git::GitOperations;
use crate::config::workflow::WorkflowConfig;
use crate::simple_state::StateManager;
use crate::worktree::WorktreeManager;
use anyhow::Result;
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
    /// Focus area
    pub focus: Option<String>,
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
                .create_session(config.command.focus.as_deref())
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
            focus: config.command.focus.clone(),
        })
    }

    async fn execute_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cook::analysis::runner::AnalysisRunnerImpl;
    use crate::cook::execution::claude::ClaudeExecutorImpl;
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::cook::interaction::mocks::MockUserInteraction;
    use crate::cook::metrics::collector::MetricsCollectorImpl;
    use crate::cook::session::tracker::SessionTrackerImpl;
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
                focus: None,
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
}
