//! Result types for command execution

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Result from executing a command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Whether the command succeeded
    pub success: bool,

    /// The output data from the command
    pub data: Option<Value>,

    /// Error message if the command failed
    pub error: Option<String>,

    /// Exit code if applicable
    pub exit_code: Option<i32>,

    /// Standard output if captured
    pub stdout: Option<String>,

    /// Standard error if captured
    pub stderr: Option<String>,

    /// Execution time in milliseconds
    pub duration_ms: Option<u64>,
}

impl CommandResult {
    /// Creates a successful result
    pub fn success(data: Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            exit_code: Some(0),
            stdout: None,
            stderr: None,
            duration_ms: None,
        }
    }

    /// Creates an error result
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            exit_code: Some(1),
            stdout: None,
            stderr: None,
            duration_ms: None,
        }
    }

    /// Creates a result from command output
    pub fn from_output(stdout: String, stderr: String, exit_code: i32) -> Self {
        let success = exit_code == 0;
        Self {
            success,
            data: if success {
                Some(Value::String(stdout.clone()))
            } else {
                None
            },
            error: if !success { Some(stderr.clone()) } else { None },
            exit_code: Some(exit_code),
            stdout: Some(stdout),
            stderr: Some(stderr),
            duration_ms: None,
        }
    }

    /// Sets the execution duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    /// Checks if the result indicates success
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// Checks if the result indicates failure
    pub fn is_error(&self) -> bool {
        !self.success
    }

    /// Gets the error message if present
    pub fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Converts to a Result type
    pub fn to_result(self) -> Result<Value, CommandError> {
        if self.success {
            Ok(self.data.unwrap_or(Value::Null))
        } else {
            Err(CommandError::ExecutionError(
                self.error.unwrap_or_else(|| "Unknown error".to_string()),
            ))
        }
    }
}

/// Errors that can occur during command execution
#[derive(Debug, Clone)]
pub enum CommandError {
    /// Validation of attributes failed
    ValidationError(String),

    /// Command execution failed
    ExecutionError(String),

    /// IO error occurred
    IoError(String),

    /// Command not found
    NotFound(String),

    /// Timeout occurred
    Timeout(String),

    /// Permission denied
    PermissionDenied(String),

    /// Other error
    Other(String),
}

impl CommandError {
    /// Converts to a CommandResult
    pub fn to_result(self) -> CommandResult {
        CommandResult::error(self.to_string())
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::ValidationError(msg) => write!(f, "Validation error: {msg}"),
            CommandError::ExecutionError(msg) => write!(f, "Execution error: {msg}"),
            CommandError::IoError(msg) => write!(f, "IO error: {msg}"),
            CommandError::NotFound(msg) => write!(f, "Command not found: {msg}"),
            CommandError::Timeout(msg) => write!(f, "Timeout: {msg}"),
            CommandError::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            CommandError::Other(msg) => write!(f, "Error: {msg}"),
        }
    }
}

impl std::error::Error for CommandError {}

impl From<std::io::Error> for CommandError {
    fn from(err: std::io::Error) -> Self {
        CommandError::IoError(err.to_string())
    }
}

impl From<crate::subprocess::SubprocessError> for CommandError {
    fn from(err: crate::subprocess::SubprocessError) -> Self {
        CommandError::ExecutionError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_result() {
        let result = CommandResult::success(Value::String("test".to_string()));
        assert!(result.is_success());
        assert!(!result.is_error());
        assert_eq!(result.data, Some(Value::String("test".to_string())));
    }

    #[test]
    fn test_error_result() {
        let result = CommandResult::error("test error".to_string());
        assert!(!result.is_success());
        assert!(result.is_error());
        assert_eq!(result.error_message(), Some("test error"));
    }

    #[test]
    fn test_from_output() {
        let result = CommandResult::from_output("output".to_string(), "".to_string(), 0);
        assert!(result.is_success());
        assert_eq!(result.stdout, Some("output".to_string()));

        let error_result = CommandResult::from_output("".to_string(), "error".to_string(), 1);
        assert!(error_result.is_error());
        assert_eq!(error_result.stderr, Some("error".to_string()));
    }

    #[test]
    fn test_with_duration() {
        let result = CommandResult::success(Value::Null).with_duration(100);
        assert_eq!(result.duration_ms, Some(100));
    }

    #[test]
    fn test_to_result() {
        let success = CommandResult::success(Value::String("data".to_string()));
        assert!(success.to_result().is_ok());

        let error = CommandResult::error("error".to_string());
        assert!(error.to_result().is_err());
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::ValidationError("test".to_string());
        assert_eq!(err.to_string(), "Validation error: test");

        let err = CommandError::ExecutionError("failed".to_string());
        assert_eq!(err.to_string(), "Execution error: failed");
    }
}
