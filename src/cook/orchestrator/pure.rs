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

/// Classify workflow type (placeholder implementation)
///
/// Returns Standard for demonstration. Full implementation will analyze
/// workflow configuration structure.
pub fn classify_workflow(_config: &WorkflowConfig) -> WorkflowType {
    WorkflowType::Standard
}

/// Validate workflow configuration (placeholder implementation)
///
/// Returns Success for demonstration. Full validation will check:
/// - Workflow has at least one command
/// - Environment variables are well-formed
/// - Command syntax is valid
pub fn validate_workflow(_config: &WorkflowConfig) -> Validation<(), Vec<WorkflowError>> {
    Validation::Success(())
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

    #[test]
    fn test_classify_workflow_returns_standard() {
        let config = WorkflowConfig::default();
        assert_eq!(classify_workflow(&config), WorkflowType::Standard);
    }

    #[test]
    fn test_validate_workflow_returns_success() {
        let config = WorkflowConfig::default();
        let result = validate_workflow(&config);
        assert!(matches!(result, Validation::Success(_)));
    }

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
}
