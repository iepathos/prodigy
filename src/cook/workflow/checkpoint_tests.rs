//! Tests for workflow checkpoint and resume functionality

#[cfg(test)]
mod tests {
    use super::super::checkpoint::*;
    use crate::cook::workflow::executor::WorkflowContext;
    use crate::cook::workflow::normalized::{NormalizedStep, NormalizedWorkflow, StepCommand};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Create a test checkpoint manager with temp directory
    #[allow(deprecated)]
    fn create_test_checkpoint_manager() -> (CheckpointManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = CheckpointManager::new(temp_dir.path().to_path_buf());
        (manager, temp_dir)
    }

    /// Create a sample normalized workflow for testing
    fn create_test_workflow() -> NormalizedWorkflow {
        use std::time::Duration;

        NormalizedWorkflow {
            name: Arc::from("test-workflow"),
            steps: Arc::from(vec![
                NormalizedStep {
                    id: Arc::from("step-1"),
                    command: StepCommand::Shell(Arc::from("echo 'Step 1'")),
                    validation: None,
                    handlers: Default::default(),
                    timeout: Some(Duration::from_secs(30)),
                    working_dir: None,
                    env: Arc::new(HashMap::new()),
                    outputs: None,
                    commit_required: false,
                    when: None,
                },
                NormalizedStep {
                    id: Arc::from("step-2"),
                    command: StepCommand::Shell(Arc::from("echo 'Step 2'")),
                    validation: None,
                    handlers: Default::default(),
                    timeout: Some(Duration::from_secs(30)),
                    working_dir: None,
                    env: Arc::new(HashMap::new()),
                    outputs: None,
                    commit_required: false,
                    when: None,
                },
                NormalizedStep {
                    id: Arc::from("step-3"),
                    command: StepCommand::Shell(Arc::from("echo 'Step 3'")),
                    validation: None,
                    handlers: Default::default(),
                    timeout: Some(Duration::from_secs(30)),
                    working_dir: None,
                    env: Arc::new(HashMap::new()),
                    outputs: None,
                    commit_required: false,
                    when: None,
                },
            ]),
            execution_mode: crate::cook::workflow::normalized::ExecutionMode::Sequential,
            variables: Arc::new(HashMap::new()),
        }
    }

    #[tokio::test]
    async fn test_checkpoint_save_and_load() {
        let (manager, _temp_dir) = create_test_checkpoint_manager();
        let workflow = create_test_workflow();
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("test_var".to_string(), "test_value".to_string());

        // Create a checkpoint
        let checkpoint = create_checkpoint(
            "test-workflow-123".to_string(),
            &workflow,
            &context,
            vec![CompletedStep {
                step_index: 0,
                command: "echo 'Step 1'".to_string(),
                success: true,
                output: Some("Step 1".to_string()),
                captured_variables: HashMap::new(),
                duration: std::time::Duration::from_secs(1),
                completed_at: chrono::Utc::now(),
                retry_state: None,
            }],
            1, // Current step
            "workflow_hash_123".to_string(),
        );

        // Save checkpoint
        manager.save_checkpoint(&checkpoint).await.unwrap();

        // Load checkpoint
        let loaded = manager.load_checkpoint("test-workflow-123").await.unwrap();

        // Verify checkpoint
        assert_eq!(loaded.workflow_id, "test-workflow-123");
        assert_eq!(loaded.execution_state.current_step_index, 1);
        assert_eq!(loaded.completed_steps.len(), 1);
        assert_eq!(loaded.workflow_hash, "workflow_hash_123");
    }

    #[tokio::test]
    async fn test_checkpoint_list() {
        let (manager, _temp_dir) = create_test_checkpoint_manager();
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();

        // Create and save multiple checkpoints
        for i in 1..=3 {
            let checkpoint = create_checkpoint(
                format!("workflow-{}", i),
                &workflow,
                &context,
                vec![],
                0,
                "hash".to_string(),
            );
            manager.save_checkpoint(&checkpoint).await.unwrap();
        }

        // List checkpoints
        let mut checkpoints = manager.list_checkpoints().await.unwrap();
        checkpoints.sort();

        assert_eq!(checkpoints.len(), 3);
        assert_eq!(checkpoints[0], "workflow-1");
        assert_eq!(checkpoints[1], "workflow-2");
        assert_eq!(checkpoints[2], "workflow-3");
    }

