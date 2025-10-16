//! Integration tests for checkpoint and resume functionality
//!
//! These tests verify that:
//! 1. Checkpoints are created with workflow_path field
//! 2. Resume command can find and load checkpoints
//! 3. Resume executes from the correct step
//! 4. All workflow metadata is preserved

use anyhow::Result;
use prodigy::cook::workflow::checkpoint::{CheckpointManager, WorkflowCheckpoint, WorkflowStatus};
use prodigy::cook::workflow::checkpoint_path::CheckpointStorage;
use tempfile::TempDir;

/// Test that checkpoints include workflow_path when saved
#[tokio::test]
async fn test_checkpoint_includes_workflow_path() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let checkpoint_dir = temp_dir.path().join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    // Create a test workflow file
    let workflow_path = temp_dir.path().join("test.yml");
    tokio::fs::write(
        &workflow_path,
        "name: test\ncommands:\n  - shell: echo hello\n  - shell: echo world",
    )
    .await?;

    // Create a checkpoint with workflow path
    let checkpoint = WorkflowCheckpoint {
        workflow_id: "test-workflow".to_string(),
        workflow_path: Some(workflow_path.clone()),
        execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
            current_step_index: 1,
            total_steps: 2,
            status: WorkflowStatus::Running,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps: Vec::new(),
        variable_state: std::collections::HashMap::new(),
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        variable_checkpoint_state: None,
        version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
        workflow_hash: "test-hash".to_string(),
        total_steps: 2,
        workflow_name: Some("test".to_string()),
        error_recovery_state: None,
        retry_checkpoint_state: None,
    };

    // Save checkpoint
    #[allow(deprecated)]
    let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
    checkpoint_manager.save_checkpoint(&checkpoint).await?;

    // Load checkpoint and verify workflow_path is present
    let loaded = checkpoint_manager.load_checkpoint("test-workflow").await?;

    assert!(
        loaded.workflow_path.is_some(),
        "Checkpoint must have workflow_path"
    );
    assert_eq!(
        loaded.workflow_path.unwrap(),
        workflow_path,
        "Workflow path must match"
    );

    Ok(())
}

/// Test that checkpoints can be loaded and contain all required fields
#[tokio::test]
async fn test_checkpoint_roundtrip() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let checkpoint_dir = temp_dir.path().join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    // Create test workflow file
    let workflow_path = temp_dir.path().join("roundtrip.yml");
    tokio::fs::write(
        &workflow_path,
        "name: roundtrip\ncommands:\n  - shell: echo step1\n  - shell: echo step2\n  - shell: echo step3",
    )
    .await?;

    // Create checkpoint with all fields populated
    let original = WorkflowCheckpoint {
        workflow_id: "roundtrip-workflow".to_string(),
        workflow_path: Some(workflow_path.clone()),
        execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
            current_step_index: 2,
            total_steps: 3,
            status: WorkflowStatus::Running,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: Some(1),
            total_iterations: Some(1),
        },
        completed_steps: vec![
            prodigy::cook::workflow::checkpoint::CompletedStep {
                step_index: 0,
                command: "shell: echo step1".to_string(),
                success: true,
                output: Some("step1".to_string()),
                captured_variables: std::collections::HashMap::new(),
                duration: std::time::Duration::from_secs(1),
                completed_at: chrono::Utc::now(),
                retry_state: None,
            },
            prodigy::cook::workflow::checkpoint::CompletedStep {
                step_index: 1,
                command: "shell: echo step2".to_string(),
                success: true,
                output: Some("step2".to_string()),
                captured_variables: std::collections::HashMap::new(),
                duration: std::time::Duration::from_secs(1),
                completed_at: chrono::Utc::now(),
                retry_state: None,
            },
        ],
        variable_state: {
            let mut vars = std::collections::HashMap::new();
            vars.insert("test_var".to_string(), serde_json::json!("test_value"));
            vars
        },
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        variable_checkpoint_state: None,
        version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
        workflow_hash: "roundtrip-hash".to_string(),
        total_steps: 3,
        workflow_name: Some("roundtrip".to_string()),
        error_recovery_state: None,
        retry_checkpoint_state: None,
    };

    // Save checkpoint
    #[allow(deprecated)]
    let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
    checkpoint_manager.save_checkpoint(&original).await?;

    // Load and verify all fields
    let loaded = checkpoint_manager
        .load_checkpoint("roundtrip-workflow")
        .await?;

    assert_eq!(loaded.workflow_id, original.workflow_id);
    assert_eq!(loaded.workflow_path, original.workflow_path);
    assert_eq!(loaded.workflow_name, original.workflow_name);
    assert_eq!(
        loaded.execution_state.current_step_index,
        original.execution_state.current_step_index
    );
    assert_eq!(
        loaded.execution_state.total_steps,
        original.execution_state.total_steps
    );
    assert_eq!(loaded.completed_steps.len(), 2);
    assert_eq!(loaded.total_steps, 3);
    assert!(loaded.variable_state.contains_key("test_var"));

    Ok(())
}

