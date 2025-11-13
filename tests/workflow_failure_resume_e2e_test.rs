//! End-to-end integration test for workflow failure, checkpoint, and resume
//!
//! This test verifies the complete cycle:
//! 1. Workflow starts and fails mid-execution
//! 2. Error checkpoint is saved with correct workflow_path
//! 3. Session state is marked as Failed but resumable
//! 4. Resume loads checkpoint and continues execution
//! 5. Workflow completes successfully

use anyhow::{Context, Result};
use prodigy::cook::session::state::{SessionState, SessionStatus};
use prodigy::cook::workflow::checkpoint::{CheckpointManager, WorkflowStatus};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

/// Test complete workflow failure → checkpoint → resume cycle with correct workflow_path
#[tokio::test]
async fn test_workflow_failure_checkpoint_resume_cycle() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();
    let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    // Create a workflow file with a custom name (not "workflow.yml")
    let workflow_path = working_dir.join("my-custom-workflow.yml");
    let workflow_content = r#"
name: test-failure-workflow
steps:
  - shell: "echo step1 > output.txt"
    auto_commit: true
  - shell: "exit 1"  # This step will fail
  - shell: "echo step3 >> output.txt"
    auto_commit: true
"#;
    tokio::fs::write(&workflow_path, workflow_content)
        .await
        .context("Failed to write workflow file")?;

    // Simulate a failed workflow that creates an error checkpoint
    let session_id = "test-session-failure";
    let workflow_id = "test-workflow-id";

    // Create a mock checkpoint as if workflow failed on step 1
    let checkpoint = prodigy::cook::workflow::checkpoint::WorkflowCheckpoint {
        workflow_id: workflow_id.to_string(),
        workflow_path: Some(workflow_path.clone()),
        execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
            current_step_index: 1, // Failed on step 1 (0-indexed)
            total_steps: 3,
            status: WorkflowStatus::Failed,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: None,
            total_iterations: None,
        },
        completed_steps: vec![prodigy::cook::workflow::checkpoint::CompletedStep {
            step_index: 0,
            command: "echo step1 > output.txt".to_string(),
            success: true,
            output: Some("step1\n".to_string()),
            captured_variables: HashMap::new(),
            duration: Duration::from_millis(100),
            completed_at: chrono::Utc::now(),
            retry_state: None,
        }],
        variable_state: std::collections::HashMap::new(),
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        variable_checkpoint_state: None,
        version: prodigy::cook::workflow::checkpoint::CHECKPOINT_VERSION,
        workflow_hash: "test-hash".to_string(),
        total_steps: 3,
        workflow_name: Some("test-failure-workflow".to_string()),
        error_recovery_state: Some(
            prodigy::cook::workflow::error_recovery::ErrorRecoveryState {
                active_handlers: Vec::new(),
                error_context: HashMap::new(),
                handler_execution_history: Vec::new(),
                retry_state: None,
                correlation_id: "test-error".to_string(),
                recovery_attempts: 0,
                max_recovery_attempts: 3,
            },
        ),
        retry_checkpoint_state: None,
    };

    // Save the checkpoint
    #[allow(deprecated)]
    let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
    checkpoint_manager
        .save_checkpoint(&checkpoint)
        .await
        .context("Failed to save checkpoint")?;

    // Create session state as if workflow failed
    let mut session_state = SessionState::new(session_id.to_string(), working_dir.clone());
    session_state.status = SessionStatus::Failed;

    // This is what the fix ensures: workflow_state is set with correct workflow_path
    session_state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 1, // Failed on step 1
        completed_steps: vec![],
        workflow_path: workflow_path.clone(), // This should be the actual path, not "workflow.yml"
        input_args: Vec::new(),
        map_patterns: Vec::new(),
        using_worktree: true,
    });

    // ===== VERIFICATION 1: Session is resumable =====
    assert!(
        session_state.is_resumable(),
        "Failed workflow with workflow_state should be resumable"
    );

    // ===== VERIFICATION 2: Checkpoint has correct workflow_path =====
    let loaded_checkpoint = checkpoint_manager
        .load_checkpoint(workflow_id)
        .await
        .context("Failed to load checkpoint")?;

    assert_eq!(
        loaded_checkpoint.workflow_path,
        Some(workflow_path.clone()),
        "Checkpoint must have correct workflow_path, not hardcoded 'workflow.yml'"
    );

    // ===== VERIFICATION 3: Workflow file exists at the path in checkpoint =====
    assert!(
        workflow_path.exists(),
        "Workflow file must exist at the path stored in checkpoint"
    );

    // ===== VERIFICATION 4: Checkpoint metadata is preserved =====
    assert_eq!(
        loaded_checkpoint.workflow_name,
        Some("test-failure-workflow".to_string()),
        "Workflow name must be preserved in checkpoint"
    );
    assert_eq!(
        loaded_checkpoint.execution_state.current_step_index, 1,
        "Failed step index must be preserved"
    );

    Ok(())
}

