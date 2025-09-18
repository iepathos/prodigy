//! Comprehensive tests for storage backends

use super::*;
use crate::storage::backends::{FileBackend, MemoryBackend};
use crate::storage::config::{BackendConfig, FileConfig, MemoryConfig, StorageConfig};
use crate::storage::error::StorageResult;
use crate::storage::traits::{EventStorage, SessionStorage, StateStorage, UnifiedStorage};
use crate::storage::types::{
    CheckpointData, EventEntry, JobState, JobStatus, SessionState, SessionStatus,
    WorkflowCheckpoint,
};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use tempfile::TempDir;
use uuid::Uuid;

/// Create a test session
fn create_test_session(id: &str) -> SessionState {
    SessionState {
        session_id: id.to_string(),
        repository: "test-repo".to_string(),
        status: SessionStatus::InProgress,
        started_at: Utc::now(),
        completed_at: None,
        workflow_path: Some("/test/workflow.yaml".to_string()),
        git_branch: Some("test-branch".to_string()),
        iterations_completed: 0,
        files_changed: 0,
        worktree_name: Some(format!("worktree-{}", id)),
        iteration_timings: HashMap::new(),
        command_timings: HashMap::new(),
        metadata: HashMap::new(),
    }
}

/// Create a test event
fn create_test_event(job_id: &str, event_type: &str) -> EventEntry {
    EventEntry {
        timestamp: Utc::now(),
        event_type: event_type.to_string(),
        job_id: job_id.to_string(),
        work_item_id: Some(format!("item-{}", Uuid::new_v4())),
        agent_id: Some(format!("agent-{}", Uuid::new_v4())),
        correlation_id: Some(Uuid::new_v4()),
        message: Some("Test event message".to_string()),
        data: serde_json::json!({"test": "data"}),
        error: None,
    }
}

/// Create a test job state
fn create_test_job_state(job_id: &str) -> JobState {
    JobState {
        job_id: job_id.to_string(),
        repository: "test-repo".to_string(),
        workflow_name: "test-workflow".to_string(),
        status: JobStatus::Running,
        started_at: Utc::now(),
        completed_at: None,
        total_items: 10,
        processed_items: 5,
        successful_items: 4,
        failed_items: 1,
        current_phase: "processing".to_string(),
        error: None,
        metadata: HashMap::new(),
    }
}

/// Create a test checkpoint
fn create_test_checkpoint(checkpoint_id: &str) -> WorkflowCheckpoint {
    WorkflowCheckpoint {
        repository: "test-repo".to_string(),
        session_id: Some("session-123".to_string()),
        job_id: Some("job-456".to_string()),
        created_at: Utc::now(),
        data: CheckpointData {
            workflow_path: "/test/workflow.yaml".to_string(),
            current_step: 3,
            total_steps: 10,
            completed_steps: vec!["step1".to_string(), "step2".to_string()],
            context: serde_json::json!({"key": "value"}),
            variables: HashMap::from([("var1".to_string(), "value1".to_string())]),
            state: serde_json::json!({"state": "data"}),
        },
        metadata: HashMap::from([("meta_key".to_string(), "meta_value".to_string())]),
    }
}

/// Test harness for running tests against any storage backend
async fn test_backend<T: UnifiedStorage>(storage: &T) -> StorageResult<()> {
    // Test session storage
    test_session_storage(storage.session_storage()).await?;

    // Test event storage
    test_event_storage(storage.event_storage()).await?;

    // Test state storage
    test_state_storage(storage.state_storage()).await?;

    // Test health check
    let health = storage.health_check().await?;
    assert!(health.healthy);

    Ok(())
}

