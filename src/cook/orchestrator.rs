//! Cook orchestrator implementation
//!
//! Coordinates all cook operations using specialized coordinators.

use crate::config::WorkflowConfig;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use super::analysis::AnalysisCoordinator;
use super::command::CookCommand;
use super::coordinators::{
    EnvironmentCoordinator, ExecutionCoordinator, SessionCoordinator, WorkflowContext,
    WorkflowCoordinator,
};

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
    /// Environment coordinator
    environment_coordinator: Arc<dyn EnvironmentCoordinator>,
    /// Session coordinator
    session_coordinator: Arc<dyn SessionCoordinator>,
    /// Execution coordinator
    execution_coordinator: Arc<dyn ExecutionCoordinator>,
    /// Analysis coordinator
    analysis_coordinator: Arc<dyn AnalysisCoordinator>,
    /// Workflow coordinator
    workflow_coordinator: Arc<dyn WorkflowCoordinator>,
}

impl DefaultCookOrchestrator {
    /// Create a new orchestrator with coordinators
    pub fn new(
        environment_coordinator: Arc<dyn EnvironmentCoordinator>,
        session_coordinator: Arc<dyn SessionCoordinator>,
        execution_coordinator: Arc<dyn ExecutionCoordinator>,
        analysis_coordinator: Arc<dyn AnalysisCoordinator>,
        workflow_coordinator: Arc<dyn WorkflowCoordinator>,
    ) -> Self {
        Self {
            environment_coordinator,
            session_coordinator,
            execution_coordinator,
            analysis_coordinator,
            workflow_coordinator,
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

        // Setup environment using coordinator
        let env_setup = self
            .environment_coordinator
            .prepare_environment(&config.command, &config.project_path)
            .await?;

        // Convert to ExecutionEnvironment
        let env = ExecutionEnvironment {
            working_dir: env_setup.working_dir,
            project_dir: env_setup.project_dir,
            worktree_name: env_setup.worktree_name,
            session_id: self.generate_session_id(),
        };

        // Start session
        self.session_coordinator
            .start_session(&env.session_id)
            .await?;

        // Execute workflow
        let result = self.execute_workflow(&env, &config).await;

        // Handle result
        match result {
            Ok(_) => {
                self.session_coordinator.complete_session(true).await?;
                self.workflow_coordinator
                    .display_progress("Cook session completed successfully!");
            }
            Err(e) => {
                self.session_coordinator.complete_session(false).await?;
                self.workflow_coordinator
                    .display_progress(&format!("Cook session failed: {e}"));
                return Err(e);
            }
        }

        // Cleanup
        self.cleanup(&env, &config).await?;

        // Get session info
        let session_info = self.session_coordinator.get_session_info().await?;
        self.workflow_coordinator
            .display_progress(&format!("Session {} complete", session_info.session_id));

        Ok(())
    }

    async fn check_prerequisites(&self) -> Result<()> {
        // Skip checks in test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            return Ok(());
        }

        // Check Claude CLI using execution coordinator
        if !self
            .execution_coordinator
            .check_command_available("claude")
            .await?
        {
            anyhow::bail!("Claude CLI is not available. Please install it first.");
        }

        Ok(())
    }

    async fn setup_environment(&self, config: &CookConfig) -> Result<ExecutionEnvironment> {
        // Delegate to environment coordinator
        let env_setup = self
            .environment_coordinator
            .prepare_environment(&config.command, &config.project_path)
            .await?;

        Ok(ExecutionEnvironment {
            working_dir: env_setup.working_dir,
            project_dir: env_setup.project_dir,
            worktree_name: env_setup.worktree_name,
            session_id: self.generate_session_id(),
        })
    }

    async fn execute_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        // Run initial analysis if needed
        if !config.command.skip_analysis {
            self.workflow_coordinator
                .display_progress("Running initial analysis...");
            let analysis = self
                .analysis_coordinator
                .analyze_project(&env.working_dir)
                .await?;
            self.analysis_coordinator
                .save_analysis(&env.working_dir, &analysis)
                .await?;
        } else {
            self.workflow_coordinator
                .display_progress("Skipping project analysis (--skip-analysis flag)");
        }

