//! Tests for WorkflowExecutor::handle_commit_verification function
//!
//! This module contains comprehensive test coverage for the handle_commit_verification
//! function which currently has 0% coverage. Tests are organized by phase following
//! the implementation plan in .prodigy/plan-item_6.md

#[cfg(test)]
mod tests {
    use crate::abstractions::git::MockGitOperations;
    use crate::cook::execution::ClaudeExecutor;
    use crate::cook::execution::ExecutionResult;
    use crate::cook::interaction::SpinnerHandle;
    use crate::cook::interaction::UserInteraction;
    use crate::cook::session::state::SessionState;
    use crate::cook::session::summary::SessionSummary;
    use crate::cook::session::SessionInfo;
    use crate::cook::session::{SessionManager, SessionUpdate};
    use crate::cook::workflow::executor::WorkflowExecutor;
    use crate::cook::workflow::{WorkflowContext, WorkflowStep};
    use crate::testing::config::TestConfiguration;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    // Mock implementations for testing

    pub(super) struct MockClaudeExecutor {
        responses: Arc<Mutex<Vec<ExecutionResult>>>,
        #[allow(clippy::type_complexity)]
        calls: Arc<Mutex<Vec<(String, PathBuf, HashMap<String, String>)>>>,
    }

    impl MockClaudeExecutor {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        fn add_response(&self, response: ExecutionResult) {
            self.responses.lock().unwrap().push(response);
        }

        #[allow(dead_code)]
        fn get_calls(&self) -> Vec<(String, PathBuf, HashMap<String, String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ClaudeExecutor for MockClaudeExecutor {
        async fn execute_claude_command(
            &self,
            command: &str,
            working_dir: &Path,
            env_vars: HashMap<String, String>,
        ) -> Result<ExecutionResult> {
            self.calls.lock().unwrap().push((
                command.to_string(),
                working_dir.to_path_buf(),
                env_vars.clone(),
            ));

            self.responses
                .lock()
                .unwrap()
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No mock response configured"))
        }

        async fn check_claude_cli(&self) -> Result<bool> {
            Ok(true)
        }

        async fn get_claude_version(&self) -> Result<String> {
            Ok("mock-version-1.0.0".to_string())
        }
    }

    pub(super) struct MockSessionManager {
        updates: Arc<Mutex<Vec<SessionUpdate>>>,
        iteration: Arc<Mutex<u32>>,
        session_id: Arc<Mutex<String>>,
    }

    impl MockSessionManager {
        fn new() -> Self {
            Self {
                updates: Arc::new(Mutex::new(Vec::new())),
                iteration: Arc::new(Mutex::new(0)),
                session_id: Arc::new(Mutex::new("test-session".to_string())),
            }
        }

        #[allow(dead_code)]
        fn get_updates(&self) -> Vec<SessionUpdate> {
            self.updates.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl SessionManager for MockSessionManager {
        async fn update_session(&self, update: SessionUpdate) -> Result<()> {
            self.updates.lock().unwrap().push(update.clone());

            if let SessionUpdate::IncrementIteration = update {
                *self.iteration.lock().unwrap() += 1;
            }

            Ok(())
        }

        async fn start_session(&self, session_id: &str) -> Result<()> {
            *self.session_id.lock().unwrap() = session_id.to_string();
            Ok(())
        }

        async fn complete_session(&self) -> Result<SessionSummary> {
            Ok(SessionSummary {
                iterations: 1,
                files_changed: 0,
            })
        }

        fn get_state(&self) -> Result<SessionState> {
            let session_id = self.session_id.lock().unwrap().clone();
            Ok(SessionState::new(session_id, PathBuf::from("/tmp")))
        }

        async fn save_state(&self, _path: &Path) -> Result<()> {
            Ok(())
        }

        async fn load_state(&self, _path: &Path) -> Result<()> {
            Ok(())
        }

        async fn load_session(&self, _session_id: &str) -> Result<SessionState> {
            Ok(SessionState::new(
                "test-session".to_string(),
                PathBuf::from("/tmp"),
            ))
        }

        async fn save_checkpoint(&self, _state: &SessionState) -> Result<()> {
            Ok(())
        }

        async fn list_resumable(&self) -> Result<Vec<SessionInfo>> {
            Ok(vec![])
        }

        async fn get_last_interrupted(&self) -> Result<Option<String>> {
            Ok(None)
        }
    }

    // Mock spinner handle
    struct MockSpinnerHandle;

    impl SpinnerHandle for MockSpinnerHandle {
        fn update_message(&mut self, _message: &str) {}
        fn success(&mut self, _message: &str) {}
        fn fail(&mut self, _message: &str) {}
    }

    pub(super) struct MockUserInteraction {
        messages: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl MockUserInteraction {
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<(String, String)> {
            self.messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl UserInteraction for MockUserInteraction {
        fn display_info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("info".to_string(), message.to_string()));
        }

        fn display_progress(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("progress".to_string(), message.to_string()));
        }

        fn display_success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("success".to_string(), message.to_string()));
        }

        fn display_error(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("error".to_string(), message.to_string()));
        }

        fn display_warning(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("warning".to_string(), message.to_string()));
        }

