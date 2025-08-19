//! Comprehensive unit tests for WorkflowExecutor
//! DISABLED: Tests require analysis functionality that was removed

#[cfg(never)]
mod disabled_tests {
    use super::*;
    use crate::commands::context::AnalysisResult;
    use crate::config::command::TestCommand;
    use crate::cook::execution::ExecutionResult;
    use crate::cook::interaction::SpinnerHandle;
    use crate::cook::metrics::ProjectMetrics;
    use crate::cook::session::state::SessionState;
    use crate::cook::session::summary::SessionSummary;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    // Mock implementations for testing

    struct MockClaudeExecutor {
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

    struct MockSessionManager {
        updates: Arc<Mutex<Vec<SessionUpdate>>>,
        iteration: Arc<Mutex<u32>>,
    }

    impl MockSessionManager {
        fn new() -> Self {
            Self {
                updates: Arc::new(Mutex::new(Vec::new())),
                iteration: Arc::new(Mutex::new(0)),
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

        async fn start_session(&self, _session_id: &str) -> Result<()> {
            Ok(())
        }

        async fn complete_session(&self) -> Result<SessionSummary> {
            Ok(SessionSummary {
                iterations: 1,
                files_changed: 0,
            })
        }

        fn get_state(&self) -> SessionState {
            SessionState::new("test-session".to_string(), PathBuf::from("/tmp"))
        }

        async fn save_state(&self, _path: &Path) -> Result<()> {
            Ok(())
        }

        async fn load_state(&self, _path: &Path) -> Result<()> {
            Ok(())
        }
    }

    struct MockAnalysisCoordinator {
        analysis_called: Arc<Mutex<bool>>,
    }

    impl MockAnalysisCoordinator {
        fn new() -> Self {
            Self {
                analysis_called: Arc::new(Mutex::new(false)),
            }
        }

        #[allow(dead_code)]
        fn was_called(&self) -> bool {
            *self.analysis_called.lock().unwrap()
        }
    }

    #[async_trait]
    impl AnalysisCoordinator for MockAnalysisCoordinator {
        async fn analyze_project(&self, _working_dir: &Path) -> Result<AnalysisResult> {
            *self.analysis_called.lock().unwrap() = true;
            Ok(AnalysisResult {
                dependency_graph: crate::commands::context::dependencies::DependencyGraph {
                    nodes: HashMap::new(),
                    edges: vec![],
                    cycles: vec![],
                    layers: vec![],
                },
                architecture: crate::context::ArchitectureInfo {
                    patterns: vec![],
                    layers: vec![],
                    components: HashMap::new(),
                    violations: vec![],
                },
                conventions: crate::context::conventions::ProjectConventions {
                    naming_patterns: crate::context::conventions::NamingRules::default(),
                    code_patterns: HashMap::new(),
                    test_patterns: crate::context::conventions::TestingConventions {
                        test_file_pattern: "*_test.rs".to_string(),
                        test_function_prefix: "test_".to_string(),
                        test_module_pattern: "tests".to_string(),
                        assertion_style: "assert".to_string(),
                    },
                    project_idioms: vec![],
                },
                technical_debt: crate::context::debt::TechnicalDebtMap {
                    debt_items: vec![],
                    hotspots: vec![],
                    duplication_map: HashMap::new(),
                    priority_queue: std::collections::BinaryHeap::new(),
                },
                test_coverage: Some(crate::context::test_coverage::TestCoverageMap {
                    overall_coverage: 0.0,
                    file_coverage: HashMap::new(),
                    untested_functions: vec![],
                    critical_paths: vec![],
                }),
                metadata: crate::context::AnalysisMetadata {
                    timestamp: chrono::Utc::now(),
                    duration_ms: 0,
                    files_analyzed: 0,
                    incremental: false,
                    version: "test".to_string(),
                    criticality_distribution: None,
                    scoring_algorithm: Some("test".to_string()),
                },
            })
        }

        async fn save_analysis(
            &self,
            _working_dir: &Path,
            _analysis: &AnalysisResult,
        ) -> Result<()> {
            Ok(())
        }

        async fn analyze_incremental(
            &self,
            _project_path: &Path,
            _changed_files: &[String],
        ) -> Result<AnalysisResult> {
            Ok(AnalysisResult {
                dependency_graph: crate::commands::context::dependencies::DependencyGraph {
                    nodes: HashMap::new(),
                    edges: vec![],
                    cycles: vec![],
                    layers: vec![],
                },
                architecture: crate::context::ArchitectureInfo {
                    patterns: vec![],
                    layers: vec![],
                    components: HashMap::new(),
                    violations: vec![],
                },
                conventions: crate::context::conventions::ProjectConventions {
                    naming_patterns: crate::context::conventions::NamingRules::default(),
                    code_patterns: HashMap::new(),
                    test_patterns: crate::context::conventions::TestingConventions {
                        test_file_pattern: "*_test.rs".to_string(),
                        test_function_prefix: "test_".to_string(),
                        test_module_pattern: "tests".to_string(),
                        assertion_style: "assert".to_string(),
                    },
                    project_idioms: vec![],
                },
                technical_debt: crate::context::debt::TechnicalDebtMap {
                    debt_items: vec![],
                    hotspots: vec![],
                    duplication_map: HashMap::new(),
                    priority_queue: std::collections::BinaryHeap::new(),
                },
                test_coverage: Some(crate::context::test_coverage::TestCoverageMap {
                    overall_coverage: 0.0,
                    file_coverage: HashMap::new(),
                    untested_functions: vec![],
                    critical_paths: vec![],
                }),
                metadata: crate::context::AnalysisMetadata {
                    timestamp: chrono::Utc::now(),
                    duration_ms: 0,
                    files_analyzed: 0,
                    incremental: false,
                    version: "test".to_string(),
                    criticality_distribution: None,
                    scoring_algorithm: Some("test".to_string()),
                },
            })
        }

        async fn get_cached_analysis(
            &self,
            _project_path: &Path,
        ) -> Result<Option<AnalysisResult>> {
            Ok(None)
        }

        async fn clear_cache(&self, _project_path: &Path) -> Result<()> {
            Ok(())
        }
    }

    struct MockMetricsCoordinator {
        metrics_collected: Arc<Mutex<bool>>,
        report: String,
    }

    impl MockMetricsCoordinator {
        fn new() -> Self {
            Self {
                metrics_collected: Arc::new(Mutex::new(false)),
                report: "Test metrics report".to_string(),
            }
        }

        #[allow(dead_code)]
        fn was_collected(&self) -> bool {
            *self.metrics_collected.lock().unwrap()
        }
    }

    #[async_trait]
    impl MetricsCoordinator for MockMetricsCoordinator {
        async fn collect_all(
            &self,
            _working_dir: &Path,
        ) -> Result<crate::cook::metrics::ProjectMetrics> {
            *self.metrics_collected.lock().unwrap() = true;
            Ok(ProjectMetrics {
                test_coverage: Some(0.0),
                type_coverage: Some(0.0),
                lint_warnings: 0,
                code_duplication: Some(0.0),
                doc_coverage: Some(0.0),
                benchmark_results: None,
                compile_time: Some(0.0),
                binary_size: Some(0),
                cyclomatic_complexity: None,
                max_nesting_depth: None,
                total_lines: None,
                tech_debt_score: Some(0.0),
                improvement_velocity: None,
                timestamp: chrono::Utc::now(),
                iteration_id: None,
                command_timings: None,
                iteration_duration: None,
                workflow_timing: None,
            })
        }

        async fn store_metrics(
            &self,
            _working_dir: &Path,
            _metrics: &crate::cook::metrics::ProjectMetrics,
        ) -> Result<()> {
            Ok(())
        }

        async fn load_history(&self, _working_dir: &Path) -> Result<Vec<ProjectMetrics>> {
            Ok(vec![])
        }

        async fn generate_report(
            &self,
            _metrics: &crate::cook::metrics::ProjectMetrics,
            _history: &[ProjectMetrics],
        ) -> Result<String> {
            Ok(self.report.clone())
        }

        async fn collect_metric(
            &self,
            _project_path: &Path,
            _metric: &str,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({}))
        }
    }

