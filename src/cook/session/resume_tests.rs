//! Tests for workflow resume functionality

use super::*;
use crate::cook::session::{ExecutionEnvironment, SessionState, StepResult, WorkflowState};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_session_is_resumable() {
    let mut state = SessionState::new("test-session".to_string(), PathBuf::from("/test"));

    // Initially not resumable without workflow state
    assert!(!state.is_resumable());

    // Add workflow state
    let workflow_state = WorkflowState {
        current_iteration: 0,
        current_step: 2,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: false,
    };
    state.update_workflow_state(workflow_state);

    // Now should be resumable
    assert!(state.is_resumable());

    // Mark as completed - no longer resumable
    state.complete();
    assert!(!state.is_resumable());
}

#[tokio::test]
async fn test_workflow_state_checkpoint() {
    let mut state = SessionState::new("test-session".to_string(), PathBuf::from("/test"));

    let workflow_state = WorkflowState {
        current_iteration: 1,
        current_step: 3,
        completed_steps: vec![
            StepResult {
                step_index: 0,
                command: "step1".to_string(),
                success: true,
                output: Some("output1".to_string()),
                duration: std::time::Duration::from_secs(1),
            },
            StepResult {
                step_index: 1,
                command: "step2".to_string(),
                success: true,
                output: None,
                duration: std::time::Duration::from_secs(2),
            },
        ],
        workflow_path: PathBuf::from("workflow.yml"),
        input_args: vec!["arg1".to_string(), "arg2".to_string()],
        map_patterns: vec!["*.rs".to_string()],
        using_worktree: true,
    };

    state.update_workflow_state(workflow_state.clone());

    // Verify checkpoint was set
    assert!(state.last_checkpoint.is_some());

    // Verify workflow state was saved
    assert!(state.workflow_state.is_some());
    let saved = state.workflow_state.unwrap();
    assert_eq!(saved.current_iteration, 1);
    assert_eq!(saved.current_step, 3);
    assert_eq!(saved.completed_steps.len(), 2);
    assert_eq!(saved.input_args.len(), 2);
    assert_eq!(saved.map_patterns.len(), 1);
    assert!(saved.using_worktree);
}

#[tokio::test]
async fn test_session_tracker_resume_functions() {
    let temp_dir = TempDir::new().unwrap();
    let tracker = SessionTrackerImpl::new(
        "test-session-123".to_string(),
        temp_dir.path().to_path_buf(),
    );

    // Test listing resumable sessions (should be empty initially)
    let sessions = tracker.list_resumable().await.unwrap();
    assert_eq!(sessions.len(), 0);

    // Test getting last interrupted (should be None)
    let last = tracker.get_last_interrupted().await.unwrap();
    assert!(last.is_none());

    // Save a session state with workflow info
    let mut state = SessionState::new(
        "test-session-123".to_string(),
        temp_dir.path().to_path_buf(),
    );
    state.status = SessionStatus::Interrupted;
    state.workflow_state = Some(WorkflowState {
        current_iteration: 0,
        current_step: 1,
        completed_steps: vec![],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: false,
    });

    // Save checkpoint
    tracker.save_checkpoint(&state).await.unwrap();

    // Now should be able to load the session
    let loaded = tracker.load_session("test-session-123").await.unwrap();
    assert_eq!(loaded.session_id, "test-session-123");
    assert_eq!(loaded.status, SessionStatus::Interrupted);
    assert!(loaded.workflow_state.is_some());
}

#[tokio::test]
async fn test_get_resume_info() {
    let mut state = SessionState::new("test-session".to_string(), PathBuf::from("/test"));

    // No info without workflow state
    assert!(state.get_resume_info().is_none());

    // Add workflow state
    let workflow_state = WorkflowState {
        current_iteration: 2,
        current_step: 4,
        completed_steps: vec![StepResult {
            step_index: 0,
            command: "step1".to_string(),
            success: true,
            output: None,
            duration: std::time::Duration::from_secs(1),
        }],
        workflow_path: PathBuf::from("test.yml"),
        input_args: vec![],
        map_patterns: vec![],
        using_worktree: false,
    };
    state.update_workflow_state(workflow_state);

    // Should have resume info now
    let info = state.get_resume_info().unwrap();
    assert!(info.contains("Step 5/2")); // current_step + 1 / completed_steps.len() + 1
    assert!(info.contains("iteration 3")); // current_iteration + 1
}

#[tokio::test]
async fn test_execution_environment_serialization() {
    let env = ExecutionEnvironment {
        working_directory: PathBuf::from("/test/dir"),
        worktree_name: Some("worktree-123".to_string()),
        environment_vars: {
            let mut vars = HashMap::new();
            vars.insert("KEY1".to_string(), "value1".to_string());
            vars.insert("KEY2".to_string(), "value2".to_string());
            vars
        },
        command_args: vec!["arg1".to_string(), "arg2".to_string()],
    };

    // Test serialization
    let json = serde_json::to_string(&env).unwrap();
    assert!(json.contains("working_directory"));
    assert!(json.contains("worktree-123"));
    assert!(json.contains("KEY1"));

    // Test deserialization
    let deserialized: ExecutionEnvironment = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.working_directory, PathBuf::from("/test/dir"));
    assert_eq!(deserialized.worktree_name, Some("worktree-123".to_string()));
    assert_eq!(deserialized.environment_vars.len(), 2);
    assert_eq!(deserialized.command_args.len(), 2);
}
