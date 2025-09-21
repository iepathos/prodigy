//! Unified session manager implementation

use super::state::{
    Checkpoint, CheckpointId, SessionConfig, SessionFilter, SessionId, SessionStatus,
    SessionSummary, UnifiedSession,
};
use crate::storage::GlobalStorage;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::fs;

/// Session update operations
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    Status(SessionStatus),
    Metadata(HashMap<String, serde_json::Value>),
    Checkpoint(serde_json::Value),
    Error(String),
    Progress {
        current: usize,
        total: usize,
    },
    Timing {
        operation: String,
        duration: std::time::Duration,
    },
}

/// Unified session manager
pub struct SessionManager {
    storage: GlobalStorage,
    active_sessions: Arc<RwLock<HashMap<SessionId, UnifiedSession>>>,
}

impl SessionManager {
    /// Create a new session manager
    pub async fn new(storage: GlobalStorage) -> Result<Self> {
        Ok(Self {
            storage,
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new session
    pub async fn create_session(&self, config: SessionConfig) -> Result<SessionId> {
        let session = match config.session_type {
            super::state::SessionType::Workflow => {
                let workflow_id = config
                    .workflow_id
                    .ok_or_else(|| anyhow!("Workflow ID required for workflow session"))?;
                UnifiedSession::new_workflow(workflow_id, String::new())
            }
            super::state::SessionType::MapReduce => {
                let job_id = config
                    .job_id
                    .ok_or_else(|| anyhow!("Job ID required for MapReduce session"))?;
                UnifiedSession::new_mapreduce(job_id, 0)
            }
        };

        let session_id = session.id.clone();

        // Add to active sessions
        {
            let mut sessions = self.active_sessions.write().map_err(|e| {
                anyhow!("Failed to acquire write lock on active sessions: {}", e)
            })?;
            sessions.insert(session_id.clone(), session.clone());
        }

        // Persist to storage
        self.save_session(&session).await?;

        Ok(session_id)
    }

    /// Load a session by ID
    pub async fn load_session(&self, id: &SessionId) -> Result<UnifiedSession> {
        // Check active sessions first
        {
            let sessions = self.active_sessions.read().map_err(|e| {
                anyhow!("Failed to acquire read lock on active sessions: {}", e)
            })?;
            if let Some(session) = sessions.get(id) {
                return Ok(session.clone());
            }
        }

        // Load from storage
        self.load_from_storage(id).await
    }

    /// Update a session
    pub async fn update_session(&self, id: &SessionId, update: SessionUpdate) -> Result<()> {
        let mut session = self.load_session(id).await?;
        session.updated_at = chrono::Utc::now();

        match update {
            SessionUpdate::Status(status) => {
                session.status = status.clone();
                if matches!(status, SessionStatus::Completed | SessionStatus::Failed) {
                    session.completed_at = Some(chrono::Utc::now());
                }
            }
            SessionUpdate::Metadata(metadata) => {
                session.metadata.extend(metadata);
            }
            SessionUpdate::Checkpoint(state) => {
                let checkpoint = Checkpoint {
                    id: CheckpointId::new(),
                    created_at: chrono::Utc::now(),
                    state,
                    metadata: HashMap::new(),
                };
                session.checkpoints.push(checkpoint);
            }
            SessionUpdate::Error(error) => {
                session.error = Some(error);
                session.status = SessionStatus::Failed;
            }
            SessionUpdate::Progress { current, total } => {
                if let Some(workflow) = &mut session.workflow_data {
                    workflow.current_step = current;
                    workflow.total_steps = total;
                } else if let Some(mapreduce) = &mut session.mapreduce_data {
                    mapreduce.processed_items = current;
                    mapreduce.total_items = total;
                }
            }
            SessionUpdate::Timing { operation, duration } => {
                session.timings.insert(operation, duration);
            }
        }

        // Update active sessions
        {
            let mut sessions = self.active_sessions.write().map_err(|e| {
                anyhow!("Failed to acquire write lock on active sessions: {}", e)
            })?;
            sessions.insert(id.clone(), session.clone());
        }

        // Persist to storage
        self.save_session(&session).await
    }

    /// Delete a session
    pub async fn delete_session(&self, id: &SessionId) -> Result<()> {
        // Remove from active sessions
        {
            let mut sessions = self.active_sessions.write().map_err(|e| {
                anyhow!("Failed to acquire write lock on active sessions: {}", e)
            })?;
            sessions.remove(id);
        }

        // Delete from storage
        self.delete_from_storage(id).await
    }

    /// Start a session
    pub async fn start_session(&self, id: &SessionId) -> Result<()> {
        self.update_session(id, SessionUpdate::Status(SessionStatus::Running))
            .await
    }

    /// Pause a session
    pub async fn pause_session(&self, id: &SessionId) -> Result<()> {
        self.update_session(id, SessionUpdate::Status(SessionStatus::Paused))
            .await
    }

    /// Resume a session
    pub async fn resume_session(&self, id: &SessionId) -> Result<()> {
        self.update_session(id, SessionUpdate::Status(SessionStatus::Running))
            .await
    }

    /// Complete a session
    pub async fn complete_session(
        &self,
        id: &SessionId,
        success: bool,
    ) -> Result<SessionSummary> {
        let status = if success {
            SessionStatus::Completed
        } else {
            SessionStatus::Failed
        };
        self.update_session(id, SessionUpdate::Status(status))
            .await?;

        let session = self.load_session(id).await?;
        Ok(session.to_summary())
    }

    /// Create a checkpoint
    pub async fn create_checkpoint(&self, id: &SessionId) -> Result<CheckpointId> {
        let session = self.load_session(id).await?;
        let checkpoint_state = serde_json::to_value(&session)?;

        let checkpoint = Checkpoint {
            id: CheckpointId::new(),
            created_at: chrono::Utc::now(),
            state: checkpoint_state,
            metadata: HashMap::new(),
        };

        let checkpoint_id = checkpoint.id.clone();
        self.update_session(id, SessionUpdate::Checkpoint(serde_json::to_value(checkpoint)?))
            .await?;

        Ok(checkpoint_id)
    }

    /// Restore from a checkpoint
    pub async fn restore_checkpoint(
        &self,
        id: &SessionId,
        checkpoint_id: &CheckpointId,
    ) -> Result<()> {
        let session = self.load_session(id).await?;

        let checkpoint = session
            .checkpoints
            .iter()
            .find(|c| c.id == *checkpoint_id)
            .ok_or_else(|| anyhow!("Checkpoint not found"))?;

        let restored_session: UnifiedSession = serde_json::from_value(checkpoint.state.clone())?;

        // Update active sessions
        {
            let mut sessions = self.active_sessions.write().map_err(|e| {
                anyhow!("Failed to acquire write lock on active sessions: {}", e)
            })?;
            sessions.insert(id.clone(), restored_session.clone());
        }

        // Persist to storage
        self.save_session(&restored_session).await
    }

    /// List checkpoints for a session
    pub async fn list_checkpoints(&self, id: &SessionId) -> Result<Vec<Checkpoint>> {
        let session = self.load_session(id).await?;
        Ok(session.checkpoints)
    }

    /// List sessions with optional filter
    pub async fn list_sessions(&self, filter: Option<SessionFilter>) -> Result<Vec<SessionSummary>> {
        let sessions = self.load_all_sessions().await?;

        let filtered = if let Some(filter) = filter {
            sessions
                .into_iter()
                .filter(|s| {
                    if let Some(status) = &filter.status {
                        if s.status != *status {
                            return false;
                        }
                    }
                    if let Some(session_type) = &filter.session_type {
                        if s.session_type != *session_type {
                            return false;
                        }
                    }
                    if let Some(after) = &filter.after {
                        if s.started_at < *after {
                            return false;
                        }
                    }
                    if let Some(before) = &filter.before {
                        if s.started_at > *before {
                            return false;
                        }
                    }
                    if let Some(worktree_name) = &filter.worktree_name {
                        if let Some(workflow_data) = &s.workflow_data {
                            if workflow_data.worktree_name.as_ref() != Some(worktree_name) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }
                    true
                })
                .collect()
        } else {
            sessions
        };

        let summaries: Vec<SessionSummary> = filtered.iter().map(|s| s.to_summary()).collect();

        Ok(summaries)
    }

    /// Get active session IDs
    pub async fn get_active_sessions(&self) -> Result<Vec<SessionId>> {
        let sessions = self.active_sessions.read().map_err(|e| {
            anyhow!("Failed to acquire read lock on active sessions: {}", e)
        })?;
        Ok(sessions.keys().cloned().collect())
    }

    // Private helper methods

    async fn save_session(&self, session: &UnifiedSession) -> Result<()> {
        let sessions_dir = self.storage.base_dir().join("sessions");
        fs::create_dir_all(&sessions_dir)
            .await
            .context("Failed to create sessions directory")?;

        let session_file = sessions_dir.join(format!("{}.json", session.id.as_str()));
        let json = serde_json::to_string_pretty(session)?;
        fs::write(&session_file, json)
            .await
            .context("Failed to write session file")?;

        Ok(())
    }

    async fn load_from_storage(&self, id: &SessionId) -> Result<UnifiedSession> {
        let session_file = self
            .storage
            .base_dir()
            .join("sessions")
            .join(format!("{}.json", id.as_str()));

        if !session_file.exists() {
            return Err(anyhow!("Session not found: {}", id.as_str()));
        }

        let json = fs::read_to_string(&session_file)
            .await
            .context("Failed to read session file")?;
        let session: UnifiedSession = serde_json::from_str(&json)?;

        // Add to active sessions cache
        {
            let mut sessions = self.active_sessions.write().map_err(|e| {
                anyhow!("Failed to acquire write lock on active sessions: {}", e)
            })?;
            sessions.insert(id.clone(), session.clone());
        }

        Ok(session)
    }

    async fn delete_from_storage(&self, id: &SessionId) -> Result<()> {
        let session_file = self
            .storage
            .base_dir()
            .join("sessions")
            .join(format!("{}.json", id.as_str()));

        if session_file.exists() {
            fs::remove_file(&session_file)
                .await
                .context("Failed to delete session file")?;
        }

        Ok(())
    }

    async fn load_all_sessions(&self) -> Result<Vec<UnifiedSession>> {
        let sessions_dir = self.storage.base_dir().join("sessions");

        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let mut entries = fs::read_dir(&sessions_dir)
            .await
            .context("Failed to read sessions directory")?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .context("Failed to read directory entry")?
        {
            if let Some(ext) = entry.path().extension() {
                if ext == "json" {
                    let json = fs::read_to_string(entry.path())
                        .await
                        .context("Failed to read session file")?;
                    if let Ok(session) = serde_json::from_str::<UnifiedSession>(&json) {
                        sessions.push(session);
                    }
                }
            }
        }

        Ok(sessions)
    }
}