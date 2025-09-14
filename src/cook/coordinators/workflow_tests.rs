//! Unit tests for workflow coordinator

#[cfg(test)]
mod tests {
    use crate::config::command::{
        Command, SimpleCommand, TestCommand, WorkflowCommand, WorkflowStepCommand,
    };
    use crate::cook::coordinators::workflow::{
        DefaultWorkflowCoordinator, WorkflowContext, WorkflowCoordinator,
    };
    use crate::cook::interaction::UserInteraction;
    use crate::cook::workflow::{CaptureOutput, WorkflowStep};
    use crate::testing::mocks::MockWorkflowExecutor;
    use anyhow::Result;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Mock user interaction for testing
    struct MockUserInteraction {
        yes_no_responses: Vec<bool>,
        response_index: std::sync::Mutex<usize>,
        messages: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl MockUserInteraction {
        fn new(responses: Vec<bool>) -> Self {
            Self {
                yes_no_responses: responses,
                response_index: std::sync::Mutex::new(0),
                messages: Arc::new(std::sync::Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<String> {
            self.messages.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl UserInteraction for MockUserInteraction {
        async fn prompt_yes_no(&self, _message: &str) -> Result<bool> {
            let mut index = self.response_index.lock().unwrap();
            let response = self.yes_no_responses.get(*index).copied().unwrap_or(false);
            *index += 1;
            Ok(response)
        }

        async fn prompt_text(&self, _message: &str, _default: Option<&str>) -> Result<String> {
            Ok("test input".to_string())
        }

        fn display_progress(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn display_info(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn display_warning(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn display_error(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn display_success(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn display_action(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn display_metric(&self, label: &str, value: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(format!("{}: {}", label, value));
        }

        fn display_status(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }

        fn start_spinner(
            &self,
            _message: &str,
        ) -> Box<dyn crate::cook::interaction::SpinnerHandle> {
            struct MockSpinner;
            impl crate::cook::interaction::SpinnerHandle for MockSpinner {
                fn update_message(&mut self, _message: &str) {}
                fn success(&mut self, _message: &str) {}
                fn fail(&mut self, _message: &str) {}
            }
            Box::new(MockSpinner)
        }

        fn iteration_start(&self, _current: u32, _total: u32) {}

        fn iteration_end(&self, _current: u32, _duration: std::time::Duration, _success: bool) {}

        fn step_start(&self, _step: u32, _total: u32, _description: &str) {}

        fn step_end(&self, _step: u32, _success: bool) {}

        fn command_output(
            &self,
            _output: &str,
            _verbosity: crate::cook::interaction::VerbosityLevel,
        ) {
        }

        fn debug_output(
            &self,
            _message: &str,
            _min_verbosity: crate::cook::interaction::VerbosityLevel,
        ) {
        }

        fn verbosity(&self) -> crate::cook::interaction::VerbosityLevel {
            crate::cook::interaction::VerbosityLevel::Normal
        }
    }

    #[tokio::test]
    async fn test_workflow_context_creation() {
        let context = WorkflowContext {
            iteration: 1,
            max_iterations: 10,
            variables: HashMap::new(),
        };

        assert_eq!(context.iteration, 1);
        assert_eq!(context.max_iterations, 10);
        assert!(context.variables.is_empty());
    }

    #[tokio::test]
    async fn test_workflow_context_with_variables() {
        let mut variables = HashMap::new();
        variables.insert("key".to_string(), "value".to_string());

        let context = WorkflowContext {
            iteration: 5,
            max_iterations: 10,
            variables,
        };

        assert_eq!(context.iteration, 5);
        assert_eq!(context.variables.get("key"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn test_execute_step() {
        // Setup
        let workflow_executor: Arc<dyn crate::cook::workflow::WorkflowExecutor> =
            Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor.clone(), user_interaction.clone());

        let step = WorkflowStep {
            name: Some("test step".to_string()),
            command: Some("/test-command".to_string()),
            claude: None,
            shell: None,
            test: None,
            goal_seek: None,
            foreach: None,
            capture_output: CaptureOutput::Disabled,
            timeout: None,
            working_dir: None,
            env: HashMap::new(),
            on_failure: None,
            on_success: None,
            on_exit_code: HashMap::new(),
            commit_required: false,
            validate: None,
            handler: None,
            when: None,
        };

        let context = WorkflowContext {
            iteration: 1,
            max_iterations: 5,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.execute_step(&step, &context).await;

        // Verify
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert!(outputs.is_empty()); // Default implementation returns empty

        // Verify progress was displayed
        let messages = user_interaction.get_messages();
        assert!(messages.iter().any(|m| m.contains("test step")));
    }

    #[tokio::test]
    async fn test_should_continue_within_limit() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator = DefaultWorkflowCoordinator::new(workflow_executor, user_interaction);

        let context = WorkflowContext {
            iteration: 3,
            max_iterations: 5,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.should_continue(&context).await;

        // Verify
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_should_continue_at_limit() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator = DefaultWorkflowCoordinator::new(workflow_executor, user_interaction);

        let context = WorkflowContext {
            iteration: 5,
            max_iterations: 5,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.should_continue(&context).await;

        // Verify
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_should_continue_beyond_limit() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        let context = WorkflowContext {
            iteration: 6,
            max_iterations: 5,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.should_continue(&context).await;

        // Verify
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Verify message was displayed
        let messages = user_interaction.get_messages();
        assert!(messages.iter().any(|m| m.contains("maximum iterations")));
    }

    #[tokio::test]
    async fn test_prompt_user() {
        // Setup
        let workflow_executor: Arc<dyn crate::cook::workflow::WorkflowExecutor> =
            Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![true, false, true]));

        let coordinator = DefaultWorkflowCoordinator::new(workflow_executor, user_interaction);

        // Test multiple prompts
        let result1 = coordinator.prompt_user("Continue?", true).await;
        assert!(result1.is_ok());
        assert!(result1.unwrap());

        let result2 = coordinator.prompt_user("Proceed?", true).await;
        assert!(result2.is_ok());
        assert!(!result2.unwrap());

        let result3 = coordinator.prompt_user("Confirm?", false).await;
        assert!(result3.is_ok());
        assert!(result3.unwrap());
    }

    #[tokio::test]
    async fn test_display_progress() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        // Test
        coordinator.display_progress("Step 1 complete");
        coordinator.display_progress("Step 2 in progress");
        coordinator.display_progress("Step 3 starting");

        // Verify
        let messages = user_interaction.get_messages();
        assert_eq!(messages.len(), 3);
        assert!(messages[0].contains("Step 1"));
        assert!(messages[1].contains("Step 2"));
        assert!(messages[2].contains("Step 3"));
    }

    #[tokio::test]
    async fn test_execute_workflow_simple_commands() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        let commands = vec![
            WorkflowCommand::Simple("/test-command-1".to_string()),
            WorkflowCommand::Simple("/test-command-2".to_string()),
        ];

        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: 2,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.execute_workflow(&commands, &mut context).await;

        // Verify
        assert!(result.is_ok());
        assert_eq!(context.iteration, 3); // Should have executed 2 iterations then stopped

        // Verify progress messages
        let messages = user_interaction.get_messages();
        assert!(messages.iter().any(|m| m.contains("iteration 1/2")));
        assert!(messages.iter().any(|m| m.contains("iteration 2/2")));
    }

    #[tokio::test]
    async fn test_execute_workflow_with_variables() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        let commands = vec![
            WorkflowCommand::Simple("/analyze".to_string()),
            WorkflowCommand::Simple("/improve".to_string()),
        ];

        let mut variables = HashMap::new();
        variables.insert("target".to_string(), "performance".to_string());
        variables.insert("threshold".to_string(), "95".to_string());

        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: 1,
            variables,
        };

        // Test
        let result = coordinator.execute_workflow(&commands, &mut context).await;

        // Verify
        assert!(result.is_ok());
        assert_eq!(context.iteration, 2); // Should have executed 1 iteration then stopped
        assert_eq!(
            context.variables.get("target"),
            Some(&"performance".to_string())
        );
    }

    #[tokio::test]
    async fn test_execute_workflow_structured_commands() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        let commands = vec![
            WorkflowCommand::Structured(Box::new(Command {
                name: "analyze".to_string(),
                args: vec![
                    crate::config::command::CommandArg::Literal("--depth".to_string()),
                    crate::config::command::CommandArg::Literal("3".to_string()),
                ],
                options: HashMap::new(),
                metadata: crate::config::command::CommandMetadata {
                    retries: None,
                    timeout: None,
                    continue_on_error: None,
                    env: HashMap::new(),
                    commit_required: true,
                    analysis: None,
                },
                id: None,
                outputs: None,
                analysis: None,
            })),
            WorkflowCommand::SimpleObject(SimpleCommand {
                name: "optimize".to_string(),
                commit_required: None,
                args: None,
                analysis: None,
            }),
        ];

        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: 1,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.execute_workflow(&commands, &mut context).await;

        // Verify
        assert!(result.is_ok());
        assert_eq!(context.iteration, 2);

        // Verify commands were processed
        let messages = user_interaction.get_messages();
        assert!(!messages.is_empty());
    }

    #[tokio::test]
    async fn test_execute_workflow_workflow_step_commands() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        let commands = vec![
            WorkflowCommand::WorkflowStep(Box::new(WorkflowStepCommand {
                analyze: None,
                claude: Some("/prodigy-analyze".to_string()),
                shell: None,
                test: None,
                goal_seek: None,
                foreach: None,
                id: Some("claude-analysis".to_string()),
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: false,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
                when: None,
            })),
            WorkflowCommand::WorkflowStep(Box::new(WorkflowStepCommand {
                analyze: None,
                claude: None,
                shell: Some("cargo test".to_string()),
                test: None,
                goal_seek: None,
                foreach: None,
                id: Some("shell-test".to_string()),
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: false,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
                when: None,
            })),
            WorkflowCommand::WorkflowStep(Box::new(WorkflowStepCommand {
                analyze: None,
                claude: None,
                shell: None,
                test: Some(TestCommand {
                    command: "cargo build".to_string(),
                    on_failure: None,
                }),
                goal_seek: None,
                foreach: None,
                id: Some("test-command".to_string()),
                commit_required: false,
                analysis: None,
                outputs: None,
                capture_output: false,
                on_failure: None,
                on_success: None,
                validate: None,
                timeout: None,
                when: None,
            })),
        ];

        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: 1,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.execute_workflow(&commands, &mut context).await;

        // Verify
        assert!(result.is_ok());
        assert_eq!(context.iteration, 2);

        // Verify all command types were processed
        let messages = user_interaction.get_messages();
        assert!(messages.len() >= 3); // At least one message per command
    }

    #[tokio::test]
    async fn test_execute_workflow_stops_at_max_iterations() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator =
            DefaultWorkflowCoordinator::new(workflow_executor, user_interaction.clone());

        let commands = vec![WorkflowCommand::Simple("/command".to_string())];

        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: 3,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.execute_workflow(&commands, &mut context).await;

        // Verify
        assert!(result.is_ok());
        assert_eq!(context.iteration, 4); // Should stop after 3 iterations

        // Verify max iterations message
        let messages = user_interaction.get_messages();
        assert!(messages.iter().any(|m| m.contains("maximum iterations")));
    }

    #[tokio::test]
    async fn test_execute_workflow_empty_commands() {
        // Setup
        let workflow_executor = Arc::new(MockWorkflowExecutor::new());
        let user_interaction = Arc::new(MockUserInteraction::new(vec![]));

        let coordinator = DefaultWorkflowCoordinator::new(workflow_executor, user_interaction);

        let commands: Vec<WorkflowCommand> = vec![];

        let mut context = WorkflowContext {
            iteration: 0,
            max_iterations: 1,
            variables: HashMap::new(),
        };

        // Test
        let result = coordinator.execute_workflow(&commands, &mut context).await;

        // Verify - should still iterate but do nothing
        assert!(result.is_ok());
        assert_eq!(context.iteration, 2);
    }
}
