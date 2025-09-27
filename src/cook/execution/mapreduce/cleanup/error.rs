//! Error types for worktree cleanup operations

use std::path::PathBuf;
use std::time::Duration;

/// Error type for cleanup operations
#[derive(Debug, thiserror::Error)]
pub enum CleanupError {
    #[error("Failed to remove worktree at {path}: {source}")]
    RemovalFailed {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Worktree cleanup timeout after {timeout:?}")]
    Timeout { timeout: Duration },

    #[error("Worktree is still active and cannot be cleaned")]
    WorktreeActive,

    #[error("Git operation failed: {0}")]
    GitError(String),

    #[error("Permission denied for worktree cleanup: {path}")]
    PermissionDenied { path: PathBuf },

    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    #[error("Cleanup coordinator error: {0}")]
    CoordinatorError(String),
}

impl CleanupError {
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            CleanupError::Timeout { .. } | CleanupError::WorktreeActive
        )
    }

    /// Check if the error should trigger a retry
    pub fn should_retry(&self) -> bool {
        matches!(self, CleanupError::Timeout { .. })
    }
}

/// Result type for cleanup operations
pub type CleanupResult<T> = Result<T, CleanupError>;
