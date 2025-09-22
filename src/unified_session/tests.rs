//! Unit tests for the unified session management module

#[cfg(test)]
use super::*;
use crate::cook::session::SessionManager as CookSessionManager;
use crate::storage::GlobalStorage;
use tempfile::TempDir;

#[tokio::test]
async fn test_session_manager_creation() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Should be able to list sessions (empty initially)
    let sessions = manager.list_sessions(None).await.unwrap();
    assert_eq!(sessions.len(), 0);
}

#[tokio::test]
async fn test_session_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Create a session
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "project_name".to_string(),
        serde_json::json!("test-project"),
    );
    metadata.insert(
        "workflow_name".to_string(),
        serde_json::json!("test-workflow"),
    );
    metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
    metadata.insert("tags".to_string(), serde_json::json!(vec!["test"]));
    metadata.insert("description".to_string(), serde_json::json!("Test session"));

    let config = SessionConfig {
        session_type: SessionType::Workflow,
        workflow_id: Some("test-workflow".to_string()),
        job_id: None,
        metadata,
    };

    let session_id = manager.create_session(config).await.unwrap();
    assert!(!session_id.as_str().is_empty());

    // Start the session
    let update = SessionUpdate::Status(SessionStatus::Running);
    manager.update_session(&session_id, update).await.unwrap();

    // Load the session
    let session = manager.load_session(&session_id).await.unwrap();
    assert_eq!(session.status, SessionStatus::Running);

    // Complete the session
    let update = SessionUpdate::Status(SessionStatus::Completed);
    manager.update_session(&session_id, update).await.unwrap();

    // Verify completion
    let session = manager.load_session(&session_id).await.unwrap();
    assert_eq!(session.status, SessionStatus::Completed);
}

#[tokio::test]
async fn test_session_progress_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Create a workflow session
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "project_name".to_string(),
        serde_json::json!("test-project"),
    );
    metadata.insert(
        "workflow_name".to_string(),
        serde_json::json!("test-workflow"),
    );
    metadata.insert("started_by".to_string(), serde_json::json!("test-user"));

    let config = SessionConfig {
        session_type: SessionType::Workflow,
        workflow_id: Some("test-workflow".to_string()),
        job_id: None,
        metadata,
    };

    let session_id = manager.create_session(config).await.unwrap();

    // Update progress multiple times
    for i in 1..=5 {
        let update = SessionUpdate::Progress {
            current: i,
            total: 10,
        };
        manager.update_session(&session_id, update).await.unwrap();
    }

    // Load and verify
    let session = manager.load_session(&session_id).await.unwrap();
    if let Some(workflow_data) = &session.workflow_data {
        // Progress updates current_step, not iterations_completed
        assert_eq!(workflow_data.current_step, 5);
        assert_eq!(workflow_data.total_steps, 10);
    } else {
        panic!("Expected workflow data to be present");
    }
}

#[tokio::test]
async fn test_session_filtering() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Create multiple sessions
    for i in 0..3 {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "project_name".to_string(),
            serde_json::json!(format!("project-{}", i)),
        );
        metadata.insert(
            "workflow_name".to_string(),
            serde_json::json!(format!("workflow-{}", i)),
        );
        metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
        metadata.insert(
            "tags".to_string(),
            serde_json::json!(vec![format!("tag-{}", i)]),
        );

        let config = SessionConfig {
            session_type: if i == 0 {
                SessionType::MapReduce
            } else {
                SessionType::Workflow
            },
            workflow_id: if i == 0 {
                None
            } else {
                Some(format!("workflow-{}", i))
            },
            job_id: if i == 0 {
                Some(format!("job-{}", i))
            } else {
                None
            },
            metadata,
        };

        let session_id = manager.create_session(config).await.unwrap();

        // Mark some as completed
        if i == 1 {
            let update = SessionUpdate::Status(SessionStatus::Completed);
            manager.update_session(&session_id, update).await.unwrap();
        }
    }

    // Filter by type
    let filter = SessionFilter {
        session_type: Some(SessionType::Workflow),
        status: None,
        after: None,
        before: None,
        worktree_name: None,
        limit: None,
    };

    let sessions = manager.list_sessions(Some(filter)).await.unwrap();
    assert_eq!(sessions.len(), 2);

    // Filter by status
    let filter = SessionFilter {
        session_type: None,
        status: Some(SessionStatus::Completed),
        after: None,
        before: None,
        worktree_name: None,
        limit: None,
    };

    let sessions = manager.list_sessions(Some(filter)).await.unwrap();
    assert_eq!(sessions.len(), 1);
}

#[tokio::test]
async fn test_cook_session_adapter() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let adapter = CookSessionAdapter::new(temp_dir.path().to_path_buf(), storage)
        .await
        .unwrap();

    // Test session lifecycle through adapter
    adapter.start_session("test-session").await.unwrap();

    // Update session
    use crate::cook::session::{SessionStatus as CookStatus, SessionUpdate as CookUpdate};
    adapter
        .update_session(CookUpdate::IncrementIteration)
        .await
        .unwrap();
    adapter
        .update_session(CookUpdate::AddFilesChanged(3))
        .await
        .unwrap();

    // Get state
    let state = adapter.get_state();
    assert_eq!(state.status, CookStatus::InProgress);

    // Complete session
    let summary = adapter.complete_session().await.unwrap();
    assert!(summary.files_changed > 0);
}

