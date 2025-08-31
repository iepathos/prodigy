//! Tests for MapReduce state persistence and checkpointing

#[cfg(test)]
mod tests {
    use crate::cook::execution::mapreduce::{AgentResult, AgentStatus, MapReduceConfig};
    use crate::cook::execution::state::{
        CheckpointManager, DefaultJobStateManager, JobStateManager, MapReduceJobState,
    };
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_job_state_creation() {
        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: Some(10),
            offset: None,
        };

        let work_items = vec![
            json!({"id": 1, "name": "item1"}),
            json!({"id": 2, "name": "item2"}),
            json!({"id": 3, "name": "item3"}),
        ];

        let state = MapReduceJobState::new(
            "test-job-123".to_string(),
            config.clone(),
            work_items.clone(),
        );

        assert_eq!(state.job_id, "test-job-123");
        assert_eq!(state.total_items, 3);
        assert_eq!(state.pending_items.len(), 3);
        assert_eq!(state.successful_count, 0);
        assert_eq!(state.failed_count, 0);
        assert!(!state.is_complete);
        assert!(state.pending_items.contains(&"item_0".to_string()));
        assert!(state.pending_items.contains(&"item_1".to_string()));
        assert!(state.pending_items.contains(&"item_2".to_string()));
    }

    #[tokio::test]
    async fn test_agent_result_update() {
        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
        };

        let work_items = vec![json!({"id": 1}), json!({"id": 2})];

        let mut state = MapReduceJobState::new("test-job".to_string(), config, work_items);

        // Add successful result
        let success_result = AgentResult {
            item_id: "item_0".to_string(),
            status: AgentStatus::Success,
            output: Some("Success output".to_string()),
            commits: vec!["abc123".to_string()],
            duration: Duration::from_secs(5),
            error: None,
            worktree_path: Some(PathBuf::from("/tmp/worktree")),
            branch_name: Some("feature-1".to_string()),
            worktree_session_id: Some("session-1".to_string()),
            files_modified: vec![PathBuf::from("file1.rs")],
        };

        state.update_agent_result(success_result.clone());

        assert_eq!(state.successful_count, 1);
        assert_eq!(state.failed_count, 0);
        assert!(state.completed_agents.contains("item_0"));
        assert!(!state.pending_items.contains(&"item_0".to_string()));
        assert_eq!(state.agent_results.len(), 1);
        assert_eq!(state.checkpoint_version, 1);

        // Add failed result
        let failed_result = AgentResult {
            item_id: "item_1".to_string(),
            status: AgentStatus::Failed("Test error".to_string()),
            output: None,
            commits: vec![],
            duration: Duration::from_secs(2),
            error: Some("Test error".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        state.update_agent_result(failed_result);

        assert_eq!(state.successful_count, 1);
        assert_eq!(state.failed_count, 1);
        assert!(state.completed_agents.contains("item_1"));
        assert!(!state.pending_items.contains(&"item_1".to_string()));
        assert_eq!(state.failed_agents.len(), 1);
        assert!(state.failed_agents.contains_key("item_1"));
        assert_eq!(state.checkpoint_version, 2);
    }

    #[tokio::test]
    async fn test_checkpoint_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let checkpoint_manager = CheckpointManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
        };

        let work_items = vec![
            json!({"id": 1, "data": "test1"}),
            json!({"id": 2, "data": "test2"}),
            json!({"id": 3, "data": "test3"}),
        ];

        let mut state = MapReduceJobState::new("checkpoint-test".to_string(), config, work_items);

        // Update state with some results
        state.update_agent_result(AgentResult {
            item_id: "item_0".to_string(),
            status: AgentStatus::Success,
            output: Some("output1".to_string()),
            commits: vec!["commit1".to_string()],
            duration: Duration::from_secs(3),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        });

        state.update_agent_result(AgentResult {
            item_id: "item_1".to_string(),
            status: AgentStatus::Failed("error".to_string()),
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: Some("error".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        });

        // Save checkpoint
        checkpoint_manager.save_checkpoint(&state).await.unwrap();

        // Load checkpoint
        let loaded_state = checkpoint_manager
            .load_checkpoint("checkpoint-test")
            .await
            .unwrap();

        // Verify loaded state matches original
        assert_eq!(loaded_state.job_id, state.job_id);
        assert_eq!(loaded_state.total_items, state.total_items);
        assert_eq!(loaded_state.successful_count, 1);
        assert_eq!(loaded_state.failed_count, 1);
        assert_eq!(loaded_state.completed_agents.len(), 2);
        assert_eq!(loaded_state.pending_items.len(), 1);
        assert_eq!(loaded_state.checkpoint_version, state.checkpoint_version);
    }

    #[tokio::test]
    async fn test_checkpoint_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let checkpoint_manager = CheckpointManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
        };

        let mut state = MapReduceJobState::new("cleanup-test".to_string(), config, vec![]);

        // Create 5 checkpoints
        for i in 0..5 {
            state.checkpoint_version = i;
            checkpoint_manager.save_checkpoint(&state).await.unwrap();
        }

        // List checkpoints
        let checkpoints = checkpoint_manager
            .list_checkpoints("cleanup-test")
            .await
            .unwrap();

        // Should only keep 3 most recent checkpoints
        assert!(checkpoints.len() <= 3);

        // Verify newest checkpoint is version 4
        if !checkpoints.is_empty() {
            assert_eq!(checkpoints[0].version, 4);
        }
    }

    #[tokio::test]
    async fn test_job_state_manager_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
        };

        let work_items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        // Create job
        let job_id = manager
            .create_job(config.clone(), work_items.clone())
            .await
            .unwrap();

        assert!(job_id.starts_with("mapreduce-"));

        // Get initial state
        let state = manager.get_job_state(&job_id).await.unwrap();
        assert_eq!(state.total_items, 3);
        assert_eq!(state.successful_count, 0);

        // Update with agent results
        let result1 = AgentResult {
            item_id: "item_0".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec![],
            duration: Duration::from_secs(2),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        };

        manager.update_agent_result(&job_id, result1).await.unwrap();

        // Verify state was updated
        let updated_state = manager.get_job_state(&job_id).await.unwrap();
        assert_eq!(updated_state.successful_count, 1);
        assert_eq!(updated_state.completed_agents.len(), 1);

        // Start reduce phase
        manager.start_reduce_phase(&job_id).await.unwrap();

        let state_with_reduce = manager.get_job_state(&job_id).await.unwrap();
        assert!(state_with_reduce.reduce_phase_state.is_some());
        assert!(state_with_reduce.reduce_phase_state.unwrap().started);

        // Complete reduce phase
        manager
            .complete_reduce_phase(&job_id, Some("reduce output".to_string()))
            .await
            .unwrap();

        let final_state = manager.get_job_state(&job_id).await.unwrap();
        assert!(final_state.is_complete);
        assert!(final_state.reduce_phase_state.unwrap().completed);

        // Clean up job
        manager.cleanup_job(&job_id).await.unwrap();

        // Verify job is cleaned up
        assert!(manager.get_job_state(&job_id).await.is_err());
    }

    #[tokio::test]
    async fn test_resume_job_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DefaultJobStateManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 3,
            max_items: None,
            offset: None,
        };

        let work_items = vec![
            json!({"id": 1}),
            json!({"id": 2}),
            json!({"id": 3}),
            json!({"id": 4}),
        ];

        // Create job
        let job_id = manager.create_job(config, work_items).await.unwrap();

        // Simulate partial completion
        manager
            .update_agent_result(
                &job_id,
                AgentResult {
                    item_id: "item_0".to_string(),
                    status: AgentStatus::Success,
                    output: Some("result1".to_string()),
                    commits: vec![],
                    duration: Duration::from_secs(1),
                    error: None,
                    worktree_path: None,
                    branch_name: None,
                    worktree_session_id: None,
                    files_modified: vec![],
                },
            )
            .await
            .unwrap();

        manager
            .update_agent_result(
                &job_id,
                AgentResult {
                    item_id: "item_1".to_string(),
                    status: AgentStatus::Failed("error".to_string()),
                    output: None,
                    commits: vec![],
                    duration: Duration::from_secs(1),
                    error: Some("error".to_string()),
                    worktree_path: None,
                    branch_name: None,
                    worktree_session_id: None,
                    files_modified: vec![],
                },
            )
            .await
            .unwrap();

        // Resume job
        let results = manager.resume_job(&job_id).await.unwrap();
        assert_eq!(results.len(), 2);

        // Check state after resume
        let state = manager.get_job_state(&job_id).await.unwrap();
        assert_eq!(state.completed_agents.len(), 2);
        assert_eq!(state.pending_items.len(), 2); // item_2 and item_3 still pending
        assert_eq!(state.successful_count, 1);
        assert_eq!(state.failed_count, 1);

        // Verify retriable items
        let retriable = state.get_retriable_items(3);
        assert_eq!(retriable.len(), 1); // item_1 can be retried
        assert!(retriable.contains(&"item_1".to_string()));
    }

    #[tokio::test]
    async fn test_atomic_checkpoint_writes() {
        let temp_dir = TempDir::new().unwrap();
        let checkpoint_manager = CheckpointManager::new(temp_dir.path().to_path_buf());

        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
        };

        let state =
            MapReduceJobState::new("atomic-test".to_string(), config, vec![json!({"id": 1})]);

        // Save checkpoint
        checkpoint_manager.save_checkpoint(&state).await.unwrap();

        // Verify temp file doesn't exist
        let job_dir = temp_dir.path().join("jobs").join("atomic-test");
        let temp_files: Vec<_> = std::fs::read_dir(&job_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension() == Some(std::ffi::OsStr::new("tmp")))
            .collect();

        assert_eq!(temp_files.len(), 0, "No temp files should remain");

        // Verify checkpoint file exists
        assert!(job_dir.join("checkpoint-v0.json").exists());
        assert!(job_dir.join("metadata.json").exists());
    }

    #[tokio::test]
    async fn test_phase_completion_tracking() {
        let config = MapReduceConfig {
            input: PathBuf::from("test.json"),
            json_path: String::new(),
            max_parallel: 5,
            timeout_per_agent: 60,
            retry_on_failure: 2,
            max_items: None,
            offset: None,
        };

        let work_items = vec![json!({"id": 1}), json!({"id": 2})];

        let mut state = MapReduceJobState::new("phase-test".to_string(), config, work_items);

        // Initially not complete
        assert!(!state.is_map_phase_complete());

        // Complete all items
        state.update_agent_result(AgentResult {
            item_id: "item_0".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec![],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        });

        state.update_agent_result(AgentResult {
            item_id: "item_1".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec![],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
        });

        // Now map phase should be complete
        assert!(state.is_map_phase_complete());
        assert_eq!(state.pending_items.len(), 0);
        assert_eq!(state.completed_agents.len(), 2);

        // Test reduce phase tracking
        state.start_reduce_phase();
        assert!(state.reduce_phase_state.is_some());
        assert!(state.reduce_phase_state.as_ref().unwrap().started);
        assert!(!state.reduce_phase_state.as_ref().unwrap().completed);

        state.complete_reduce_phase(Some("reduce output".to_string()));
        assert!(state.reduce_phase_state.as_ref().unwrap().completed);
        assert_eq!(
            state.reduce_phase_state.as_ref().unwrap().output,
            Some("reduce output".to_string())
        );
        assert!(state.is_complete);
    }
}