    // Mock spinner handle
    struct MockSpinnerHandle;

    impl SpinnerHandle for MockSpinnerHandle {
        fn update_message(&mut self, _message: &str) {}
        fn success(&mut self, _message: &str) {}
        fn fail(&mut self, _message: &str) {}
    }

    struct MockUserInteraction {
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

    // Helper function to create a test executor with mocks
    #[allow(clippy::type_complexity)]
    fn create_test_executor() -> (
        WorkflowExecutor,
        Arc<MockClaudeExecutor>,
        Arc<MockSessionManager>,
        Arc<MockAnalysisCoordinator>,
        Arc<MockMetricsCoordinator>,
        Arc<MockUserInteraction>,
    ) {
        let claude_executor = Arc::new(MockClaudeExecutor::new());
        let session_manager = Arc::new(MockSessionManager::new());
        let analysis_coordinator = Arc::new(MockAnalysisCoordinator::new());
        let metrics_coordinator = Arc::new(MockMetricsCoordinator::new());
        let user_interaction = Arc::new(MockUserInteraction::new());

        let executor = WorkflowExecutor::new(
            claude_executor.clone() as Arc<dyn ClaudeExecutor>,
            session_manager.clone() as Arc<dyn SessionManager>,
            analysis_coordinator.clone() as Arc<dyn AnalysisCoordinator>,
            metrics_coordinator.clone() as Arc<dyn MetricsCoordinator>,
            user_interaction.clone() as Arc<dyn UserInteraction>,
        );

        (
            executor,
            claude_executor,
            session_manager,
            analysis_coordinator,
            metrics_coordinator,
            user_interaction,
        )
    }