    #[tokio::test]
    async fn test_checkpoint_delete() {
        let (manager, _temp_dir) = create_test_checkpoint_manager();
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();

        // Create and save checkpoint
        let checkpoint = create_checkpoint(
            "workflow-to-delete".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "hash".to_string(),
        );
        manager.save_checkpoint(&checkpoint).await.unwrap();

        // Verify it exists
        assert!(manager.load_checkpoint("workflow-to-delete").await.is_ok());

        // Delete checkpoint
        manager
            .delete_checkpoint("workflow-to-delete")
            .await
            .unwrap();

        // Verify it's gone
        assert!(manager.load_checkpoint("workflow-to-delete").await.is_err());
    }

    #[tokio::test]
    async fn test_checkpoint_validation() {
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();

        // Create checkpoint with valid state
        let checkpoint = create_checkpoint(
            "test-workflow".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "original_hash".to_string(),
        );

        // Validate with matching hash
        assert!(CheckpointManager::validate_checkpoint(&checkpoint, "original_hash").is_ok());

        // Validate with different hash (should still pass but with warning)
        assert!(CheckpointManager::validate_checkpoint(&checkpoint, "different_hash").is_ok());

        // Create invalid checkpoint (step index out of bounds)
        let mut invalid_checkpoint = checkpoint.clone();
        invalid_checkpoint.execution_state.current_step_index = 100;
        invalid_checkpoint.execution_state.total_steps = 3;

        // Should fail validation
        assert!(
            CheckpointManager::validate_checkpoint(&invalid_checkpoint, "original_hash").is_err()
        );
    }

