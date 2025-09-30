//! Session management for cook operations
//!
//! This module provides the SessionManager trait and related types used by the
//! CookSessionAdapter to bridge between the cook orchestrator and UnifiedSessionManager.
//!
//! Note: The actual session tracking is now handled by UnifiedSessionManager through
//! CookSessionAdapter. This module only contains the trait definition and supporting types.

pub mod state;
pub mod summary;
pub use state::{
    ExecutionContext, ExecutionEnvironment, SessionState, SessionStatus, StepResult, WorkflowState,
    WorkflowType,
};
pub use summary::SessionSummary;

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
    fn get_state(&self) -> Result<SessionState>;

    /// Save session state to disk
    async fn save_state(&self, path: &Path) -> Result<()>;

    /// Load session state from disk
    async fn load_state(&self, path: &Path) -> Result<()>;

    /// Load session by ID for resuming
    async fn load_session(&self, session_id: &str) -> Result<SessionState>;

    /// Save checkpoint for resume
    async fn save_checkpoint(&self, state: &SessionState) -> Result<()>;

    /// List resumable sessions
    async fn list_resumable(&self) -> Result<Vec<SessionInfo>>;

    /// Get last interrupted session
    async fn get_last_interrupted(&self) -> Result<Option<String>>;
}

/// Information about a resumable session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub status: SessionStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub workflow_path: std::path::PathBuf,
    pub progress: String,
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
    /// Start workflow timing
    StartWorkflow,
    /// Start iteration timing
    StartIteration(u32),
    /// Complete iteration timing
    CompleteIteration,
    /// Record command timing
    RecordCommandTiming(String, std::time::Duration),
    /// Update workflow state for checkpoint
    UpdateWorkflowState(state::WorkflowState),
    /// Mark session as interrupted
    MarkInterrupted,
    /// Set workflow hash for validation
    SetWorkflowHash(String),
    /// Set workflow type
    SetWorkflowType(state::WorkflowType),
    /// Update execution context
    UpdateExecutionContext(state::ExecutionContext),
}
