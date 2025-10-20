//! Comprehensive tests for state management module

#[cfg(test)]
mod state_tests {
    use super::super::*;
    use crate::cook::execution::mapreduce::state::persistence::InMemoryStateStore;
    use crate::cook::execution::mapreduce::{AgentResult, AgentStatus, MapReduceConfig};
    use std::sync::Arc;
    use std::time::Duration;

    /// Helper function to create a test state manager
    async fn create_test_state_manager() -> Arc<StateManager> {
        let store = Arc::new(InMemoryStateStore::new());
        Arc::new(StateManager::new(store))
    }

    /// Helper function to create a test job with sample data
    async fn create_test_job(manager: &StateManager, job_id: &str) -> JobState {
        let config = MapReduceConfig::default();
        let _state = manager
            .create_job(&config, job_id.to_string())
            .await
            .unwrap();

        // Add some processed items
        manager
            .update_state(job_id, |state| {
                // Set up some sample data
                state.total_items = 10;

                for i in 0..5 {
                    state.processed_items.insert(format!("item_{}", i));
                    state.agent_results.insert(
                        format!("item_{}", i),
                        AgentResult {
                            item_id: format!("item_{}", i),
                            status: AgentStatus::Success,
                            output: Some(format!("output_{}", i)),
                            commits: vec![],
                            duration: Duration::from_secs(1),
                            error: None,
                            worktree_path: None,
                            branch_name: None,
                            worktree_session_id: None,
                            files_modified: vec![],
                            json_log_location: None,
                            cleanup_status: None,
                        },
                    );
                }
                Ok(())
            })
            .await
            .unwrap();

        // Return the updated state from the manager
        manager.get_state(job_id).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn test_state_lifecycle() {
        let manager = create_test_state_manager().await;
        let job_id = "test-lifecycle";

        // Create job
        let state = manager
            .create_job(&MapReduceConfig::default(), job_id.to_string())
            .await
            .unwrap();
        assert_eq!(state.phase, PhaseType::Setup);
        assert!(!state.is_complete);

        // Start job (Setup -> Map)
        manager.mark_job_started(job_id).await.unwrap();
        let state = manager.get_state(job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Map);

        // Move to reduce phase
        manager.mark_reduce_started(job_id).await.unwrap();
        let state = manager.get_state(job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Reduce);

        // Complete job
        manager.mark_job_completed(job_id).await.unwrap();
        let state = manager.get_state(job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Completed);
        assert!(state.is_complete);
    }

    #[tokio::test]
    async fn test_checkpoint_creation_and_recovery() {
        let manager = create_test_state_manager().await;
        let job_id = "test-checkpoint";

        // Create job with progress
        let _ = create_test_job(&manager, job_id).await;

        // Create checkpoint
        let checkpoint = manager.create_checkpoint(job_id).await.unwrap();
        assert_eq!(checkpoint.version, 1);
        assert_eq!(checkpoint.items_processed.len(), 5);

        // Create recovery plan
        let recovery_plan = manager.recover_from_checkpoint(job_id, None).await.unwrap();
        assert_eq!(recovery_plan.skip_items.len(), 5);
        assert!(!recovery_plan.pending_items.is_empty());
    }

    #[tokio::test]
    async fn test_state_transitions_validation() {
        let manager = create_test_state_manager().await;
        let job_id = "test-transitions";

        // Create job
        manager
            .create_job(&MapReduceConfig::default(), job_id.to_string())
            .await
            .unwrap();

        // Valid transitions
        assert!(manager
            .can_transition(job_id, PhaseType::Map)
            .await
            .unwrap());
        assert!(manager
            .can_transition(job_id, PhaseType::Failed)
            .await
            .unwrap());

        // Invalid transitions
        assert!(!manager
            .can_transition(job_id, PhaseType::Reduce)
            .await
            .unwrap());
        assert!(!manager
            .can_transition(job_id, PhaseType::Completed)
            .await
            .unwrap());

        // Get valid transitions
        let transitions = manager.get_valid_transitions(job_id).await.unwrap();
        assert!(transitions.contains(&PhaseType::Map));
        assert!(transitions.contains(&PhaseType::Failed));
    }

    #[tokio::test]
    async fn test_concurrent_state_updates() {
        let manager = create_test_state_manager().await;
        let job_id = "test-concurrent";

        // Create job
        manager
            .create_job(&MapReduceConfig::default(), job_id.to_string())
            .await
            .unwrap();

        // Simulate concurrent updates
        let manager1 = manager.clone();
        let job_id1 = job_id.to_string();
        let handle1 = tokio::spawn(async move {
            for i in 0..5 {
                manager1
                    .mark_items_processed(&job_id1, vec![format!("item_{}", i)])
                    .await
                    .unwrap();
            }
        });

        let manager2 = manager.clone();
        let job_id2 = job_id.to_string();
        let handle2 = tokio::spawn(async move {
            for i in 5..10 {
                manager2
                    .mark_items_processed(&job_id2, vec![format!("item_{}", i)])
                    .await
                    .unwrap();
            }
        });

        // Wait for both to complete
        handle1.await.unwrap();
        handle2.await.unwrap();

        // Verify all items were processed
        let state = manager.get_state(job_id).await.unwrap().unwrap();
        assert_eq!(state.processed_items.len(), 10);
    }

    #[tokio::test]
    async fn test_failed_items_tracking() {
        let manager = create_test_state_manager().await;
        let job_id = "test-failed";

        // Create job
        manager
            .create_job(&MapReduceConfig::default(), job_id.to_string())
            .await
            .unwrap();

        // Mark some items as failed
        manager
            .mark_items_failed(job_id, vec!["item_0".to_string(), "item_1".to_string()])
            .await
            .unwrap();

        let state = manager.get_state(job_id).await.unwrap().unwrap();
        assert_eq!(state.failed_items.len(), 2);
        assert!(state.failed_items.contains(&"item_0".to_string()));
        assert!(state.failed_items.contains(&"item_1".to_string()));
    }

    #[tokio::test]
    async fn test_state_history() {
        let manager = create_test_state_manager().await;
        let job_id = "test-history";

        // Create job and perform operations
        manager
            .create_job(&MapReduceConfig::default(), job_id.to_string())
            .await
            .unwrap();
        manager.mark_job_started(job_id).await.unwrap();
        manager
            .mark_items_processed(job_id, vec!["item_0".to_string()])
            .await
            .unwrap();
        manager
            .mark_items_failed(job_id, vec!["item_1".to_string()])
            .await
            .unwrap();
        manager.mark_job_completed(job_id).await.unwrap();

        // Get history
        let history = manager.get_state_history(job_id).await;
        assert!(history.len() >= 5); // At least 5 events recorded

        // Verify event types
        let event_types: Vec<_> = history.iter().map(|e| &e.event_type).collect();
        assert!(event_types
            .iter()
            .any(|e| matches!(e, StateEventType::JobCreated)));
        assert!(event_types
            .iter()
            .any(|e| matches!(e, StateEventType::JobCompleted)));
    }

    #[tokio::test]
    async fn test_recovery_with_partial_completion() {
        let manager = create_test_state_manager().await;
        let job_id = "test-partial-recovery";

        // Create job with partial completion
        let config = MapReduceConfig::default();
        manager
            .create_job(&config, job_id.to_string())
            .await
            .unwrap();

        manager
            .update_state(job_id, |state| {
                state.total_items = 10;

                // Mark half as processed
                for i in 0..5 {
                    state.processed_items.insert(format!("item_{}", i));
                    // Add corresponding agent results to keep state consistent
                    state.agent_results.insert(
                        format!("item_{}", i),
                        AgentResult {
                            item_id: format!("item_{}", i),
                            status: AgentStatus::Success,
                            output: Some(format!("output_{}", i)),
                            commits: vec![],
                            duration: Duration::from_secs(1),
                            error: None,
                            worktree_path: None,
                            branch_name: None,
                            worktree_session_id: None,
                            files_modified: vec![],
                            json_log_location: None,
                            cleanup_status: None,
                        },
                    );
                }

                // Mark some as failed
                state.failed_items.push("item_5".to_string());
                state.failed_items.push("item_6".to_string());

                Ok(())
            })
            .await
            .unwrap();

        // Create checkpoint
        manager.create_checkpoint(job_id).await.unwrap();

        // Create recovery plan
        let plan = manager.recover_from_checkpoint(job_id, None).await.unwrap();

        // Should skip processed items
        assert_eq!(plan.skip_items.len(), 5);

        // Should include failed and never-attempted items
        assert!(plan.pending_items.len() >= 5); // 2 failed + 3 never-attempted
    }

    #[tokio::test]
    async fn test_terminal_state_restrictions() {
        let manager = create_test_state_manager().await;
        let job_id = "test-terminal";

        // Create and complete job
        manager
            .create_job(&MapReduceConfig::default(), job_id.to_string())
            .await
            .unwrap();
        manager.mark_job_started(job_id).await.unwrap();
        manager.mark_job_completed(job_id).await.unwrap();

        // Try to transition from completed state - should fail
        let result = manager.transition_to_phase(job_id, PhaseType::Map).await;
        assert!(result.is_err());

        // Try to mark as failed - should fail
        let result = manager.mark_job_failed(job_id, "test".to_string()).await;
        assert!(result.is_err());

        // Verify state hasn't changed
        let state = manager.get_state(job_id).await.unwrap().unwrap();
        assert_eq!(state.phase, PhaseType::Completed);
        assert!(state.is_complete);
    }

    #[tokio::test]
    async fn test_checkpoint_version_management() {
        let manager = create_test_state_manager().await;
        let job_id = "test-versions";

        // Create job and multiple checkpoints
        create_test_job(&manager, job_id).await;

        let checkpoint1 = manager.create_checkpoint(job_id).await.unwrap();
        assert_eq!(checkpoint1.version, 1);

        // Add more progress
        manager
            .mark_items_processed(job_id, vec!["item_5".to_string()])
            .await
            .unwrap();

        let checkpoint2 = manager.create_checkpoint(job_id).await.unwrap();
        assert_eq!(checkpoint2.version, 2);

        // Verify we can get the latest checkpoint
        let latest = manager.get_checkpoint(job_id, None).await.unwrap().unwrap();
        assert_eq!(latest.version, 2);
    }
}
