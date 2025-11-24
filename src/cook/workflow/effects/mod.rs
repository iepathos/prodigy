//! Effect-based I/O operations for workflow execution
//!
//! This module provides Stillwater Effect abstractions for workflow I/O operations,
//! following the "pure core, imperative shell" pattern. All I/O is encapsulated in
//! Effects that can be composed, tested with mock environments, and executed with
//! proper error handling.
//!
//! # Architecture
//!
//! The effects module separates concerns:
//! - **Pure logic** lives in `pure/` module (command_builder, output_parser)
//! - **I/O effects** live here (claude, shell, handler operations)
//! - **Environment** provides dependencies via dependency injection
//!
//! # Effect Composition
//!
//! Effects can be composed using `and_then`, `map`, and parallel combinators:
//!
//! ```ignore
//! use stillwater::Effect;
//!
//! // Sequential composition
//! let workflow_effect = execute_claude_command_effect("/task", &vars)
//!     .and_then(|result| execute_shell_command_effect("cargo test", &vars))
//!     .map(|result| process_output(result));
//!
//! // Execute with environment
//! let output = workflow_effect.run_async(&env).await?;
//! ```
//!
//! # Testing
//!
//! Effects can be tested with mock environments without performing actual I/O:
//!
//! ```ignore
//! let mock_env = MockWorkflowEnv::default();
//! let effect = execute_shell_command_effect("echo test", &vars);
//! let result = effect.run_async(&mock_env).await;
//! assert!(result.is_ok());
//! ```

pub mod claude;
pub mod environment;
pub mod handler;
pub mod shell;

pub use claude::execute_claude_command_effect;
pub use environment::{WorkflowEnv, WorkflowEnvBuilder};
pub use handler::execute_handler_effect;
pub use shell::execute_shell_command_effect;

/// Output from command execution
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Standard output from the command
    pub stdout: String,
    /// Standard error from the command
    pub stderr: String,
    /// Exit code of the command
    pub exit_code: Option<i32>,
    /// Whether the command succeeded
    pub success: bool,
    /// Variables extracted from output
    pub variables: std::collections::HashMap<String, String>,
    /// Location of JSON log file (for Claude commands)
    pub json_log_location: Option<String>,
}

impl CommandOutput {
    /// Create a new successful command output
    pub fn success(stdout: String) -> Self {
        Self {
            stdout,
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
            variables: std::collections::HashMap::new(),
            json_log_location: None,
        }
    }

    /// Create a new failed command output
    pub fn failure(stderr: String, exit_code: Option<i32>) -> Self {
        Self {
            stdout: String::new(),
            stderr,
            exit_code,
            success: false,
            variables: std::collections::HashMap::new(),
            json_log_location: None,
        }
    }

    /// Add extracted variables to the output
    pub fn with_variables(mut self, variables: std::collections::HashMap<String, String>) -> Self {
        self.variables = variables;
        self
    }

    /// Add JSON log location to the output
    pub fn with_json_log_location(mut self, location: String) -> Self {
        self.json_log_location = Some(location);
        self
    }
}

/// Error type for command execution
#[derive(Debug, Clone)]
pub enum CommandError {
    /// Command execution failed
    ExecutionFailed {
        message: String,
        exit_code: Option<i32>,
    },
    /// Command timed out
    Timeout { seconds: u64 },
    /// Handler not found
    HandlerNotFound { name: String },
    /// Invalid command configuration
    InvalidConfiguration { message: String },
    /// I/O error during execution
    IoError { message: String },
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::ExecutionFailed { message, exit_code } => {
                write!(
                    f,
                    "Command execution failed: {} (exit code: {:?})",
                    message, exit_code
                )
            }
            CommandError::Timeout { seconds } => {
                write!(f, "Command timed out after {} seconds", seconds)
            }
            CommandError::HandlerNotFound { name } => {
                write!(f, "Handler not found: {}", name)
            }
            CommandError::InvalidConfiguration { message } => {
                write!(f, "Invalid command configuration: {}", message)
            }
            CommandError::IoError { message } => {
                write!(f, "I/O error: {}", message)
            }
        }
    }
}

impl std::error::Error for CommandError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_output_success() {
        let output = CommandOutput::success("hello world".to_string());
        assert!(output.success);
        assert_eq!(output.exit_code, Some(0));
        assert_eq!(output.stdout, "hello world");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn test_command_output_failure() {
        let output = CommandOutput::failure("error occurred".to_string(), Some(1));
        assert!(!output.success);
        assert_eq!(output.exit_code, Some(1));
        assert_eq!(output.stderr, "error occurred");
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn test_command_output_with_variables() {
        let mut vars = std::collections::HashMap::new();
        vars.insert("key".to_string(), "value".to_string());

        let output = CommandOutput::success("output".to_string()).with_variables(vars);

        assert_eq!(output.variables.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_command_output_with_json_log() {
        let output = CommandOutput::success("output".to_string())
            .with_json_log_location("/tmp/log.json".to_string());

        assert_eq!(output.json_log_location, Some("/tmp/log.json".to_string()));
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::ExecutionFailed {
            message: "test failure".to_string(),
            exit_code: Some(1),
        };
        assert!(err.to_string().contains("test failure"));

        let err = CommandError::Timeout { seconds: 30 };
        assert!(err.to_string().contains("30 seconds"));

        let err = CommandError::HandlerNotFound {
            name: "missing".to_string(),
        };
        assert!(err.to_string().contains("missing"));
    }
}
