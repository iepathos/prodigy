use std::fmt::Display;
use std::path::PathBuf;
use thiserror::Error;

pub mod codes;
pub mod helpers;

pub use codes::{describe_error_code, ErrorCode};
pub use helpers::{common, ErrorExt};

/// The unified error type for the entire Prodigy application
#[derive(Error, Debug)]
pub enum ProdigyError {
    #[error("[E{code:04}] Configuration error: {message}")]
    Config {
        code: u16,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] Session error: {message}")]
    Session {
        code: u16,
        message: String,
        session_id: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] Storage error: {message}")]
    Storage {
        code: u16,
        message: String,
        path: Option<PathBuf>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] Execution error: {message}")]
    Execution {
        code: u16,
        message: String,
        command: Option<String>,
        exit_code: Option<i32>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] Workflow error: {message}")]
    Workflow {
        code: u16,
        message: String,
        workflow_name: Option<String>,
        step: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] Git operation failed: {message}")]
    Git {
        code: u16,
        message: String,
        operation: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] Validation error: {message}")]
    Validation {
        code: u16,
        message: String,
        field: Option<String>,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("[E{code:04}] {message}")]
    Other {
        code: u16,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl ProdigyError {
    /// Create a configuration error with default code
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            code: ErrorCode::CONFIG_GENERIC,
            message: message.into(),
            source: None,
        }
    }

    /// Create a configuration error with specific code
    pub fn config_with_code(code: u16, message: impl Into<String>) -> Self {
        Self::Config {
            code,
            message: message.into(),
            source: None,
        }
    }

    /// Create a session error with default code
    pub fn session(message: impl Into<String>) -> Self {
        Self::Session {
            code: ErrorCode::SESSION_GENERIC,
            message: message.into(),
            session_id: None,
            source: None,
        }
    }

    /// Create a session error with specific code and session ID
    pub fn session_with_code(
        code: u16,
        message: impl Into<String>,
        session_id: Option<String>,
    ) -> Self {
        Self::Session {
            code,
            message: message.into(),
            session_id,
            source: None,
        }
    }

