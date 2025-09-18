//! Unit tests for command execution modules

#[cfg(test)]
mod executor_tests {
    use crate::cook::execution::mapreduce::command::{
        CommandError, CommandExecutor, CommandResult, CommandRouter, ExecutionContext,
    };
    use crate::cook::workflow::{CommandType, WorkflowStep};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    /// Mock executor for testing
    struct MockExecutor {
        supported_type: String,
        should_succeed: bool,
    }

    #[async_trait]
    impl CommandExecutor for MockExecutor {
        async fn execute(
            &self,
            _step: &WorkflowStep,
            _context: &ExecutionContext,
        ) -> Result<CommandResult, CommandError> {
            if self.should_succeed {
                Ok(CommandResult {
                    output: Some("Mock output".to_string()),
                    exit_code: 0,
                    variables: HashMap::new(),
                    duration: Duration::from_secs(1),
                    success: true,
                    stderr: String::new(),
                })
            } else {
                Err(CommandError::ExecutionFailed(
                    "Mock execution failed".to_string(),
                ))
            }
        }

        fn supports(&self, command_type: &CommandType) -> bool {
            match command_type {
                CommandType::Shell(cmd) => {
                    self.supported_type == "shell" && cmd.starts_with("echo")
                }
                CommandType::Claude(_cmd) => self.supported_type == "claude",
                _ => false,
            }
        }
    }

    #[tokio::test]
    async fn test_command_router_execute_success() {
        let mut router = CommandRouter::new();

        // Register mock executor
        let executor = MockExecutor {
            supported_type: "shell".to_string(),
            should_succeed: true,
        };
        router.register("shell".to_string(), Arc::new(executor));

        // Create test step
        let step = WorkflowStep {
            shell: Some("echo test".to_string()),
            ..Default::default()
        };

        // Create execution context
        let context = ExecutionContext::new(
            std::path::PathBuf::from("/test"),
            "test-worktree".to_string(),
            "test-item".to_string(),
        );

        // Execute
        let result = router.execute(&step, &context).await;
        assert!(result.is_ok());

        let command_result = result.unwrap();
        assert!(command_result.success);
        assert_eq!(command_result.exit_code, 0);
        assert_eq!(command_result.output, Some("Mock output".to_string()));
    }

    #[tokio::test]
    async fn test_command_router_no_executor_found() {
        let router = CommandRouter::new();

        // Create test step with no registered executor
        let step = WorkflowStep {
            shell: Some("unsupported command".to_string()),
            ..Default::default()
        };

        let context = ExecutionContext::new(
            std::path::PathBuf::from("/test"),
            "test-worktree".to_string(),
            "test-item".to_string(),
        );

        // Execute should fail
        let result = router.execute(&step, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_command_result_to_step_result_conversion() {
        let command_result = CommandResult {
            output: Some("test output".to_string()),
            exit_code: 0,
            variables: HashMap::new(),
            duration: Duration::from_secs(1),
            success: true,
            stderr: "test stderr".to_string(),
        };

        let step_result: crate::cook::workflow::StepResult = command_result.into();

        assert!(step_result.success);
        assert_eq!(step_result.exit_code, Some(0));
        assert_eq!(step_result.stdout, "test output");
        assert_eq!(step_result.stderr, "test stderr");
    }
}

#[cfg(test)]
mod types_tests {
    use crate::cook::execution::mapreduce::command::types::{
        collect_command_types, determine_command_type, validate_command_count,
    };
    use crate::cook::workflow::{CommandType, WorkflowStep};

    #[test]
    fn test_determine_single_command_type() {
        let step = WorkflowStep {
            shell: Some("echo test".to_string()),
            ..Default::default()
        };

        let result = determine_command_type(&step);
        assert!(result.is_ok());

        match result.unwrap() {
            CommandType::Shell(cmd) => assert_eq!(cmd, "echo test"),
            _ => panic!("Expected Shell command type"),
        }
    }

    #[test]
    fn test_determine_no_command_type() {
        let step = WorkflowStep::default();

        let result = determine_command_type(&step);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("No command type specified"));
    }

    #[test]
    fn test_determine_multiple_command_types() {
        let step = WorkflowStep {
            shell: Some("echo test".to_string()),
            claude: Some("claude command".to_string()),
            ..Default::default()
        };

        let result = determine_command_type(&step);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("Multiple commands specified"));
    }

    #[test]
    fn test_collect_command_types() {
        let step = WorkflowStep {
            shell: Some("shell cmd".to_string()),
            claude: Some("claude cmd".to_string()),
            ..Default::default()
        };

        let commands = collect_command_types(&step);
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_validate_command_count_success() {
        let commands = vec![CommandType::Shell("echo test".to_string())];
        let result = validate_command_count(&commands);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_command_count_empty() {
        let commands = Vec::new();
        let result = validate_command_count(&commands);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_command_count_multiple() {
        let commands = vec![
            CommandType::Shell("cmd1".to_string()),
            CommandType::Claude("cmd2".to_string()),
        ];
        let result = validate_command_count(&commands);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod context_tests {
    use crate::cook::execution::mapreduce::command::context::ExecutionContext;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_execution_context_creation() {
        let context = ExecutionContext::new(
            PathBuf::from("/test/path"),
            "test-worktree".to_string(),
            "test-item".to_string(),
        );

        assert_eq!(context.worktree_path, PathBuf::from("/test/path"));
        assert_eq!(context.worktree_name, "test-worktree");
        assert_eq!(context.item_id, "test-item");
        assert!(context.variables.is_empty());
        assert!(context.captured_outputs.is_empty());
        assert!(context.environment.is_empty());
    }

    #[test]
    fn test_execution_context_with_variable() {
        let context = ExecutionContext::new(
            PathBuf::from("/test"),
            "worktree".to_string(),
            "item".to_string(),
        )
        .with_variable("key1".to_string(), "value1".to_string())
        .with_variable("key2".to_string(), "value2".to_string());

        assert_eq!(context.get_variable("key1"), Some(&"value1".to_string()));
        assert_eq!(context.get_variable("key2"), Some(&"value2".to_string()));
        assert_eq!(context.get_variable("key3"), None);
    }

    #[test]
    fn test_execution_context_with_variables() {
        let mut vars = HashMap::new();
        vars.insert("var1".to_string(), "val1".to_string());
        vars.insert("var2".to_string(), "val2".to_string());

        let context = ExecutionContext::new(
            PathBuf::from("/test"),
            "worktree".to_string(),
            "item".to_string(),
        )
        .with_variables(vars);

        assert_eq!(context.variables.len(), 2);
        assert_eq!(context.get_variable("var1"), Some(&"val1".to_string()));
        assert_eq!(context.get_variable("var2"), Some(&"val2".to_string()));
    }

    #[test]
    fn test_execution_context_with_captured_output() {
        let context = ExecutionContext::new(
            PathBuf::from("/test"),
            "worktree".to_string(),
            "item".to_string(),
        )
        .with_captured_output("output1".to_string(), "data1".to_string());

        assert_eq!(
            context.get_captured_output("output1"),
            Some(&"data1".to_string())
        );
        assert_eq!(context.get_captured_output("output2"), None);
    }

    #[test]
    fn test_execution_context_with_env() {
        let context = ExecutionContext::new(
            PathBuf::from("/test"),
            "worktree".to_string(),
            "item".to_string(),
        )
        .with_env("ENV_VAR".to_string(), "env_value".to_string());

        assert_eq!(context.get_env("ENV_VAR"), Some(&"env_value".to_string()));
        assert_eq!(context.get_env("OTHER_VAR"), None);
    }
}