    #[tokio::test]
    async fn test_build_resume_context() {
        let workflow = create_test_workflow();
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("var1".to_string(), "value1".to_string());

        let completed_steps = vec![CompletedStep {
            step_index: 0,
            command: "echo 'Step 1'".to_string(),
            success: true,
            output: Some("Step 1 output".to_string()),
            captured_variables: HashMap::from([(
                "step1_var".to_string(),
                "step1_value".to_string(),
            )]),
            duration: std::time::Duration::from_secs(1),
            completed_at: chrono::Utc::now(),
            retry_state: None,
        }];

        let checkpoint = create_checkpoint(
            "test-workflow".to_string(),
            &workflow,
            &context,
            completed_steps.clone(),
            1,
            "hash".to_string(),
        );

        let resume_context = build_resume_context(checkpoint);

        assert_eq!(resume_context.skip_steps.len(), 1);
        assert_eq!(resume_context.start_from_step, 1);
        assert!(resume_context.variable_state.contains_key("var1"));
        assert_eq!(
            resume_context.variable_state.get("var1").unwrap(),
            &serde_json::Value::String("value1".to_string())
        );
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_checkpoint_auto_interval() {
        let (mut manager, _temp_dir) = create_test_checkpoint_manager();

        // Configure with 1 second interval for testing (to avoid timing issues)
        manager.configure(std::time::Duration::from_secs(1), true);

        // Create fixed timestamps relative to a baseline
        let baseline = chrono::Utc::now();

        // Check with last checkpoint 10 seconds ago (should checkpoint)
        let old_checkpoint = baseline - chrono::Duration::seconds(10);
        assert!(manager.should_checkpoint(old_checkpoint).await);

        // Check with last checkpoint 500ms ago (should not checkpoint yet with 1s interval)
        let recent_checkpoint = baseline - chrono::Duration::milliseconds(500);
        assert!(!manager.should_checkpoint(recent_checkpoint).await);

        // Check with last checkpoint 2 seconds ago (should checkpoint since interval is 1s)
        let old_enough_checkpoint = baseline - chrono::Duration::seconds(2);
        assert!(manager.should_checkpoint(old_enough_checkpoint).await);
    }

    #[test]
    fn test_workflow_status_equality() {
        assert_eq!(WorkflowStatus::Running, WorkflowStatus::Running);
        assert_ne!(WorkflowStatus::Running, WorkflowStatus::Completed);
        assert_ne!(WorkflowStatus::Failed, WorkflowStatus::Interrupted);
    }

    #[tokio::test]
    async fn test_checkpoint_workflow_path_persistence() {
        let (manager, _temp_dir) = create_test_checkpoint_manager();
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();

        // Create a checkpoint with workflow path
        let mut checkpoint = create_checkpoint(
            "test-workflow-path".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "workflow_hash".to_string(),
        );

        // Set the workflow path
        let test_path = std::path::PathBuf::from("/test/workflows/implement.yml");
        checkpoint.workflow_path = Some(test_path.clone());

        // Save checkpoint
        manager.save_checkpoint(&checkpoint).await.unwrap();

        // Load checkpoint
        let loaded = manager.load_checkpoint("test-workflow-path").await.unwrap();

        // Verify workflow path was persisted
        assert_eq!(loaded.workflow_path, Some(test_path));
    }

    #[test]
    fn test_create_error_checkpoint_includes_context() {
        // Setup test data
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();
        let error = anyhow::anyhow!("Test error: commit required but no commits were created");
        let failed_step_index = 5;

        // Create error checkpoint
        let checkpoint = create_error_checkpoint(
            "test-workflow".to_string(),
            &workflow,
            &context,
            vec![],
            "hash123".to_string(),
            &error,
            failed_step_index,
        )
        .expect("Failed to create error checkpoint");

        // Verify status is Failed
        assert_eq!(checkpoint.execution_state.status, WorkflowStatus::Failed);

        // Verify error message is captured
        let error_msg = checkpoint
            .variable_state
            .get("__error_message")
            .expect("Missing __error_message");
        assert!(
            error_msg.as_str().unwrap().contains("commit required"),
            "Error message should contain 'commit required', got: {}",
            error_msg
        );

        // Verify failed step index is captured
        let failed_step = checkpoint
            .variable_state
            .get("__failed_step_index")
            .expect("Missing __failed_step_index");
        assert_eq!(
            failed_step.as_u64().unwrap(),
            failed_step_index as u64,
            "Failed step index should be {}",
            failed_step_index
        );

        // Verify timestamp is present
        let timestamp = checkpoint
            .variable_state
            .get("__error_timestamp")
            .expect("Missing __error_timestamp");
        assert!(
            timestamp.is_string(),
            "Timestamp should be a string ISO 8601 format"
        );
    }

    #[test]
    fn test_create_completion_checkpoint_status() {
        // Setup test data
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();
        let current_step_index = 3;

        // Create completion checkpoint
        let checkpoint = create_completion_checkpoint(
            "test-workflow".to_string(),
            &workflow,
            &context,
            vec![],
            current_step_index,
            "hash123".to_string(),
        )
        .expect("Failed to create completion checkpoint");

        // Verify status is Completed
        assert_eq!(checkpoint.execution_state.status, WorkflowStatus::Completed);

        // Verify no error context variables
        assert!(
            !checkpoint.variable_state.contains_key("__error_message"),
            "Completion checkpoint should not have error message"
        );
        assert!(
            !checkpoint
                .variable_state
                .contains_key("__failed_step_index"),
            "Completion checkpoint should not have failed step index"
        );
    }

    #[test]
    fn test_error_checkpoint_preserves_workflow_context() {
        // Setup test data with variables
        let workflow = create_test_workflow();
        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("user_var".to_string(), "user_value".to_string());
        context
            .captured_outputs
            .insert("output_var".to_string(), "output_value".to_string());

        let error = anyhow::anyhow!("Test error");

        // Create error checkpoint
        let checkpoint = create_error_checkpoint(
            "test-workflow".to_string(),
            &workflow,
            &context,
            vec![],
            "hash123".to_string(),
            &error,
            2,
        )
        .expect("Failed to create error checkpoint");

        // Verify user variables are preserved
        assert!(
            checkpoint.variable_state.contains_key("user_var"),
            "User variables should be preserved in error checkpoint"
        );
        assert!(
            checkpoint.variable_state.contains_key("output_var"),
            "Captured outputs should be preserved in error checkpoint"
        );

        // Verify error context variables are added
        assert!(
            checkpoint.variable_state.contains_key("__error_message"),
            "Error message should be added"
        );
        assert!(
            checkpoint
                .variable_state
                .contains_key("__failed_step_index"),
            "Failed step index should be added"
        );
    }

    // ============================================================================
    // Tests for builder pattern and immutability (Spec 124)
    // ============================================================================

    #[tokio::test]
    async fn test_checkpoint_manager_builder_pattern() {
        use crate::cook::workflow::checkpoint_path::CheckpointStorage;
        use std::time::Duration;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage = CheckpointStorage::Local(temp_dir.path().to_path_buf());

        // Test builder pattern chaining
        let manager = CheckpointManager::with_storage(storage)
            .with_interval(Duration::from_secs(30))
            .with_enabled(true);

        // Verify manager works correctly by testing behavior
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();
        let checkpoint = create_checkpoint(
            "test-builder".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "hash".to_string(),
        );

        // Should successfully save checkpoint (verifies enabled=true)
        manager.save_checkpoint(&checkpoint).await.unwrap();
        let checkpoint_path = temp_dir.path().join("test-builder.checkpoint.json");
        assert!(checkpoint_path.exists(), "Checkpoint should be saved when enabled");
    }

    #[test]
    fn test_builder_pattern_with_disabled_checkpointing() {
        use crate::cook::workflow::checkpoint_path::CheckpointStorage;
        use std::time::Duration;

        let storage = CheckpointStorage::Local(std::path::PathBuf::from("/tmp"));

        // Test that builder pattern allows chaining
        let _manager = CheckpointManager::with_storage(storage)
            .with_interval(Duration::from_secs(60))
            .with_enabled(false);

        // Manager is created successfully (behavior verified in other tests)
    }

    #[tokio::test]
    async fn test_builder_pattern_default_values() {
        use crate::cook::workflow::checkpoint_path::CheckpointStorage;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage = CheckpointStorage::Local(temp_dir.path().to_path_buf());

        // Create manager with only storage (should use defaults)
        let manager = CheckpointManager::with_storage(storage);

        // Verify defaults by testing behavior
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();
        let checkpoint = create_checkpoint(
            "test-defaults".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "hash".to_string(),
        );

        // Should save checkpoint (verifies enabled=true by default)
        manager.save_checkpoint(&checkpoint).await.unwrap();
        let checkpoint_path = temp_dir.path().join("test-defaults.checkpoint.json");
        assert!(checkpoint_path.exists(), "Default should be enabled");
    }

    #[tokio::test]
    async fn test_checkpoint_save_respects_enabled_flag() {
        use crate::cook::workflow::checkpoint_path::CheckpointStorage;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage = CheckpointStorage::Local(temp_dir.path().to_path_buf());

        // Create manager with checkpointing disabled
        let manager = CheckpointManager::with_storage(storage).with_enabled(false);

        let workflow = create_test_workflow();
        let context = WorkflowContext::default();
        let checkpoint = create_checkpoint(
            "test-disabled".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "hash".to_string(),
        );

        // Save should succeed but not actually write file
        manager.save_checkpoint(&checkpoint).await.unwrap();

        // Verify file was not created
        let checkpoint_path = temp_dir.path().join("test-disabled.checkpoint.json");
        assert!(
            !checkpoint_path.exists(),
            "Checkpoint file should not exist when disabled"
        );
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_deprecated_api_backwards_compatibility() {
        use std::time::Duration;
        use tempfile::TempDir;

        // Test that deprecated API still works
        let temp_dir = TempDir::new().unwrap();
        let mut manager = CheckpointManager::new(temp_dir.path().to_path_buf());
        manager.configure(Duration::from_secs(30), true);

        // Verify old API works by testing behavior
        let workflow = create_test_workflow();
        let context = WorkflowContext::default();
        let checkpoint = create_checkpoint(
            "test-deprecated".to_string(),
            &workflow,
            &context,
            vec![],
            0,
            "hash".to_string(),
        );

        // Should save checkpoint (verifies configure worked)
        manager.save_checkpoint(&checkpoint).await.unwrap();
        let checkpoint_path = temp_dir.path().join("test-deprecated.checkpoint.json");
        assert!(checkpoint_path.exists(), "Deprecated API should still work");
    }

    #[tokio::test]
    async fn test_builder_pattern_interval_configuration() {
        use crate::cook::workflow::checkpoint_path::CheckpointStorage;
        use std::time::Duration;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let storage = CheckpointStorage::Local(temp_dir.path().to_path_buf());

        let custom_interval = Duration::from_secs(120);
        let manager = CheckpointManager::with_storage(storage).with_interval(custom_interval);

        // Verify interval configuration by testing should_checkpoint behavior
        let baseline = chrono::Utc::now();

        // With 120s interval, should not checkpoint after 60s
        let recent = baseline - chrono::Duration::seconds(60);
        assert!(!manager.should_checkpoint(recent).await);

        // With 120s interval, should checkpoint after 130s
        let old = baseline - chrono::Duration::seconds(130);
        assert!(manager.should_checkpoint(old).await);
    }
}
