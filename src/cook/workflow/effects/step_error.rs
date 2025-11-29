//! Error types for workflow step execution
//!
//! This module defines error types for individual step execution and workflow-level failures.
//! Errors are designed to provide rich context for debugging and distinguish between
//! retryable (transient) and non-retryable errors.

use super::CommandError;
use stillwater::ContextError;

/// Errors that can occur during step execution
#[derive(Debug, Clone, thiserror::Error)]
pub enum StepError {
    #[error("Claude command failed after {attempts} attempts: {last_error}")]
    ClaudeRetryExhausted { attempts: u32, last_error: String },

    #[error("Claude command failed (non-retryable): {0}")]
    ClaudeNonRetryable(String),

    #[error("Shell command exited with code {code:?}: {stderr}")]
    ShellNonZeroExit { code: Option<i32>, stderr: String },

    #[error("Shell command failed: {0}")]
    ShellFailed(String),

    #[error("Variable interpolation failed: {0}")]
    InterpolationFailed(String),

    #[error("Checkpoint save failed: {0}")]
    CheckpointFailed(String),

    #[error("Command error: {0}")]
    CommandError(#[from] CommandError),
}

impl StepError {
    /// Check if this error is retryable (transient)
    pub fn is_retryable(&self) -> bool {
        matches!(self, StepError::ClaudeRetryExhausted { .. })
    }

    /// Check if the underlying error is transient (for retry decision)
    pub fn is_transient(&self) -> bool {
        match self {
            StepError::CommandError(CommandError::ExecutionFailed { message, .. }) => {
                // Claude 500 errors and overload are transient
                message.contains("500")
                    || message.contains("overloaded")
                    || message.contains("rate limit")
                    || message.contains("ECONNRESET")
            }
            _ => false,
        }
    }
}

/// Workflow-level errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum WorkflowError {
    #[error("Step {step_index} failed: {error}")]
    StepFailed {
        step_index: usize,
        error: ContextError<StepError>,
    },

    #[error("Workflow validation failed: {0}")]
    ValidationFailed(String),

    #[error("Resume failed: {0}")]
    ResumeFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        let retryable = StepError::ClaudeRetryExhausted {
            attempts: 3,
            last_error: "500 error".to_string(),
        };
        assert!(retryable.is_retryable());

        let non_retryable = StepError::ClaudeNonRetryable("invalid command".to_string());
        assert!(!non_retryable.is_retryable());
    }

    #[test]
    fn test_is_transient() {
        let transient_500 = StepError::CommandError(CommandError::ExecutionFailed {
            message: "Server returned 500".to_string(),
            exit_code: None,
        });
        assert!(transient_500.is_transient());

        let transient_overloaded = StepError::CommandError(CommandError::ExecutionFailed {
            message: "Service overloaded".to_string(),
            exit_code: None,
        });
        assert!(transient_overloaded.is_transient());

        let non_transient = StepError::ShellFailed("command not found".to_string());
        assert!(!non_transient.is_transient());
    }

    #[test]
    fn test_error_display() {
        let err = StepError::ClaudeRetryExhausted {
            attempts: 5,
            last_error: "timeout".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("5 attempts"));
        assert!(display.contains("timeout"));
    }
}
