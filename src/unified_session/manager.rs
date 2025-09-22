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
        let mut session = match config.session_type {
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

        // Set initial metadata from config
        session.metadata = config.metadata;

        let session_id = session.id.clone();

        // Add to active sessions
        {
            let mut sessions = self
                .active_sessions
                .write()
                .map_err(|e| anyhow!("Failed to acquire write lock on active sessions: {}", e))?;
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
            let sessions = self
                .active_sessions
                .read()
                .map_err(|e| anyhow!("Failed to acquire read lock on active sessions: {}", e))?;
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
                // Handle special metadata keys
                for (key, value) in metadata.iter() {
                    match key.as_str() {
                        "files_changed_delta" => {
                            if let Some(count) = value.as_u64() {
                                if let Some(workflow) = &mut session.workflow_data {
                                    workflow.files_changed += count as u32;
                                }
                            }
                        }
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
            SessionUpdate::Timing {
                operation,
                duration,
            } => {
                session.timings.insert(operation, duration);
            }
        }

        // Update active sessions
        {
            let mut sessions = self
                .active_sessions
                .write()
                .map_err(|e| anyhow!("Failed to acquire write lock on active sessions: {}", e))?;
            sessions.insert(id.clone(), session.clone());
        }

        // Persist to storage
        self.save_session(&session).await
    }

    /// Delete a session
    pub async fn delete_session(&self, id: &SessionId) -> Result<()> {
        // Remove from active sessions
        {
            let mut sessions = self
                .active_sessions
                .write()
                .map_err(|e| anyhow!("Failed to acquire write lock on active sessions: {}", e))?;
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
    pub async fn complete_session(&self, id: &SessionId, success: bool) -> Result<SessionSummary> {
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

        // Store just the state, the checkpoint object will be created in update_session
        self.update_session(id, SessionUpdate::Checkpoint(checkpoint_state))
            .await?;

        // Get the session again to retrieve the actual checkpoint ID
        let updated_session = self.load_session(id).await?;
        if let Some(checkpoint) = updated_session.checkpoints.last() {
            Ok(checkpoint.id.clone())
        } else {
            Err(anyhow!("Failed to create checkpoint"))
        }
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
            let mut sessions = self
                .active_sessions
                .write()
                .map_err(|e| anyhow!("Failed to acquire write lock on active sessions: {}", e))?;
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
    pub async fn list_sessions(
        &self,
        filter: Option<SessionFilter>,
    ) -> Result<Vec<SessionSummary>> {
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
        let sessions = self
            .active_sessions
            .read()
            .map_err(|e| anyhow!("Failed to acquire read lock on active sessions: {}", e))?;
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
            let mut sessions = self
                .active_sessions
                .write()
                .map_err(|e| anyhow!("Failed to acquire write lock on active sessions: {}", e))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestContext {
        _temp_dir: TempDir,
        manager: SessionManager,
    }

    impl TestContext {
        async fn new() -> Result<Self> {
            let _temp_dir = TempDir::new()?;
            let storage = GlobalStorage::new()?;
            let manager = SessionManager::new(storage).await?;
            Ok(Self { _temp_dir, manager })
        }

        fn workflow_config(&self, id: &str) -> SessionConfig {
            SessionConfig {
                session_type: super::super::state::SessionType::Workflow,
                workflow_id: Some(id.to_string()),
                job_id: None,
                metadata: HashMap::new(),
            }
        }

        fn mapreduce_config(&self, id: &str) -> SessionConfig {
            SessionConfig {
                session_type: super::super::state::SessionType::MapReduce,
                job_id: Some(id.to_string()),
                workflow_id: None,
                metadata: HashMap::new(),
            }
        }
    }

    #[tokio::test]
    async fn test_create_workflow_session() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("test-workflow");

        let session_id = ctx.manager.create_session(config).await?;
        assert!(!session_id.as_str().is_empty());

        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(session.status, SessionStatus::Initializing);
        assert!(session.workflow_data.is_some());
        assert!(session.mapreduce_data.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_create_mapreduce_session() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.mapreduce_config("test-job");

        let session_id = ctx.manager.create_session(config).await?;
        let session = ctx.manager.load_session(&session_id).await?;

        assert_eq!(session.status, SessionStatus::Initializing);
        assert!(session.workflow_data.is_none());
        assert!(session.mapreduce_data.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_create_session_missing_workflow_id() {
        let ctx = TestContext::new().await.unwrap();
        let config = SessionConfig {
            session_type: super::super::state::SessionType::Workflow,
            workflow_id: None,
            job_id: None,
            metadata: HashMap::new(),
        };

        let result = ctx.manager.create_session(config).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Workflow ID required"));
    }

    #[tokio::test]
    async fn test_create_session_missing_job_id() {
        let ctx = TestContext::new().await.unwrap();
        let config = SessionConfig {
            session_type: super::super::state::SessionType::MapReduce,
            workflow_id: None,
            job_id: None,
            metadata: HashMap::new(),
        };

        let result = ctx.manager.create_session(config).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Job ID required"));
    }

    #[tokio::test]
    async fn test_session_lifecycle() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("lifecycle-test");
        let session_id = ctx.manager.create_session(config).await?;

        // Start session
        ctx.manager.start_session(&session_id).await?;
        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(session.status, SessionStatus::Running);

        // Pause session
        ctx.manager.pause_session(&session_id).await?;
        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(session.status, SessionStatus::Paused);

        // Resume session
        ctx.manager.resume_session(&session_id).await?;
        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(session.status, SessionStatus::Running);

        // Complete session
        let summary = ctx.manager.complete_session(&session_id, true).await?;
        assert_eq!(summary.status, SessionStatus::Completed);
        assert!(summary.duration.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_session_failure() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("failure-test");
        let session_id = ctx.manager.create_session(config).await?;

        ctx.manager.start_session(&session_id).await?;
        let summary = ctx.manager.complete_session(&session_id, false).await?;

        assert_eq!(summary.status, SessionStatus::Failed);
        assert!(summary.duration.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_update_metadata() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("metadata-test");
        let session_id = ctx.manager.create_session(config).await?;

        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), serde_json::json!("value1"));
        metadata.insert("key2".to_string(), serde_json::json!(42));

        ctx.manager
            .update_session(&session_id, SessionUpdate::Metadata(metadata.clone()))
            .await?;

        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(
            session.metadata.get("key1"),
            Some(&serde_json::json!("value1"))
        );
        assert_eq!(session.metadata.get("key2"), Some(&serde_json::json!(42)));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_files_changed_delta() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("files-changed-test");
        let session_id = ctx.manager.create_session(config).await?;

        let mut metadata = HashMap::new();
        metadata.insert("files_changed_delta".to_string(), serde_json::json!(5));

        ctx.manager
            .update_session(&session_id, SessionUpdate::Metadata(metadata))
            .await?;

        let session = ctx.manager.load_session(&session_id).await?;
        if let Some(workflow) = &session.workflow_data {
            assert_eq!(workflow.files_changed, 5);
        } else {
            panic!("Expected workflow data");
        }

        // Update again to test accumulation
        let mut metadata = HashMap::new();
        metadata.insert("files_changed_delta".to_string(), serde_json::json!(3));

        ctx.manager
            .update_session(&session_id, SessionUpdate::Metadata(metadata))
            .await?;

        let session = ctx.manager.load_session(&session_id).await?;
        if let Some(workflow) = &session.workflow_data {
            assert_eq!(workflow.files_changed, 8);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_update_error() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("error-test");
        let session_id = ctx.manager.create_session(config).await?;

        ctx.manager.start_session(&session_id).await?;
        ctx.manager
            .update_session(&session_id, SessionUpdate::Error("Test error".to_string()))
            .await?;

        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(session.status, SessionStatus::Failed);
        assert_eq!(session.error, Some("Test error".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_update_progress() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("progress-test");
        let session_id = ctx.manager.create_session(config).await?;

        ctx.manager
            .update_session(
                &session_id,
                SessionUpdate::Progress {
                    current: 3,
                    total: 10,
                },
            )
            .await?;

        let session = ctx.manager.load_session(&session_id).await?;
        if let Some(workflow) = &session.workflow_data {
            assert_eq!(workflow.current_step, 3);
            assert_eq!(workflow.total_steps, 10);
        } else {
            panic!("Expected workflow data");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_checkpoint_creation_and_restore() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("checkpoint-test");
        let session_id = ctx.manager.create_session(config).await?;

        // Modify session state
        ctx.manager.start_session(&session_id).await?;
        let mut metadata = HashMap::new();
        metadata.insert("test_key".to_string(), serde_json::json!("test_value"));
        ctx.manager
            .update_session(&session_id, SessionUpdate::Metadata(metadata))
            .await?;

        // Create checkpoint
        let checkpoint_id = ctx.manager.create_checkpoint(&session_id).await?;
        assert!(!checkpoint_id.as_str().is_empty());

        // Verify checkpoint was stored
        let session = ctx.manager.load_session(&session_id).await?;
        assert_eq!(session.checkpoints.len(), 1);
        assert_eq!(session.checkpoints[0].id, checkpoint_id);

        // Modify session further
        ctx.manager.pause_session(&session_id).await?;

        // Restore checkpoint
        ctx.manager
            .restore_checkpoint(&session_id, &checkpoint_id)
            .await?;

        // Verify state was restored
        let restored = ctx.manager.load_session(&session_id).await?;
        assert_eq!(restored.status, SessionStatus::Running);
        assert_eq!(
            restored.metadata.get("test_key"),
            Some(&serde_json::json!("test_value"))
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_session() -> Result<()> {
        let ctx = TestContext::new().await?;
        let config = ctx.workflow_config("delete-test");
        let session_id = ctx.manager.create_session(config).await?;

        // Verify session exists
        let _ = ctx.manager.load_session(&session_id).await?;

        // Delete session
        ctx.manager.delete_session(&session_id).await?;

        // Verify session no longer exists
        let result = ctx.manager.load_session(&session_id).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_list_sessions() -> Result<()> {
        let ctx = TestContext::new().await?;

        // Create multiple sessions
        let id1 = ctx
            .manager
            .create_session(ctx.workflow_config("list-test-1"))
            .await?;
        let id2 = ctx
            .manager
            .create_session(ctx.workflow_config("list-test-2"))
            .await?;
        let id3 = ctx
            .manager
            .create_session(ctx.mapreduce_config("list-test-3"))
            .await?;

        // Start some sessions
        ctx.manager.start_session(&id1).await?;
        ctx.manager.complete_session(&id2, true).await?;

        // List all sessions - should have at least the 3 we created
        let all_sessions = ctx.manager.list_sessions(None).await?;
        assert!(all_sessions.len() >= 3);

        // List running sessions
        let running_filter = SessionFilter {
            status: Some(SessionStatus::Running),
            ..Default::default()
        };
        let running = ctx.manager.list_sessions(Some(running_filter)).await?;
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, id1);

        // List completed sessions
        let completed_filter = SessionFilter {
            status: Some(SessionStatus::Completed),
            ..Default::default()
        };
        let completed = ctx.manager.list_sessions(Some(completed_filter)).await?;
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, id2);

        // List by type
        let workflow_filter = SessionFilter {
            session_type: Some(super::super::state::SessionType::Workflow),
            ..Default::default()
        };
        let workflows = ctx.manager.list_sessions(Some(workflow_filter)).await?;
        // Should have at least the 2 workflow sessions we created
        assert!(workflows.len() >= 2);

        let mapreduce_filter = SessionFilter {
            session_type: Some(super::super::state::SessionType::MapReduce),
            ..Default::default()
        };
        let mapreduce = ctx.manager.list_sessions(Some(mapreduce_filter)).await?;
        // Should have at least the 1 mapreduce session we created
        assert!(mapreduce.len() >= 1);
        // Check if our created session is in the list
        assert!(mapreduce.iter().any(|s| s.id == id3));

        Ok(())
    }

    #[tokio::test]
    async fn test_session_persistence() -> Result<()> {
        let _temp_dir = TempDir::new()?;
        let session_id;

        // Create session and manager in a scope
        {
            let storage = GlobalStorage::new()?;
            let manager = SessionManager::new(storage).await?;

            let config = SessionConfig {
                session_type: super::super::state::SessionType::Workflow,
                workflow_id: Some("persistence-test".to_string()),
                job_id: None,
                metadata: HashMap::new(),
            };

            session_id = manager.create_session(config).await?;
            manager.start_session(&session_id).await?;
        }

        // Create new manager with same storage path
        {
            let storage = GlobalStorage::new()?;
            let manager = SessionManager::new(storage).await?;

            // Should be able to load the persisted session
            let session = manager.load_session(&session_id).await?;
            assert_eq!(session.status, SessionStatus::Running);
        }

        Ok(())
    }
}