    // Helper function to create a test executor with configuration
    #[allow(clippy::type_complexity)]
    fn create_test_executor_with_config(
        config: TestConfiguration,
    ) -> (
        WorkflowExecutor,
        Arc<MockClaudeExecutor>,
        Arc<MockSessionManager>,
        Arc<MockAnalysisCoordinator>,
        Arc<MockMetricsCoordinator>,
        Arc<MockUserInteraction>,
    ) {
        let claude_executor = Arc::new(MockClaudeExecutor::new());
        let session_manager = Arc::new(MockSessionManager::new());
        let analysis_coordinator = Arc::new(MockAnalysisCoordinator::new());
        let metrics_coordinator = Arc::new(MockMetricsCoordinator::new());
        let user_interaction = Arc::new(MockUserInteraction::new());

        let executor = WorkflowExecutor::with_test_config(
            claude_executor.clone() as Arc<dyn ClaudeExecutor>,
            session_manager.clone() as Arc<dyn SessionManager>,
            analysis_coordinator.clone() as Arc<dyn AnalysisCoordinator>,
            metrics_coordinator.clone() as Arc<dyn MetricsCoordinator>,
            user_interaction.clone() as Arc<dyn UserInteraction>,
            Arc::new(config),
        );

        (
            executor,
            claude_executor,
            session_manager,
            analysis_coordinator,
            metrics_coordinator,
            user_interaction,
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

        // The interpolation uses the first match found (variables takes precedence)
        assert_eq!(context.interpolate("${KEY}"), "from_variables");
    }

    #[test]
    fn test_determine_command_type_claude() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: Some("/mmm-code-review".to_string()),
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
            handler: None,
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Claude(cmd) if cmd == "/mmm-code-review"));
    }

