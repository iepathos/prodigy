//! Mock unified session manager for testing

use crate::unified_session::{
    SessionUpdate, SessionConfig, SessionId,
    SessionStatus, SessionSummary, UnifiedSession, SessionType,
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock unified session manager for testing
pub struct MockUnifiedSessionManager {
    pub sessions: Arc<Mutex<HashMap<SessionId, UnifiedSession>>>,
    pub update_calls: Arc<Mutex<Vec<(SessionId, SessionUpdate)>>>,
    pub should_fail: bool,
    pub start_called: Arc<Mutex<bool>>,
    pub complete_called: Arc<Mutex<bool>>,
}

impl Default for MockUnifiedSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MockUnifiedSessionManager {
    /// Create new mock unified session manager
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            update_calls: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
            start_called: Arc::new(Mutex::new(false)),
            complete_called: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a failing mock
    pub fn failing() -> Self {
        let mut mock = Self::new();
        mock.should_fail = true;
        mock
    }

    /// Get update calls for verification
    pub fn get_update_calls(&self) -> Vec<SessionUpdate> {
        self.update_calls
            .lock()
            .unwrap()
            .iter()
            .map(|(_, update)| update.clone())
            .collect()
    }

    /// Check if start was called
    pub fn was_start_called(&self) -> bool {
        *self.start_called.lock().unwrap()
    }

    /// Check if complete was called
    pub fn was_complete_called(&self) -> bool {
        *self.complete_called.lock().unwrap()
    }

    // Mock UnifiedSessionManager methods
    pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }

        let session = match config.session_type {
            SessionType::Workflow => {
                let workflow_id = config
                    .workflow_id
                    .unwrap_or_else(|| "test-workflow".to_string());
                UnifiedSession::new_workflow(workflow_id, String::new())
            }
            SessionType::MapReduce => {
                let job_id = config
                    .job_id
                    .unwrap_or_else(|| "test-job".to_string());
                UnifiedSession::new_mapreduce(job_id, 0)
            }
        };

        let session_id = session.id.clone();
        self.sessions.lock().unwrap().insert(session_id.clone(), session);
        Ok(session_id)
    }

    pub async fn load_session(&self, id: &SessionId) -> Result<UnifiedSession> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }

        self.sessions
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Session not found"))
    }

    pub async fn update_session(&self, id: &SessionId, update: SessionUpdate) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }

        self.update_calls.lock().unwrap().push((id.clone(), update.clone()));

        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(id) {
            match update {
                SessionUpdate::Status(status) => {
                    session.status = status;
                }
                SessionUpdate::Metadata(metadata) => {
                    // Handle special metadata keys
                    for (key, value) in metadata.iter() {
                        match key.as_str() {
                            "increment_iteration" => {
                                if value.as_bool().unwrap_or(false) {
                                    if let Some(workflow) = &mut session.workflow_data {
                                        workflow.iterations_completed += 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    session.metadata.extend(metadata);
                }
                SessionUpdate::Progress { current, total } => {
                    if let Some(workflow) = &mut session.workflow_data {
                        workflow.current_step = current;
                        workflow.total_steps = total;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn start_session(&self, id: &SessionId) -> Result<()> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        *self.start_called.lock().unwrap() = true;
        self.update_session(id, SessionUpdate::Status(SessionStatus::Running)).await
    }

    pub async fn complete_session(&self, id: &SessionId, success: bool) -> Result<SessionSummary> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock failure"));
        }
        *self.complete_called.lock().unwrap() = true;

        let status = if success {
            SessionStatus::Completed
        } else {
            SessionStatus::Failed
        };
        self.update_session(id, SessionUpdate::Status(status)).await?;

        let session = self.load_session(id).await?;
        Ok(SessionSummary {
            id: id.clone(),
            session_type: session.session_type,
            status: session.status,
            started_at: session.started_at,
            updated_at: session.updated_at,
            duration: Some(Duration::from_secs(60)),
            metadata: session.metadata.clone(),
        })
    }
}