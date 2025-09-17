//! Tests for step validation functionality

use super::executor::WorkflowStep;
use super::step_validation::*;
use crate::cook::execution::ExecutionContext;
use crate::cook::execution::{CommandExecutor, ExecutionResult};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// Mock command executor for testing
struct MockCommandExecutor {
    responses: HashMap<String, ExecutionResult>,
}

impl MockCommandExecutor {
    fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    fn with_response(mut self, command: &str, result: ExecutionResult) -> Self {
        self.responses.insert(command.to_string(), result);
        self
    }
}

#[async_trait::async_trait]
impl CommandExecutor for MockCommandExecutor {
    async fn execute(
        &self,
        _command_type: &str,
        args: &[String],
        _context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        let command = args.join(" ");
        self.responses
            .get(&command)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No mock response for command: {}", command))
    }
}

#[tokio::test]
async fn test_single_validation_success() {
    let mock_executor = Arc::new(MockCommandExecutor::new().with_response(
        "cargo test",
        ExecutionResult {
            success: true,
            stdout: "All tests passed".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        },
    ));

    let validation_executor = StepValidationExecutor::new(mock_executor);
    let validation_spec = StepValidationSpec::Single("cargo test".to_string());
    let context = ExecutionContext {
        working_directory: std::path::PathBuf::from("/test"),
        env_vars: HashMap::new(),
        capture_output: true,
        timeout_seconds: None,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    };

    let result = validation_executor
        .validate_step(&validation_spec, &context, "test step")
        .await
        .unwrap();

    assert!(result.passed);
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.attempts, 1);
}

#[tokio::test]
async fn test_single_validation_failure() {
    let mock_executor = Arc::new(MockCommandExecutor::new().with_response(
        "cargo test",
        ExecutionResult {
            success: false,
            stdout: "Test failed".to_string(),
            stderr: "Error".to_string(),
            exit_code: Some(1),
        },
    ));

    let validation_executor = StepValidationExecutor::new(mock_executor);
    let validation_spec = StepValidationSpec::Single("cargo test".to_string());
    let context = ExecutionContext {
        working_directory: std::path::PathBuf::from("/test"),
        env_vars: HashMap::new(),
        capture_output: true,
        timeout_seconds: None,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    };

    let result = validation_executor
        .validate_step(&validation_spec, &context, "test step")
        .await
        .unwrap();

    assert!(!result.passed);
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0].exit_code, 1);
}

#[tokio::test]
async fn test_multiple_validations_all_pass() {
    let mock_executor = Arc::new(
        MockCommandExecutor::new()
            .with_response(
                "cargo test",
                ExecutionResult {
                    success: true,
                    stdout: "Tests passed".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                },
            )
            .with_response(
                "cargo clippy",
                ExecutionResult {
                    success: true,
                    stdout: "No warnings".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                },
            ),
    );

    let validation_executor = StepValidationExecutor::new(mock_executor);
    let validation_spec =
        StepValidationSpec::Multiple(vec!["cargo test".to_string(), "cargo clippy".to_string()]);
    let context = ExecutionContext {
        working_directory: std::path::PathBuf::from("/test"),
        env_vars: HashMap::new(),
        capture_output: true,
        timeout_seconds: None,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    };

    let result = validation_executor
        .validate_step(&validation_spec, &context, "test step")
        .await
        .unwrap();

    assert!(result.passed);
    assert_eq!(result.results.len(), 2);
}

#[tokio::test]
async fn test_multiple_validations_one_fails() {
    let mock_executor = Arc::new(
        MockCommandExecutor::new()
            .with_response(
                "cargo test",
                ExecutionResult {
                    success: true,
                    stdout: "Tests passed".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                },
            )
            .with_response(
                "cargo clippy",
                ExecutionResult {
                    success: false,
                    stdout: "Warnings found".to_string(),
                    stderr: "Error".to_string(),
                    exit_code: Some(1),
                },
            ),
    );

    let validation_executor = StepValidationExecutor::new(mock_executor);
    let validation_spec =
        StepValidationSpec::Multiple(vec!["cargo test".to_string(), "cargo clippy".to_string()]);
    let context = ExecutionContext {
        working_directory: std::path::PathBuf::from("/test"),
        env_vars: HashMap::new(),
        capture_output: true,
        timeout_seconds: None,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    };

    let result = validation_executor
        .validate_step(&validation_spec, &context, "test step")
        .await
        .unwrap();

    assert!(!result.passed); // Default is All criteria, so one failure means overall failure
    assert_eq!(result.results.len(), 2);
}

