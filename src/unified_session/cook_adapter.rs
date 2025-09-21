//! Adapter to bridge cook module with unified session management

use super::{
    manager::{SessionManager as UnifiedSessionManager, SessionUpdate as UnifiedSessionUpdate},
    state::{SessionConfig, SessionId, SessionStatus, SessionType, UnifiedSession},
};
use crate::cook::session::{
    SessionInfo, SessionManager as CookSessionManager, SessionState as CookSessionState,
    SessionStatus as CookSessionStatus, SessionSummary as CookSessionSummary,
    SessionUpdate as CookSessionUpdate,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Adapter that implements Cook's SessionManager trait using unified session management
pub struct CookSessionAdapter {
    unified_manager: Arc<UnifiedSessionManager>,
    current_session: Mutex<Option<SessionId>>,
    working_dir: std::path::PathBuf,
}

impl CookSessionAdapter {
    /// Create new adapter
    pub async fn new(
        working_dir: std::path::PathBuf,
        storage: crate::storage::GlobalStorage,
    ) -> Result<Self> {
        let unified_manager = Arc::new(UnifiedSessionManager::new(storage).await?);
        Ok(Self {
            unified_manager,
            current_session: Mutex::new(None),
            working_dir,
        })
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

    /// Convert unified session to Cook session state
    fn unified_to_cook_state(
        session: &UnifiedSession,
        working_dir: &std::path::Path,
    ) -> CookSessionState {
        let mut state =
            CookSessionState::new(session.id.as_str().to_string(), working_dir.to_path_buf());
        state.status = Self::unified_status_to_cook(session.status.clone());
        state.started_at = session.started_at;

        // Map workflow-specific data
        if let Some(workflow_data) = &session.workflow_data {
            state.iterations_completed = workflow_data.iterations_completed as usize;
            state.files_changed = workflow_data.files_changed as usize;
            state.worktree_name = workflow_data.worktree_name.clone();
        }

        // Map error if present
        if let Some(error) = &session.error {
            state.errors.push(error.clone());
        }

        state
    }

    /// Convert Cook session update to unified session updates
    fn cook_update_to_unified(update: CookSessionUpdate) -> Vec<UnifiedSessionUpdate> {
        match update {
            CookSessionUpdate::IncrementIteration => {
                // Increment iteration counter through metadata
                let mut metadata = std::collections::HashMap::new();
                metadata.insert("increment_iteration".to_string(), serde_json::json!(true));
                vec![UnifiedSessionUpdate::Metadata(metadata)]
            }
            CookSessionUpdate::AddFilesChanged(count) => {
                // Store in metadata and we'll accumulate this in workflow_data
                let mut metadata = std::collections::HashMap::new();
                metadata.insert("files_changed_delta".to_string(), serde_json::json!(count));
                vec![UnifiedSessionUpdate::Metadata(metadata)]
            }
            CookSessionUpdate::UpdateStatus(status) => {
                vec![UnifiedSessionUpdate::Status(Self::cook_status_to_unified(
                    status,
                ))]
            }
            CookSessionUpdate::StartIteration(_) | CookSessionUpdate::CompleteIteration => {
                vec![]
            }
            CookSessionUpdate::RecordCommandTiming(command, duration) => {
                vec![UnifiedSessionUpdate::Timing {
                    operation: command,
                    duration,
                }]
            }
            CookSessionUpdate::MarkInterrupted => {
                vec![UnifiedSessionUpdate::Status(SessionStatus::Paused)]
            }
            CookSessionUpdate::AddError(error) => {
                vec![UnifiedSessionUpdate::Error(error)]
            }
            _ => vec![],
        }
    }
}

#[async_trait]
impl CookSessionManager for CookSessionAdapter {
    async fn start_session(&self, session_id: &str) -> Result<()> {
        let config = SessionConfig {
            session_type: SessionType::Workflow,
            workflow_id: Some(session_id.to_string()),
            job_id: None,
            metadata: Default::default(),
        };

        let id = self.unified_manager.create_session(config).await?;
        *self.current_session.lock().await = Some(id.clone());
        self.unified_manager.start_session(&id).await
    }

    async fn update_session(&self, update: CookSessionUpdate) -> Result<()> {
        if let Some(id) = &*self.current_session.lock().await {
            let unified_updates = Self::cook_update_to_unified(update);
            for unified_update in unified_updates {
                self.unified_manager
                    .update_session(id, unified_update)
                    .await?;
            }
        }
        Ok(())
    }

    async fn complete_session(&self) -> Result<CookSessionSummary> {
        if let Some(id) = &*self.current_session.lock().await {
            let session = self.unified_manager.load_session(id).await?;
            let _ = self.unified_manager.complete_session(id, true).await?;

            let iterations = if let Some(workflow_data) = &session.workflow_data {
                workflow_data.iterations_completed as usize
            } else {
                0
            };

            let files_changed = if let Some(workflow_data) = &session.workflow_data {
                workflow_data.files_changed as usize
            } else {
                0
            };

            Ok(CookSessionSummary {
                iterations,
                files_changed,
            })
        } else {
            Ok(CookSessionSummary {
                iterations: 0,
                files_changed: 0,
            })
        }
    }

    fn get_state(&self) -> CookSessionState {
        // This is synchronous in the trait but we need async for unified manager
        // Return a default state with working directory
        CookSessionState::new("default-session".to_string(), self.working_dir.clone())
    }

    async fn save_state(&self, _path: &Path) -> Result<()> {
        // State is automatically persisted by unified manager
        Ok(())
    }

    async fn load_state(&self, _path: &Path) -> Result<()> {
        // State is automatically loaded by unified manager
        Ok(())
    }

    async fn load_session(&self, session_id: &str) -> Result<CookSessionState> {
        let id = SessionId::from_string(session_id.to_string());
        let session = self.unified_manager.load_session(&id).await?;
        Ok(Self::unified_to_cook_state(&session, &self.working_dir))
    }

    async fn save_checkpoint(&self, state: &CookSessionState) -> Result<()> {
        if let Some(id) = &*self.current_session.lock().await {
            let checkpoint_data = serde_json::to_value(state)?;
            self.unified_manager
                .update_session(id, UnifiedSessionUpdate::Checkpoint(checkpoint_data))
                .await
        } else {
            Ok(())
        }
    }

    async fn list_resumable(&self) -> Result<Vec<SessionInfo>> {
        let filter = super::state::SessionFilter {
            status: Some(SessionStatus::Paused),
            ..Default::default()
        };
        let summaries = self.unified_manager.list_sessions(Some(filter)).await?;

        Ok(summaries
            .into_iter()
            .map(|s| SessionInfo {
                session_id: s.id.as_str().to_string(),
                status: Self::unified_status_to_cook(s.status),
                started_at: s.started_at,
                workflow_path: self.working_dir.clone(),
                progress: format!("Session {}", s.id.as_str()),
            })
            .collect())
    }

    async fn get_last_interrupted(&self) -> Result<Option<String>> {
        let filter = super::state::SessionFilter {
            status: Some(SessionStatus::Paused),
            limit: Some(1),
            ..Default::default()
        };
        let summaries = self.unified_manager.list_sessions(Some(filter)).await?;
        Ok(summaries.first().map(|s| s.id.as_str().to_string()))
    }
}