    /// Create a storage error with default code
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            code: ErrorCode::STORAGE_GENERIC,
            message: message.into(),
            path: None,
            source: None,
        }
    }

    /// Create a storage error with specific code and path
    pub fn storage_with_code(code: u16, message: impl Into<String>, path: Option<PathBuf>) -> Self {
        Self::Storage {
            code,
            message: message.into(),
            path,
            source: None,
        }
    }

    /// Create an execution error with default code
    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            code: ErrorCode::EXEC_GENERIC,
            message: message.into(),
            command: None,
            exit_code: None,
            source: None,
        }
    }

    /// Create an execution error with specific code
    pub fn execution_with_code(
        code: u16,
        message: impl Into<String>,
        command: Option<String>,
    ) -> Self {
        Self::Execution {
            code,
            message: message.into(),
            command,
            exit_code: None,
            source: None,
        }
    }

    /// Create a workflow error with default code
    pub fn workflow(message: impl Into<String>) -> Self {
        Self::Workflow {
            code: ErrorCode::WORKFLOW_GENERIC,
            message: message.into(),
            workflow_name: None,
            step: None,
            source: None,
        }
    }

    /// Create a workflow error with specific code
    pub fn workflow_with_code(
        code: u16,
        message: impl Into<String>,
        workflow_name: Option<String>,
    ) -> Self {
        Self::Workflow {
            code,
            message: message.into(),
            workflow_name,
            step: None,
            source: None,
        }
    }

    /// Create a git error with specific code and operation
    pub fn git(code: u16, message: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::Git {
            code,
            message: message.into(),
            operation: operation.into(),
            source: None,
        }
    }

    /// Create a validation error with default code
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            code: ErrorCode::VALIDATION_GENERIC,
            message: message.into(),
            field: None,
            source: None,
        }
    }

    /// Create a validation error with specific code and field
    pub fn validation_with_code(
        code: u16,
        message: impl Into<String>,
        field: Option<String>,
    ) -> Self {
        Self::Validation {
            code,
            message: message.into(),
            field,
            source: None,
        }
    }

    /// Create a generic other error
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other {
            code: ErrorCode::OTHER_GENERIC,
            message: message.into(),
            source: None,
        }
    }

    /// Add a source error to this error
    pub fn with_source(
        mut self,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        match &mut self {
            Self::Config { source: src, .. }
            | Self::Session { source: src, .. }
            | Self::Storage { source: src, .. }
            | Self::Execution { source: src, .. }
            | Self::Workflow { source: src, .. }
            | Self::Git { source: src, .. }
            | Self::Validation { source: src, .. }
            | Self::Other { source: src, .. } => {
                *src = Some(source.into());
            }
        }
        self
    }

    /// Add context to the error message
    pub fn with_context(mut self, context: impl Display) -> Self {
        match &mut self {
            Self::Config { message, .. }
            | Self::Session { message, .. }
            | Self::Storage { message, .. }
            | Self::Execution { message, .. }
            | Self::Workflow { message, .. }
            | Self::Git { message, .. }
            | Self::Validation { message, .. }
            | Self::Other { message, .. } => {
                *message = format!("{}: {}", message, context);
            }
        }
        self
    }

    /// Get the exit code for this error
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Config { .. } => 2,
            Self::Session { .. } => 3,
            Self::Storage { .. } => 4,
            Self::Execution { .. } => 5,
            Self::Workflow { .. } => 6,
            Self::Git { .. } => 7,
            Self::Validation { .. } => 8,
            Self::Other { .. } => 1,
        }
    }

    /// Get the error code
    pub fn code(&self) -> u16 {
        match self {
            Self::Config { code, .. }
            | Self::Session { code, .. }
            | Self::Storage { code, .. }
            | Self::Execution { code, .. }
            | Self::Workflow { code, .. }
            | Self::Git { code, .. }
            | Self::Validation { code, .. }
            | Self::Other { code, .. } => *code,
        }
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Self::Config { message, .. } => format!("Configuration problem: {}", message),
            Self::Session {
                message,
                session_id,
                ..
            } => {
                if let Some(id) = session_id {
                    format!("Session {} error: {}", id, message)
                } else {
                    format!("Session error: {}", message)
                }
            }
            Self::Storage { message, path, .. } => {
                if let Some(p) = path {
                    format!("Storage error at {}: {}", p.display(), message)
                } else {
                    format!("Storage error: {}", message)
                }
            }
            Self::Execution {
                message, command, ..
            } => {
                if let Some(cmd) = command {
                    format!("Command '{}' failed: {}", cmd, message)
                } else {
                    format!("Execution error: {}", message)
                }
            }
            Self::Workflow {
                message,
                workflow_name,
                step,
                ..
            } => {
                let mut msg = String::from("Workflow error");
                if let Some(name) = workflow_name {
                    msg.push_str(&format!(" in '{}'", name));
                }
                if let Some(s) = step {
                    msg.push_str(&format!(" at step '{}'", s));
                }
                format!("{}: {}", msg, message)
            }
            Self::Git {
                message, operation, ..
            } => {
                format!("Git {} failed: {}", operation, message)
            }
            Self::Validation { message, field, .. } => {
                if let Some(f) = field {
                    format!("Validation error for '{}': {}", f, message)
                } else {
                    format!("Validation error: {}", message)
                }
            }
            Self::Other { message, .. } => message.clone(),
        }
    }

    /// Get a developer-friendly error message with full chain
    pub fn developer_message(&self) -> String {
        format!("{:#}", self)
    }

    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Execution {
                exit_code: Some(code),
                ..
            } => {
                // Non-zero but not fatal exit codes
                *code != 0 && *code < 128
            }
            Self::Storage { code, .. } => {
                // Temporary storage issues are recoverable
                *code == ErrorCode::STORAGE_TEMPORARY || *code == ErrorCode::STORAGE_LOCK_BUSY
            }
            _ => false,
        }
    }

    /// Set the exit code for an execution error
    pub fn with_exit_code(mut self, exit_code: i32) -> Self {
        if let Self::Execution {
            exit_code: ref mut ec,
            ..
        } = self
        {
            *ec = Some(exit_code);
        }
        self
    }

    /// Set the workflow step for a workflow error
    pub fn with_step(mut self, step: impl Into<String>) -> Self {
        if let Self::Workflow {
            step: ref mut s, ..
        } = self
        {
            *s = Some(step.into());
        }
        self
    }
}

/// Type alias for Results using ProdigyError
pub type Result<T> = std::result::Result<T, ProdigyError>;

/// Type alias for library Results (same as Result for now)
pub type LibResult<T> = std::result::Result<T, ProdigyError>;