#[tokio::test]
async fn test_validation_with_any_criteria() {
    let mock_executor = Arc::new(
        MockCommandExecutor::new()
            .with_response(
                "test1.sh",
                ExecutionResult {
                    success: false,
                    stdout: "Failed".to_string(),
                    stderr: String::new(),
                    exit_code: Some(1),
                },
            )
            .with_response(
                "test2.sh",
                ExecutionResult {
                    success: true,
                    stdout: "Passed".to_string(),
                    stderr: String::new(),
                    exit_code: Some(0),
                },
            ),
    );

    let validation_executor = StepValidationExecutor::new(mock_executor);
    let validation_spec = StepValidationSpec::Detailed(StepValidationConfig {
        commands: vec![
            ValidationCommand {
                command: "test1.sh".to_string(),
                expect_output: None,
                expect_exit_code: 0,
                command_type: Some(ValidationCommandType::Shell),
            },
            ValidationCommand {
                command: "test2.sh".to_string(),
                expect_output: None,
                expect_exit_code: 0,
                command_type: Some(ValidationCommandType::Shell),
            },
        ],
        success_criteria: SuccessCriteria::Any,
        max_attempts: 1,
        retry_delay: 0,
    });
    let context = ExecutionContext {
        working_directory: std::path::PathBuf::from("/test"),
        env_vars: HashMap::new(),
        capture_output: true,
        timeout_seconds: None,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    };

    let result = validation_executor
        .validate_step(&validation_spec, &context, "test step")
        .await
        .unwrap();

    assert!(result.passed); // Any criteria: one success is enough
}

#[tokio::test]
async fn test_validation_with_expected_output() {
    let mock_executor = Arc::new(MockCommandExecutor::new().with_response(
        "version.sh",
        ExecutionResult {
            success: true,
            stdout: "Version 1.2.3".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        },
    ));

    let validation_executor = StepValidationExecutor::new(mock_executor);
    let validation_spec = StepValidationSpec::Detailed(StepValidationConfig {
        commands: vec![ValidationCommand {
            command: "version.sh".to_string(),
            expect_output: Some(r"Version \d+\.\d+\.\d+".to_string()),
            expect_exit_code: 0,
            command_type: Some(ValidationCommandType::Shell),
        }],
        success_criteria: SuccessCriteria::All,
        max_attempts: 1,
        retry_delay: 0,
    });
    let context = ExecutionContext {
        working_directory: std::path::PathBuf::from("/test"),
        env_vars: HashMap::new(),
        capture_output: true,
        timeout_seconds: None,
        stdin: None,
        capture_streaming: false,
        streaming_config: None,
    };

    let result = validation_executor
        .validate_step(&validation_spec, &context, "test step")
        .await
        .unwrap();

    assert!(result.passed);
}

#[tokio::test]
async fn test_claude_validation_command() {
    let spec = StepValidationSpec::Single("claude: /check-quality".to_string());
    let commands = spec.to_validation_commands().unwrap();

    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "/check-quality");
    assert_eq!(
        commands[0].command_type,
        Some(ValidationCommandType::Claude)
    );
}

#[test]
fn test_workflow_step_with_validation() {
    let yaml = r#"
name: "Test with validation"
claude: "/refactor module.py"
step_validate: "python -m pytest tests/test_module.py"
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml).unwrap();
    assert!(step.step_validate.is_some());

    if let Some(StepValidationSpec::Single(cmd)) = &step.step_validate {
        assert_eq!(cmd, "python -m pytest tests/test_module.py");
    } else {
        panic!("Expected Single validation spec");
    }
}

#[test]
fn test_workflow_step_with_multiple_validations() {
    let yaml = r#"
shell: "npm build"
step_validate:
  - "npm test"
  - "npm run lint"
  - "claude: /check-api-compatibility"
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml).unwrap();
    assert!(step.step_validate.is_some());

    if let Some(StepValidationSpec::Multiple(cmds)) = &step.step_validate {
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0], "npm test");
        assert_eq!(cmds[1], "npm run lint");
        assert_eq!(cmds[2], "claude: /check-api-compatibility");
    } else {
        panic!("Expected Multiple validation spec");
    }
}

#[test]
fn test_workflow_step_with_detailed_validation() {
    let yaml = r#"
shell: "./deploy.sh"
step_validate:
  commands:
    - command: "curl -f https://api.example.com/health"
      expect_exit_code: 0
    - command: "check-deployment.sh"
      expect_output: "Deployment successful"
  max_attempts: 5
  retry_delay: 10
  success_criteria: all
"#;

    let step: WorkflowStep = serde_yaml::from_str(yaml).unwrap();
    assert!(step.step_validate.is_some());

    if let Some(StepValidationSpec::Detailed(config)) = &step.step_validate {
        assert_eq!(config.commands.len(), 2);
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.retry_delay, 10);
        assert!(matches!(config.success_criteria, SuccessCriteria::All));
    } else {
        panic!("Expected Detailed validation spec");
    }
}