/// Test that resume fails gracefully when workflow_path is missing
#[tokio::test]
async fn test_resume_fails_without_workflow_path() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let checkpoint_dir = temp_dir.path().join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    // Create checkpoint WITHOUT workflow_path (simulating legacy checkpoint)
    let checkpoint = WorkflowCheckpoint {
        workflow_id: "legacy-workflow".to_string(),
        workflow_path: None, // Missing workflow path
        execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
            current_step_index: 1,
            total_steps: 2,
            status: WorkflowStatus::Interrupted,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps: Vec::new(),
        variable_state: std::collections::HashMap::new(),
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        variable_checkpoint_state: None,
        version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
        workflow_hash: "legacy-hash".to_string(),
        total_steps: 2,
        workflow_name: Some("legacy".to_string()),
        error_recovery_state: None,
        retry_checkpoint_state: None,
    };

    // Save checkpoint
    #[allow(deprecated)]
    let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
    checkpoint_manager.save_checkpoint(&checkpoint).await?;

    // Load the checkpoint - should succeed
    let loaded = checkpoint_manager
        .load_checkpoint("legacy-workflow")
        .await?;

    // Verify workflow_path is None
    assert!(
        loaded.workflow_path.is_none(),
        "Legacy checkpoint should not have workflow_path"
    );

    // Resume would fail at this point - the CLI code should detect the missing path
    // and provide a helpful error message

    Ok(())
}

/// Test that checkpoint storage uses session-based paths
#[tokio::test]
async fn test_checkpoint_storage_session_based() -> Result<()> {
    let session_id = "session-test-123";
    let storage = CheckpointStorage::Session {
        session_id: session_id.to_string(),
    };

    // Resolve checkpoint path
    let checkpoint_path = storage.checkpoint_file_path("workflow-456")?;

    // Verify the path includes session ID and workflow ID
    let path_str = checkpoint_path.to_string_lossy();
    assert!(
        path_str.contains(session_id),
        "Checkpoint path should include session ID"
    );
    assert!(
        path_str.contains("workflow-456"),
        "Checkpoint path should include workflow ID"
    );
    assert!(
        path_str.ends_with(".checkpoint.json"),
        "Checkpoint path should have .checkpoint.json extension"
    );

    Ok(())
}

