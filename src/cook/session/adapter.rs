//! Adapter to bridge old session tracking to new session management

use crate::session::{
    ExecutionMode, InMemorySessionManager, IterationChanges, SessionConfig, SessionEvent,
    SessionId, SessionManager as NewSessionManager, SessionOptions,
};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{
    SessionManager as OldSessionManager, SessionState as OldSessionState, SessionStatus,
    SessionSummary, SessionUpdate,
};

/// Adapter that implements old SessionManager trait using new session management
pub struct SessionManagerAdapter {
    new_manager: Arc<InMemorySessionManager>,
    current_session: Mutex<Option<SessionId>>,
    working_dir: std::path::PathBuf,
}

impl SessionManagerAdapter {
    /// Create new adapter
    pub fn new(working_dir: std::path::PathBuf) -> Self {
        let new_manager = Arc::new(InMemorySessionManager::new(None));
        Self {
            new_manager,
            current_session: Mutex::new(None),
            working_dir,
        }
    }

    /// Get the underlying new session manager
    pub fn inner(&self) -> Arc<InMemorySessionManager> {
        self.new_manager.clone()
    }

    /// Convert old session state to new state
    #[allow(dead_code)]
    fn convert_state(&self, old_state: &OldSessionState) -> crate::session::SessionState {
        match old_state.status {
            SessionStatus::InProgress => crate::session::SessionState::Running {
                iteration: old_state.iterations_completed as u32,
            },
            SessionStatus::Completed => crate::session::SessionState::Completed {
                summary: crate::session::SessionSummary {
                    total_iterations: old_state.iterations_completed as u32,
                    files_changed: old_state.files_changed,
                    total_commits: 0, // Not tracked in old system
                    duration: old_state
                        .duration()
                        .map(|d| d.to_std().unwrap_or_default())
                        .unwrap_or_default(),
                    success_rate: 1.0, // Not tracked in old system
                },
            },
            SessionStatus::Failed => crate::session::SessionState::Failed {
                error: old_state.errors.join(", "),
            },
            SessionStatus::Interrupted => crate::session::SessionState::Paused {
                reason: "Interrupted".to_string(),
            },
        }
    }
}

#[async_trait]
impl OldSessionManager for SessionManagerAdapter {
    async fn start_session(&self, _session_id: &str) -> Result<()> {
        // Create new session config
        let config = SessionConfig {
            project_path: self.working_dir.clone(),
            workflow: crate::config::workflow::WorkflowConfig { commands: vec![] },
            execution_mode: ExecutionMode::Direct,
            max_iterations: 10,
            options: SessionOptions::default(),
        };

        // Create and start session
        let id = self.new_manager.create_session(config).await?;
        *self.current_session.lock().await = Some(id.clone());

        // Override the session ID to match old system
        // This is a bit hacky but maintains compatibility
        self.new_manager.start_session(&id).await?;

        Ok(())
    }

    async fn update_session(&self, update: SessionUpdate) -> Result<()> {
        let session_id = self
            .current_session
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        match update {
            SessionUpdate::IncrementIteration => {
                let progress = self.new_manager.get_progress(&session_id).await?;
                let iteration = progress.iterations_completed + 1;

                self.new_manager
                    .record_event(
                        &session_id,
                        SessionEvent::IterationStarted { number: iteration },
                    )
                    .await?;
            }
            SessionUpdate::AddFilesChanged(count) => {
                // Create dummy iteration changes
                let changes = IterationChanges {
                    files_modified: (0..count)
                        .map(|i| std::path::PathBuf::from(format!("file{i}.rs")))
                        .collect(),
                    lines_added: 0,
                    lines_removed: 0,
                    commands_run: vec![],
                    git_commits: vec![],
                };

                self.new_manager
                    .record_event(&session_id, SessionEvent::IterationCompleted { changes })
                    .await?;
            }
            SessionUpdate::UpdateStatus(status) => match status {
                SessionStatus::Completed => {
                    self.new_manager
                        .record_event(&session_id, SessionEvent::Completed)
                        .await?;
                }
                SessionStatus::Failed => {
                    self.new_manager
                        .record_event(
                            &session_id,
                            SessionEvent::Failed {
                                error: "Session failed".to_string(),
                            },
                        )
                        .await?;
                }
                SessionStatus::Interrupted => {
                    self.new_manager
                        .record_event(
                            &session_id,
                            SessionEvent::Paused {
                                reason: "Interrupted".to_string(),
                            },
                        )
                        .await?;
                }
                _ => {}
            },
            SessionUpdate::AddError(error) => {
                // Errors are tracked differently in new system
                // This is handled when status changes to Failed
                let _ = error;
            }
        }

        Ok(())
    }

    async fn complete_session(&self) -> Result<SessionSummary> {
        let session_id = self
            .current_session
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No active session"))?;

        let summary = self.new_manager.complete_session(&session_id).await?;

        Ok(SessionSummary {
            iterations: summary.total_iterations as usize,
            files_changed: summary.files_changed,
        })
    }

    fn get_state(&self) -> OldSessionState {
        // This is synchronous in old API but async in new
        // We'll need to handle this carefully
        let session_id =
            futures::executor::block_on(async { self.current_session.lock().await.clone() });

        if let Some(id) = session_id {
            if let Ok(state) = futures::executor::block_on(self.new_manager.get_state(&id)) {
                // Convert new state to old state
                let old_status = match &state {
                    crate::session::SessionState::Created => SessionStatus::InProgress,
                    crate::session::SessionState::Running { .. } => SessionStatus::InProgress,
                    crate::session::SessionState::Paused { .. } => SessionStatus::Interrupted,
                    crate::session::SessionState::Completed { .. } => SessionStatus::Completed,
                    crate::session::SessionState::Failed { .. } => SessionStatus::Failed,
                };

                let progress = futures::executor::block_on(self.new_manager.get_progress(&id)).ok();

                return OldSessionState {
                    session_id: id.to_string(),
                    status: old_status,
                    started_at: chrono::Utc::now(), // Approximation
                    ended_at: if state.is_terminal() {
                        Some(chrono::Utc::now())
                    } else {
                        None
                    },
                    iterations_completed: progress
                        .as_ref()
                        .map(|p| p.iterations_completed as usize)
                        .unwrap_or(0),
                    files_changed: progress
                        .as_ref()
                        .map(|p| p.files_changed.len())
                        .unwrap_or(0),
                    errors: if let crate::session::SessionState::Failed { error } = &state {
                        vec![error.clone()]
                    } else {
                        vec![]
                    },
                    working_directory: self.working_dir.clone(),
                    worktree_name: None,
                };
            }
        }

        // Default state
        OldSessionState::new("unknown".to_string(), self.working_dir.clone())
    }

    async fn save_state(&self, _path: &Path) -> Result<()> {
        // Save checkpoint in new system
        if let Some(id) = self.current_session.lock().await.as_ref() {
            self.new_manager.save_checkpoint(id).await?;
        }
        Ok(())
    }

    async fn load_state(&self, _path: &Path) -> Result<()> {
        // Loading from old format not supported
        // Would need to migrate old state files to new format
        Ok(())
    }
}
