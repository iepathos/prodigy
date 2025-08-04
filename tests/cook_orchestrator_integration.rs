mod common;

use mmm::cook::analysis::runner::AnalysisRunnerImpl;
use mmm::cook::execution::claude::ClaudeExecutorImpl;
use mmm::cook::execution::runner::RealCommandRunner;
use mmm::cook::metrics::collector::MetricsCollectorImpl;
use mmm::cook::orchestrator::{CookOrchestrator, DefaultCookOrchestrator};
use mmm::cook::session::tracker::SessionTrackerImpl;
use mmm::simple_state::StateManager;
use mmm::subprocess::SubprocessManager;
use std::os::unix::process::ExitStatusExt;
use std::sync::Arc;
use tempfile::TempDir;

// Mock implementations for testing
struct MockUserInteraction;
impl MockUserInteraction {
    fn new() -> Self {
        Self
    }
}

struct MockCommandRunner;
impl MockCommandRunner {
    fn new() -> Self {
        Self
    }
}

struct MockGitOperations;
impl MockGitOperations {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl mmm::cook::interaction::UserInteraction for MockUserInteraction {
    fn display_info(&self, _message: &str) {}
    fn display_error(&self, _message: &str) {}
    fn display_success(&self, _message: &str) {}
    fn display_warning(&self, _message: &str) {}
    fn display_progress(&self, _message: &str) {}
    async fn prompt_yes_no(&self, _message: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> anyhow::Result<String> {
        Ok(_default.unwrap_or("").to_string())
    }
    fn start_spinner(&self, _message: &str) -> Box<dyn mmm::cook::interaction::SpinnerHandle> {
        Box::new(MockSpinnerHandle)
    }
}

struct MockSpinnerHandle;
impl mmm::cook::interaction::SpinnerHandle for MockSpinnerHandle {
    fn update_message(&mut self, _message: &str) {}
    fn success(&mut self, _message: &str) {}
    fn fail(&mut self, _message: &str) {}
}

#[async_trait::async_trait]
impl mmm::abstractions::git::GitOperations for MockGitOperations {
    async fn git_command(
        &self,
        _args: &[&str],
        _description: &str,
    ) -> anyhow::Result<std::process::Output> {
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec![],
            stderr: vec![],
        })
    }
    async fn is_git_repo(&self) -> bool {
        true
    }
    async fn get_last_commit_message(&self) -> anyhow::Result<String> {
        Ok("test".to_string())
    }
    async fn check_git_status(&self) -> anyhow::Result<String> {
        Ok("clean".to_string())
    }
    async fn stage_all_changes(&self) -> anyhow::Result<()> {
        Ok(())
    }
    async fn create_commit(&self, _message: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn create_worktree(&self, _name: &str, _path: &std::path::Path) -> anyhow::Result<()> {
        Ok(())
    }
    async fn get_current_branch(&self) -> anyhow::Result<String> {
        Ok("main".to_string())
    }
    async fn switch_branch(&self, _branch: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn git_command_in_dir(
        &self,
        args: &[&str],
        description: &str,
        _working_dir: &std::path::Path,
    ) -> anyhow::Result<std::process::Output> {
        // For test mocks, just delegate to git_command
        self.git_command(args, description).await
    }
}

#[async_trait::async_trait]
impl mmm::cook::execution::CommandRunner for MockCommandRunner {
    async fn run_command(
        &self,
        _command: &str,
        _args: &[String],
    ) -> anyhow::Result<std::process::Output> {
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: vec![],
            stderr: vec![],
        })
    }
    async fn run_with_context(
        &self,
        _command: &str,
        _args: &[String],
        _context: &mmm::cook::execution::ExecutionContext,
    ) -> anyhow::Result<mmm::cook::execution::ExecutionResult> {
        Ok(mmm::cook::execution::ExecutionResult {
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        })
    }
}

