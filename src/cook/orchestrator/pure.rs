//! Pure orchestration logic (no I/O, no side effects)
//!
//! This module contains pure functions for workflow classification, validation,
//! and decision logic. All functions are testable without any I/O setup.
//!
//! ## Validation Patterns (Spec 176)
//!
//! This module uses Stillwater's `Validation` applicative functor for error accumulation.
//! Instead of fail-fast validation that stops at the first error, all errors are
//! collected and reported together, providing better user experience.
//!
//! ### Error Accumulation Example
//!
//! ```rust
//! use prodigy::cook::orchestrator::pure::{validate_workflow, WorkflowError};
//! use stillwater::Validation;
//!
//! // Validation accumulates ALL errors:
//! // - Empty commands? Error 1
//! // - Invalid env var? Error 2
//! // - Invalid secret? Error 3
//! // All reported in one pass!
//! ```

use crate::config::mapreduce::MergeWorkflow;
use crate::config::WorkflowConfig;
use crate::cook::environment::SecretValue;
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

/// Workflow validation errors with detailed context
///
/// These errors provide actionable information for users to fix their workflows.
/// All errors include context about what was being validated and why it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    /// Workflow has no steps
    NoSteps,
    /// Invalid environment variable key format
    InvalidEnvKey { key: String, reason: String },
    /// Invalid environment variable value type
    InvalidEnvValue {
        key: String,
        expected: String,
        got: String,
    },
    /// Secret configuration error
    SecretError { key: String, reason: String },
    /// Invalid command syntax
    InvalidCommand {
        command_index: usize,
        reason: String,
    },
    /// Invalid merge configuration
    InvalidMerge(String),
    /// Invalid timeout value
    InvalidTimeout { value: u64, max: u64 },
    /// Legacy: Invalid environment variable (for backward compatibility)
    InvalidEnvVar(String),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowError::NoSteps => {
                write!(f, "Workflow must have at least one command")
            }
            WorkflowError::InvalidEnvKey { key, reason } => {
                write!(f, "Invalid environment variable key '{}': {}", key, reason)
            }
            WorkflowError::InvalidEnvValue { key, expected, got } => {
                write!(
                    f,
                    "Invalid value for environment variable '{}': expected {}, got {}",
                    key, expected, got
                )
            }
            WorkflowError::SecretError { key, reason } => {
                write!(f, "Secret configuration error for '{}': {}", key, reason)
            }
            WorkflowError::InvalidCommand {
                command_index,
                reason,
            } => {
                write!(f, "Invalid command at index {}: {}", command_index, reason)
            }
            WorkflowError::InvalidMerge(msg) => {
                write!(f, "Invalid merge configuration: {}", msg)
            }
            WorkflowError::InvalidTimeout { value, max } => {
                write!(
                    f,
                    "Invalid timeout value {}: must be between 1 and {} seconds",
                    value, max
                )
            }
            // Legacy format for backward compatibility
            WorkflowError::InvalidEnvVar(msg) => {
                write!(f, "Invalid environment variable: {}", msg)
            }
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

/// Maximum allowed timeout in seconds (10 minutes)
const MAX_TIMEOUT_SECS: u64 = 600;

/// Validate workflow configuration with error accumulation
///
/// Uses Stillwater's `Validation` applicative functor to accumulate ALL errors
/// before reporting. This provides a better user experience by showing all
/// problems at once rather than requiring iterative fixes.
///
/// ## Validation Checks
///
/// - Workflow has at least one command
/// - Environment variable keys are well-formed (no empty names, no '=')
/// - Environment variable keys follow naming conventions
/// - Secret keys are valid
/// - Merge workflow has valid commands if present
/// - Timeout values are within bounds
///
/// ## Example
///
/// ```rust
/// use prodigy::cook::orchestrator::pure::validate_workflow;
/// use stillwater::Validation;
///
/// // Validation reports ALL errors at once:
/// // Error 1: Workflow has no commands
/// // Error 2: Invalid env key ''
/// // Error 3: Invalid env key 'KEY=BAD'
/// ```
pub fn validate_workflow(config: &WorkflowConfig) -> Validation<(), Vec<WorkflowError>> {
    let mut errors = Vec::new();

    // FR1: Validate workflow has commands
    errors.extend(validate_has_commands(config));

    // FR1: Validate environment variables
    errors.extend(validate_env_vars(&config.env));

    // FR1: Validate secrets
    errors.extend(validate_secrets(&config.secrets));

    // FR1: Validate merge workflow
    errors.extend(validate_merge_workflow(&config.merge));

    // FR1: Validate command syntax
    errors.extend(validate_commands(&config.commands));

    if errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(errors)
    }
}