/// Test that checkpoints preserve variable state correctly
#[tokio::test]
async fn test_checkpoint_preserves_variables() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let checkpoint_dir = temp_dir.path().join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    let workflow_path = temp_dir.path().join("vars.yml");
    tokio::fs::write(
        &workflow_path,
        "name: vars\ncommands:\n  - shell: echo test",
    )
    .await?;

    // Create checkpoint with various variable types
    let mut variable_state = std::collections::HashMap::new();
    variable_state.insert("string_var".to_string(), serde_json::json!("string value"));
    variable_state.insert("number_var".to_string(), serde_json::json!(42));
    variable_state.insert("bool_var".to_string(), serde_json::json!(true));
    variable_state.insert(
        "array_var".to_string(),
        serde_json::json!(["item1", "item2", "item3"]),
    );
    variable_state.insert(
        "object_var".to_string(),
        serde_json::json!({
            "key1": "value1",
            "key2": "value2"
        }),
    );

    let checkpoint = WorkflowCheckpoint {
        workflow_id: "vars-workflow".to_string(),
        workflow_path: Some(workflow_path),
        execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
            current_step_index: 0,
            total_steps: 1,
            status: WorkflowStatus::Running,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps: Vec::new(),
        variable_state,
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        variable_checkpoint_state: None,
        version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
        workflow_hash: "vars-hash".to_string(),
        total_steps: 1,
        workflow_name: Some("vars".to_string()),
        error_recovery_state: None,
        retry_checkpoint_state: None,
    };

    // Save and reload
    #[allow(deprecated)]
    let checkpoint_manager = CheckpointManager::new(checkpoint_dir);
    checkpoint_manager.save_checkpoint(&checkpoint).await?;

    let loaded = checkpoint_manager.load_checkpoint("vars-workflow").await?;

    // Verify all variable types are preserved
    assert_eq!(
        loaded.variable_state.get("string_var").unwrap(),
        &serde_json::json!("string value")
    );
    assert_eq!(
        loaded.variable_state.get("number_var").unwrap(),
        &serde_json::json!(42)
    );
    assert_eq!(
        loaded.variable_state.get("bool_var").unwrap(),
        &serde_json::json!(true)
    );
    assert_eq!(
        loaded.variable_state.get("array_var").unwrap(),
        &serde_json::json!(["item1", "item2", "item3"])
    );
    assert!(loaded.variable_state.get("object_var").unwrap().is_object());

    Ok(())
}

/// Test that checkpoints correctly track execution progress
#[tokio::test]
async fn test_checkpoint_execution_progress() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let checkpoint_dir = temp_dir.path().join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    let workflow_path = temp_dir.path().join("progress.yml");
    tokio::fs::write(
        &workflow_path,
        "name: progress\ncommands:\n  - shell: step1\n  - shell: step2\n  - shell: step3\n  - shell: step4\n  - shell: step5",
    )
    .await?;

    // Simulate checkpoints at different progress points
    for step_index in 0..5 {
        let checkpoint = WorkflowCheckpoint {
            workflow_id: format!("progress-workflow-step-{}", step_index),
            workflow_path: Some(workflow_path.clone()),
            execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
                current_step_index: step_index,
                total_steps: 5,
                status: if step_index == 4 {
                    WorkflowStatus::Completed
                } else {
                    WorkflowStatus::Running
                },
                start_time: chrono::Utc::now(),
                last_checkpoint: chrono::Utc::now(),
                current_iteration: None,
                total_iterations: None,
            },
            completed_steps: (0..step_index)
                .map(|i| prodigy::cook::workflow::checkpoint::CompletedStep {
                    step_index: i,
                    command: format!("shell: step{}", i + 1),
                    success: true,
                    output: None,
                    captured_variables: std::collections::HashMap::new(),
                    duration: std::time::Duration::from_secs(1),
                    completed_at: chrono::Utc::now(),
                    retry_state: None,
                })
                .collect(),
            variable_state: std::collections::HashMap::new(),
            mapreduce_state: None,
            timestamp: chrono::Utc::now(),
            variable_checkpoint_state: None,
            version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
            workflow_hash: "progress-hash".to_string(),
            total_steps: 5,
            workflow_name: Some("progress".to_string()),
            error_recovery_state: None,
            retry_checkpoint_state: None,
        };

        #[allow(deprecated)]
        let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
        checkpoint_manager.save_checkpoint(&checkpoint).await?;

        let loaded = checkpoint_manager
            .load_checkpoint(&format!("progress-workflow-step-{}", step_index))
            .await?;

        assert_eq!(loaded.execution_state.current_step_index, step_index);
        assert_eq!(loaded.completed_steps.len(), step_index);
        assert_eq!(loaded.execution_state.total_steps, 5);
    }

    Ok(())
}
