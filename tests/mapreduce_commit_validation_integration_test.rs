//! Integration tests for MapReduce commit validation enforcement (Spec 163)
//!
//! Tests the complete flow of commit_required validation including:
//! - Event stream integration with commit metadata
//! - DLQ integration for commit validation failures
//! - Merge logic skipping agents without commits

use prodigy::cook::execution::dlq::ErrorType;
use prodigy::cook::execution::mapreduce::agent::types::AgentResult;
use prodigy::cook::execution::mapreduce::dlq_integration::agent_result_to_dlq_item;
use prodigy::cook::execution::mapreduce::event::{FailureReason, MapReduceEvent};
use serde_json::json;
use std::time::Duration;

#[test]
fn test_event_stream_includes_commits_on_success() {
    // Test that AgentCompleted events include commit metadata
    let commits = vec!["abc123".to_string(), "def456".to_string()];
    let event = MapReduceEvent::agent_completed(
        "agent-1".to_string(),
        "item-1".to_string(),
        chrono::Duration::seconds(30),
        None,
        commits.clone(),
        Some("/path/to/log.json".to_string()),
    );

    match event {
        MapReduceEvent::AgentCompleted {
            commits: event_commits,
            json_log_location,
            ..
        } => {
            assert_eq!(event_commits, commits);
            assert_eq!(json_log_location, Some("/path/to/log.json".to_string()));
        }
        _ => panic!("Expected AgentCompleted event"),
    }
}

#[test]
fn test_event_stream_includes_failure_reason_on_commit_validation_error() {
    // Test that AgentFailed events include CommitValidationFailed reason
    let event = MapReduceEvent::agent_failed(
        "agent-2".to_string(),
        "item-2".to_string(),
        "Commit required but no commit was created".to_string(),
        FailureReason::CommitValidationFailed {
            command: "shell: echo test".to_string(),
        },
        Some("/path/to/log.json".to_string()),
    );

    match event {
        MapReduceEvent::AgentFailed {
            failure_reason,
            json_log_location,
            ..
        } => {
            assert!(matches!(
                failure_reason,
                FailureReason::CommitValidationFailed { .. }
            ));
            assert_eq!(json_log_location, Some("/path/to/log.json".to_string()));
        }
        _ => panic!("Expected AgentFailed event"),
    }
}

#[test]
fn test_dlq_captures_commit_validation_failures() {
    use prodigy::cook::execution::mapreduce::agent::types::AgentStatus;

    // Create a failed agent result with commit validation error
    let result = AgentResult {
        item_id: "item-1".to_string(),
        status: AgentStatus::Failed(
            "Commit required but no commit was created. Command: shell: echo test".to_string(),
        ),
        output: None,
        commits: vec![],
        files_modified: vec![],
        duration: Duration::from_secs(10),
        error: Some("Commit validation failed".to_string()),
        worktree_path: Some(std::path::PathBuf::from("/tmp/worktree")),
        branch_name: Some("agent-branch".to_string()),
        worktree_session_id: Some("session-123".to_string()),
        json_log_location: Some("/path/to/log.json".to_string()),
        cleanup_status: None,
    };

    let work_item = json!({"id": 1, "file": "test.txt"});
    let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

    assert!(dlq_item.is_some());
    let item = dlq_item.unwrap();

    // Check that error type is correctly classified
    assert_eq!(
        item.failure_history[0].error_type,
        ErrorType::CommitValidationFailed
    );

    // Check that manual review is required
    assert!(item.manual_review_required);

    // Check that JSON log location is captured
    assert_eq!(
        item.failure_history[0].json_log_location,
        Some("/path/to/log.json".to_string())
    );
}

#[test]
fn test_dlq_classification_from_error_message() {
    use prodigy::cook::execution::mapreduce::agent::types::AgentStatus;

    // Test various commit validation error message patterns
    let test_cases = vec![
        "Commit required but no commit was created",
        "commit validation failed",
        "COMMIT REQUIRED: No commits detected",
    ];

    for error_msg in test_cases {
        let result = AgentResult {
            item_id: "item-test".to_string(),
            status: AgentStatus::Failed(error_msg.to_string()),
            output: None,
            commits: vec![],
            files_modified: vec![],
            duration: Duration::from_secs(5),
            error: Some(error_msg.to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            json_log_location: None,
            cleanup_status: None,
        };

        let work_item = json!({"test": true});
        let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

        assert!(
            dlq_item.is_some(),
            "Should create DLQ item for: {}",
            error_msg
        );
        let item = dlq_item.unwrap();
        assert_eq!(
            item.failure_history[0].error_type,
            ErrorType::CommitValidationFailed,
            "Should classify as CommitValidationFailed for: {}",
            error_msg
        );
    }
}

#[test]
fn test_successful_agents_not_added_to_dlq() {
    use prodigy::cook::execution::mapreduce::agent::types::AgentStatus;

    // Successful agents should not be added to DLQ
    let result = AgentResult {
        item_id: "item-success".to_string(),
        status: AgentStatus::Success,
        output: Some("Success".to_string()),
        commits: vec!["abc123".to_string()],
        files_modified: vec!["file.txt".to_string()],
        duration: Duration::from_secs(5),
        error: None,
        worktree_path: None,
        branch_name: None,
        worktree_session_id: None,
        json_log_location: Some("/path/to/log.json".to_string()),
        cleanup_status: None,
    };

    let work_item = json!({"test": true});
    let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

    assert!(
        dlq_item.is_none(),
        "Successful agents should not create DLQ items"
    );
}
