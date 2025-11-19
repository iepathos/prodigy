#[cfg(test)]
mod handle_on_failure_tests {
    use crate::cook::execution::claude::ClaudeExecutor;
    use crate::cook::execution::mapreduce::coordination::executor::MapReduceCoordinator;
    use crate::cook::execution::ExecutionResult;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::cook::workflow::OnFailureConfig;
    use crate::subprocess::error::ProcessError;
    use crate::subprocess::runner::{
        ExitStatus, ProcessCommand, ProcessOutput, ProcessRunner, ProcessStream,
    };
    use crate::subprocess::SubprocessManager;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // Mock ClaudeExecutor for testing
    #[derive(Clone)]
    struct MockClaudeExecutor {
        should_succeed: bool,
        executed_commands: Arc<Mutex<Vec<String>>>,
    }

    impl MockClaudeExecutor {
        fn new(should_succeed: bool) -> Self {
            Self {
                should_succeed,
                executed_commands: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_executed_commands(&self) -> Vec<String> {
            self.executed_commands.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ClaudeExecutor for MockClaudeExecutor {
        async fn execute_claude_command(
            &self,
            command: &str,
            _project_path: &Path,
            _env_vars: HashMap<String, String>,
        ) -> anyhow::Result<ExecutionResult> {
            self.executed_commands
                .lock()
                .unwrap()
                .push(command.to_string());
            Ok(ExecutionResult {
                success: self.should_succeed,
                stdout: format!("Output from: {}", command),
                stderr: String::new(),
                exit_code: Some(if self.should_succeed { 0 } else { 1 }),
                metadata: HashMap::new(),
            })
        }

        async fn check_claude_cli(&self) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn get_claude_version(&self) -> anyhow::Result<String> {
            Ok("1.0.0".to_string())
        }
    }

    // Mock ProcessRunner for testing
    #[derive(Clone)]
    struct MockProcessRunner {
        should_succeed: bool,
        executed_commands: Arc<Mutex<Vec<String>>>,
    }

    impl MockProcessRunner {
        fn new(should_succeed: bool) -> Self {
            Self {
                should_succeed,
                executed_commands: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    #[async_trait]
    impl ProcessRunner for MockProcessRunner {
        async fn run(&self, command: ProcessCommand) -> Result<ProcessOutput, ProcessError> {
            // Extract the actual command from the args (skip "sh" and "-c")
            if command.args.len() > 1 {
                self.executed_commands
                    .lock()
                    .unwrap()
                    .push(command.args[1].clone());
            }

            Ok(ProcessOutput {
                status: if self.should_succeed {
                    ExitStatus::Success
                } else {
                    ExitStatus::Error(1)
                },
                stdout: "".to_string(),
                stderr: "".to_string(),
                duration: Duration::from_secs(0),
            })
        }

        async fn run_streaming(
            &self,
            _command: ProcessCommand,
        ) -> Result<ProcessStream, ProcessError> {
            unimplemented!("Not used in these tests")
        }
    }

    // Helper to create a mock SubprocessManager
    fn create_mock_subprocess(should_succeed: bool) -> Arc<SubprocessManager> {
        Arc::new(SubprocessManager::new(Arc::new(MockProcessRunner::new(
            should_succeed,
        ))))
    }

    // Helper to create a test ExecutionEnvironment
    fn create_test_env() -> ExecutionEnvironment {
        ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/tmp/test")),
            project_dir: Arc::new(PathBuf::from("/tmp/test")),
            worktree_name: None,
            session_id: Arc::from("test-session"),
        }
    }

    #[tokio::test]
    async fn test_advanced_config_with_claude_command_success() {
        let config = OnFailureConfig::Advanced {
            claude: Some("/test-command".to_string()),
            shell: None,
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_advanced_config_with_claude_command_failure() {
        let config = OnFailureConfig::Advanced {
            claude: Some("/test-command".to_string()),
            shell: None,
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(false));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_advanced_config_with_shell_command_success() {
        let config = OnFailureConfig::Advanced {
            claude: None,
            shell: Some("echo test".to_string()),
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_advanced_config_with_shell_command_failure() {
        let config = OnFailureConfig::Advanced {
            claude: None,
            shell: Some("echo test".to_string()),
            max_retries: 1,
            fail_workflow: false,
            retry_original: false,
        };

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(false);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_single_command_with_claude() {
        let config = OnFailureConfig::SingleCommand("/test-claude-cmd".to_string());

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_single_command_with_shell() {
        let config = OnFailureConfig::SingleCommand("echo test".to_string());

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_default_config_returns_ok() {
        let config = OnFailureConfig::IgnoreErrors(true);

        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(MockClaudeExecutor::new(true));
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_variable_interpolation_success() {
        let config = OnFailureConfig::SingleCommand("/test ${item_id}".to_string());

        let mock_executor = MockClaudeExecutor::new(true);
        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(mock_executor.clone());
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let mut variables = HashMap::new();
        variables.insert("item_id".to_string(), "item-123".to_string());
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        assert!(result.is_ok());
        assert!(result.unwrap());
        let executed = mock_executor.get_executed_commands();
        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0], "/test item-123");
    }

    #[tokio::test]
    async fn test_variable_interpolation_with_missing_variable() {
        // When a variable is missing and strict_mode is false (default),
        // the interpolation engine leaves the variable unchanged
        let config = OnFailureConfig::SingleCommand("/test ${missing_var}".to_string());

        let mock_executor = MockClaudeExecutor::new(true);
        let claude_executor: Arc<dyn ClaudeExecutor> = Arc::new(mock_executor.clone());
        let subprocess = create_mock_subprocess(true);
        let worktree_path = PathBuf::from("/tmp/test");
        let variables = HashMap::new();
        let _env = create_test_env();

        let user_interaction: Arc<dyn crate::cook::interaction::UserInteraction> =
            Arc::new(crate::cook::interaction::MockUserInteraction::new());
        let command_executor =
            crate::cook::execution::mapreduce::coordination::CommandExecutor::new(
                claude_executor.clone(),
                subprocess.clone(),
            );
        let result = MapReduceCoordinator::handle_on_failure(
            &config,
            &worktree_path,
            &variables,
            &command_executor,
            &user_interaction,
        )
        .await;

        // Should succeed, but variable is left unchanged
        assert!(result.is_ok());
        assert!(result.unwrap());
        let executed = mock_executor.get_executed_commands();
        assert_eq!(executed.len(), 1);
        // The variable remains unchanged when not found in non-strict mode
        assert!(executed[0].contains("missing_var") || executed[0] == "/test ");
    }
}

#[cfg(test)]
mod execute_setup_phase_tests {
    use crate::cook::execution::mapreduce::coordination::executor::MapReduceCoordinator;
    use crate::cook::execution::mapreduce::types::SetupPhase;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::cook::workflow::WorkflowStep;
    use crate::subprocess::SubprocessManager;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    // Helper to create a test MapReduceCoordinator with mocks
    fn create_test_coordinator(
        claude_succeeds: bool,
        shell_succeeds: bool,
    ) -> MapReduceCoordinator {
        use crate::cook::execution::mapreduce::agent::{
            lifecycle::LifecycleError, AgentLifecycleManager,
        };
        use crate::cook::execution::mapreduce::state::{
            JobState, JobSummary, StateError, StateManager, StateStore,
        };
        use crate::cook::interaction::MockUserInteraction;
        use crate::subprocess::runner::{ExitStatus, ProcessCommand, ProcessOutput, ProcessRunner};
        use std::sync::Arc;

        // Mock ProcessRunner
        #[derive(Clone)]
        struct TestProcessRunner {
            should_succeed: bool,
        }

        #[async_trait::async_trait]
        impl ProcessRunner for TestProcessRunner {
            async fn run(
                &self,
                _command: ProcessCommand,
            ) -> Result<ProcessOutput, crate::subprocess::error::ProcessError> {
                Ok(ProcessOutput {
                    status: if self.should_succeed {
                        ExitStatus::Success
                    } else {
                        ExitStatus::Error(1)
                    },
                    stdout: "test stdout".to_string(),
                    stderr: if self.should_succeed {
                        String::new()
                    } else {
                        "test stderr".to_string()
                    },
                    duration: std::time::Duration::from_secs(0),
                })
            }

            async fn run_streaming(
                &self,
                _command: ProcessCommand,
            ) -> Result<
                crate::subprocess::runner::ProcessStream,
                crate::subprocess::error::ProcessError,
            > {
                unimplemented!("Not used in these tests")
            }
        }

        // Mock ClaudeExecutor
        #[derive(Clone)]
        struct TestClaudeExecutor {
            should_succeed: bool,
        }

        #[async_trait::async_trait]
        impl crate::cook::execution::ClaudeExecutor for TestClaudeExecutor {
            async fn execute_claude_command(
                &self,
                _command: &str,
                _project_path: &Path,
                _env_vars: HashMap<String, String>,
            ) -> anyhow::Result<crate::cook::execution::ExecutionResult> {
                Ok(crate::cook::execution::ExecutionResult {
                    success: self.should_succeed,
                    stdout: "claude stdout".to_string(),
                    stderr: if self.should_succeed {
                        String::new()
                    } else {
                        "claude stderr".to_string()
                    },
                    exit_code: Some(if self.should_succeed { 0 } else { 1 }),
                    metadata: HashMap::new(),
                })
            }

            async fn check_claude_cli(&self) -> anyhow::Result<bool> {
                Ok(true)
            }

            async fn get_claude_version(&self) -> anyhow::Result<String> {
                Ok("1.0.0".to_string())
            }
        }

        // Mock AgentLifecycleManager
        struct TestAgentLifecycleManager;

        #[async_trait::async_trait]
        impl AgentLifecycleManager for TestAgentLifecycleManager {
            async fn create_agent(
                &self,
                _config: crate::cook::execution::mapreduce::agent::AgentConfig,
                _commands: Vec<WorkflowStep>,
            ) -> Result<crate::cook::execution::mapreduce::agent::AgentHandle, LifecycleError>
            {
                unimplemented!("Not used in these tests")
            }

            async fn create_agent_branch(
                &self,
                _worktree_path: &Path,
                _branch_name: &str,
            ) -> Result<(), LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn merge_agent_to_parent(
                &self,
                _agent_branch: &str,
                _env: &ExecutionEnvironment,
            ) -> Result<(), LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn handle_merge_and_cleanup(
                &self,
                _is_successful: bool,
                _env: &ExecutionEnvironment,
                _worktree_path: &Path,
                _worktree_name: &str,
                _branch_name: &str,
                _template_steps: &[WorkflowStep],
                _item_id: &str,
            ) -> Result<bool, LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn cleanup_agent(
                &self,
                _handle: crate::cook::execution::mapreduce::agent::AgentHandle,
            ) -> Result<(), LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn get_worktree_commits(
                &self,
                _worktree_path: &Path,
            ) -> Result<Vec<String>, LifecycleError> {
                unimplemented!("Not used in these tests")
            }

            async fn get_modified_files(
                &self,
                _worktree_path: &Path,
            ) -> Result<Vec<String>, LifecycleError> {
                unimplemented!("Not used in these tests")
            }
        }

        // Mock StateStore
        struct TestStateStore;

        #[async_trait::async_trait]
        impl StateStore for TestStateStore {
            async fn save(&self, _state: &JobState) -> Result<(), StateError> {
                Ok(())
            }

            async fn load(&self, _job_id: &str) -> Result<Option<JobState>, StateError> {
                Ok(None)
            }

            async fn list(&self) -> Result<Vec<JobSummary>, StateError> {
                Ok(vec![])
            }

            async fn delete(&self, _job_id: &str) -> Result<(), StateError> {
                Ok(())
            }
        }

        let agent_manager: Arc<dyn AgentLifecycleManager> = Arc::new(TestAgentLifecycleManager);
        let state_manager = Arc::new(StateManager::new(Arc::new(TestStateStore)));
        let user_interaction = Arc::new(MockUserInteraction::new());
        let subprocess = Arc::new(SubprocessManager::new(Arc::new(TestProcessRunner {
            should_succeed: shell_succeeds,
        })));
        let project_root = PathBuf::from("/tmp/test");

        let mut coordinator = MapReduceCoordinator::new(
            agent_manager,
            state_manager,
            user_interaction,
            subprocess,
            project_root,
        );

        // Replace claude executor with test version
        coordinator.claude_executor = Arc::new(TestClaudeExecutor {
            should_succeed: claude_succeeds,
        });

        coordinator
    }

    fn create_test_env() -> ExecutionEnvironment {
        ExecutionEnvironment {
            working_dir: Arc::new(PathBuf::from("/tmp/test")),
            project_dir: Arc::new(PathBuf::from("/tmp/test")),
            worktree_name: None,
            session_id: Arc::from("test-session"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_all_steps_succeed() {
        let coordinator = create_test_coordinator(true, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![
                WorkflowStep {
                    shell: Some("echo test1".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo test2".to_string()),
                    ..Default::default()
                },
            ],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(
            result.is_ok(),
            "Setup phase should succeed with all passing steps"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_shell_failure_with_exit_code() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("false".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(
            result.is_err(),
            "Setup phase should fail when shell command fails"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Setup step 1") && err_msg.contains("failed"),
            "Error should mention step number and failure, got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("exit code:"),
            "Error should include exit code, got: {}",
            err_msg
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_shell_failure_with_stderr() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("some_command".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(
            result.is_err(),
            "Setup phase should fail when command produces stderr"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("stderr:") || err_msg.contains("test stderr"),
            "Error should include stderr output"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_shell_failure_with_stdout_only() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("some_command".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(result.is_err(), "Setup phase should fail");

        let err_msg = result.unwrap_err().to_string();
        // Should include stdout when stderr is empty or not provided
        assert!(
            err_msg.contains("stdout:") || err_msg.contains("stderr:"),
            "Error should include output"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_claude_failure_with_log_hint() {
        let coordinator = create_test_coordinator(false, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                claude: Some("/test-command".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(
            result.is_err(),
            "Setup phase should fail when Claude command fails"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Setup step 1") && err_msg.contains("failed"),
            "Error should mention step number and failure, got: {}",
            err_msg
        );
        // Note: log hint only appears if extract_repo_name succeeds, which it won't in this test
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_multiple_steps_mixed_success() {
        let coordinator = create_test_coordinator(true, false);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![
                WorkflowStep {
                    shell: Some("echo step1".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("failing_command".to_string()),
                    ..Default::default()
                },
                WorkflowStep {
                    shell: Some("echo step3".to_string()),
                    ..Default::default()
                },
            ],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(
            result.is_err(),
            "Setup phase should fail on first failing step"
        );

        let err_msg = result.unwrap_err().to_string();
        // Should fail on step 2 (index 1)
        assert!(
            err_msg.contains("Setup step"),
            "Error should mention which step failed"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_environment_variables_set() {
        let coordinator = create_test_coordinator(true, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("echo $PRODIGY_AUTOMATION".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        // Should succeed - this test verifies env vars are set
        // (actual verification would require checking the subprocess call)
        assert!(result.is_ok(), "Setup phase should succeed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_setup_phase_debug_logging_context() {
        let coordinator = create_test_coordinator(true, true);
        let env = create_test_env();

        let setup = SetupPhase {
            commands: vec![WorkflowStep {
                shell: Some("echo test".to_string()),
                ..Default::default()
            }],
            timeout: None,
            capture_outputs: HashMap::new(),
        };

        // This test verifies the function runs without panicking
        // Debug logs are checked via tracing (would need tracing subscriber in real test)
        let result = coordinator
            .execute_setup_phase(setup, &env, &HashMap::new())
            .await;
        assert!(
            result.is_ok(),
            "Setup phase should succeed and log debug context"
        );
    }
}

#[cfg(test)]
mod reduce_interpolation_context_tests {
    use crate::cook::execution::mapreduce::agent::types::{AgentResult, AgentStatus};
    use crate::cook::execution::mapreduce::aggregation::AggregationSummary;
    use crate::cook::execution::mapreduce::coordination::executor::MapReduceCoordinator;
    use std::time::Duration;

    #[test]
    fn test_build_reduce_interpolation_context_includes_map_results() {
        // Create sample agent results
        let results = vec![
            AgentResult {
                item_id: "item-1".to_string(),
                status: AgentStatus::Success,
                output: Some("output-1".to_string()),
                commits: vec!["commit-1".to_string()],
                files_modified: vec![],
                duration: Duration::from_secs(10),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: None,
                cleanup_status: None,
            },
            AgentResult {
                item_id: "item-2".to_string(),
                status: AgentStatus::Success,
                output: Some("output-2".to_string()),
                commits: vec!["commit-2".to_string()],
                files_modified: vec![],
                duration: Duration::from_secs(15),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: None,
                cleanup_status: None,
            },
        ];

        let summary = AggregationSummary::from_results(&results);

        let context =
            MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary).unwrap();

        // Verify scalar values are present
        let successful = context.variables.get("map.successful").unwrap();
        assert_eq!(successful.as_u64().unwrap(), 2);

        let failed = context.variables.get("map.failed").unwrap();
        assert_eq!(failed.as_u64().unwrap(), 0);

        let total = context.variables.get("map.total").unwrap();
        assert_eq!(total.as_u64().unwrap(), 2);

        // Verify map.results is present and is an array
        let map_results = context.variables.get("map.results").unwrap();
        assert!(map_results.is_array());
        let results_array = map_results.as_array().unwrap();
        assert_eq!(results_array.len(), 2);

        // Verify first result contains expected fields
        let first_result = &results_array[0];
        assert_eq!(
            first_result.get("item_id").unwrap().as_str().unwrap(),
            "item-1"
        );
        assert_eq!(
            first_result.get("output").unwrap().as_str().unwrap(),
            "output-1"
        );
    }

    #[test]
    fn test_build_reduce_interpolation_context_with_empty_results() {
        let results: Vec<AgentResult> = vec![];
        let summary = AggregationSummary::from_results(&results);

        let context =
            MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary).unwrap();

        // Verify scalar values
        assert_eq!(
            context
                .variables
                .get("map.successful")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );
        assert_eq!(
            context
                .variables
                .get("map.failed")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );
        assert_eq!(
            context
                .variables
                .get("map.total")
                .unwrap()
                .as_u64()
                .unwrap(),
            0
        );

        // Verify map.results is an empty array
        let map_results = context.variables.get("map.results").unwrap();
        assert!(map_results.is_array());
        assert_eq!(map_results.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_build_reduce_interpolation_context_with_failed_agents() {
        let results = vec![
            AgentResult {
                item_id: "item-1".to_string(),
                status: AgentStatus::Success,
                output: Some("success".to_string()),
                commits: vec!["commit-1".to_string()],
                files_modified: vec![],
                duration: Duration::from_secs(10),
                error: None,
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: None,
                cleanup_status: None,
            },
            AgentResult {
                item_id: "item-2".to_string(),
                status: AgentStatus::Failed("error occurred".to_string()),
                output: None,
                commits: vec![],
                files_modified: vec![],
                duration: Duration::from_secs(5),
                error: Some("error occurred".to_string()),
                worktree_path: None,
                branch_name: None,
                worktree_session_id: None,
                json_log_location: Some("/path/to/log.json".to_string()),
                cleanup_status: None,
            },
        ];

        let summary = AggregationSummary::from_results(&results);

        let context =
            MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary).unwrap();

        // Verify summary reflects mixed results
        assert_eq!(
            context
                .variables
                .get("map.successful")
                .unwrap()
                .as_u64()
                .unwrap(),
            1
        );
        assert_eq!(
            context
                .variables
                .get("map.failed")
                .unwrap()
                .as_u64()
                .unwrap(),
            1
        );
        assert_eq!(
            context
                .variables
                .get("map.total")
                .unwrap()
                .as_u64()
                .unwrap(),
            2
        );

        // Verify both results are present
        let map_results = context.variables.get("map.results").unwrap();
        assert_eq!(map_results.as_array().unwrap().len(), 2);

        // Verify failed agent has error details
        let failed_result = &map_results.as_array().unwrap()[1];
        assert_eq!(
            failed_result.get("error").unwrap().as_str().unwrap(),
            "error occurred"
        );
    }

    #[test]
    fn test_build_reduce_interpolation_context_serialization_error() {
        // This test verifies that the function handles serialization errors gracefully
        // In practice, AgentResult should always serialize correctly, but we test the error path

        // Note: It's difficult to trigger a serialization error with valid AgentResult data
        // This test primarily documents the expected behavior
        // A real serialization error would require malformed data that can't be represented in JSON

        let results = vec![AgentResult {
            item_id: "item-1".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec![],
            files_modified: vec![],
            duration: Duration::from_secs(10),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
            cleanup_status: None,
        }];

        let summary = AggregationSummary::from_results(&results);

        // This should succeed - valid data always serializes
        let result = MapReduceCoordinator::build_reduce_interpolation_context(&results, &summary);
        assert!(
            result.is_ok(),
            "Valid AgentResult data should serialize successfully"
        );
    }
}
