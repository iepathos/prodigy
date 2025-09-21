use super::{ErrorCode, ProdigyError};
use std::path::PathBuf;

/// Extension trait for convenient error conversion
pub trait ErrorExt<T> {
    /// Convert to ProdigyError with context
    fn to_prodigy(self, context: impl Into<String>) -> Result<T, ProdigyError>;

    /// Convert to ProdigyError with specific error type
    fn to_config_error(self, message: impl Into<String>) -> Result<T, ProdigyError>;
    fn to_storage_error(self, message: impl Into<String>) -> Result<T, ProdigyError>;
    fn to_execution_error(self, message: impl Into<String>) -> Result<T, ProdigyError>;
    fn to_session_error(self, message: impl Into<String>) -> Result<T, ProdigyError>;
}

impl<T, E> ErrorExt<T> for Result<T, E>
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    fn to_prodigy(self, context: impl Into<String>) -> Result<T, ProdigyError> {
        self.map_err(|e| ProdigyError::other(context).with_source(e))
    }

    fn to_config_error(self, message: impl Into<String>) -> Result<T, ProdigyError> {
        self.map_err(|e| ProdigyError::config(message).with_source(e))
    }

    fn to_storage_error(self, message: impl Into<String>) -> Result<T, ProdigyError> {
        self.map_err(|e| ProdigyError::storage(message).with_source(e))
    }

    fn to_execution_error(self, message: impl Into<String>) -> Result<T, ProdigyError> {
        self.map_err(|e| ProdigyError::execution(message).with_source(e))
    }

    fn to_session_error(self, message: impl Into<String>) -> Result<T, ProdigyError> {
        self.map_err(|e| ProdigyError::session(message).with_source(e))
    }
}

/// Helper functions for common error scenarios
pub mod common {
    use super::*;

    /// Create a not found error for configuration
    pub fn config_not_found(path: impl AsRef<std::path::Path>) -> ProdigyError {
        ProdigyError::config_with_code(
            ErrorCode::CONFIG_NOT_FOUND,
            format!("Configuration file not found: {}", path.as_ref().display()),
        )
    }

    /// Create a storage IO error
    pub fn storage_io_error(path: Option<PathBuf>, operation: &str) -> ProdigyError {
        ProdigyError::storage_with_code(
            ErrorCode::STORAGE_IO_ERROR,
            format!("Storage {} failed", operation),
            path,
        )
    }

    /// Create a command not found error
    pub fn command_not_found(command: &str) -> ProdigyError {
        ProdigyError::execution_with_code(
            ErrorCode::EXEC_COMMAND_NOT_FOUND,
            format!("Command '{}' not found", command),
            Some(command.to_string()),
        )
    }

    /// Create a session not found error
    pub fn session_not_found(session_id: &str) -> ProdigyError {
        ProdigyError::session_with_code(
            ErrorCode::SESSION_NOT_FOUND,
            format!("Session '{}' not found", session_id),
            Some(session_id.to_string()),
        )
    }

    /// Create a workflow validation error
    pub fn workflow_validation_failed(workflow_name: &str, reason: &str) -> ProdigyError {
        ProdigyError::workflow_with_code(
            ErrorCode::WORKFLOW_VALIDATION_FAILED,
            format!("Workflow '{}' validation failed: {}", workflow_name, reason),
            Some(workflow_name.to_string()),
        )
    }

    /// Create a git repository not found error
    pub fn git_repo_not_found(path: impl AsRef<std::path::Path>) -> ProdigyError {
        ProdigyError::git(
            ErrorCode::GIT_NOT_REPO,
            format!("Not a git repository: {}", path.as_ref().display()),
            "repository check",
        )
    }

    /// Create a timeout error
    pub fn execution_timeout(command: &str, timeout_secs: u64) -> ProdigyError {
        ProdigyError::execution_with_code(
            ErrorCode::EXEC_TIMEOUT,
            format!(
                "Command '{}' timed out after {} seconds",
                command, timeout_secs
            ),
            Some(command.to_string()),
        )
    }

    /// Create a validation error for missing field
    pub fn missing_required_field(field: &str) -> ProdigyError {
        ProdigyError::validation_with_code(
            ErrorCode::VALIDATION_REQUIRED_FIELD,
            format!("Required field '{}' is missing", field),
            Some(field.to_string()),
        )
    }
}

/// Macro for quick error creation with context
#[macro_export]
macro_rules! prodigy_error {
    (config: $msg:expr) => {
        $crate::error::ProdigyError::config($msg)
    };
    (config: $msg:expr, $source:expr) => {
        $crate::error::ProdigyError::config($msg).with_source($source)
    };
    (session: $msg:expr) => {
        $crate::error::ProdigyError::session($msg)
    };
    (session: $msg:expr, $source:expr) => {
        $crate::error::ProdigyError::session($msg).with_source($source)
    };
    (storage: $msg:expr) => {
        $crate::error::ProdigyError::storage($msg)
    };
    (storage: $msg:expr, $source:expr) => {
        $crate::error::ProdigyError::storage($msg).with_source($source)
    };
    (execution: $msg:expr) => {
        $crate::error::ProdigyError::execution($msg)
    };
    (execution: $msg:expr, $source:expr) => {
        $crate::error::ProdigyError::execution($msg).with_source($source)
    };
    (workflow: $msg:expr) => {
        $crate::error::ProdigyError::workflow($msg)
    };
    (workflow: $msg:expr, $source:expr) => {
        $crate::error::ProdigyError::workflow($msg).with_source($source)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_extension_trait() {
        let io_result: Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));

        let prodigy_result = io_result.to_storage_error("Failed to open file");
        assert!(prodigy_result.is_err());

        let err = prodigy_result.unwrap_err();
        assert_eq!(err.code(), ErrorCode::STORAGE_GENERIC);
    }

    #[test]
    fn test_common_error_helpers() {
        let err = common::config_not_found("/etc/prodigy/config.yml");
        assert_eq!(err.code(), ErrorCode::CONFIG_NOT_FOUND);
        assert!(err.user_message().contains("Configuration problem"));

        let err = common::execution_timeout("long_command", 30);
        assert_eq!(err.code(), ErrorCode::EXEC_TIMEOUT);
        assert!(err.user_message().contains("timed out"));
    }

    #[test]
    fn test_error_macro() {
        let err = prodigy_error!(config: "Test error");
        assert_eq!(err.code(), ErrorCode::CONFIG_GENERIC);

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let err_with_source = prodigy_error!(storage: "Storage failed", io_err);
        assert_eq!(err_with_source.code(), ErrorCode::STORAGE_GENERIC);
    }
}
