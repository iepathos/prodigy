//! Session management for cook operations
//!
//! Handles session state, tracking, and persistence.

pub mod adapter;
pub mod state;
pub mod summary;
pub mod tracker;

pub use adapter::SessionManagerAdapter;
pub use state::{SessionState, SessionStatus};
pub use summary::SessionSummary;
pub use tracker::{SessionTracker, SessionTrackerImpl};

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

/// Trait for managing cook sessions
#[async_trait]
pub trait SessionManager: Send + Sync {
    /// Start a new session
    async fn start_session(&self, session_id: &str) -> Result<()>;

    /// Update session state
    async fn update_session(&self, update: SessionUpdate) -> Result<()>;

    /// Complete the current session
    async fn complete_session(&self) -> Result<SessionSummary>;

    /// Get current session state
    fn get_state(&self) -> SessionState;

    /// Save session state to disk
    async fn save_state(&self, path: &Path) -> Result<()>;

    /// Load session state from disk
    async fn load_state(&self, path: &Path) -> Result<()>;
}

/// Updates that can be applied to a session
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    /// Increment iteration count
    IncrementIteration,
    /// Add files changed
    AddFilesChanged(usize),
    /// Update status
    UpdateStatus(SessionStatus),
    /// Add error
    AddError(String),
}
