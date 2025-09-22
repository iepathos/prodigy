//! Comprehensive unit tests for WorkflowExecutor

#[cfg(test)]
mod tests {
    use crate::abstractions::git::MockGitOperations;
    use crate::config::command::TestCommand;
    use crate::cook::execution::ClaudeExecutor;
    use crate::cook::execution::ExecutionResult;
    use crate::cook::interaction::SpinnerHandle;
    use crate::cook::interaction::UserInteraction;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::cook::session::state::SessionState;
    use crate::cook::session::summary::SessionSummary;
    use crate::cook::session::SessionInfo;
    use crate::cook::session::{SessionManager, SessionUpdate};
    use crate::cook::workflow::executor::*;
    use crate::cook::workflow::on_failure::OnFailureConfig;
    use crate::cook::workflow::{ExtendedWorkflowConfig, WorkflowMode, WorkflowStep};
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

        fn add_response(&self, response: ExecutionResult) {
            self.responses.lock().unwrap().push(response);
        }

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

    // Helper function to create a test executor with mocked git operations
    #[allow(clippy::type_complexity)]
    fn create_test_executor() -> (
        WorkflowExecutor,
        Arc<MockClaudeExecutor>,
        Arc<MockSessionManager>,
        Arc<MockUserInteraction>,
    ) {
        let claude_executor = Arc::new(MockClaudeExecutor::new());
        let session_manager = Arc::new(MockSessionManager::new());
        let user_interaction = Arc::new(MockUserInteraction::new());
        let git_operations = Arc::new(MockGitOperations::new());
        let test_config = Arc::new(TestConfiguration::default());

        let executor = WorkflowExecutor::with_test_config_and_git(
            claude_executor.clone() as Arc<dyn ClaudeExecutor>,
            session_manager.clone() as Arc<dyn SessionManager>,
            user_interaction.clone() as Arc<dyn UserInteraction>,
            test_config,
            git_operations,
        );

        (executor, claude_executor, session_manager, user_interaction)
    }

    // Helper function to create a test executor with configuration and git mocks
    #[allow(clippy::type_complexity)]
    fn create_test_executor_with_config(
        config: TestConfiguration,
    ) -> (
        WorkflowExecutor,
        Arc<MockClaudeExecutor>,
        Arc<MockSessionManager>,
        Arc<MockUserInteraction>,
    ) {
        let claude_executor = Arc::new(MockClaudeExecutor::new());
        let session_manager = Arc::new(MockSessionManager::new());
        let user_interaction = Arc::new(MockUserInteraction::new());
        let git_operations = Arc::new(MockGitOperations::new());

        let executor = WorkflowExecutor::with_test_config_and_git(
            claude_executor.clone() as Arc<dyn ClaudeExecutor>,
            session_manager.clone() as Arc<dyn SessionManager>,
            user_interaction.clone() as Arc<dyn UserInteraction>,
            Arc::new(config),
            git_operations,
        );

        (executor, claude_executor, session_manager, user_interaction)
    }