/// Validate workflow has at least one command
fn validate_has_commands(config: &WorkflowConfig) -> Vec<WorkflowError> {
    if config.commands.is_empty() {
        vec![WorkflowError::NoSteps]
    } else {
        vec![]
    }
}

/// Validate environment variables - accumulates all errors
fn validate_env_vars(
    env: &Option<std::collections::HashMap<String, String>>,
) -> Vec<WorkflowError> {
    let Some(env_map) = env else {
        return vec![];
    };

    let mut errors = Vec::new();

    for key in env_map.keys() {
        // Check for empty key
        if key.is_empty() {
            errors.push(WorkflowError::InvalidEnvKey {
                key: "(empty)".to_string(),
                reason: "Environment variable name cannot be empty".to_string(),
            });
            continue;
        }

        // Check for '=' in key
        if key.contains('=') {
            errors.push(WorkflowError::InvalidEnvKey {
                key: key.clone(),
                reason: "Environment variable name cannot contain '='".to_string(),
            });
        }

        // Check for valid env var format (starts with letter or underscore)
        if !is_valid_env_key_format(key) {
            errors.push(WorkflowError::InvalidEnvKey {
                key: key.clone(),
                reason: "Must start with a letter or underscore, and contain only alphanumeric characters or underscores".to_string(),
            });
        }
    }

    errors
}

/// Check if an environment variable key has valid format
///
/// Valid format: starts with letter or underscore, contains only [A-Za-z0-9_]
fn is_valid_env_key_format(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }

    let mut chars = key.chars();
    let first = chars.next().unwrap();

    // First character must be letter or underscore
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    // Rest must be alphanumeric or underscore
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Validate secrets configuration - accumulates all errors
fn validate_secrets(
    secrets: &Option<std::collections::HashMap<String, SecretValue>>,
) -> Vec<WorkflowError> {
    let Some(secrets_map) = secrets else {
        return vec![];
    };

    let mut errors = Vec::new();

    for key in secrets_map.keys() {
        if key.is_empty() {
            errors.push(WorkflowError::SecretError {
                key: "(empty)".to_string(),
                reason: "Secret name cannot be empty".to_string(),
            });
            continue;
        }

        // Validate secret key format
        if !is_valid_env_key_format(key) {
            errors.push(WorkflowError::SecretError {
                key: key.clone(),
                reason: "Must start with a letter or underscore, and contain only alphanumeric characters or underscores".to_string(),
            });
        }
    }

    errors
}

/// Validate merge workflow configuration - accumulates all errors
fn validate_merge_workflow(merge: &Option<MergeWorkflow>) -> Vec<WorkflowError> {
    let Some(merge_config) = merge else {
        return vec![];
    };

    let mut errors = Vec::new();

    // Check for at least one command
    if merge_config.commands.is_empty() {
        errors.push(WorkflowError::InvalidMerge(
            "Merge workflow must have at least one command".to_string(),
        ));
    }

    // Validate timeout if present
    if let Some(timeout) = merge_config.timeout {
        if timeout == 0 || timeout > MAX_TIMEOUT_SECS {
            errors.push(WorkflowError::InvalidTimeout {
                value: timeout,
                max: MAX_TIMEOUT_SECS,
            });
        }
    }

    errors
}

/// Validate command syntax - accumulates all errors
fn validate_commands(commands: &[crate::config::command::WorkflowCommand]) -> Vec<WorkflowError> {
    let mut errors = Vec::new();

    for (index, cmd) in commands.iter().enumerate() {
        // Validate each command
        if let Some(validation_error) = validate_single_command(cmd, index) {
            errors.push(validation_error);
        }
    }

    errors
}

/// Validate a single command
fn validate_single_command(
    cmd: &crate::config::command::WorkflowCommand,
    index: usize,
) -> Option<WorkflowError> {
    use crate::config::command::WorkflowCommand;

    match cmd {
        WorkflowCommand::Simple(s) if s.trim().is_empty() => Some(WorkflowError::InvalidCommand {
            command_index: index,
            reason: "Command cannot be empty".to_string(),
        }),
        _ => None,
    }
}

