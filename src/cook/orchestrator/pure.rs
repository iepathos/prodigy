//! Pure orchestration logic (no I/O, no side effects)
//!
//! This module contains pure functions for workflow classification, validation,
//! and decision logic. All functions are testable without any I/O setup.

use crate::config::WorkflowConfig;
use stillwater::Validation;

/// Workflow type classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowType {
    /// MapReduce workflow
    MapReduce,
    /// Standard workflow with outputs
    StructuredWithOutputs,
    /// Workflow with arguments (iterative)
    WithArguments,
    /// Standard workflow
    Standard,
    /// Empty workflow (no steps)
    Empty,
}

/// Workflow validation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    /// Workflow has no steps
    NoSteps,
    /// Invalid environment variable configuration
    InvalidEnvVar(String),
    /// Invalid command syntax
    InvalidCommand(String),
    /// Invalid merge configuration
    InvalidMerge(String),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::NoSteps => write!(f, "Workflow has no steps"),
            WorkflowError::InvalidEnvVar(msg) => write!(f, "Invalid environment variable: {}", msg),
            WorkflowError::InvalidCommand(msg) => write!(f, "Invalid command: {}", msg),
            WorkflowError::InvalidMerge(msg) => write!(f, "Invalid merge configuration: {}", msg),
        }
    }
}

impl std::error::Error for WorkflowError {}

/// Classify workflow type based on configuration
///
/// Analyzes the workflow structure to determine its execution pattern:
/// - MapReduce: Contains map/reduce configuration
/// - StructuredWithOutputs: Has output capture configuration
/// - WithArguments: Designed for iterative execution with different args
/// - Empty: No commands
/// - Standard: Simple sequential execution
pub fn classify_workflow(config: &WorkflowConfig) -> WorkflowType {
    // Check if workflow is empty
    if config.commands.is_empty() {
        return WorkflowType::Empty;
    }

    // Check for MapReduce pattern (would be in a separate MapReduceConfig)
    // For now, we focus on regular workflow classification

    // Check for structured outputs (commands that capture output)
    let has_outputs = config.commands.iter().any(|cmd| {
        use crate::config::command::WorkflowCommand;
        matches!(cmd, WorkflowCommand::Structured(_))
    });

    if has_outputs {
        return WorkflowType::StructuredWithOutputs;
    }

    // Check if workflow is designed for arguments (multiple iterations)
    // This would be indicated by workflow name or structure
    if let Some(name) = &config.name {
        if name.contains("iterate") || name.contains("argument") {
            return WorkflowType::WithArguments;
        }
    }

    WorkflowType::Standard
}

/// Validate workflow configuration
///
/// Performs comprehensive validation:
/// - Workflow has at least one command
/// - Environment variables are well-formed (no empty names)
/// - Secret values are properly configured
/// - Merge workflows have valid commands
pub fn validate_workflow(config: &WorkflowConfig) -> Validation<(), Vec<WorkflowError>> {
    let mut errors = Vec::new();

    // Check for at least one command
    if config.commands.is_empty() {
        errors.push(WorkflowError::NoSteps);
    }

    // Validate environment variables
    if let Some(env) = &config.env {
        for (key, _value) in env {
            if key.is_empty() {
                errors.push(WorkflowError::InvalidEnvVar(
                    "Environment variable name cannot be empty".to_string(),
                ));
            }
            if key.contains('=') {
                errors.push(WorkflowError::InvalidEnvVar(format!(
                    "Environment variable name cannot contain '=': {}",
                    key
                )));
            }
        }
    }

    // Validate secrets
    if let Some(secrets) = &config.secrets {
        for (key, _secret) in secrets {
            if key.is_empty() {
                errors.push(WorkflowError::InvalidEnvVar(
                    "Secret variable name cannot be empty".to_string(),
                ));
            }
        }
    }

    // Validate merge workflow if present
    if let Some(merge) = &config.merge {
        if merge.commands.is_empty() {
            errors.push(WorkflowError::InvalidMerge(
                "Merge workflow must have at least one command".to_string(),
            ));
        }
    }

    if errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(errors)
    }
}

/// Iteration decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IterationDecision {
    /// Continue iterating
    Continue,
    /// Stop iteration with reason
    Stop(String),
    /// Ask user for decision
    AskUser,
}

