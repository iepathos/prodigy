//! Git worktree management for parallel MMM sessions
//!
//! This module provides sophisticated git worktree management capabilities that enable
//! multiple MMM sessions to run concurrently without interfering with each other.
//! Each session runs in its own isolated git worktree with its own branch.
//!
//! # Key Features
//!
//! - **Parallel Sessions**: Multiple improvement sessions can run simultaneously
//! - **Isolation**: Each session has its own working directory and branch
//! - **State Management**: Persistent session state with recovery support
//! - **Automatic Cleanup**: Manages worktree lifecycle and cleanup
//! - **Conflict Resolution**: Smart merging and conflict detection
//!
//! # Architecture
//!
//! The worktree system consists of:
//! - [`WorktreeManager`] - High-level worktree operations and lifecycle management
//! - [`WorktreeSession`] - Represents an active worktree session
//! - [`WorktreeState`] - Persistent state tracking for sessions
//! - [`WorktreeStatus`] - Current status of a worktree session
//!
//! # Examples
//!
//! ## Creating a Worktree Session
//!
//! ```rust
//! use prodigy::worktree::WorktreeManager;
//! use prodigy::subprocess::SubprocessManager;
//! use std::path::PathBuf;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let subprocess = SubprocessManager::production();
//! let manager = WorktreeManager::new(PathBuf::from("/repo"), subprocess)?;
//!
//! let session = manager.create_session().await?;
//! println!("Created session: {}", session.name);
//! # Ok(())
//! # }
//! ```
//!
//! ## Managing Session Lifecycle
//!
//! ```rust
//! # use prodigy::worktree::WorktreeManager;
//! # use prodigy::subprocess::SubprocessManager;
//! # use std::path::PathBuf;
//! # async fn example() -> anyhow::Result<()> {
//! # let subprocess = SubprocessManager::production();
//! # let manager = WorktreeManager::new(PathBuf::from("/repo"), subprocess)?;
//! // List active sessions
//! let sessions = manager.list_sessions().await?;
//!
//! // Merge completed session
//! manager.merge_session("feature-improvement").await?;
//!
//! // Cleanup session
//! manager.cleanup_session("feature-improvement", false).await?;
//! # Ok(())
//! # }
//! ```

use chrono::{DateTime, Utc};
use std::path::PathBuf;

pub mod builder;
pub mod display;
pub mod manager;
pub mod manager_construction;
pub mod manager_queries;
pub mod parsing;
pub mod pool;
pub mod state;
#[cfg(test)]
mod test_state;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tracking_tests;

pub use display::{DetailedWorktreeList, EnhancedSessionInfo, SessionDisplay, WorktreeSummary};
pub use manager::{CleanupConfig, CleanupPolicy, WorktreeManager};
pub use pool::{
    AllocationStrategy, CleanupPolicy as PoolCleanupPolicy, PooledWorktree, ResourceLimits,
    ResourceUsage, ReuseCriteria, WorktreeHandle, WorktreeMetrics, WorktreePool,
    WorktreePoolConfig, WorktreeRequest, WorktreeStatus as PoolWorktrStatus,
};
pub use state::{
    Checkpoint, CommandType, InterruptionType, IterationInfo, WorktreeState, WorktreeStats,
    WorktreeStatus,
};

/// Represents an active git worktree session for MMM operations
///
/// A `WorktreeSession` encapsulates all the information needed to manage
/// an isolated MMM improvement session running in its own git worktree.
/// Each session has its own branch and working directory, allowing multiple
/// sessions to run concurrently without conflicts.
///
/// # Fields
///
/// - `name`: Unique identifier for the session
/// - `branch`: Git branch name for this session
/// - `path`: File system path to the worktree directory
/// - `created_at`: Timestamp when the session was created
///
/// # Examples
///
/// ```rust
/// use prodigy::worktree::WorktreeSession;
/// use std::path::PathBuf;
///
/// let session = WorktreeSession::new(
///     "performance-improvements".to_string(),
///     "prodigy/performance-123".to_string(),
///     PathBuf::from("/tmp/prodigy-worktrees/performance-improvements")
/// );
///
/// println!("Session {} created at {}", session.name, session.created_at);
/// ```
#[derive(Debug, Clone)]
pub struct WorktreeSession {
    /// Unique name identifying this worktree session
    pub name: String,
    /// Git branch name for this session
    pub branch: String,
    /// File system path to the worktree directory
    pub path: PathBuf,
    /// Timestamp when this session was created
    pub created_at: DateTime<Utc>,
}

impl WorktreeSession {
    /// Create a new worktree session
    ///
    /// Creates a new `WorktreeSession` with the specified name, branch, and path.
    /// The creation timestamp is automatically set to the current UTC time.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique identifier for this session
    /// * `branch` - Git branch name to use for this session
    /// * `path` - File system path where the worktree will be located
    ///
    /// # Examples
    ///
    /// ```rust
    /// use prodigy::worktree::WorktreeSession;
    /// use std::path::PathBuf;
    ///
    /// let session = WorktreeSession::new(
    ///     "feature-xyz".to_string(),
    ///     "prodigy/feature-xyz-123".to_string(),
    ///     PathBuf::from("/tmp/worktrees/feature-xyz")
    /// );
    /// ```
    #[must_use]
    pub fn new(name: String, branch: String, path: PathBuf) -> Self {
        Self {
            name,
            branch,
            path,
            created_at: Utc::now(),
        }
    }
}