        fn display_action(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("action".to_string(), message.to_string()));
        }

        fn display_metric(&self, label: &str, value: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("metric".to_string(), format!("{}: {}", label, value)));
        }

        fn display_status(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("status".to_string(), message.to_string()));
        }

        async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
            Ok(true)
        }

        async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
            Ok("test".to_string())
        }

        fn start_spinner(&self, _message: &str) -> Box<dyn SpinnerHandle> {
            Box::new(MockSpinnerHandle)
        }

        fn iteration_start(&self, current: u32, total: u32) {
            self.messages.lock().unwrap().push((
                "iteration_start".to_string(),
                format!("{}/{}", current, total),
            ));
        }

        fn iteration_end(&self, current: u32, duration: std::time::Duration, success: bool) {
            self.messages.lock().unwrap().push((
                "iteration_end".to_string(),
                format!("{} {:?} {}", current, duration, success),
            ));
        }

        fn step_start(&self, step: u32, total: u32, description: &str) {
            self.messages.lock().unwrap().push((
                "step_start".to_string(),
                format!("{}/{} {}", step, total, description),
            ));
        }

        fn step_end(&self, step: u32, success: bool) {
            self.messages
                .lock()
                .unwrap()
                .push(("step_end".to_string(), format!("{} {}", step, success)));
        }

        fn command_output(
            &self,
            output: &str,
            _verbosity: crate::cook::interaction::VerbosityLevel,
        ) {
            self.messages
                .lock()
                .unwrap()
                .push(("command_output".to_string(), output.to_string()));
        }

        fn debug_output(
            &self,
            message: &str,
            _min_verbosity: crate::cook::interaction::VerbosityLevel,
        ) {
            self.messages
                .lock()
                .unwrap()
                .push(("debug".to_string(), message.to_string()));
        }

        fn verbosity(&self) -> crate::cook::interaction::VerbosityLevel {
            crate::cook::interaction::VerbosityLevel::Normal
        }
    }

    // Helper function to create a test executor with git mock that returns expected responses
    #[allow(clippy::type_complexity)]
    async fn create_test_executor_with_git_mock() -> (
        WorkflowExecutor,
        Arc<MockClaudeExecutor>,
        Arc<MockSessionManager>,
        Arc<MockUserInteraction>,
        Arc<MockGitOperations>,
    ) {
        let claude_executor = Arc::new(MockClaudeExecutor::new());
        let session_manager = Arc::new(MockSessionManager::new());
        let user_interaction = Arc::new(MockUserInteraction::new());
        let git_operations = Arc::new(MockGitOperations::new());

        // Disable test mode so Claude commands actually hit the mock
        let test_config = Arc::new(TestConfiguration {
            test_mode: false,
            ..Default::default()
        });

        let executor = WorkflowExecutor::with_test_config_and_git(
            claude_executor.clone() as Arc<dyn ClaudeExecutor>,
            session_manager.clone() as Arc<dyn SessionManager>,
            user_interaction.clone() as Arc<dyn UserInteraction>,
            test_config,
            git_operations.clone(),
        );

        (
            executor,
            claude_executor,
            session_manager,
            user_interaction,
            git_operations,
        )
    }

    // ==================== PHASE 1: FOUNDATION TESTS ====================

    #[tokio::test]
    async fn test_handle_commit_verification_with_commits_created() {
        let (mut executor, _, _, user_mock, git_mock) = create_test_executor_with_git_mock().await;

        // Scenario: Commits were created (HEAD changed)
        // Mock: get_current_head returns different value than head_before
        git_mock.add_success_response("def456ghi789").await; // HEAD after (different)

        // Mock: get_commits_between returns commit metadata
        git_mock
            .add_success_response(
                "abc123|Test commit|Author|2024-01-01T12:00:00Z\nsrc/file1.rs\nsrc/file2.rs",
            )
            .await;
        git_mock
            .add_success_response(" 2 files changed, 10 insertions(+), 3 deletions(-)")
            .await;

        let temp_dir = TempDir::new().unwrap();
        let step = WorkflowStep {
            claude: Some("/test-command".to_string()),
            ..Default::default()
        };
        let mut context = WorkflowContext::default();

        let result = executor
            .handle_commit_verification(
                temp_dir.path(),
                "abc123def456", // head_before
                &step,
                "test-step",
                &mut context,
            )
            .await;

        // Should return true (commits were created)
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Should display success message
        let messages = user_mock.get_messages();
        assert!(messages
            .iter()
            .any(|(t, m)| t == "success" && m.contains("test-step")));
    }

    #[tokio::test]
    async fn test_handle_commit_verification_auto_commit_with_changes() {
        let (mut executor, _, _, user_mock, git_mock) = create_test_executor_with_git_mock().await;

        // Scenario: No commits created, auto_commit enabled, has changes
        git_mock.add_success_response("abc123def456").await; // HEAD same as before
        git_mock.add_success_response("M  src/file1.rs\n").await; // check_for_changes - has changes
        git_mock.add_success_response("").await; // git add . (stage changes)
        git_mock.add_success_response("").await; // git commit (create commit)
        git_mock.add_success_response("abc123def456").await; // get_current_head after commit

        // Mock for get_commits_between (to get created commit details)
        git_mock
            .add_success_response("abc123|Auto commit|Author|2024-01-01T12:00:00Z\nsrc/file1.rs")
            .await;
        git_mock
            .add_success_response(" 1 file changed, 5 insertions(+)")
            .await;

        let temp_dir = TempDir::new().unwrap();
        let step = WorkflowStep {
            auto_commit: true,
            ..Default::default()
        };
        let mut context = WorkflowContext::default();

        let result = executor
            .handle_commit_verification(
                temp_dir.path(),
                "abc123def456",
                &step,
                "test-step",
                &mut context,
            )
            .await;

        // Should return true (auto-commit successful)
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Should display success message about auto-commit
        let messages = user_mock.get_messages();
        assert!(messages
            .iter()
            .any(|(t, m)| t == "success" && m.contains("auto-committed")));
    }

    #[tokio::test]
    async fn test_handle_commit_verification_auto_commit_no_changes() {
        let (mut executor, _, _, _, git_mock) = create_test_executor_with_git_mock().await;

        // Scenario: No commits created, auto_commit enabled, no changes
        git_mock.add_success_response("abc123def456").await; // HEAD same as before
        git_mock.add_success_response("").await; // check_for_changes - no changes

        let temp_dir = TempDir::new().unwrap();
        let step = WorkflowStep {
            auto_commit: true,
            commit_required: false,
            ..Default::default()
        };
        let mut context = WorkflowContext::default();

        let result = executor
            .handle_commit_verification(
                temp_dir.path(),
                "abc123def456",
                &step,
                "test-step",
                &mut context,
            )
            .await;

        // Should return false (no commits)
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_handle_commit_verification_commit_required_failure() {
        let (mut executor, _, _, _, git_mock) = create_test_executor_with_git_mock().await;

        // Scenario: No commits created, commit_required, no auto_commit
        git_mock.add_success_response("abc123def456").await; // HEAD same as before

        let temp_dir = TempDir::new().unwrap();
        let step = WorkflowStep {
            claude: Some("/test-command".to_string()),
            auto_commit: false,
            commit_required: true,
            ..Default::default()
        };
        let mut context = WorkflowContext::default();

        let result = executor
            .handle_commit_verification(
                temp_dir.path(),
                "abc123def456",
                &step,
                "test-step",
                &mut context,
            )
            .await;

        // Should return error (commit required but not created)
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        eprintln!("Error message: {}", err_msg);
        // The error message should mention commits not being created
        assert!(
            err_msg.contains("No commits")
                || err_msg.contains("commit")
                || err_msg.contains("required")
        );
    }
}