/// Test session storage operations
async fn test_session_storage(storage: &dyn SessionStorage) -> StorageResult<()> {
    let session1 = create_test_session("session-1");
    let session2 = create_test_session("session-2");

    // Test save
    storage.save_session(&session1).await?;
    storage.save_session(&session2).await?;

    // Test load
    let loaded = storage.load_session("session-1").await?;
    assert_eq!(loaded.session_id, session1.session_id);
    assert_eq!(loaded.repository, session1.repository);

    // Test update
    let mut updated_session = session1.clone();
    updated_session.status = SessionStatus::Completed;
    updated_session.completed_at = Some(Utc::now());
    storage.save_session(&updated_session).await?;

    let loaded = storage.load_session("session-1").await?;
    assert_eq!(loaded.status, SessionStatus::Completed);
    assert!(loaded.completed_at.is_some());

    // Test list
    let sessions = storage.list_sessions("test-repo").await?;
    assert!(sessions.len() >= 2);

    // Test delete
    storage.delete_session("session-1").await?;
    let sessions = storage.list_sessions("test-repo").await?;
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, "session-2");

    Ok(())
}

/// Test event storage operations
async fn test_event_storage(storage: &dyn EventStorage) -> StorageResult<()> {
    let job_id = "test-job-1";
    let event1 = create_test_event(job_id, "started");
    let event2 = create_test_event(job_id, "progress");
    let event3 = create_test_event(job_id, "completed");

    // Test append
    storage.append_event("test-repo", job_id, &event1).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    storage.append_event("test-repo", job_id, &event2).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    storage.append_event("test-repo", job_id, &event3).await?;

    // Test read all events
    let events = storage.read_events("test-repo", job_id, None).await?;
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, "started");
    assert_eq!(events[1].event_type, "progress");
    assert_eq!(events[2].event_type, "completed");

    // Test read events since timestamp
    let since = event1.timestamp + Duration::milliseconds(5);
    let events = storage.read_events("test-repo", job_id, Some(since)).await?;
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "progress");
    assert_eq!(events[1].event_type, "completed");

    // Test list job IDs
    let job_id2 = "test-job-2";
    let event4 = create_test_event(job_id2, "started");
    storage.append_event("test-repo", job_id2, &event4).await?;

    let job_ids = storage.list_job_ids("test-repo").await?;
    assert!(job_ids.contains(&job_id.to_string()));
    assert!(job_ids.contains(&job_id2.to_string()));

    // Test cleanup old events
    let older_than = Utc::now() + Duration::seconds(1);
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let deleted = storage.cleanup_old_events("test-repo", older_than).await?;
    assert!(deleted > 0);

    let events = storage.read_events("test-repo", job_id, None).await?;
    assert_eq!(events.len(), 0);

    Ok(())
}