/// Format validation errors for user display
///
/// Groups related errors and provides a clear summary.
pub fn format_validation_errors(errors: &[WorkflowError]) -> String {
    if errors.is_empty() {
        return "No validation errors".to_string();
    }

    let mut output = format!(
        "Workflow validation failed with {} error(s):\n",
        errors.len()
    );

    for (i, error) in errors.iter().enumerate() {
        output.push_str(&format!("  {}. {}\n", i + 1, error));
    }

    output
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
                    .any(|e| matches!(e, WorkflowError::InvalidEnvKey { .. })));
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
                    matches!(e, WorkflowError::InvalidEnvKey { key, reason }
                        if key.contains('=') || reason.contains('='))
                });
                assert!(has_equals_error);
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_workflow_error_display() {
        let err = WorkflowError::NoSteps;
        assert_eq!(err.to_string(), "Workflow must have at least one command");

        let err = WorkflowError::InvalidEnvVar("test".to_string());
        assert_eq!(err.to_string(), "Invalid environment variable: test");

        let err = WorkflowError::InvalidEnvKey {
            key: "BAD=KEY".to_string(),
            reason: "contains '='".to_string(),
        };
        assert!(err.to_string().contains("BAD=KEY"));
        assert!(err.to_string().contains("contains '='"));

        let err = WorkflowError::InvalidCommand {
            command_index: 0,
            reason: "empty command".to_string(),
        };
        assert!(err.to_string().contains("index 0"));
        assert!(err.to_string().contains("empty command"));

        let err = WorkflowError::InvalidMerge("no cmds".to_string());
        assert_eq!(err.to_string(), "Invalid merge configuration: no cmds");
    }

    // === New tests for validation accumulation (Spec 176) ===

    #[test]
    fn test_validation_accumulates_multiple_errors() {
        // Create workflow with multiple validation errors
        let mut env = HashMap::new();
        env.insert("".to_string(), "value1".to_string()); // Error 1: empty key
        env.insert("KEY=BAD".to_string(), "value2".to_string()); // Error 2: contains '='

        let config = WorkflowConfig {
            name: Some("multi-error".to_string()),
            commands: vec![], // Error 3: no commands
            env: Some(env),
            secrets: None,
            env_files: None,
            profiles: None,
            merge: None,
        };

        let result = validate_workflow(&config);
        match result {
            Validation::Failure(errors) => {
                // Should have accumulated ALL errors (at least 3)
                assert!(
                    errors.len() >= 3,
                    "Expected at least 3 errors, got {}",
                    errors.len()
                );

                // Verify specific errors present
                assert!(
                    errors.iter().any(|e| matches!(e, WorkflowError::NoSteps)),
                    "Expected NoSteps error"
                );
                assert!(
                    errors
                        .iter()
                        .any(|e| matches!(e, WorkflowError::InvalidEnvKey { .. })),
                    "Expected InvalidEnvKey error"
                );
            }
            _ => panic!("Expected validation failure with multiple errors"),
        }
    }

    #[test]
    fn test_env_key_format_validation() {
        // Valid keys
        assert!(is_valid_env_key_format("VALID_KEY"));
        assert!(is_valid_env_key_format("_PRIVATE"));
        assert!(is_valid_env_key_format("key123"));
        assert!(is_valid_env_key_format("A"));

        // Invalid keys
        assert!(!is_valid_env_key_format("")); // Empty
        assert!(!is_valid_env_key_format("123KEY")); // Starts with number
        assert!(!is_valid_env_key_format("KEY-NAME")); // Contains hyphen
        assert!(!is_valid_env_key_format("KEY.NAME")); // Contains period
    }

    #[test]
    fn test_format_validation_errors() {
        let errors = vec![
            WorkflowError::NoSteps,
            WorkflowError::InvalidEnvKey {
                key: "BAD".to_string(),
                reason: "test".to_string(),
            },
        ];

        let formatted = format_validation_errors(&errors);
        assert!(formatted.contains("2 error(s)"));
        assert!(formatted.contains("1."));
        assert!(formatted.contains("2."));
    }

    #[test]
    fn test_secret_validation_accumulates_errors() {
        use crate::cook::environment::SecretValue;

        let mut secrets = HashMap::new();
        secrets.insert("".to_string(), SecretValue::Simple("secret1".to_string())); // Empty key
        secrets.insert(
            "123_INVALID".to_string(),
            SecretValue::Simple("secret2".to_string()),
        ); // Invalid format

        let config = WorkflowConfig {
            name: Some("secrets-test".to_string()),
            commands: vec![WorkflowCommand::Simple("echo test".to_string())],
            env: None,
            secrets: Some(secrets),
            env_files: None,
            profiles: None,
            merge: None,
        };

        let result = validate_workflow(&config);
        match result {
            Validation::Failure(errors) => {
                // Should have at least 2 secret errors
                let secret_errors: Vec<_> = errors
                    .iter()
                    .filter(|e| matches!(e, WorkflowError::SecretError { .. }))
                    .collect();
                assert!(
                    secret_errors.len() >= 2,
                    "Expected at least 2 secret errors, got {}",
                    secret_errors.len()
                );
            }
            _ => panic!("Expected validation failure"),
        }
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