/// Type alias for application Results (using anyhow for flexibility)
pub type AppResult<T> = anyhow::Result<T>;

// Conversion from common error types

impl From<std::io::Error> for ProdigyError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;

        let (code, message) = match err.kind() {
            ErrorKind::NotFound => (ErrorCode::STORAGE_NOT_FOUND, "File or directory not found"),
            ErrorKind::PermissionDenied => {
                (ErrorCode::STORAGE_PERMISSION_DENIED, "Permission denied")
            }
            ErrorKind::AlreadyExists => (ErrorCode::STORAGE_ALREADY_EXISTS, "Already exists"),
            ErrorKind::InvalidInput => (ErrorCode::VALIDATION_INVALID_INPUT, "Invalid input"),
            ErrorKind::InvalidData => (ErrorCode::VALIDATION_INVALID_DATA, "Invalid data"),
            ErrorKind::TimedOut => (ErrorCode::EXEC_TIMEOUT, "Operation timed out"),
            ErrorKind::Interrupted => (ErrorCode::EXEC_INTERRUPTED, "Operation interrupted"),
            ErrorKind::WouldBlock => (
                ErrorCode::STORAGE_LOCK_BUSY,
                "Resource temporarily unavailable",
            ),
            _ => (ErrorCode::STORAGE_IO_ERROR, "IO operation failed"),
        };

        ProdigyError::storage_with_code(code, message, None).with_source(err)
    }
}

impl From<serde_yaml::Error> for ProdigyError {
    fn from(err: serde_yaml::Error) -> Self {
        ProdigyError::config_with_code(ErrorCode::CONFIG_INVALID_YAML, "Invalid YAML syntax")
            .with_source(err)
    }
}

impl From<serde_json::Error> for ProdigyError {
    fn from(err: serde_json::Error) -> Self {
        ProdigyError::config_with_code(ErrorCode::CONFIG_INVALID_JSON, "Invalid JSON syntax")
            .with_source(err)
    }
}

// Note: ProdigyError automatically converts to anyhow::Error because it implements std::error::Error

// Conversion from storage module errors
impl From<crate::storage::error::StorageError> for ProdigyError {
    fn from(err: crate::storage::error::StorageError) -> Self {
        use crate::storage::error::StorageError;

        match err {
            StorageError::Io(io_err) => ProdigyError::from(io_err),
            StorageError::NotFound(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_NOT_FOUND, msg, None)
            }
            StorageError::Lock(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_LOCK_FAILED, msg, None)
            }
            StorageError::Serialization(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_SERIALIZATION_ERROR, msg, None)
            }
            StorageError::Timeout(duration) => ProdigyError::storage_with_code(
                ErrorCode::STORAGE_TEMPORARY,
                format!("Operation timed out after {:?}", duration),
                None,
            ),
            StorageError::Database(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_BACKEND_ERROR, msg, None)
            }
            StorageError::Conflict(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_LOCK_BUSY, msg, None)
            }
            StorageError::Unavailable(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_TEMPORARY, msg, None)
            }
            StorageError::Configuration(msg) => {
                ProdigyError::config_with_code(ErrorCode::CONFIG_INVALID_VALUE, msg)
            }
            StorageError::Transaction(msg) | StorageError::Connection(msg) => {
                ProdigyError::storage_with_code(ErrorCode::STORAGE_BACKEND_ERROR, msg, None)
            }
            StorageError::Other(anyhow_err) => ProdigyError::storage(anyhow_err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation_and_chaining() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test file");
        let err = ProdigyError::storage("Cannot read file")
            .with_source(io_err)
            .with_context("while processing workflow");

        assert_eq!(err.code(), ErrorCode::STORAGE_GENERIC);
        assert!(err.to_string().contains("[E3000]"));
        assert!(err.user_message().contains("Cannot read file"));
    }

    #[test]
    fn test_error_codes() {
        let err = ProdigyError::config_with_code(ErrorCode::CONFIG_NOT_FOUND, "Config not found");
        assert_eq!(err.code(), ErrorCode::CONFIG_NOT_FOUND);
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn test_recoverable_errors() {
        let recoverable = ProdigyError::execution("Command failed").with_exit_code(1);
        assert!(recoverable.is_recoverable());

        let fatal = ProdigyError::execution("Command killed").with_exit_code(137);
        assert!(!fatal.is_recoverable());
    }
}
