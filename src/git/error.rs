//! Git operation error types

use crate::error::{ErrorCode, ProdigyError};
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

/// Convert GitError to ProdigyError
impl From<GitError> for ProdigyError {
    fn from(err: GitError) -> Self {
        let (code, operation) = match &err {
            GitError::NotARepository => (ErrorCode::GIT_NOT_REPO, "repository check"),
            GitError::BranchNotFound(_) => (ErrorCode::GIT_BRANCH_NOT_FOUND, "branch lookup"),
            GitError::BranchExists(_) => (ErrorCode::GIT_BRANCH_EXISTS, "branch creation"),
            GitError::MergeConflict { .. } => (ErrorCode::GIT_MERGE_CONFLICT, "merge"),
            GitError::UncommittedChanges => (ErrorCode::GIT_UNCOMMITTED, "status check"),
            GitError::NothingToCommit => (ErrorCode::GIT_NOTHING_TO_COMMIT, "commit"),
            GitError::WorktreeExists(_) => (ErrorCode::GIT_WORKTREE_EXISTS, "worktree creation"),
            GitError::WorktreeNotFound(_) => (ErrorCode::GIT_WORKTREE_NOT_FOUND, "worktree lookup"),
            GitError::CommitNotFound(_) => (ErrorCode::GIT_COMMIT_NOT_FOUND, "commit lookup"),
            GitError::DetachedHead => (ErrorCode::GIT_DETACHED_HEAD, "branch check"),
            GitError::CommandFailed(_) => (ErrorCode::GIT_COMMAND_FAILED, "command execution"),
            GitError::InvalidReference(_) => (ErrorCode::GIT_INVALID_REF, "reference parsing"),
            GitError::DirtyWorkingTree => (ErrorCode::GIT_DIRTY, "working tree check"),
            GitError::RemoteNotFound(_) => (ErrorCode::GIT_REMOTE_NOT_FOUND, "remote lookup"),
            GitError::AuthenticationFailed => (ErrorCode::GIT_AUTH_FAILED, "authentication"),
            GitError::NetworkError(_) => (ErrorCode::GIT_NETWORK, "network operation"),
            GitError::RepositoryLocked => (ErrorCode::GIT_REPO_LOCKED, "repository access"),
            GitError::InvalidPath(_) => (ErrorCode::GIT_INVALID_PATH, "path validation"),
            GitError::PermissionDenied => (ErrorCode::GIT_PERMISSION, "file access"),
        };

        ProdigyError::git(code, err.to_string(), operation).with_source(err)
    }
}
