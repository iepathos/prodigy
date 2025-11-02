//! Integration tests for resuming workflows after failure
//!
//! These tests verify that:
//! 1. Failed workflows create proper checkpoints
//! 2. Failed sessions can be resumed (currently fails - this is the bug)
//! 3. Session status transitions correctly from Failed to Running on resume
//! 4. Checkpoints preserve execution state across failure/resume

use anyhow::Result;
use prodigy::cook::session::state::{SessionState, SessionStatus};
use prodigy::cook::workflow::checkpoint::{
    CheckpointManager, WorkflowCheckpoint, WorkflowStatus, CHECKPOINT_VERSION,
};
use std::path::PathBuf;
use tempfile::TempDir;

/// Test that verifies the fix: is_resumable() now returns true for Failed status with checkpoint data
#[tokio::test]
async fn test_failed_session_is_now_resumable() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    // Create a session state with Failed status
    let mut state = SessionState::new("failed-session".to_string(), working_dir);
    state.status = SessionStatus::Failed;

    // Add workflow state to indicate we have checkpoint data
    state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: true,
    });

    // FIXED: Failed sessions with checkpoint data ARE now resumable
    assert_eq!(state.status, SessionStatus::Failed);
    assert!(
        state.workflow_state.is_some(),
        "Session has checkpoint data"
    );

    // This assertion now passes - failed sessions with checkpoint data are resumable
    assert!(
        state.is_resumable(),
        "Failed session with checkpoint data should be resumable!"
    );

    Ok(())
}

/// Test that Interrupted sessions ARE resumable (expected behavior)
#[tokio::test]
async fn test_interrupted_session_is_resumable() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    let mut state = SessionState::new("interrupted-session".to_string(), working_dir);
    state.status = SessionStatus::Interrupted;

    state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: true,
    });

    // Interrupted sessions ARE resumable
    assert_eq!(state.status, SessionStatus::Interrupted);
    assert!(state.workflow_state.is_some());
    assert!(state.is_resumable(), "Interrupted sessions are resumable");

    Ok(())
}

/// Test that InProgress sessions ARE resumable (expected behavior)
#[tokio::test]
async fn test_inprogress_session_is_resumable() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    let mut state = SessionState::new("inprogress-session".to_string(), working_dir);
    state.status = SessionStatus::InProgress;

    state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: true,
    });

    // InProgress sessions ARE resumable
    assert_eq!(state.status, SessionStatus::InProgress);
    assert!(state.workflow_state.is_some());
    assert!(state.is_resumable(), "InProgress sessions are resumable");

    Ok(())
}

/// Test checkpoint creation with Failed status
#[tokio::test]
async fn test_checkpoint_with_failed_status() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let checkpoint_dir = temp_dir.path().join("checkpoints");
    let workflow_path = temp_dir.path().join("test.yml");

    tokio::fs::create_dir_all(&checkpoint_dir).await?;
    tokio::fs::write(
        &workflow_path,
        r#"
name: test-workflow
commands:
  - shell: echo "step1"
  - shell: echo "step2"
  - shell: exit 1  # This fails
  - shell: echo "step4"
"#,
    )
    .await?;

    // Create checkpoint representing a failure
    let checkpoint = WorkflowCheckpoint {
        workflow_id: "test-workflow".to_string(),
        workflow_path: Some(workflow_path.clone()),
        execution_state: prodigy::cook::workflow::checkpoint::ExecutionState {
            current_step_index: 2,
            total_steps: 4,
            status: WorkflowStatus::Failed,
            start_time: chrono::Utc::now(),
            last_checkpoint: chrono::Utc::now(),
            current_iteration: Some(1),
            total_iterations: Some(1),
        },
        completed_steps: vec![
            prodigy::cook::workflow::checkpoint::CompletedStep {
                step_index: 0,
                command: "shell: echo \"step1\"".to_string(),
                success: true,
                output: Some("step1".to_string()),
                captured_variables: std::collections::HashMap::new(),
                duration: std::time::Duration::from_secs(1),
                completed_at: chrono::Utc::now(),
                retry_state: None,
            },
            prodigy::cook::workflow::checkpoint::CompletedStep {
                step_index: 1,
                command: "shell: echo \"step2\"".to_string(),
                success: true,
                output: Some("step2".to_string()),
                captured_variables: std::collections::HashMap::new(),
                duration: std::time::Duration::from_secs(1),
                completed_at: chrono::Utc::now(),
                retry_state: None,
            },
        ],
        variable_state: std::collections::HashMap::new(),
        mapreduce_state: None,
        timestamp: chrono::Utc::now(),
        variable_checkpoint_state: None,
        version: CHECKPOINT_VERSION,
        workflow_hash: "test-hash".to_string(),
        total_steps: 4,
        workflow_name: Some("test-workflow".to_string()),
        error_recovery_state: None,
        retry_checkpoint_state: None,
    };

    #[allow(deprecated)]
    let checkpoint_manager = CheckpointManager::new(checkpoint_dir.clone());
    checkpoint_manager.save_checkpoint(&checkpoint).await?;

    // Verify checkpoint was saved with Failed status
    let loaded = checkpoint_manager.load_checkpoint("test-workflow").await?;
    assert_eq!(loaded.execution_state.status, WorkflowStatus::Failed);
    assert_eq!(loaded.execution_state.current_step_index, 2);
    assert_eq!(loaded.completed_steps.len(), 2);

    Ok(())
}

/// Test that documents expected behavior once bug is fixed
#[tokio::test]
#[ignore] // Will pass once bug is fixed
async fn test_failed_session_should_be_resumable() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    let mut state = SessionState::new("should-resume".to_string(), working_dir);
    state.status = SessionStatus::Failed;

    state.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: true,
    });

    // Once bug is fixed: Failed sessions WITH checkpoint data should be resumable
    assert_eq!(state.status, SessionStatus::Failed);
    assert!(state.workflow_state.is_some());

    // This should return true once the bug is fixed
    assert!(
        state.is_resumable(),
        "Failed sessions with checkpoint data should be resumable"
    );

    Ok(())
}

/// Test the fix: is_resumable should check for workflow_state, not just status
#[tokio::test]
#[ignore] // Will pass once bug is fixed
async fn test_is_resumable_logic_after_fix() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    // Test 1: Failed WITH checkpoint data = resumable
    let mut failed_with_checkpoint = SessionState::new("test1".to_string(), working_dir.clone());
    failed_with_checkpoint.status = SessionStatus::Failed;
    failed_with_checkpoint.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: true,
    });
    assert!(
        failed_with_checkpoint.is_resumable(),
        "Failed with checkpoint should be resumable"
    );

    // Test 2: Failed WITHOUT checkpoint data = not resumable
    let mut failed_without_checkpoint = SessionState::new("test2".to_string(), working_dir.clone());
    failed_without_checkpoint.status = SessionStatus::Failed;
    failed_without_checkpoint.workflow_state = None;
    assert!(
        !failed_without_checkpoint.is_resumable(),
        "Failed without checkpoint should not be resumable"
    );

    // Test 3: Completed = not resumable (even with checkpoint)
    let mut completed = SessionState::new("test3".to_string(), working_dir.clone());
    completed.status = SessionStatus::Completed;
    completed.workflow_state = Some(prodigy::cook::session::state::WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: true,
    });
    assert!(
        !completed.is_resumable(),
        "Completed should not be resumable"
    );

    Ok(())
}