/// Determine if iteration should continue (pure)
///
/// # Examples
///
/// ```
/// use prodigy::cook::orchestrator::pure::{should_continue_iteration, IterationDecision};
///
/// // Should continue
/// assert_eq!(
///     should_continue_iteration(5, 10, 3),
///     IterationDecision::Continue
/// );
///
/// // Reached max iterations
/// assert_eq!(
///     should_continue_iteration(10, 10, 3),
///     IterationDecision::Stop("Reached max iterations: 10".to_string())
/// );
///
/// // No files changed
/// assert_eq!(
///     should_continue_iteration(2, 10, 0),
///     IterationDecision::Stop("No files changed".to_string())
/// );
/// ```
pub fn should_continue_iteration(
    iteration: u32,
    max_iterations: u32,
    files_changed: usize,
) -> IterationDecision {
    if iteration >= max_iterations {
        IterationDecision::Stop(format!("Reached max iterations: {}", max_iterations))
    } else if files_changed == 0 {
        IterationDecision::Stop("No files changed".to_string())
    } else {
        IterationDecision::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::command::WorkflowCommand;
    use std::collections::HashMap;

    // Test fixtures
    fn simple_workflow() -> WorkflowConfig {
        WorkflowConfig {
            name: Some("test".to_string()),
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn empty_workflow() -> WorkflowConfig {
        WorkflowConfig {
            name: Some("empty".to_string()),
            commands: vec![],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn workflow_with_env() -> WorkflowConfig {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());
        WorkflowConfig {
            name: Some("with-env".to_string()),
            commands: vec![WorkflowCommand::Simple("echo $KEY".to_string())],
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn workflow_with_invalid_env() -> WorkflowConfig {
        let mut env = HashMap::new();
        env.insert("".to_string(), "value".to_string());
        WorkflowConfig {
            name: Some("invalid-env".to_string()),
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    fn workflow_with_equals_in_env() -> WorkflowConfig {
        let mut env = HashMap::new();
        env.insert("KEY=BAD".to_string(), "value".to_string());
        WorkflowConfig {
            name: Some("equals-env".to_string()),
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        }
    }

    // Classification tests
    #[test]
    fn test_classify_workflow_standard() {
        let config = simple_workflow();
        assert_eq!(classify_workflow(&config), WorkflowType::Standard);
    }

    #[test]
    fn test_classify_workflow_empty() {
        let config = empty_workflow();
        assert_eq!(classify_workflow(&config), WorkflowType::Empty);
    }

    #[test]
    fn test_classify_workflow_with_arguments_name() {
        let mut config = simple_workflow();
        config.name = Some("iterate-test".to_string());
        assert_eq!(classify_workflow(&config), WorkflowType::WithArguments);
    }

    #[test]
    fn test_classify_workflow_with_argument_keyword() {
        let mut config = simple_workflow();
        config.name = Some("argument-based".to_string());
        assert_eq!(classify_workflow(&config), WorkflowType::WithArguments);
    }

    // Validation tests
    #[test]
    fn test_validate_workflow_success() {
        let config = simple_workflow();
        let result = validate_workflow(&config);
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_workflow_no_steps() {
        let config = empty_workflow();
        let result = validate_workflow(&config);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(errors[0], WorkflowError::NoSteps));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_workflow_with_valid_env() {
        let config = workflow_with_env();
        let result = validate_workflow(&config);
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_workflow_empty_env_name() {
        let config = workflow_with_invalid_env();
        let result = validate_workflow(&config);
        match result {
            Validation::Failure(errors) => {
                assert!(!errors.is_empty());
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkflowError::InvalidEnvVar(_))));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_workflow_equals_in_env_name() {
        let config = workflow_with_equals_in_env();
        let result = validate_workflow(&config);
        match result {
            Validation::Failure(errors) => {
                assert!(!errors.is_empty());
                let has_equals_error = errors.iter().any(|e| {
                    if let WorkflowError::InvalidEnvVar(msg) = e {
                        msg.contains("=")
                    } else {
                        false
                    }
                });
                assert!(has_equals_error);
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_workflow_error_display() {
        let err = WorkflowError::NoSteps;
        assert_eq!(err.to_string(), "Workflow has no steps");

        let err = WorkflowError::InvalidEnvVar("test".to_string());
        assert_eq!(err.to_string(), "Invalid environment variable: test");

        let err = WorkflowError::InvalidCommand("bad".to_string());
        assert_eq!(err.to_string(), "Invalid command: bad");

        let err = WorkflowError::InvalidMerge("no cmds".to_string());
        assert_eq!(err.to_string(), "Invalid merge configuration: no cmds");
    }

    // Iteration decision tests
    #[test]
    fn test_should_continue_iteration_continue() {
        assert_eq!(
            should_continue_iteration(5, 10, 3),
            IterationDecision::Continue
        );
    }

    #[test]
    fn test_should_continue_iteration_max_reached() {
        let result = should_continue_iteration(10, 10, 3);
        match result {
            IterationDecision::Stop(msg) => assert!(msg.contains("max iterations")),
            _ => panic!("Expected Stop decision"),
        }
    }

    #[test]
    fn test_should_continue_iteration_no_changes() {
        let result = should_continue_iteration(2, 10, 0);
        match result {
            IterationDecision::Stop(msg) => assert!(msg.contains("No files changed")),
            _ => panic!("Expected Stop decision"),
        }
    }

    #[test]
    fn test_should_continue_iteration_boundary_conditions() {
        // First iteration with changes
        assert_eq!(
            should_continue_iteration(0, 10, 1),
            IterationDecision::Continue
        );

        // Last iteration before max
        assert_eq!(
            should_continue_iteration(9, 10, 1),
            IterationDecision::Continue
        );

        // Exactly at max
        match should_continue_iteration(10, 10, 1) {
            IterationDecision::Stop(_) => {}
            _ => panic!("Expected Stop at max iteration"),
        }
    }

    #[test]
    fn test_should_continue_iteration_zero_max() {
        match should_continue_iteration(0, 0, 1) {
            IterationDecision::Stop(_) => {}
            _ => panic!("Expected Stop when max is 0"),
        }
    }

    // Edge case tests
    #[test]
    fn test_classify_empty_name_workflow() {
        let mut config = simple_workflow();
        config.name = None;
        assert_eq!(classify_workflow(&config), WorkflowType::Standard);
    }

    #[test]
    fn test_validate_workflow_multiple_env_vars() {
        let mut env = HashMap::new();
        env.insert("VAR1".to_string(), "value1".to_string());
        env.insert("VAR2".to_string(), "value2".to_string());
        env.insert("VAR3".to_string(), "value3".to_string());

        let config = WorkflowConfig {
            name: Some("multi-env".to_string()),
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let result = validate_workflow(&config);
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_workflow_complex_command_list() {
        let config = WorkflowConfig {
            name: Some("complex".to_string()),
            commands: vec![
                WorkflowCommand::Simple("echo step1".to_string()),
                WorkflowCommand::Simple("echo step2".to_string()),
                WorkflowCommand::Simple("echo step3".to_string()),
            ],
            env: None,
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let result = validate_workflow(&config);
        assert!(matches!(result, Validation::Success(_)));
    }
}