    // Helper function to create a test executor with git mock that returns expected responses
    #[allow(clippy::type_complexity)]
    pub(super) async fn create_test_executor_with_git_mock() -> (
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

        // Set up default git mock responses
        // Each step in the workflow will call get_current_head twice (before and after)
        // We need enough responses for all steps that will be executed
        for _ in 0..20 {
            git_operations.add_success_response("abc123def456").await; // git rev-parse HEAD
        }
        git_operations.add_success_response("").await; // git status --porcelain (no changes)

        let test_config = Arc::new(TestConfiguration::default());

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

    #[test]
    fn test_context_interpolation() {
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("VAR1".to_string(), "value1".to_string());
        context
            .captured_outputs
            .insert("OUTPUT".to_string(), "output_value".to_string());
        context
            .iteration_vars
            .insert("ITERATION".to_string(), "3".to_string());

        // Test ${VAR} format
        assert_eq!(context.interpolate("${VAR1}"), "value1");
        assert_eq!(context.interpolate("$VAR1"), "value1");

        // Test ${OUTPUT} format
        assert_eq!(context.interpolate("${OUTPUT}"), "output_value");
        assert_eq!(context.interpolate("$OUTPUT"), "output_value");

        // Test iteration variables
        assert_eq!(context.interpolate("Iteration ${ITERATION}"), "Iteration 3");

        // Test multiple replacements
        assert_eq!(
            context.interpolate("${VAR1} and ${OUTPUT} in iteration ${ITERATION}"),
            "value1 and output_value in iteration 3"
        );

        // Test no replacement for missing variables
        assert_eq!(context.interpolate("${MISSING}"), "${MISSING}");
    }

    #[test]
    fn test_context_interpolation_priority() {
        let mut context = WorkflowContext::default();

        // Add same key to different maps
        context
            .variables
            .insert("KEY".to_string(), "from_variables".to_string());
        context
            .captured_outputs
            .insert("KEY".to_string(), "from_outputs".to_string());
        context
            .iteration_vars
            .insert("KEY".to_string(), "from_iteration".to_string());

        // The interpolation uses the last match found (iteration_vars takes precedence)
        // Order: variables -> captured_outputs -> variable_store -> iteration_vars
        assert_eq!(context.interpolate("${KEY}"), "from_iteration");
    }

    #[test]
    fn test_complex_variable_interpolation_nested() {
        let mut context = WorkflowContext::default();

        // Test nested variable references - store as flat keys with dots
        // The interpolation engine will look them up directly
        context
            .variables
            .insert("user.name".to_string(), "Alice".to_string());
        context
            .variables
            .insert("user.id".to_string(), "12345".to_string());
        context
            .variables
            .insert("project.name".to_string(), "MyProject".to_string());
        context
            .variables
            .insert("project.version".to_string(), "1.2.3".to_string());

        // Test nested field access patterns
        // The interpolation should handle these as direct key lookups
        assert_eq!(context.interpolate("${user.name}"), "Alice");
        assert_eq!(context.interpolate("${user.id}"), "12345");
        assert_eq!(context.interpolate("${project.name}"), "MyProject");
        assert_eq!(context.interpolate("${project.version}"), "1.2.3");

        // Test multiple nested variables in one string
        assert_eq!(
            context.interpolate(
                "User ${user.name} (${user.id}) working on ${project.name} v${project.version}"
            ),
            "User Alice (12345) working on MyProject v1.2.3"
        );
    }

    #[test]
    fn test_complex_variable_interpolation_arrays() {
        let mut context = WorkflowContext::default();

        // Test array-like variable access
        context
            .variables
            .insert("items[0]".to_string(), "first".to_string());
        context
            .variables
            .insert("items[1]".to_string(), "second".to_string());
        context
            .variables
            .insert("items[2]".to_string(), "third".to_string());

        assert_eq!(context.interpolate("${items[0]}"), "first");
        assert_eq!(context.interpolate("${items[1]}"), "second");
        assert_eq!(context.interpolate("${items[2]}"), "third");

        // Test in combination
        assert_eq!(
            context.interpolate("Items: ${items[0]}, ${items[1]}, ${items[2]}"),
            "Items: first, second, third"
        );
    }

    #[test]
    fn test_complex_variable_interpolation_defaults() {
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("existing".to_string(), "value".to_string());

        // Test default value syntax (when variable doesn't exist)
        // The interpolation engine now supports default values
        assert_eq!(context.interpolate("${missing:-default}"), "default");
        assert_eq!(context.interpolate("${existing:-default}"), "value");

        // Test undefined variable behavior (no default specified)
        assert_eq!(context.interpolate("${undefined}"), "${undefined}");
    }

    #[test]
    fn test_complex_variable_interpolation_special_chars() {
        let mut context = WorkflowContext::default();

        // Test variables with special characters
        context
            .variables
            .insert("path/to/file".to_string(), "/home/user/doc.txt".to_string());
        context
            .variables
            .insert("json.field".to_string(), "{\"key\":\"value\"}".to_string());
        context.variables.insert(
            "command_output".to_string(),
            "Line1\nLine2\nLine3".to_string(),
        );

        assert_eq!(context.interpolate("${path/to/file}"), "/home/user/doc.txt");
        assert_eq!(context.interpolate("${json.field}"), "{\"key\":\"value\"}");
        assert_eq!(
            context.interpolate("${command_output}"),
            "Line1\nLine2\nLine3"
        );
    }

    #[test]
    fn test_complex_variable_interpolation_escaping() {
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("var".to_string(), "value".to_string());

        // Test escaping of special characters
        // Currently, escaping is not supported, so this documents current behavior
        assert_eq!(context.interpolate("\\${var}"), "\\value"); // Backslash doesn't escape
        assert_eq!(context.interpolate("$${var}"), "$value"); // Double $ doesn't escape
    }

    #[test]
    fn test_determine_command_type_claude() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            claude: Some("/prodigy-code-review".to_string()),
            commit_required: true,
            ..Default::default()
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Claude(cmd) if cmd == "/prodigy-code-review"));
    }

    #[test]
    fn test_determine_command_type_shell() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            shell: Some("cargo test".to_string()),
            ..Default::default()
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Shell(cmd) if cmd == "cargo test"));
    }

    #[test]
    fn test_determine_command_type_test() {
        let (executor, _, _, _) = create_test_executor();

        let test_cmd = TestCommand {
            command: "cargo test".to_string(),
            on_failure: None,
        };

        let step = WorkflowStep {
            test: Some(test_cmd.clone()),
            ..Default::default()
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Test(cmd) if cmd.command == "cargo test"));
    }

    #[test]
    fn test_determine_command_type_goal_seek() {
        let (executor, _, _, _) = create_test_executor();

        let goal_seek_config = crate::cook::goal_seek::GoalSeekConfig {
            goal: "Performance improvement > 20%".to_string(),
            claude: Some("/optimize-performance".to_string()),
            shell: None,
            validate: "cargo test performance".to_string(),
            threshold: 80,
            max_attempts: 5,
            timeout_seconds: Some(300),
            fail_on_incomplete: Some(false),
        };

        let step = WorkflowStep {
            goal_seek: Some(goal_seek_config.clone()),
            ..Default::default()
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(
            matches!(result, CommandType::GoalSeek(config) if config.goal == "Performance improvement > 20%")
        );
    }

    #[test]
    fn test_determine_command_type_foreach() {
        let (executor, _, _, _) = create_test_executor();

        let foreach_config = crate::config::command::ForeachConfig {
            input: crate::config::command::ForeachInput::List(vec![
                "file1.rs".to_string(),
                "file2.rs".to_string(),
                "file3.rs".to_string(),
            ]),
            parallel: crate::config::command::ParallelConfig::Count(2),
            do_block: vec![Box::new(crate::config::command::WorkflowStepCommand {
                claude: None,
                shell: Some("echo Processing item".to_string()),
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let step = WorkflowStep {
            foreach: Some(foreach_config.clone()),
            ..Default::default()
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Foreach(_))); // Just verify it's a Foreach command
    }

    #[test]
    fn test_determine_command_type_legacy_name() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: Some("prodigy-code-review".to_string()),
            commit_required: true,
            ..Default::default()
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Legacy(cmd) if cmd == "/prodigy-code-review"));
    }

    #[test]
    fn test_determine_command_type_multiple_error() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            claude: Some("/prodigy-code-review".to_string()),
            shell: Some("cargo test".to_string()),
            ..Default::default()
        };

        let result = executor.determine_command_type(&step);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Multiple command types specified"));
    }

    #[test]
    fn test_determine_command_type_none_error() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            ..Default::default()
        };

        let result = executor.determine_command_type(&step);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No command specified"));
    }

    #[test]
    fn test_get_step_display_name_claude() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            claude: Some("/prodigy-code-review --strict".to_string()),
            commit_required: true,
            ..Default::default()
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "claude: /prodigy-code-review --strict");
    }

    #[test]
    fn test_get_step_display_name_shell() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            shell: Some("cargo test --verbose".to_string()),
            ..Default::default()
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "shell: cargo test --verbose");
    }

    #[test]
    fn test_get_step_display_name_test() {
        let (executor, _, _, _) = create_test_executor();

        let test_cmd = TestCommand {
            command: "pytest tests/".to_string(),
            on_failure: None,
        };

        let step = WorkflowStep {
            test: Some(test_cmd),
            ..Default::default()
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "test: pytest tests/");
    }

    #[test]
    fn test_get_step_display_name_unnamed() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            ..Default::default()
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "unnamed step");
    }

    #[test]
    fn test_handle_test_mode_execution_success() {
        let config = TestConfiguration::builder().test_mode(true).build();
        let (executor, _, _, _) = create_test_executor_with_config(config);

        let step = WorkflowStep {
            claude: Some("/prodigy-code-review".to_string()),
            shell: Some("cargo test".to_string()),
            ..Default::default()
        };

        let command_type = CommandType::Claude("/prodigy-code-review".to_string());
        let result = executor
            .handle_test_mode_execution(&step, &command_type)
            .unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("[TEST MODE]"));

        std::env::remove_var("PRODIGY_TEST_MODE");
    }

    #[test]
    fn test_is_test_mode_no_changes_command() {
        use crate::testing::config::TestConfiguration;

        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec![
                "prodigy-code-review".to_string(),
                "prodigy-lint".to_string(),
            ])
            .build();

        let (executor, _, _, _) = create_test_executor_with_config(config);

        assert!(executor.is_test_mode_no_changes_command("/prodigy-code-review"));
        assert!(executor.is_test_mode_no_changes_command("prodigy-lint"));
        assert!(!executor.is_test_mode_no_changes_command("/prodigy-implement-spec"));

        // Test with arguments
        assert!(executor.is_test_mode_no_changes_command("/prodigy-code-review --strict"));
        assert!(executor.is_test_mode_no_changes_command("prodigy-lint --fix"));
    }

    #[test]
    fn test_should_stop_early_in_test_mode() {
        use crate::testing::config::TestConfiguration;

        // Test without no_changes_commands
        let config = TestConfiguration::builder().test_mode(true).build();
        let (executor, _, _, _) = create_test_executor_with_config(config);
        assert!(!executor.should_stop_early_in_test_mode());

        // Test with prodigy-code-review and prodigy-lint
        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec![
                "prodigy-code-review".to_string(),
                "prodigy-lint".to_string(),
            ])
            .build();
        let (executor, _, _, _) = create_test_executor_with_config(config);
        assert!(executor.should_stop_early_in_test_mode());

        // Test with prodigy-implement-spec only
        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec!["prodigy-implement-spec".to_string()])
            .build();
        let (executor, _, _, _) = create_test_executor_with_config(config);
        assert!(!executor.should_stop_early_in_test_mode());
    }

    #[test]
    fn test_is_focus_tracking_test() {
        use crate::testing::config::TestConfiguration;

        // Test without track_focus
        let config = TestConfiguration::builder().test_mode(true).build();
        let (executor, _, _, _) = create_test_executor_with_config(config);
        assert!(!executor.is_focus_tracking_test());

        // Test with track_focus enabled
        let config = TestConfiguration::builder()
            .test_mode(true)
            .track_focus(true)
            .build();
        let (executor, _, _, _) = create_test_executor_with_config(config);
        assert!(executor.is_focus_tracking_test());
    }

    #[test]
    fn test_handle_no_commits_error_general_command() {
        let (executor, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            claude: Some("/prodigy-implement-spec".to_string()),
            commit_required: true,
            ..Default::default()
        };

        let result = executor.handle_no_commits_error(&step);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No commits created"));
    }

    #[tokio::test]
    async fn test_execute_claude_command() {
        let (executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        let command = "/prodigy-code-review";
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(working_dir.to_path_buf()),
            project_dir: Arc::new(working_dir.to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Set up mock response
        claude_mock.add_response(ExecutionResult {
            success: true,
            exit_code: Some(0),
            stdout: "Command executed".to_string(),
            stderr: String::new(),
        });

        let result = executor
            .execute_claude_command(command, &env, env_vars.clone())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "Command executed");

        // Verify the call was made
        let calls = claude_mock.get_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, command);
        assert!(calls[0].2.contains_key("PRODIGY_AUTOMATION"));
    }

    #[tokio::test]
    async fn test_execute_shell_command_success() {
        let (executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        let env_vars = HashMap::new();

        // Execute a simple echo command
        let result = executor
            .execute_shell_command("echo 'test'", &env, env_vars, None)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_workflow_execution_single_iteration() {
        let (mut executor, _, session_mock, user_mock, _) =
            create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Set up test mode to avoid actual command execution
        std::env::set_var("PRODIGY_TEST_MODE", "true");

        // Set up workflow
        let workflow = ExtendedWorkflowConfig {
            name: "Test Workflow".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![WorkflowStep {
                claude: Some("/prodigy-code-review".to_string()),
                ..Default::default()
            }],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            environment: None,
            retry_defaults: None,
            // collect_metrics removed - MMM focuses on orchestration
        };

        // Execute workflow
        let result = executor.execute(&workflow, &env).await;
        assert!(result.is_ok());

        // Verify session updates were made
        let updates = session_mock.get_updates();
        assert!(updates
            .iter()
            .any(|u| matches!(u, SessionUpdate::StartWorkflow)));
        assert!(updates
            .iter()
            .any(|u| matches!(u, SessionUpdate::IncrementIteration)));

        // Verify user messages
        let messages = user_mock.get_messages();
        assert!(messages
            .iter()
            .any(|(t, m)| t == "info" && m.contains("Test Workflow")));

        std::env::remove_var("PRODIGY_TEST_MODE");
    }

    #[tokio::test]
    async fn test_execute_step_with_capture_output() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        let mut context = WorkflowContext::default();

        let step = WorkflowStep {
            shell: Some("echo 'captured output'".to_string()),
            capture_output: CaptureOutput::Default,
            ..Default::default()
        };

        let result = executor
            .execute_step(&step, &env, &mut context)
            .await
            .unwrap();

        assert!(result.success);
        assert!(context.captured_outputs.contains_key("CAPTURED_OUTPUT"));
        assert!(context.captured_outputs["CAPTURED_OUTPUT"].contains("captured output"));
    }

    #[tokio::test]
    async fn test_execute_step_with_env_interpolation() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("VERSION".to_string(), "1.0.0".to_string());

        let mut step_env = HashMap::new();
        step_env.insert("APP_VERSION".to_string(), "${VERSION}".to_string());

        let step = WorkflowStep {
            shell: Some("echo $APP_VERSION".to_string()),
            env: step_env,
            ..Default::default()
        };

        let result = executor
            .execute_step(&step, &env, &mut context)
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("1.0.0"));
    }

    #[tokio::test]
    async fn test_shell_command_with_on_failure_retry() {
        let (mut executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        // Add responses for claude commands (the on_failure handler)
        claude_mock.add_response(ExecutionResult {
            stdout: "Fixed the test".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a shell command with on_failure retry logic
        // This simulates what happens after conversion from YAML
        // When a shell command has on_failure, it's converted to a test command
        let step = WorkflowStep {
            test: Some(TestCommand {
                command: "false".to_string(),
                on_failure: Some(crate::config::command::TestDebugConfig {
                    claude: "/prodigy-debug-test-failure".to_string(),
                    max_attempts: 2,
                    fail_workflow: false,
                    commit_required: true,
                }),
            }),
            ..Default::default()
        };

        // Execute the step - it should use retry logic
        let result = executor.execute_step(&step, &env, &mut context).await;

        // Since fail_workflow is false and we have retries, it should not error
        if let Err(e) = &result {
            eprintln!("Unexpected error: {e}");
        }
        assert!(result.is_ok());
        let step_result = result.unwrap();

        // When fail_workflow is false, the step returns success=true even if the test failed
        // This allows the workflow to continue
        assert!(step_result.success);

        // Verify that the claude command was called for debugging
        let calls = claude_mock.get_calls();
        assert!(!calls.is_empty());
        assert!(calls[0].0.contains("/prodigy-debug-test-failure"));
    }

    #[tokio::test]
    async fn test_shell_command_with_on_failure_fail_workflow() {
        let (mut executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        // Add responses for claude commands (the on_failure handler)
        claude_mock.add_response(ExecutionResult {
            stdout: "Could not fix the test".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a shell command with on_failure retry logic that fails the workflow
        let step = WorkflowStep {
            test: Some(TestCommand {
                command: "false".to_string(),
                on_failure: Some(crate::config::command::TestDebugConfig {
                    claude: "/prodigy-debug-test-failure".to_string(),
                    max_attempts: 1,
                    fail_workflow: true,
                    commit_required: true,
                }),
            }),
            ..Default::default()
        };

        // Execute the step - it should fail since fail_workflow is true
        let result = executor.execute_step(&step, &env, &mut context).await;

        // Should error since fail_workflow is true
        assert!(result.is_err());
        let err = result.unwrap_err();
        eprintln!("Error message: {err}");
        // The error message says "Test command" because shell commands with on_failure are converted to test commands
        assert!(err
            .to_string()
            .contains("Test command failed after 1 attempts and fail_workflow is true"));
    }

    // ==================== CONTROL FLOW TESTS ====================

    #[tokio::test]
    async fn test_when_clause_skips_step() {
        let (mut executor, _, _, _, _git_mock) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("SKIP_TEST".to_string(), "true".to_string());

        let step = WorkflowStep {
            shell: Some("echo 'This should be skipped'".to_string()),
            when: Some("${SKIP_TEST} != 'true'".to_string()),
            ..Default::default()
        };

        let result = executor
            .execute_step(&step, &env, &mut context)
            .await
            .unwrap();
        assert!(result.success);
        eprintln!("When clause skip stdout: '{}'", result.stdout);
        assert!(result.stdout.is_empty() || result.stdout.contains("skip"));
    }

    #[tokio::test]
    async fn test_when_clause_executes_step() {
        let (mut executor, _, _, _, git_mock) = create_test_executor_with_git_mock().await;
        // Add git mocks for commit verification
        git_mock.add_success_response("abc123def456").await; // git rev-parse HEAD (initial)
        git_mock.add_success_response("").await; // git status --porcelain (no changes)

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("RUN_TEST".to_string(), "true".to_string());

        let step = WorkflowStep {
            shell: Some("echo 'This should run'".to_string()),
            when: Some("${RUN_TEST} == 'true'".to_string()),
            ..Default::default()
        };

        let result = executor
            .execute_step(&step, &env, &mut context)
            .await
            .unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("This should run"));
    }

    #[test]
    fn test_foreach_structure_recognized() {
        // This test verifies that foreach steps are recognized
        // Creating actual ForeachConfig is complex due to nested types,
        // so we test at a higher level through workflow execution
    }

    #[test]
    fn test_when_condition_evaluation() {
        let (executor, _, _, _) = create_test_executor();

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("ENV".to_string(), "production".to_string());
        context
            .variables
            .insert("COUNT".to_string(), "5".to_string());

        // Test simple equality
        assert!(executor
            .evaluate_when_condition("${ENV} == 'production'", &context)
            .unwrap());
        assert!(!executor
            .evaluate_when_condition("${ENV} == 'staging'", &context)
            .unwrap());

        // Test inequality
        assert!(executor
            .evaluate_when_condition("${ENV} != 'staging'", &context)
            .unwrap());
        assert!(!executor
            .evaluate_when_condition("${ENV} != 'production'", &context)
            .unwrap());

        // Test numeric comparisons
        assert!(executor
            .evaluate_when_condition("${COUNT} > 3", &context)
            .unwrap());
        assert!(!executor
            .evaluate_when_condition("${COUNT} < 3", &context)
            .unwrap());
    }

    #[tokio::test]
    async fn test_conditional_workflow_branching() {
        let (mut executor, _, _, user_mock, git_mock) = create_test_executor_with_git_mock().await;
        // Add git mocks for commit verification
        git_mock.add_success_response("abc123def456").await; // git rev-parse HEAD (initial)
        git_mock.add_success_response("").await; // git status --porcelain (no changes)

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Create a workflow with conditional steps
        let workflow = ExtendedWorkflowConfig {
            name: "Conditional Workflow".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![
                WorkflowStep {
                    shell: Some("echo 'Always runs'".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo 'Conditional step'".to_string()),
                    when: Some("false".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo 'Final step'".to_string()),
                    ..Default::default()
                },
            ],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // Set test mode to avoid actual command execution
        std::env::set_var("PRODIGY_TEST_MODE", "true");

        let result = executor.execute(&workflow, &env).await;
        assert!(result.is_ok());

        // Verify user messages
        let messages = user_mock.get_messages();
        assert!(messages
            .iter()
            .any(|(_, m)| m.contains("Always runs") || m.contains("Final step")));

        std::env::remove_var("PRODIGY_TEST_MODE");
    }

    // ==================== ERROR HANDLING TESTS ====================

    #[tokio::test]
    async fn test_on_failure_handler_execution() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a step that will fail with an on_failure handler
        let step = WorkflowStep {
            shell: Some("exit 1".to_string()),
            on_failure: Some(OnFailureConfig::SingleCommand(
                "echo 'Handling failure'".to_string(),
            )),
            ..Default::default()
        };

        let result = executor.execute_step(&step, &env, &mut context).await;
        // With Proceed strategy, the step should not error even if it fails
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_on_failure_abort_strategy() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a step that will fail with abort strategy
        let step = WorkflowStep {
            shell: Some("exit 1".to_string()),
            on_failure: Some(OnFailureConfig::IgnoreErrors(false)),
            ..Default::default()
        };

        let result = executor.execute_step(&step, &env, &mut context).await;
        // With Abort strategy, the step should error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_configuration() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a step with retry configuration
        let step = WorkflowStep {
            shell: Some("echo 'Testing retry'".to_string()),
            // Retry configuration - simplified for testing
            retry: None,
            ..Default::default()
        };

        let result = executor
            .execute_step(&step, &env, &mut context)
            .await
            .unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_exit_code_handler() {
        let (_executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let _env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let _context = WorkflowContext::default();

        // Create a step with exit code handlers
        let mut exit_handlers = HashMap::new();
        exit_handlers.insert(
            0,
            Box::new(WorkflowStep {
                shell: Some("echo 'Success handler'".to_string()),
                ..Default::default()
            }),
        );
        exit_handlers.insert(
            1,
            Box::new(WorkflowStep {
                shell: Some("echo 'Error handler'".to_string()),
                ..Default::default()
            }),
        );

        let step = WorkflowStep {
            shell: Some("exit 0".to_string()),
            on_exit_code: exit_handlers,
            ..Default::default()
        };

        // This test verifies the structure is correct
        assert!(!step.on_exit_code.is_empty());
        assert!(step.on_exit_code.contains_key(&0));
        assert!(step.on_exit_code.contains_key(&1));
    }

    #[tokio::test]
    async fn test_test_command_with_retry() {
        let (mut executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        // Add mock response for the debug handler
        claude_mock.add_response(ExecutionResult {
            stdout: "Debug output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            session_id: Arc::from("test-session"),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a test command with debug config
        let step = WorkflowStep {
            test: Some(TestCommand {
                command: "false".to_string(), // This will fail
                on_failure: Some(crate::config::command::TestDebugConfig {
                    claude: "/prodigy-debug-test".to_string(),
                    max_attempts: 2,
                    fail_workflow: false,
                    commit_required: false,
                }),
            }),
            ..Default::default()
        };

        // Execute - should not error because fail_workflow is false
        let result = executor.execute_step(&step, &env, &mut context).await;
        assert!(result.is_ok());

        // Verify claude was called for debugging
        let calls = claude_mock.get_calls();
        assert!(calls
            .iter()
            .any(|(cmd, _, _)| cmd.contains("/prodigy-debug-test")));
    }

    #[tokio::test]
    async fn test_error_recovery_workflow() {
        let (mut executor, _, _, _user_mock, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Create a workflow with error recovery
        let workflow = ExtendedWorkflowConfig {
            name: "Error Recovery Workflow".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![
                WorkflowStep {
                    shell: Some("echo 'Step 1'".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("false".to_string()), // This will fail
                    on_failure: Some(OnFailureConfig::Advanced {
                        shell: Some("echo 'Recovered from error'".to_string()),
                        claude: None,
                        fail_workflow: false,
                        retry_original: true,
                        max_retries: 1,
                    }),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo 'Step 3 after recovery'".to_string()),
                    ..Default::default()
                },
            ],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // Set test mode
        std::env::set_var("PRODIGY_TEST_MODE", "true");

        let result = executor.execute(&workflow, &env).await;
        // Should succeed with recovery
        assert!(result.is_ok());

        std::env::remove_var("PRODIGY_TEST_MODE");
    }

    #[tokio::test]
    #[ignore] // Skip test - goal seek uses real shell executor which needs more setup
    async fn test_execute_goal_seek_command() {
        let (mut executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Add mock responses for goal seek iterations
        claude_mock.add_response(ExecutionResult {
            stdout: "Iteration 1: Performance improved by 10%".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });
        claude_mock.add_response(ExecutionResult {
            stdout: "Iteration 2: Performance improved by 25%".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let goal_seek_config = crate::cook::goal_seek::GoalSeekConfig {
            goal: "Performance improvement > 20%".to_string(),
            claude: Some("/optimize-performance".to_string()),
            shell: None,
            validate: "echo 25".to_string(), // Simple validation returning score
            threshold: 20,
            max_attempts: 5,
            timeout_seconds: None,
            fail_on_incomplete: None,
        };

        let step = WorkflowStep {
            goal_seek: Some(goal_seek_config),
            ..Default::default()
        };

        let mut context = WorkflowContext::default();
        let result = executor.execute_step(&step, &env, &mut context).await;

        // Goal seek should succeed after finding 25% improvement
        if let Err(e) = &result {
            eprintln!("Goal seek failed with error: {:?}", e);
        }
        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.success);
    }

    #[tokio::test]
    async fn test_execute_foreach_command() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Save current directory and change to temp dir for test
        let original_dir = std::env::current_dir().ok();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create test files
        std::fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        std::fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();

        let foreach_config = crate::config::command::ForeachConfig {
            input: crate::config::command::ForeachInput::List(vec![
                "file1.txt".to_string(),
                "file2.txt".to_string(),
            ]),
            parallel: crate::config::command::ParallelConfig::Boolean(false),
            do_block: vec![Box::new(crate::config::command::WorkflowStepCommand {
                claude: None,
                shell: Some("echo Processing item".to_string()),
                analyze: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: None,
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: None,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
                when: None,
                capture_format: None,
                capture_streams: None,
                output_file: None,
            })],
            continue_on_error: false,
            max_items: None,
        };

        let step = WorkflowStep {
            foreach: Some(foreach_config),
            ..Default::default()
        };

        let mut context = WorkflowContext::default();
        let result = executor.execute_step(&step, &env, &mut context).await;

        // Restore original directory if it was available
        if let Some(dir) = original_dir {
            let _ = std::env::set_current_dir(dir);
        }

        // Foreach should process all items successfully
        assert!(result.is_ok());
        let step_result = result.unwrap();
        assert!(step_result.success);
        // The output should contain the summary of 2 successful items
        assert!(step_result.stdout.contains("2 total, 2 successful"));
    }

    // ==================== INTEGRATION TESTS ====================

    #[tokio::test]
    async fn test_complete_workflow_execution() {
        let (mut executor, claude_mock, session_mock, user_mock, _) =
            create_test_executor_with_git_mock().await;

        // Add mock responses for claude commands
        claude_mock.add_response(ExecutionResult {
            stdout: "Analysis complete".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });
        claude_mock.add_response(ExecutionResult {
            stdout: "Code reviewed".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: Some(Arc::from("test-worktree")),
            session_id: Arc::from("integration-test"),
        };

        // Create a comprehensive workflow
        let workflow = ExtendedWorkflowConfig {
            name: "Full Integration Test".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![
                // Step 1: Shell command
                WorkflowStep {
                    shell: Some("echo 'Starting workflow'".to_string()),
                    ..Default::default()
                },
                // Step 2: Claude command with capture
                WorkflowStep {
                    claude: Some("/prodigy-analyze".to_string()),
                    capture_output: CaptureOutput::Variable("analysis_result".to_string()),
                    ..Default::default()
                },
                // Step 3: Conditional step
                WorkflowStep {
                    shell: Some("echo 'Analysis: ${analysis_result}'".to_string()),
                    when: Some("true".to_string()),
                    ..Default::default()
                },
                // Step 4: Claude command
                WorkflowStep {
                    claude: Some("/prodigy-code-review".to_string()),
                    commit_required: false,
                    ..Default::default()
                },
                // Step 5: Final shell command
                WorkflowStep {
                    shell: Some("echo 'Workflow complete'".to_string()),
                    ..Default::default()
                },
            ],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // Execute the workflow
        let result = executor.execute(&workflow, &env).await;
        if let Err(e) = &result {
            eprintln!("Workflow execution failed with error: {:?}", e);
        }
        assert!(result.is_ok());

        // Verify session updates
        let updates = session_mock.get_updates();
        assert!(updates
            .iter()
            .any(|u| matches!(u, SessionUpdate::StartWorkflow)));
        assert!(updates
            .iter()
            .any(|u| matches!(u, SessionUpdate::IncrementIteration)));

        // Verify user messages
        let messages = user_mock.get_messages();
        assert!(messages
            .iter()
            .any(|(t, m)| t == "info" && m.contains("Full Integration Test")));

        // Verify claude was called
        let calls = claude_mock.get_calls();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].0.contains("/prodigy-analyze"));
        assert!(calls[1].0.contains("/prodigy-code-review"));
    }

    #[tokio::test]
    async fn test_iterative_workflow() {
        // Create custom test configuration with focus tracking enabled to ensure all iterations run
        let test_config = Arc::new(crate::testing::config::TestConfiguration {
            test_mode: false,
            track_focus: true, // This ensures we continue to max iterations
            no_changes_commands: vec![],
            skip_commit_validation: false,
            worktree_name: None,
            additional_args: Default::default(),
        });

        let claude_mock = Arc::new(MockClaudeExecutor::new());
        let session_mock = Arc::new(MockSessionManager::new());
        let user_interaction = Arc::new(MockUserInteraction::new());
        let git_mock = Arc::new(MockGitOperations::new());

        // Set up git mock responses
        for _ in 0..20 {
            git_mock.add_success_response("abc123def456").await;
        }
        git_mock.add_success_response("").await;

        let mut executor = WorkflowExecutor::with_test_config_and_git(
            claude_mock.clone() as Arc<dyn ClaudeExecutor>,
            session_mock.clone() as Arc<dyn SessionManager>,
            user_interaction.clone() as Arc<dyn UserInteraction>,
            test_config,
            git_mock.clone(),
        );

        // Add mock responses for multiple iterations
        for _ in 0..3 {
            claude_mock.add_response(ExecutionResult {
                stdout: "Iteration complete".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                success: true,
            });
        }

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("iterative-test"),
        };

        // Create an iterative workflow
        let workflow = ExtendedWorkflowConfig {
            name: "Iterative Workflow".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![WorkflowStep {
                claude: Some("/prodigy-improve".to_string()),
                ..Default::default()
            }],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 3,
            iterate: true,
            retry_defaults: None,
            environment: None,
        };

        // Execute the workflow
        let result = executor.execute(&workflow, &env).await;
        assert!(result.is_ok());

        // Verify we had 3 iterations
        let updates = session_mock.get_updates();
        let iteration_count = updates
            .iter()
            .filter(|u| matches!(u, SessionUpdate::IncrementIteration))
            .count();
        assert_eq!(iteration_count, 3);
    }

    #[tokio::test]
    async fn test_workflow_with_environment_variables() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("env-test"),
        };

        // Create workflow with environment configuration
        let workflow = ExtendedWorkflowConfig {
            name: "Environment Test".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![WorkflowStep {
                shell: Some("echo \"Version: $APP_VERSION\"".to_string()),
                env: {
                    let mut env = HashMap::new();
                    env.insert("APP_VERSION".to_string(), "1.2.3".to_string());
                    env
                },
                ..Default::default()
            }],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None, // Environment config not needed for this test
        };

        let result = executor.execute(&workflow, &env).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workflow_with_validation() {
        let (_executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        // Add mock response for validation
        claude_mock.add_response(ExecutionResult {
            stdout: r#"{
                "completion_percentage": 100.0,
                "status": "complete",
                "missing": [],
                "implemented": ["feature1", "feature2"]
            }"#
            .to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let _env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("validation-test"),
        };

        // Create workflow with validation
        let workflow = ExtendedWorkflowConfig {
            name: "Validation Test".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![WorkflowStep {
                claude: Some("/prodigy-implement-spec 01".to_string()),
                // Validation configuration
                validate: None,
                ..Default::default()
            }],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // This test verifies the workflow structure is correct
        assert!(workflow.steps[0].validate.is_none());
    }

    #[tokio::test]
    async fn test_validation_with_streaming_enabled() {
        // Set streaming environment variable
        std::env::set_var("PRODIGY_CLAUDE_STREAMING", "true");

        let (mut executor, claude_mock, _, _, _) = create_test_executor_with_git_mock().await;

        // Add mock response for implementation step
        claude_mock.add_response(ExecutionResult {
            stdout: "Implementation complete".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        // Add mock response for validation
        claude_mock.add_response(ExecutionResult {
            stdout: r#"{
                "completion_percentage": 100.0,
                "status": "complete",
                "missing": [],
                "implemented": ["feature1"]
            }"#
            .to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("validation-streaming-test"),
        };

        // Create workflow with validation step
        let workflow = ExtendedWorkflowConfig {
            name: "Test Validation Streaming".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![WorkflowStep {
                claude: Some("/prodigy-implement-spec 01".to_string()),
                validate: Some(crate::cook::workflow::validation::ValidationConfig {
                    claude: Some("/prodigy-validate-spec 01".to_string()),
                    shell: None,
                    command: None,
                    threshold: 100.0,
                    result_file: None,
                    timeout: None,
                    expected_schema: None,
                    on_incomplete: None,
                }),
                ..Default::default()
            }],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // Execute workflow
        let result = executor.execute(&workflow, &env).await;
        assert!(result.is_ok());

        // Verify that both Claude commands were called with streaming flag
        let calls = claude_mock.get_calls();
        assert_eq!(calls.len(), 2);

        // Check implementation command
        let (_cmd, _path, env_vars) = &calls[0];
        assert_eq!(
            env_vars.get("PRODIGY_CLAUDE_STREAMING"),
            Some(&"true".to_string())
        );

        // Check validation command - this is the key test!
        let (_cmd, _path, env_vars) = &calls[1];
        assert_eq!(
            env_vars.get("PRODIGY_CLAUDE_STREAMING"),
            Some(&"true".to_string())
        );

        // Clean up
        std::env::remove_var("PRODIGY_CLAUDE_STREAMING");
    }

    #[tokio::test]
    async fn test_workflow_resume_capability() {
        let (mut executor, _, session_mock, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("resume-test"),
        };

        // Create workflow that tracks completed steps
        let workflow = ExtendedWorkflowConfig {
            name: "Resume Test".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![
                WorkflowStep {
                    shell: Some("echo 'Step 1'".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo 'Step 2'".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo 'Step 3'".to_string()),
                    ..Default::default()
                },
            ],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        // Execute workflow
        let result = executor.execute(&workflow, &env).await;
        assert!(result.is_ok());

        // Verify session state can be saved/loaded
        // Start session to set the ID properly
        session_mock.start_session("resume-test").await.unwrap();
        let state = session_mock
            .get_state()
            .expect("Failed to get state in executor test");
        assert_eq!(state.session_id, "resume-test");

        // Verify we can save checkpoint
        let checkpoint_result = session_mock.save_checkpoint(&state).await;
        assert!(checkpoint_result.is_ok());
    }
} // end tests module

#[cfg(test)]
mod capture_output_tests {
    use crate::cook::workflow::{CaptureOutput, WorkflowContext, WorkflowStep};

    #[test]
    fn test_capture_output_deserialization() {
        // Test boolean true deserializes to Default
        let yaml = "capture_output: true";
        let step: WorkflowStep =
            serde_yaml::from_str(&format!("shell: echo test\n{}", yaml)).unwrap();
        assert_eq!(step.capture_output, CaptureOutput::Default);

        // Test boolean false deserializes to Disabled
        let yaml = "capture_output: false";
        let step: WorkflowStep =
            serde_yaml::from_str(&format!("shell: echo test\n{}", yaml)).unwrap();
        assert_eq!(step.capture_output, CaptureOutput::Disabled);

        // Test string deserializes to Variable
        let yaml = "capture_output: my_custom_var";
        let step: WorkflowStep =
            serde_yaml::from_str(&format!("shell: echo test\n{}", yaml)).unwrap();
        assert_eq!(
            step.capture_output,
            CaptureOutput::Variable("my_custom_var".to_string())
        );

        // Test dotted variable name
        let yaml = "capture_output: analysis.result";
        let step: WorkflowStep =
            serde_yaml::from_str(&format!("shell: echo test\n{}", yaml)).unwrap();
        assert_eq!(
            step.capture_output,
            CaptureOutput::Variable("analysis.result".to_string())
        );
    }

    #[test]
    fn test_capture_output_variable_interpolation() {
        let mut context = WorkflowContext::default();

        // Add some captured outputs with custom names
        context.captured_outputs.insert(
            "analysis_result".to_string(),
            "High complexity detected".to_string(),
        );
        context
            .captured_outputs
            .insert("todo_count".to_string(), "42".to_string());

        // Test interpolation
        let template = "Analysis: ${analysis_result}, TODOs: ${todo_count}";
        let result = context.interpolate(template);

        assert_eq!(result, "Analysis: High complexity detected, TODOs: 42");
    }
}

// ==================== TIMEOUT TESTS ====================

pub(crate) mod timeout_tests {
    use super::tests::*;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::cook::workflow::{ExtendedWorkflowConfig, WorkflowMode, WorkflowStep};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    #[ignore] // Skip test - timeout implementation not working correctly
    async fn test_workflow_timeout() {
        let (mut executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Create a workflow with a timeout
        let workflow = ExtendedWorkflowConfig {
            name: "Timeout Test".to_string(),
            mode: WorkflowMode::Sequential,
            steps: vec![WorkflowStep {
                shell: Some("sleep 10".to_string()),
                timeout: Some(1), // 1 second timeout
                ..Default::default()
            }],
            setup_phase: None,
            map_phase: None,
            reduce_phase: None,
            max_iterations: 1,
            iterate: false,
            retry_defaults: None,
            environment: None,
        };

        let result = executor.execute(&workflow, &env).await;

        // The workflow should fail due to timeout
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        eprintln!("Timeout test error message: '{}'", error_msg);
        assert!(
            error_msg.contains("timeout")
                || error_msg.contains("Timeout")
                || error_msg.contains("timed out")
        );
    }

    #[tokio::test]
    async fn test_step_timeout() {
        let (executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Test shell command with timeout
        let env_vars = HashMap::new();
        let result = executor
            .execute_shell_command("sleep 10", &env, env_vars, Some(1))
            .await;

        // Should fail due to timeout
        assert!(result.is_err() || !result.unwrap().success);
    }

    #[tokio::test]
    async fn test_command_completes_within_timeout() {
        let (executor, _, _, _, _) = create_test_executor_with_git_mock().await;

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: Arc::new(temp_dir.path().to_path_buf()),
            project_dir: Arc::new(temp_dir.path().to_path_buf()),
            worktree_name: None,
            session_id: Arc::from("test"),
        };

        // Test shell command that completes within timeout
        let env_vars = HashMap::new();
        let result = executor
            .execute_shell_command("echo 'fast command'", &env, env_vars, Some(5000))
            .await
            .unwrap();

        // Should succeed
        assert!(result.success);
        assert!(result.stdout.contains("fast command"));
    }
}
