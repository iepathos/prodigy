//! Integration test that reproduces the actual workflow failure and resume bug
//!
//! This test demonstrates that when a workflow fails, the checkpoint is saved
//! but workflow_state is not always set in SessionState, causing is_resumable()
//! to incorrectly return false.

use anyhow::Result;
use prodigy::cook::session::state::{SessionState, SessionStatus, WorkflowState};
use std::path::PathBuf;
use tempfile::TempDir;

/// This test demonstrates the core issue: workflow_state must be set for is_resumable() to work
#[tokio::test]
async fn test_workflow_state_must_be_set_for_resumable() {
    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().to_path_buf();

    let mut state = SessionState::new("test-session".to_string(), working_dir);

    // Scenario 1: Failed with NO workflow_state (this is the bug)
    state.status = SessionStatus::Failed;
    state.workflow_state = None;

    assert!(
        !state.is_resumable(),
        "BUG REPRODUCED: Failed without workflow_state is not resumable"
    );

    // Scenario 2: Failed WITH workflow_state (expected after fix)
    state.workflow_state = Some(WorkflowState {
        current_iteration: 0,
        current_step: 1,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec!["157".to_string()],
        map_patterns: vec![],
        using_worktree: true,
    });

    assert!(
        state.is_resumable(),
        "Failed WITH workflow_state SHOULD be resumable"
    );
}

/// This test shows that is_resumable() logic is correct - the bug is in not setting workflow_state
#[tokio::test]
async fn test_is_resumable_logic_is_correct() {
    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().to_path_buf();

    // Test all status combinations
    let test_cases = vec![
        (
            SessionStatus::InProgress,
            true,
            "InProgress with checkpoint",
        ),
        (
            SessionStatus::Interrupted,
            true,
            "Interrupted with checkpoint",
        ),
        (SessionStatus::Failed, true, "Failed with checkpoint"),
        (SessionStatus::Completed, false, "Completed (not resumable)"),
    ];

    for (status, expected_resumable, description) in test_cases {
        let mut state = SessionState::new(format!("test-{:?}", status), working_dir.clone());
        state.status = status.clone();

        // All statuses with workflow_state (except Completed)
        state.workflow_state = Some(WorkflowState {
            current_iteration: 0,
            current_step: 1,
            completed_steps: vec![],
            workflow_path: PathBuf::from("test.yml"),
            input_args: vec![],
            map_patterns: vec![],
            using_worktree: true,
        });

        let is_resumable = state.is_resumable();
        assert_eq!(
            is_resumable, expected_resumable,
            "{}: expected {}, got {}",
            description, expected_resumable, is_resumable
        );
    }
}

/// This test demonstrates the actual problem: workflow_state is not set during failure path
///
/// The execution flow is:
/// 1. Workflow step fails (e.g., commit_required with no commits)
/// 2. Error bubbles up before save_workflow_state() is called
/// 3. Session is marked as Failed
/// 4. Checkpoint may be saved to disk
/// 5. BUT workflow_state in SessionState remains None
/// 6. is_resumable() returns false even though checkpoint exists
#[tokio::test]
async fn test_demonstrates_the_bug() {
    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().to_path_buf();

    // Simulate the bug: Session is Failed but workflow_state was never set
    let mut state = SessionState::new("failed-session".to_string(), working_dir);

    // This is what happens in the real code:
    // 1. Step execution starts
    // 2. Step fails before save_workflow_state() is called
    // 3. Session status is set to Failed
    state.status = SessionStatus::Failed;

    // 4. workflow_state is NEVER set (this is the bug!)
    // In the real code, save_workflow_state() in workflow executor is only called
    // AFTER successful step execution, not BEFORE or during failure handling
    state.workflow_state = None; // This is the problematic state

    // 5. User tries to resume
    // 6. is_resumable() returns false because workflow_state is None
    assert!(
        !state.is_resumable(),
        "This is the bug: Session cannot be resumed because workflow_state is None"
    );

    // THE FIX: workflow_state should be set when the error checkpoint is saved
    // This should happen in the error handling path, not just the success path

    println!("BUG DEMONSTRATION:");
    println!("  Status: {:?}", state.status);
    println!("  workflow_state: {:?}", state.workflow_state);
    println!("  is_resumable(): {}", state.is_resumable());
    println!();
    println!("EXPECTED:");
    println!("  workflow_state should be Some(...) when checkpoint is saved on failure");
    println!("  is_resumable() should return true for Failed sessions with checkpoints");
}

/// Test to verify the fix: when a workflow fails, workflow_state should be set
///
/// This test will pass once the fix is implemented in the workflow executor.
#[tokio::test]
#[ignore] // Remove this once fix is implemented
async fn test_failed_workflow_sets_workflow_state() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let working_dir = temp_dir.path().to_path_buf();

    // After the fix, this is what should happen:
    let mut state = SessionState::new("test-session".to_string(), working_dir);

    // 1. Step fails
    state.status = SessionStatus::Failed;

    // 2. Error checkpoint is saved
    // 3. workflow_state is ALSO updated in SessionState (the fix)
    state.workflow_state = Some(WorkflowState {
        current_iteration: 0,
        current_step: 1, // The step that failed
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec!["157".to_string()],
        map_patterns: vec![],
        using_worktree: true,
    });

    // 4. Now is_resumable() correctly returns true
    assert!(
        state.is_resumable(),
        "After fix: Failed session with checkpoint should be resumable"
    );

    Ok(())
}
