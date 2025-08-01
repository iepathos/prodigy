//! Session state management refactored
//!
//! Provides a clean, event-driven session management abstraction with
//! support for persistence, recovery, and concurrent sessions.

pub mod config;
pub mod events;
pub mod manager;
pub mod persistence;
pub mod state;
pub mod storage;

pub use config::{ExecutionMode, SessionConfig, SessionOptions};
pub use events::{SessionEvent, SessionObserver, TimestampedEvent};
pub use manager::{InMemorySessionManager, SessionManager};
pub use persistence::{PersistedSession, SessionCheckpoint, StateSnapshot};
pub use state::{SessionProgress, SessionState, SessionSummary};
pub use storage::{FileSessionStorage, SessionStorage};

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a session
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new session ID
    pub fn new() -> Self {
        Self(format!("session-{}", Uuid::new_v4()))
    }

    /// Create from an existing string
    pub fn from_string(id: String) -> Self {
        Self(id)
    }

    /// Get the inner string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session information for listing
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: SessionId,
    pub state: SessionState,
    pub config: SessionConfig,
    pub progress: SessionProgress,
}

/// Changes made during an iteration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IterationChanges {
    pub files_modified: Vec<std::path::PathBuf>,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub commands_run: Vec<String>,
    pub git_commits: Vec<CommitInfo>,
}

/// Information about a git commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Executed command information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutedCommand {
    pub command: String,
    pub success: bool,
    pub duration: std::time::Duration,
    pub output_size: usize,
}

#[cfg(test)]
mod tests;
