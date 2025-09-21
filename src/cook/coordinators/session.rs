//! Session coordinator for managing session lifecycle

use crate::cook::session::{SessionManager, SessionStatus, SessionUpdate};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Session information
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session ID
    pub session_id: String,
    /// Current status
    pub status: SessionStatus,
}

/// Trait for session coordination
#[async_trait]
pub trait SessionCoordinator: Send + Sync {
    /// Start a new session
    async fn start_session(&self, session_id: &str) -> Result<()>;

    /// Update session status
    async fn update_status(&self, status: SessionStatus) -> Result<()>;

    /// Track iteration progress
    async fn track_iteration(&self, iteration: usize) -> Result<()>;

    /// Complete session with summary
    async fn complete_session(&self, success: bool) -> Result<()>;

    /// Get current session info
    async fn get_session_info(&self) -> Result<SessionInfo>;

    /// Resume interrupted session if available
    async fn resume_session(&self, session_id: &str) -> Result<Option<usize>>;
}

/// Default implementation of session coordinator
pub struct DefaultSessionCoordinator {
    session_manager: Arc<dyn SessionManager>,
    current_session_id: std::sync::Mutex<Option<String>>,
}

impl DefaultSessionCoordinator {
    /// Create new session coordinator
    pub fn new(session_manager: Arc<dyn SessionManager>) -> Self {
        Self {
            session_manager,
            current_session_id: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait]
impl SessionCoordinator for DefaultSessionCoordinator {
    async fn start_session(&self, session_id: &str) -> Result<()> {
        // Store current session ID
        *self.current_session_id.lock().unwrap() = Some(session_id.to_string());

        // Start session in manager
        self.session_manager.start_session(session_id).await?;

        // State update would happen here if state_manager had mutable methods

        Ok(())
    }

    async fn update_status(&self, status: SessionStatus) -> Result<()> {
        self.session_manager
            .update_session(SessionUpdate::UpdateStatus(status))
            .await
    }

    async fn track_iteration(&self, _iteration: usize) -> Result<()> {
        // Track iteration by incrementing counter
        self.session_manager
            .update_session(SessionUpdate::IncrementIteration)
            .await
    }

    async fn complete_session(&self, success: bool) -> Result<()> {
        let status = if success {
            SessionStatus::Completed
        } else {
            SessionStatus::Failed
        };

        self.update_status(status).await?;

        // State update would happen here if needed
        let _ = success; // avoid unused warning

        Ok(())
    }

    async fn get_session_info(&self) -> Result<SessionInfo> {
        let session_id = self
            .current_session_id
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        // Get state from session manager
        let state = self.session_manager.get_state();

        Ok(SessionInfo {
            session_id,
            status: state.status,
        })
    }

    async fn resume_session(&self, _session_id: &str) -> Result<Option<usize>> {
        // Check if session can be resumed
        let state = self.session_manager.get_state();
        if state.status == SessionStatus::InProgress {
            // Return current iteration count
            Ok(Some(state.iterations_completed))
        } else {
            Ok(None)
        }
    }
}