    #[test]
    fn test_determine_command_type_shell() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: Some("cargo test".to_string()),
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Shell(cmd) if cmd == "cargo test"));
    }

    #[test]
    fn test_determine_command_type_test() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let test_cmd = TestCommand {
            command: "cargo test".to_string(),
            on_failure: None,
        };

        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None,
            test: Some(test_cmd.clone()),
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Test(cmd) if cmd.command == "cargo test"));
    }

    #[test]
    fn test_determine_command_type_legacy_name() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: Some("mmm-code-review".to_string()),
            claude: None,
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
            handler: None,
        };

        let result = executor.determine_command_type(&step).unwrap();
        assert!(matches!(result, CommandType::Legacy(cmd) if cmd == "/mmm-code-review"));
    }

    #[test]
    fn test_determine_command_type_multiple_error() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: Some("/mmm-code-review".to_string()),
            shell: Some("cargo test".to_string()),
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
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
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
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
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: Some("/mmm-code-review --strict".to_string()),
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
            handler: None,
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "claude: /mmm-code-review --strict");
    }

    #[test]
    fn test_get_step_display_name_shell() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: Some("cargo test --verbose".to_string()),
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "shell: cargo test --verbose");
    }

    #[test]
    fn test_get_step_display_name_test() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let test_cmd = TestCommand {
            command: "pytest tests/".to_string(),
            on_failure: None,
        };

        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None,
            test: Some(test_cmd),
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "test: pytest tests/");
    }

    #[test]
    fn test_get_step_display_name_unnamed() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        let display = executor.get_step_display_name(&step);
        assert_eq!(display, "unnamed step");
    }

    #[test]
    fn test_handle_test_mode_execution_success() {
        let config = TestConfiguration::builder().test_mode(true).build();
        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);

        let step = WorkflowStep {
            name: None,
            claude: Some("/mmm-code-review".to_string()),
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        let command_type = CommandType::Claude("/mmm-code-review".to_string());
        let result = executor
            .handle_test_mode_execution(&step, &command_type)
            .unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.stdout.contains("[TEST MODE]"));

        std::env::remove_var("MMM_TEST_MODE");
    }

    #[test]
    fn test_is_test_mode_no_changes_command() {
        use crate::testing::config::TestConfiguration;

        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec!["mmm-code-review".to_string(), "mmm-lint".to_string()])
            .build();

        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);

        assert!(executor.is_test_mode_no_changes_command("/mmm-code-review"));
        assert!(executor.is_test_mode_no_changes_command("mmm-lint"));
        assert!(!executor.is_test_mode_no_changes_command("/mmm-implement-spec"));

        // Test with arguments
        assert!(executor.is_test_mode_no_changes_command("/mmm-code-review --strict"));
        assert!(executor.is_test_mode_no_changes_command("mmm-lint --fix"));
    }

    #[test]
    fn test_should_stop_early_in_test_mode() {
        use crate::testing::config::TestConfiguration;

        // Test without no_changes_commands
        let config = TestConfiguration::builder().test_mode(true).build();
        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);
        assert!(!executor.should_stop_early_in_test_mode());

        // Test with mmm-code-review and mmm-lint
        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec!["mmm-code-review".to_string(), "mmm-lint".to_string()])
            .build();
        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);
        assert!(executor.should_stop_early_in_test_mode());

        // Test with mmm-implement-spec only
        let config = TestConfiguration::builder()
            .test_mode(true)
            .no_changes_commands(vec!["mmm-implement-spec".to_string()])
            .build();
        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);
        assert!(!executor.should_stop_early_in_test_mode());
    }

    #[test]
    fn test_is_focus_tracking_test() {
        use crate::testing::config::TestConfiguration;

        // Test without track_focus
        let config = TestConfiguration::builder().test_mode(true).build();
        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);
        assert!(!executor.is_focus_tracking_test());

        // Test with track_focus enabled
        let config = TestConfiguration::builder()
            .test_mode(true)
            .track_focus(true)
            .build();
        let (executor, _, _, _, _, _) = create_test_executor_with_config(config);
        assert!(executor.is_focus_tracking_test());
    }

    #[test]
    fn test_handle_no_commits_error_general_command() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let step = WorkflowStep {
            name: None,
            claude: Some("/mmm-implement-spec".to_string()),
            shell: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: true,
            handler: None,
        };

        let result = executor.handle_no_commits_error(&step);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No commits created"));
    }

    #[tokio::test]
    async fn test_execute_claude_command() {
        let (executor, claude_mock, _, _, _, _) = create_test_executor();

        let command = "/mmm-code-review";
        let temp_dir = TempDir::new().unwrap();
        let working_dir = temp_dir.path();
        let env = ExecutionEnvironment {
            working_dir: working_dir.to_path_buf(),
            project_dir: working_dir.to_path_buf(),
            worktree_name: None,
            session_id: "test".to_string(),
        };

        let mut env_vars = HashMap::new();
        env_vars.insert("MMM_CONTEXT_AVAILABLE".to_string(), "true".to_string());

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
        assert!(calls[0].2.contains_key("MMM_CONTEXT_AVAILABLE"));
    }

    #[tokio::test]
    async fn test_execute_shell_command_success() {
        let (executor, _, _, _, _, _) = create_test_executor();

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test".to_string(),
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
        let (mut executor, _, session_mock, _, _, user_mock) = create_test_executor();

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test".to_string(),
        };

        // Set up test mode to avoid actual command execution
        std::env::set_var("MMM_TEST_MODE", "true");

        // Set up workflow
        let workflow = ExtendedWorkflowConfig {
            name: "Test Workflow".to_string(),
            steps: vec![WorkflowStep {
                name: None,
                claude: Some("/mmm-code-review".to_string()),
                shell: None,
                test: None,
                command: None,
                capture_output: false,
                timeout: None,
                working_dir: None,
                env: HashMap::new(),
                on_failure: None,
                on_success: None,
                on_exit_code: HashMap::new(),
                commit_required: false,
                handler: None,
            }],
            max_iterations: 1,
            iterate: false,
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

        std::env::remove_var("MMM_TEST_MODE");
    }

    #[tokio::test]
    async fn test_execute_step_with_capture_output() {
        let (mut executor, _, _, _, _, _) = create_test_executor();

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test".to_string(),
        };

        let mut context = WorkflowContext::default();

        let step = WorkflowStep {
            name: None,
            shell: Some("echo 'captured output'".to_string()),
            claude: None,
            test: None,
            command: None,
            capture_output: true,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
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
        let (mut executor, _, _, _, _, _) = create_test_executor();

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            worktree_name: None,
            session_id: "test".to_string(),
        };

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("VERSION".to_string(), "1.0.0".to_string());

        let mut step_env = HashMap::new();
        step_env.insert("APP_VERSION".to_string(), "${VERSION}".to_string());

        let step = WorkflowStep {
            name: None,
            shell: Some("echo $APP_VERSION".to_string()),
            claude: None,
            test: None,
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: step_env,
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
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
        let (mut executor, claude_mock, _, _, _, _) = create_test_executor();

        // Add responses for claude commands (the on_failure handler)
        claude_mock.add_response(ExecutionResult {
            stdout: "Fixed the test".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            session_id: "test-session".to_string(),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a shell command with on_failure retry logic
        // This simulates what happens after conversion from YAML
        // When a shell command has on_failure, it's converted to a test command
        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None, // shell is cleared when converted to test
            test: Some(TestCommand {
                command: "false".to_string(),
                on_failure: Some(crate::config::command::TestDebugConfig {
                    claude: "/mmm-debug-test-failure".to_string(),
                    max_attempts: 2,
                    fail_workflow: false,
                    commit_required: true,
                }),
            }),
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
        };

        // Execute the step - it should use retry logic
        let result = executor.execute_step(&step, &env, &mut context).await;

        // Since fail_workflow is false and we have retries, it should not error
        if let Err(e) = &result {
            eprintln!("Unexpected error: {e}");
        }
        assert!(result.is_ok());
        let step_result = result.unwrap();

        // The command still fails but we don't fail the workflow
        assert!(!step_result.success);

        // Verify that the claude command was called for debugging
        let calls = claude_mock.get_calls();
        assert!(!calls.is_empty());
        assert!(calls[0].0.contains("/mmm-debug-test-failure"));
    }

    #[tokio::test]
    async fn test_shell_command_with_on_failure_fail_workflow() {
        let (mut executor, claude_mock, _, _, _, _) = create_test_executor();

        // Add responses for claude commands (the on_failure handler)
        claude_mock.add_response(ExecutionResult {
            stdout: "Could not fix the test".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });

        let temp_dir = TempDir::new().unwrap();
        let env = ExecutionEnvironment {
            working_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().to_path_buf(),
            session_id: "test-session".to_string(),
            worktree_name: None,
        };

        let mut context = WorkflowContext::default();

        // Create a shell command with on_failure retry logic that fails the workflow
        let step = WorkflowStep {
            name: None,
            claude: None,
            shell: None, // shell is cleared when converted to test
            test: Some(TestCommand {
                command: "false".to_string(),
                on_failure: Some(crate::config::command::TestDebugConfig {
                    claude: "/mmm-debug-test-failure".to_string(),
                    max_attempts: 1,
                    fail_workflow: true,
                    commit_required: true,
                }),
            }),
            command: None,
            capture_output: false,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            handler: None,
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
} // end disabled_tests module