/// Test state storage operations
async fn test_state_storage(storage: &dyn StateStorage) -> StorageResult<()> {
    let job_id = "test-job-state-1";
    let job_state = create_test_job_state(job_id);

    // Test save job state
    storage.save_job_state(job_id, &job_state).await?;

    // Test load job state
    let loaded = storage.load_job_state(job_id).await?;
    assert_eq!(loaded.job_id, job_state.job_id);
    assert_eq!(loaded.status, JobStatus::Running);
    assert_eq!(loaded.processed_items, 5);

    // Test update job state
    let mut updated_state = job_state.clone();
    updated_state.status = JobStatus::Completed;
    updated_state.completed_at = Some(Utc::now());
    updated_state.processed_items = 10;
    storage.save_job_state(job_id, &updated_state).await?;

    let loaded = storage.load_job_state(job_id).await?;
    assert_eq!(loaded.status, JobStatus::Completed);
    assert_eq!(loaded.processed_items, 10);
    assert!(loaded.completed_at.is_some());

    // Test checkpoint operations
    let checkpoint_id = "test-checkpoint-1";
    let checkpoint = create_test_checkpoint(checkpoint_id);

    // Test save checkpoint
    storage.save_checkpoint(checkpoint_id, &checkpoint).await?;

    // Test load checkpoint
    let loaded_checkpoint = storage.load_checkpoint(checkpoint_id).await?;
    assert_eq!(loaded_checkpoint.repository, checkpoint.repository);
    assert_eq!(loaded_checkpoint.data.current_step, 3);
    assert_eq!(loaded_checkpoint.data.completed_steps.len(), 2);

    // Test list checkpoints
    let checkpoint_id2 = "test-checkpoint-2";
    let checkpoint2 = create_test_checkpoint(checkpoint_id2);
    storage.save_checkpoint(checkpoint_id2, &checkpoint2).await?;

    let checkpoint_ids = storage.list_checkpoints("test-repo").await?;
    assert!(checkpoint_ids.contains(&checkpoint_id.to_string()));
    assert!(checkpoint_ids.contains(&checkpoint_id2.to_string()));

    // Test delete checkpoint
    storage.delete_checkpoint(checkpoint_id).await?;
    let checkpoint_ids = storage.list_checkpoints("test-repo").await?;
    assert!(!checkpoint_ids.contains(&checkpoint_id.to_string()));
    assert!(checkpoint_ids.contains(&checkpoint_id2.to_string()));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_backend() {
        let config = StorageConfig {
            backend: super::config::BackendType::Memory,
            backend_config: BackendConfig::Memory(MemoryConfig::default()),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        };

        let backend = MemoryBackend::new(&config).unwrap();
        test_backend(&backend).await.unwrap();
    }

    #[tokio::test]
    async fn test_file_backend() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            backend: super::config::BackendType::File,
            backend_config: BackendConfig::File(FileConfig {
                base_dir: temp_dir.path().to_path_buf(),
                use_global: false,
                enable_file_locks: true,
                max_file_size: 1024 * 1024,
                enable_compression: false,
            }),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        };

        let backend = FileBackend::new(&config).await.unwrap();
        test_backend(&backend).await.unwrap();
    }

    #[tokio::test]
    async fn test_factory_file_backend() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            backend: super::config::BackendType::File,
            backend_config: BackendConfig::File(FileConfig {
                base_dir: temp_dir.path().to_path_buf(),
                use_global: false,
                enable_file_locks: true,
                max_file_size: 1024 * 1024,
                enable_compression: false,
            }),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        };

        let storage = StorageFactory::from_config(&config).await.unwrap();
        let health = storage.health_check().await.unwrap();
        assert!(health.healthy);
        assert_eq!(health.backend_type, "file");
    }

    #[test]
    fn test_factory_memory_backend() {
        let storage = StorageFactory::create_test_storage();
        let _ = storage.session_storage();
        let _ = storage.event_storage();
        let _ = storage.state_storage();
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let config = StorageConfig {
            backend: super::config::BackendType::Memory,
            backend_config: BackendConfig::Memory(MemoryConfig::default()),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        };

        let backend = MemoryBackend::new(&config).unwrap();
        let backend = std::sync::Arc::new(backend);

        // Test concurrent session saves
        let mut handles = vec![];
        for i in 0..10 {
            let backend_clone = backend.clone();
            let handle = tokio::spawn(async move {
                let session = create_test_session(&format!("concurrent-{}", i));
                backend_clone.session_storage().save_session(&session).await
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        let sessions = backend.session_storage().list_sessions("test-repo").await.unwrap();
        assert_eq!(sessions.len(), 10);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let backend = MemoryBackend::new(&StorageConfig {
            backend: super::config::BackendType::Memory,
            backend_config: BackendConfig::Memory(MemoryConfig::default()),
            connection_pool_size: 10,
            retry_policy: Default::default(),
            timeout: std::time::Duration::from_secs(30),
            enable_locking: true,
            enable_cache: false,
            cache_config: Default::default(),
        })
        .unwrap();

        // Test loading non-existent session
        let result = backend.session_storage().load_session("non-existent").await;
        assert!(result.is_err());

        // Test loading non-existent job state
        let result = backend.state_storage().load_job_state("non-existent").await;
        assert!(result.is_err());

        // Test loading non-existent checkpoint
        let result = backend.state_storage().load_checkpoint("non-existent").await;
        assert!(result.is_err());
    }
}