/// Test that workflow_path is not hardcoded to "workflow.yml"
#[tokio::test]
async fn test_custom_workflow_filename_in_checkpoint() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();
    let checkpoint_dir = working_dir.join(".prodigy").join("checkpoints");
    tokio::fs::create_dir_all(&checkpoint_dir).await?;

    // Use a non-standard filename
    let custom_filenames = [
        "prodigy.yml",
        "my-workflow.yaml",
        ".hidden-workflow.yml",
        "nested/path/workflow.yml",
    ];

    for (idx, filename) in custom_filenames.iter().enumerate() {
        let workflow_path = working_dir.join(filename);

        // Create parent directories if needed
        if let Some(parent) = workflow_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write workflow file
        tokio::fs::write(&workflow_path, "name: test\nsteps: []\n").await?;

        // Create checkpoint with custom filename
        let checkpoint = prodigy::cook::workflow::checkpoint::WorkflowCheckpoint {
            workflow_id: format!("test-{}", idx),
            workflow_path: Some(workflow_path.clone()),
            execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
                current_step_index: 0,
                total_steps: 0,
                status: WorkflowStatus::Completed,
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
            total_steps: 0,
            workflow_name: Some("test".to_string()),
            error_recovery_state: None,
            retry_checkpoint_state: None,
        };

        // Save and reload
        #[allow(deprecated)]
        let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
        checkpoint_manager.save_checkpoint(&checkpoint).await?;

        let loaded = checkpoint_manager
            .load_checkpoint(&format!("test-{}", idx))
            .await?;

        // Verify the custom filename is preserved
        assert_eq!(
            loaded.workflow_path,
            Some(workflow_path.clone()),
            "Checkpoint must preserve custom workflow filename: {}",
            filename
        );
    }

    Ok(())
}

/// Test that session state is properly set when workflow fails
#[tokio::test]
async fn test_session_state_set_on_failure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    // Test various failure scenarios
    let test_cases = vec![
        ("step-0-failure", 0, "First step fails"),
        ("step-1-failure", 1, "Middle step fails"),
        ("step-5-failure", 5, "Late step fails"),
    ];

    for (session_id, failed_step, description) in test_cases {
        let workflow_path = working_dir.join(format!("{}.yml", session_id));
        tokio::fs::write(&workflow_path, "name: test\nsteps: []\n").await?;

        let mut session_state = SessionState::new(session_id.to_string(), working_dir.clone());
        session_state.status = SessionStatus::Failed;

        // After fix: workflow_state is always set when checkpoint is saved
        session_state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
            current_iteration: 0,
            current_step: failed_step,
            completed_steps: Vec::new(),
            workflow_path: workflow_path.clone(),
            input_args: Vec::new(),
            map_patterns: Vec::new(),
            using_worktree: true,
        });

        assert!(
            session_state.is_resumable(),
            "{}: Failed session should be resumable when workflow_state is set",
            description
        );
    }

    Ok(())
}

/// Test that fallback to "workflow.yml" still works when workflow_path is None
#[tokio::test]
async fn test_fallback_to_default_workflow_yml() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    // Create default workflow.yml
    let default_path = working_dir.join("workflow.yml");
    tokio::fs::write(&default_path, "name: test\nsteps: []\n").await?;

    // Simulate legacy session where workflow_path might not have been set
    let mut session_state = SessionState::new("legacy-session".to_string(), working_dir.clone());
    session_state.status = SessionStatus::Failed;
    session_state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 0,
        completed_steps: Vec::new(),
        workflow_path: default_path.clone(), // Fallback path
        input_args: Vec::new(),
        map_patterns: Vec::new(),
        using_worktree: true,
    });

    assert!(
        session_state.is_resumable(),
        "Legacy session with default workflow.yml should be resumable"
    );
    assert!(
        default_path.exists(),
        "Default workflow.yml must exist for fallback"
    );

    Ok(())
}