#[tokio::test]
async fn test_session_checkpointing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Create a session
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "project_name".to_string(),
        serde_json::json!("checkpoint-test"),
    );
    metadata.insert(
        "workflow_name".to_string(),
        serde_json::json!("test-workflow"),
    );
    metadata.insert("started_by".to_string(), serde_json::json!("test-user"));

    let config = SessionConfig {
        session_type: SessionType::Workflow,
        workflow_id: Some("test-workflow".to_string()),
        job_id: None,
        metadata,
    };

    let session_id = manager.create_session(config).await.unwrap();

    // Create a checkpoint
    let checkpoint_id = manager.create_checkpoint(&session_id).await.unwrap();

    assert!(!checkpoint_id.as_str().is_empty());

    // List checkpoints to verify it was created
    let checkpoints = manager.list_checkpoints(&session_id).await.unwrap();

    assert!(!checkpoints.is_empty());
    // Find the checkpoint we just created
    assert!(checkpoints.iter().any(|c| c.id == checkpoint_id));
}

#[tokio::test]
async fn test_mapreduce_session() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Create a MapReduce session
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "project_name".to_string(),
        serde_json::json!("mapreduce-test"),
    );
    metadata.insert(
        "workflow_name".to_string(),
        serde_json::json!("mapreduce-workflow"),
    );
    metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
    metadata.insert("tags".to_string(), serde_json::json!(vec!["mapreduce"]));
    metadata.insert(
        "description".to_string(),
        serde_json::json!("Test MapReduce session"),
    );

    let config = SessionConfig {
        session_type: SessionType::MapReduce,
        workflow_id: None,
        job_id: Some("mapreduce-job".to_string()),
        metadata,
    };

    let session_id = manager.create_session(config).await.unwrap();

    // Load and verify initial state
    let session = manager.load_session(&session_id).await.unwrap();
    assert_eq!(session.session_type, SessionType::MapReduce);

    if let Some(mapreduce_data) = &session.mapreduce_data {
        // Default phase should be Setup
        assert_eq!(mapreduce_data.phase, MapReducePhase::Setup);
        assert_eq!(mapreduce_data.job_id, "mapreduce-job");
    } else {
        panic!("Expected MapReduce data to be present");
    }
}

#[tokio::test]
async fn test_session_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Try to load non-existent session
    let fake_id = SessionId::new();
    let result = manager.load_session(&fake_id).await;
    assert!(result.is_err());

    // Try to update non-existent session
    let update = SessionUpdate::Status(SessionStatus::Running);
    let result = manager.update_session(&fake_id, update).await;
    assert!(result.is_err());
}

#[test]
fn test_timing_tracker() {
    let mut tracker = TimingTracker::new();

    // Start and complete iteration
    tracker.start_iteration();
    assert!(tracker.is_iteration_in_progress());

    std::thread::sleep(std::time::Duration::from_millis(10));
    let duration = tracker.complete_iteration();
    assert!(duration.is_some());
    assert!(!tracker.is_iteration_in_progress());

    // Start and complete command
    tracker.start_command("test-command".to_string());
    assert!(tracker.is_command_in_progress());

    std::thread::sleep(std::time::Duration::from_millis(10));
    let (name, duration) = tracker.complete_command().unwrap();
    assert_eq!(name, "test-command");
    assert!(duration.as_millis() >= 10);
    assert!(!tracker.is_command_in_progress());
}

#[test]
fn test_format_duration() {
    use std::time::Duration;

    assert_eq!(format_duration(Duration::from_secs(0)), "0s");
    assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
    assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
    assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m");
    assert_eq!(format_duration(Duration::from_secs(7320)), "2h 2m");
}

#[tokio::test]
async fn test_session_metadata_update() {
    let temp_dir = TempDir::new().unwrap();
    let storage = GlobalStorage::new_with_root(temp_dir.path().to_path_buf()).unwrap();
    let manager = SessionManager::new(storage).await.unwrap();

    // Create a session
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "project_name".to_string(),
        serde_json::json!("metadata-test"),
    );
    metadata.insert(
        "workflow_name".to_string(),
        serde_json::json!("test-workflow"),
    );
    metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
    metadata.insert("tags".to_string(), serde_json::json!(vec!["initial"]));
    metadata.insert(
        "description".to_string(),
        serde_json::json!("Initial description"),
    );

    let config = SessionConfig {
        session_type: SessionType::Workflow,
        workflow_id: Some("test-workflow".to_string()),
        job_id: None,
        metadata,
    };

    let session_id = manager.create_session(config).await.unwrap();

    // Update metadata
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "custom_field".to_string(),
        serde_json::json!("custom_value"),
    );
    metadata.insert("iteration_count".to_string(), serde_json::json!(5));

    let update = SessionUpdate::Metadata(metadata);
    manager.update_session(&session_id, update).await.unwrap();

    // Load and verify
    let session = manager.load_session(&session_id).await.unwrap();
    assert!(session.metadata.contains_key("project_name"));
    assert_eq!(
        session.metadata["project_name"],
        serde_json::json!("metadata-test")
    );
}
