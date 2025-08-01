//! Git operation error types

use std::path::PathBuf;
use thiserror::Error;

/// Git-specific errors
#[derive(Debug, Error, Clone)]
pub enum GitError {
    #[error("Not a git repository")]
    NotARepository,

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Branch already exists: {0}")]
    BranchExists(String),

    #[error("Merge conflict in files: {files:?}")]
    MergeConflict { files: Vec<PathBuf> },

    #[error("Uncommitted changes present")]
    UncommittedChanges,

    #[error("Nothing to commit, working tree clean")]
    NothingToCommit,

    #[error("Worktree already exists: {0}")]
    WorktreeExists(String),

    #[error("Worktree not found: {0}")]
    WorktreeNotFound(String),

    #[error("Commit not found: {0}")]
    CommitNotFound(String),

    #[error("Repository is in detached HEAD state")]
    DetachedHead,

    #[error("Git command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid git reference: {0}")]
    InvalidReference(String),

    #[error("Working directory is dirty")]
    DirtyWorkingTree,

    #[error("Remote not found: {0}")]
    RemoteNotFound(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Repository locked")]
    RepositoryLocked,

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Permission denied")]
    PermissionDenied,
}

impl GitError {
    /// Check if this is a recoverable error
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            GitError::UncommittedChanges
                | GitError::NothingToCommit
                | GitError::DirtyWorkingTree
                | GitError::RepositoryLocked
        )
    }

    /// Check if this is a transient error that might succeed on retry
    pub fn is_transient(&self) -> bool {
        matches!(self, GitError::NetworkError(_) | GitError::RepositoryLocked)
    }
}