        // Create workflow context
        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: config.command.max_iterations as usize,
            variables: std::collections::HashMap::new(),
        };

        // Add args and map variables if present
        if !config.command.args.is_empty() {
            for (i, arg) in config.command.args.iter().enumerate() {
                context.variables.insert(format!("ARG{i}"), arg.clone());
            }
            context
                .variables
                .insert("ARG".to_string(), config.command.args[0].clone());
        }

        // Execute the workflow
        self.workflow_coordinator
            .execute_workflow(&config.workflow.commands, &mut context)
            .await?;

        Ok(())
    }

    async fn cleanup(&self, env: &ExecutionEnvironment, config: &CookConfig) -> Result<()> {
        // Clean up worktree if needed
        if let Some(ref _worktree_name) = env.worktree_name {
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
                self.workflow_coordinator
                    .prompt_user("Would you like to merge the worktree changes?", true)
                    .await?
            };

            if should_merge {
                // TODO: Add merge support to environment coordinator
                self.workflow_coordinator
                    .display_progress("Merging worktree changes...");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abstractions::git::GitOperations;
    use tempfile::TempDir;

    // Mock coordinators for testing
    use crate::config::ConfigLoader;
    use crate::cook::coordinators::{
        DefaultEnvironmentCoordinator, DefaultExecutionCoordinator, DefaultSessionCoordinator,
        DefaultWorkflowCoordinator,
    };
    use crate::simple_state::StateManager;
    use crate::subprocess::SubprocessManager;
    use crate::worktree::WorktreeManager;

    // Custom mock git operations for testing
    struct TestMockGitOperations;

    #[async_trait]
    impl GitOperations for TestMockGitOperations {
        async fn git_command(
            &self,
            _args: &[&str],
            _description: &str,
        ) -> Result<std::process::Output> {
            use std::os::unix::process::ExitStatusExt;
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }

        async fn is_git_repo(&self) -> bool {
            true
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

        async fn create_worktree(&self, _name: &str, _path: &std::path::Path) -> Result<()> {
            Ok(())
        }

        async fn get_current_branch(&self) -> Result<String> {
            Ok("main".to_string())
        }

        async fn switch_branch(&self, _branch: &str) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_orchestrator_with_coordinators() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path().to_path_buf();

        // Create dependencies
        let config_loader = Arc::new(ConfigLoader::new().await.unwrap());
        let subprocess = SubprocessManager::production();
        let worktree_manager =
            Arc::new(WorktreeManager::new(project_path.clone(), subprocess.clone()).unwrap());
        let git_operations = Arc::new(TestMockGitOperations);

        // Create coordinators
        let env_coordinator = Arc::new(DefaultEnvironmentCoordinator::new(
            config_loader,
            worktree_manager,
            git_operations,
        ));

        let session_manager = Arc::new(crate::cook::session::tracker::SessionTrackerImpl::new(
            "test".to_string(),
            project_path.clone(),
        ));
        let state_manager = Arc::new(StateManager::new().unwrap());
        let session_coordinator = Arc::new(DefaultSessionCoordinator::new(
            session_manager.clone(),
            state_manager,
        ));

        let command_executor = Arc::new(crate::cook::execution::runner::RealCommandRunner::new());
        let claude_executor = Arc::new(crate::cook::execution::claude::ClaudeExecutorImpl::new(
            crate::cook::execution::runner::RealCommandRunner::new(),
        ));
        let subprocess_mgr = Arc::new(subprocess);
        let execution_coordinator = Arc::new(DefaultExecutionCoordinator::new(
            command_executor,
            claude_executor.clone(),
            subprocess_mgr,
        ));

        let analysis_coordinator =
            Arc::new(crate::cook::analysis::runner::AnalysisRunnerImpl::new(
                crate::cook::execution::runner::RealCommandRunner::new(),
            ));

        let workflow_executor = Arc::new(crate::cook::workflow::WorkflowExecutor::new(
            claude_executor,
            session_manager,
            analysis_coordinator.clone(),
            Arc::new(crate::cook::metrics::collector::MetricsCollectorImpl::new(
                crate::cook::execution::runner::RealCommandRunner::new(),
            )),
            Arc::new(crate::cook::interaction::DefaultUserInteraction::new()),
        ));

        let workflow_coordinator = Arc::new(DefaultWorkflowCoordinator::new(
            workflow_executor,
            Arc::new(crate::cook::interaction::DefaultUserInteraction::new()),
        ));

        // Create orchestrator
        let orchestrator = DefaultCookOrchestrator::new(
            env_coordinator,
            session_coordinator,
            execution_coordinator,
            analysis_coordinator,
            workflow_coordinator,
        );

        // Test basic setup
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
            project_path: project_path.clone(),
            workflow: WorkflowConfig { commands: vec![] },
        };

        let env = orchestrator.setup_environment(&config).await.unwrap();
        assert_eq!(env.project_dir, project_path);
        assert_eq!(env.working_dir, project_path);
        assert!(env.worktree_name.is_none());
        assert!(env.session_id.starts_with("cook-"));
    }
}
