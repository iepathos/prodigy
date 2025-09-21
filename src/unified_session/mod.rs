//! Unified session management for Prodigy
//!
//! This module provides a single, consolidated session management system
//! that handles all session-related functionality consistently across the application.

mod cook_adapter;
mod manager;
mod state;
mod storage;

pub use cook_adapter::CookSessionAdapter;
pub use manager::{SessionManager, SessionUpdate};
pub use state::{
    Checkpoint, CheckpointId, MapReducePhase, MapReduceSession, SessionConfig, SessionFilter,
    SessionId, SessionMetadata, SessionStatus, SessionSummary, SessionTimings, SessionType,
    UnifiedSession, WorkflowSession,
};
pub use storage::SessionStorageAdapter;

use anyhow::Result;

/// Create a new session manager with default configuration
pub async fn create_session_manager(storage: crate::storage::GlobalStorage) -> Result<SessionManager> {
    SessionManager::new(storage).await
}

/// Get a session by ID (convenience function)
pub async fn get_session(
    manager: &SessionManager,
    id: &SessionId,
) -> Result<UnifiedSession> {
    manager.load_session(id).await
}

/// List all sessions with an optional filter
pub async fn list_sessions(
    manager: &SessionManager,
    filter: Option<state::SessionFilter>,
) -> Result<Vec<state::SessionSummary>> {
    manager.list_sessions(filter).await
}