//! Unit tests for the unified session management module

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::storage::GlobalStorage;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_session_manager_creation() {
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Should be able to list sessions (empty initially)
        let sessions = manager.list_sessions(None).await.unwrap();
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Create a session
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("project_name".to_string(), serde_json::json!("test-project"));
        metadata.insert("workflow_name".to_string(), serde_json::json!("test-workflow"));
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
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Create a workflow session
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("project_name".to_string(), serde_json::json!("test-project"));
        metadata.insert("workflow_name".to_string(), serde_json::json!("test-workflow"));
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
            manager.update_session(&session_id, &update).await.unwrap();
        }

        // Load and verify
        let session = manager.load_session(&session_id).await.unwrap();
        if let Some(workflow_data) = &session.workflow_data {
            assert!(workflow_data.iterations_completed > 0);
        }
    }

    #[tokio::test]
    async fn test_session_filtering() {
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Create multiple sessions
        for i in 0..3 {
            let mut metadata = std::collections::HashMap::new();
            metadata.insert("project_name".to_string(), serde_json::json!(format!("project-{}", i)));
            metadata.insert("workflow_name".to_string(), serde_json::json!(format!("workflow-{}", i)));
            metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
            metadata.insert("tags".to_string(), serde_json::json!(vec![format!("tag-{}", i)]));

            let config = SessionConfig {
                session_type: if i == 0 {
                    SessionType::MapReduce
                } else {
                    SessionType::Workflow
                },
                workflow_id: Some(format!("workflow-{}", i)),
                job_id: None,
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
            project_name: None,
            started_after: None,
            started_before: None,
        };

        let sessions = manager.list_sessions(Some(filter)).await.unwrap();
        assert_eq!(sessions.len(), 2);

        // Filter by status
        let filter = SessionFilter {
            session_type: None,
            status: Some(SessionStatus::Completed),
            project_name: None,
            started_after: None,
            started_before: None,
        };

        let sessions = manager.list_sessions(Some(filter)).await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_cook_session_adapter() {
        let temp_dir = TempDir::new().unwrap();
        let storage = GlobalStorage::new().unwrap();
        let adapter = CookSessionAdapter::new(
            temp_dir.path().to_path_buf(),
            storage,
        )
        .await
        .unwrap();

        // Test session lifecycle through adapter
        adapter.start_session("test-session").await.unwrap();

        // Update session
        use crate::cook::session::{SessionUpdate as CookUpdate, SessionStatus as CookStatus};
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
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Create a session
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("project_name".to_string(), serde_json::json!("checkpoint-test"));
        metadata.insert("workflow_name".to_string(), serde_json::json!("test-workflow"));
        metadata.insert("started_by".to_string(), serde_json::json!("test-user"));

        let config = SessionConfig {
            session_type: SessionType::Workflow,
            workflow_id: Some("test-workflow".to_string()),
            job_id: None,
            metadata,
        };

        let session_id = manager.create_session(config).await.unwrap();

        // Create a checkpoint
        let checkpoint_data = serde_json::json!({
            "step": 5,
            "state": "in_progress"
        });

        let checkpoint_id = manager
            .save_checkpoint(&session_id, checkpoint_data.clone())
            .await
            .unwrap();

        assert!(!checkpoint_id.as_str().is_empty());

        // Load checkpoint
        let loaded = manager
            .load_checkpoint(&session_id, &checkpoint_id)
            .await
            .unwrap();

        assert_eq!(loaded.data, checkpoint_data);
    }

    #[tokio::test]
    async fn test_mapreduce_session() {
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Create a MapReduce session
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("project_name".to_string(), serde_json::json!("mapreduce-test"));
        metadata.insert("workflow_name".to_string(), serde_json::json!("mapreduce-workflow"));
        metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
        metadata.insert("tags".to_string(), serde_json::json!(vec!["mapreduce"]));
        metadata.insert("description".to_string(), serde_json::json!("Test MapReduce session"));

        let config = SessionConfig {
            session_type: SessionType::MapReduce,
            workflow_id: None,
            job_id: Some("mapreduce-job".to_string()),
            metadata,
        };

        let session_id = manager.create_session(config).await.unwrap();

        // Set MapReduce phase
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "phase".to_string(),
            serde_json::json!(MapReducePhase::Map),
        );
        let update = SessionUpdate::Metadata(metadata);
        manager.update_session(&session_id, update).await.unwrap();

        // Load and verify
        let session = manager.load_session(&session_id).await.unwrap();
        assert_eq!(session.session_type, SessionType::MapReduce);

        if let Some(mapreduce_data) = &session.mapreduce_data {
            assert_eq!(mapreduce_data.phase, MapReducePhase::Map);
        }
    }

    #[tokio::test]
    async fn test_session_error_handling() {
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Try to load non-existent session
        let fake_id = SessionId::from("non-existent-session");
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
        let storage = GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        // Create a session
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("project_name".to_string(), serde_json::json!("metadata-test"));
        metadata.insert("workflow_name".to_string(), serde_json::json!("test-workflow"));
        metadata.insert("started_by".to_string(), serde_json::json!("test-user"));
        metadata.insert("tags".to_string(), serde_json::json!(vec!["initial"]));
        metadata.insert("description".to_string(), serde_json::json!("Initial description"));

        let config = SessionConfig {
            session_type: SessionType::Workflow,
            workflow_id: Some("test-workflow".to_string()),
            job_id: None,
            metadata,
        };

        let session_id = manager.create_session(config).await.unwrap();

        // Update metadata
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("custom_field".to_string(), serde_json::json!("custom_value"));
        metadata.insert("iteration_count".to_string(), serde_json::json!(5));

        let update = SessionUpdate::Metadata(metadata);
        manager.update_session(&session_id, update).await.unwrap();

        // Load and verify
        let session = manager.load_session(&session_id).await.unwrap();
        assert!(session.metadata.contains_key("project_name"));
        assert_eq!(session.metadata["project_name"], serde_json::json!("metadata-test"));
    }
}