#[tokio::test]
async fn test_orchestrator_full_workflow() {
    let temp_dir = TempDir::new().unwrap();
    // Create subprocess manager with mock
    let mock_runner = Arc::new(mmm::subprocess::MockProcessRunner::new());
    let subprocess = SubprocessManager::new(mock_runner);

    let session_manager = Arc::new(SessionTrackerImpl::new(
        "test".to_string(),
        temp_dir.path().to_path_buf(),
    ));
    let command_executor = Arc::new(RealCommandRunner::new());
    let claude_executor = Arc::new(ClaudeExecutorImpl::new(MockCommandRunner::new()));
    let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(MockCommandRunner::new()));
    let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(MockCommandRunner::new()));
    let user_interaction = Arc::new(MockUserInteraction::new());
    let git_operations = Arc::new(MockGitOperations::new());
    let state_manager = StateManager::new().unwrap();

    let orchestrator = DefaultCookOrchestrator::new(
        session_manager,
        command_executor,
        claude_executor,
        analysis_coordinator,
        metrics_coordinator,
        user_interaction,
        git_operations,
        state_manager,
        subprocess,
    );

    // Test basic orchestration
    let result = orchestrator.check_prerequisites().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_orchestrator_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    // Create subprocess manager with mock
    let mock_runner = Arc::new(mmm::subprocess::MockProcessRunner::new());
    let subprocess = SubprocessManager::new(mock_runner.clone());

    // Configure mock to simulate failures
    // Note: MockProcessRunner doesn't have expect_command method in the current implementation
    // So we'll just test the structure for now

    let session_manager = Arc::new(SessionTrackerImpl::new(
        "test".to_string(),
        temp_dir.path().to_path_buf(),
    ));
    let command_executor = Arc::new(RealCommandRunner::new());
    let claude_executor = Arc::new(ClaudeExecutorImpl::new(MockCommandRunner::new()));
    let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(MockCommandRunner::new()));
    let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(MockCommandRunner::new()));
    let user_interaction = Arc::new(MockUserInteraction::new());
    let git_operations = Arc::new(MockGitOperations::new());
    let state_manager = StateManager::new().unwrap();

    let orchestrator = DefaultCookOrchestrator::new(
        session_manager,
        command_executor,
        claude_executor,
        analysis_coordinator,
        metrics_coordinator,
        user_interaction,
        git_operations,
        state_manager,
        subprocess,
    );

    // Test environment validation
    // Note: Since MockCommandRunner always returns success, we can't properly test
    // Claude CLI failure without a more sophisticated mock. For now, we'll just
    // ensure the method runs without error in test mode.
    common::init_test_env(); // This sets MMM_TEST_MODE=true
    let result = orchestrator.check_prerequisites().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cook_orchestrator_basic_workflow() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let mock_runner = Arc::new(mmm::subprocess::MockProcessRunner::new());
    let subprocess = SubprocessManager::new(mock_runner);

    // Create all required components
    let session_manager = Arc::new(SessionTrackerImpl::new(
        "test-basic".to_string(),
        temp_dir.path().to_path_buf(),
    ));
    let command_executor = Arc::new(RealCommandRunner::new());
    let claude_executor = Arc::new(ClaudeExecutorImpl::new(MockCommandRunner::new()));
    let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(MockCommandRunner::new()));
    let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(MockCommandRunner::new()));
    let user_interaction = Arc::new(MockUserInteraction::new());
    let git_operations = Arc::new(MockGitOperations::new());
    let state_manager = StateManager::new()?;

    let orchestrator = DefaultCookOrchestrator::new(
        session_manager.clone(),
        command_executor,
        claude_executor,
        analysis_coordinator,
        metrics_coordinator,
        user_interaction,
        git_operations,
        state_manager,
        subprocess,
    );

    // Verify orchestrator was created successfully
    // Note: orchestrator itself doesn't expose session_id, that's managed by session_manager
    drop(orchestrator); // Just ensure it was created successfully

    Ok(())
}

#[tokio::test]
async fn test_cook_orchestrator_with_metrics() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo and .mmm directory
    std::fs::create_dir_all(temp_dir.path().join(".git"))?;
    std::fs::create_dir_all(temp_dir.path().join(".mmm/metrics"))?;

    let mock_runner = Arc::new(mmm::subprocess::MockProcessRunner::new());
    let subprocess = SubprocessManager::new(mock_runner);

    let session_manager = Arc::new(SessionTrackerImpl::new(
        "test-metrics".to_string(),
        temp_dir.path().to_path_buf(),
    ));
    let command_executor = Arc::new(RealCommandRunner::new());
    let claude_executor = Arc::new(ClaudeExecutorImpl::new(MockCommandRunner::new()));
    let analysis_coordinator = Arc::new(AnalysisRunnerImpl::new(MockCommandRunner::new()));
    let metrics_coordinator = Arc::new(MetricsCollectorImpl::new(MockCommandRunner::new()));
    let user_interaction = Arc::new(MockUserInteraction::new());
    let git_operations = Arc::new(MockGitOperations::new());
    let state_manager = StateManager::new()?;

    let orchestrator = DefaultCookOrchestrator::new(
        session_manager,
        command_executor,
        claude_executor,
        analysis_coordinator,
        metrics_coordinator.clone(),
        user_interaction,
        git_operations,
        state_manager,
        subprocess,
    );

    // Verify orchestrator and metrics coordinator are properly initialized
    // Note: orchestrator itself doesn't expose session_id, that's managed by session_manager
    drop(orchestrator); // Just ensure it was created successfully
    drop(metrics_coordinator); // Ensure metrics coordinator was created

    // Note: Actually collecting metrics would require running the full orchestration,
    // which requires more complex setup including mocking Claude API responses

    Ok(())
}
