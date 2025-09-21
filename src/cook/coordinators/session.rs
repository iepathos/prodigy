//! Session coordinator for managing session lifecycle

use crate::cook::session::SessionStatus as CookSessionStatus;
use crate::unified_session::{
    SessionConfig, SessionId, SessionManager as UnifiedSessionManager, SessionStatus, SessionType,
    SessionUpdate as UnifiedSessionUpdate,
};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Session information
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Session ID
    pub session_id: String,
    /// Current status
    pub status: CookSessionStatus,
}

/// Trait for session coordination
#[async_trait]
pub trait SessionCoordinator: Send + Sync {
    /// Start a new session
    async fn start_session(&self, session_id: &str) -> Result<()>;

    /// Update session status
    async fn update_status(&self, status: CookSessionStatus) -> Result<()>;

    /// Track iteration progress
    async fn track_iteration(&self, iteration: usize) -> Result<()>;

    /// Complete session with summary
    async fn complete_session(&self, success: bool) -> Result<()>;

    /// Get current session info
    async fn get_session_info(&self) -> Result<SessionInfo>;

    /// Resume interrupted session if available
    async fn resume_session(&self, session_id: &str) -> Result<Option<usize>>;
}

/// Default implementation of session coordinator using UnifiedSessionManager
pub struct DefaultSessionCoordinator {
    unified_manager: Arc<UnifiedSessionManager>,
    current_session_id: Mutex<Option<SessionId>>,
    #[allow(dead_code)]
    working_dir: std::path::PathBuf,
}

impl DefaultSessionCoordinator {
    /// Create new session coordinator
    pub fn new(
        unified_manager: Arc<UnifiedSessionManager>,
        working_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            unified_manager,
            current_session_id: Mutex::new(None),
            working_dir,
        }
    }

    /// Convert Cook session status to unified session status
    fn cook_status_to_unified(status: CookSessionStatus) -> SessionStatus {
        match status {
            CookSessionStatus::InProgress => SessionStatus::Running,
            CookSessionStatus::Completed => SessionStatus::Completed,
            CookSessionStatus::Failed => SessionStatus::Failed,
            CookSessionStatus::Interrupted => SessionStatus::Paused,
        }
    }

    /// Convert unified session status to Cook session status
    fn unified_status_to_cook(status: SessionStatus) -> CookSessionStatus {
        match status {
            SessionStatus::Initializing => CookSessionStatus::InProgress,
            SessionStatus::Running => CookSessionStatus::InProgress,
            SessionStatus::Paused => CookSessionStatus::Interrupted,
            SessionStatus::Completed => CookSessionStatus::Completed,
            SessionStatus::Failed => CookSessionStatus::Failed,
            SessionStatus::Cancelled => CookSessionStatus::Interrupted,
        }
    }
}

#[async_trait]
impl SessionCoordinator for DefaultSessionCoordinator {
    async fn start_session(&self, session_id: &str) -> Result<()> {
        // Create session configuration
        let config = SessionConfig {
            session_type: SessionType::Workflow,
            workflow_id: Some(session_id.to_string()),
            job_id: None,
            metadata: Default::default(),
        };

        // Create and start session
        let id = self.unified_manager.create_session(config).await?;
        *self.current_session_id.lock().await = Some(id.clone());
        self.unified_manager.start_session(&id).await?;

        Ok(())
    }

    async fn update_status(&self, status: CookSessionStatus) -> Result<()> {
        if let Some(id) = &*self.current_session_id.lock().await {
            let unified_status = Self::cook_status_to_unified(status);
            self.unified_manager
                .update_session(id, UnifiedSessionUpdate::Status(unified_status))
                .await?;
        }
        Ok(())
    }

    async fn track_iteration(&self, _iteration: usize) -> Result<()> {
        if let Some(id) = &*self.current_session_id.lock().await {
            // Increment iteration counter through metadata
            let mut metadata = std::collections::HashMap::new();
            metadata.insert("increment_iteration".to_string(), serde_json::json!(true));
            self.unified_manager
                .update_session(id, UnifiedSessionUpdate::Metadata(metadata))
                .await?;
        }
        Ok(())
    }

    async fn complete_session(&self, success: bool) -> Result<()> {
        let status = if success {
            CookSessionStatus::Completed
        } else {
            CookSessionStatus::Failed
        };

        self.update_status(status).await?;

        if let Some(id) = &*self.current_session_id.lock().await {
            let _ = self.unified_manager.complete_session(id, success).await?;
        }

        Ok(())
    }

    async fn get_session_info(&self) -> Result<SessionInfo> {
        if let Some(id) = &*self.current_session_id.lock().await {
            let session = self.unified_manager.load_session(id).await?;
            Ok(SessionInfo {
                session_id: id.as_str().to_string(),
                status: Self::unified_status_to_cook(session.status),
            })
        } else {
            Ok(SessionInfo {
                session_id: "unknown".to_string(),
                status: CookSessionStatus::InProgress,
            })
        }
    }

    async fn resume_session(&self, session_id: &str) -> Result<Option<usize>> {
        // Try to load the session
        let id = SessionId::from_string(session_id.to_string());
        match self.unified_manager.load_session(&id).await {
            Ok(session) => {
                if session.status == SessionStatus::Running
                    || session.status == SessionStatus::Paused
                {
                    *self.current_session_id.lock().await = Some(id);
                    // Return current iteration count from workflow data
                    if let Some(workflow_data) = &session.workflow_data {
                        Ok(Some(workflow_data.iterations_completed as usize))
                    } else {
                        Ok(Some(0))
                    }
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }
}
