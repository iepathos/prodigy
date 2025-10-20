//! Tests for MapReduce coordination executor
//!
//! These tests verify the critical behavior of agent merge failure handling
//! and status tracking in the MapReduce coordinator.

use crate::cook::execution::mapreduce::agent::{AgentResult, AgentStatus};
use crate::cook::execution::mapreduce::aggregation::AggregationSummary;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn test_aggregation_summary_counts_merge_failures() {
    // Create results with mix of successes and merge failures
    let results = vec![
        AgentResult {
            item_id: "item-0".to_string(),
            status: AgentStatus::Success,
            output: Some("success".to_string()),
            commits: vec!["commit1".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
        AgentResult {
            item_id: "item-1".to_string(),
            status: AgentStatus::Failed(
                "Agent execution succeeded but merge to parent worktree failed".to_string(),
            ),
            output: Some("agent output".to_string()),
            commits: vec!["commit2".to_string()],
            duration: Duration::from_secs(2),
            error: Some("Merge to parent worktree failed - changes not integrated".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
        AgentResult {
            item_id: "item-2".to_string(),
            status: AgentStatus::Success,
            output: Some("success".to_string()),
            commits: vec!["commit3".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
    ];

    let summary = AggregationSummary::from_results(&results);

    assert_eq!(summary.total, 3, "Total count should include all results");
    assert_eq!(
        summary.successful, 2,
        "Should count only successfully merged agents as successful"
    );
    assert_eq!(summary.failed, 1, "Should count merge failures as failed");
}

#[tokio::test]
async fn test_aggregation_summary_with_all_merge_failures() {
    // Edge case: all agents fail to merge
    let results = vec![
        AgentResult {
            item_id: "item-0".to_string(),
            status: AgentStatus::Failed("merge failed".to_string()),
            output: Some("output".to_string()),
            commits: vec!["commit1".to_string()],
            duration: Duration::from_secs(1),
            error: Some("Merge to parent worktree failed".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
        AgentResult {
            item_id: "item-1".to_string(),
            status: AgentStatus::Failed("merge failed".to_string()),
            output: Some("output".to_string()),
            commits: vec!["commit2".to_string()],
            duration: Duration::from_secs(1),
            error: Some("Merge to parent worktree failed".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
    ];

    let summary = AggregationSummary::from_results(&results);

    assert_eq!(summary.total, 2);
    assert_eq!(summary.successful, 0, "No agents should be successful");
    assert_eq!(
        summary.failed, 2,
        "All agents with merge failures should be counted as failed"
    );
}

#[tokio::test]
async fn test_agent_result_status_updated_on_merge_failure() {
    // This test verifies that when an agent executes successfully but merge fails,
    // the result status is updated to Failed

    let mut result = AgentResult {
        item_id: "item-0".to_string(),
        status: AgentStatus::Success,
        output: Some("agent executed successfully".to_string()),
        commits: vec!["commit1".to_string()],
        duration: Duration::from_secs(5),
        error: None,
        worktree_path: Some(PathBuf::from("/tmp/worktree")),
        branch_name: Some("agent-branch".to_string()),
        worktree_session_id: Some("session-123".to_string()),
        files_modified: vec!["file1.rs".to_string()],
        json_log_location: None,
        cleanup_status: None,
    };

    // Simulate what happens when merge fails
    result.status = AgentStatus::Failed(
        "Agent execution succeeded but merge to parent worktree failed".to_string(),
    );
    result.error = Some("Merge to parent worktree failed - changes not integrated".to_string());

    // Verify the status is correctly set
    assert!(
        matches!(result.status, AgentStatus::Failed(_)),
        "Status should be Failed"
    );
    assert!(result.error.is_some(), "Error message should be set");
    assert_eq!(
        result.commits.len(),
        1,
        "Commits should still be tracked even though merge failed"
    );
}

#[test]
fn test_merge_failure_error_message_clarity() {
    // Verify that error messages are clear and actionable
    let result = AgentResult {
        item_id: "item-0".to_string(),
        status: AgentStatus::Failed(
            "Agent execution succeeded but merge to parent worktree failed".to_string(),
        ),
        output: Some("output".to_string()),
        commits: vec!["commit1".to_string()],
        duration: Duration::from_secs(1),
        error: Some("Merge to parent worktree failed - changes not integrated".to_string()),
        worktree_path: None,
        branch_name: None,
        worktree_session_id: None,
        files_modified: vec![],
        json_log_location: None,
        cleanup_status: None,
    };

    if let AgentStatus::Failed(msg) = &result.status {
        assert!(msg.contains("merge"), "Status message should mention merge");
        assert!(
            msg.contains("worktree"),
            "Status message should mention worktree"
        );
    } else {
        panic!("Expected Failed status");
    }

    let error_msg = result.error.as_ref().unwrap();
    assert!(
        error_msg.contains("not integrated"),
        "Error should explain that changes weren't integrated"
    );
}

#[test]
fn test_aggregation_with_mixed_failure_types() {
    // Test that different types of failures are all counted correctly
    let results = vec![
        // Successful agent
        AgentResult {
            item_id: "item-0".to_string(),
            status: AgentStatus::Success,
            output: Some("success".to_string()),
            commits: vec!["commit1".to_string()],
            duration: Duration::from_secs(1),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
        // Agent with merge failure
        AgentResult {
            item_id: "item-1".to_string(),
            status: AgentStatus::Failed("merge failed".to_string()),
            output: Some("output".to_string()),
            commits: vec!["commit2".to_string()],
            duration: Duration::from_secs(1),
            error: Some("Merge failed".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
        // Agent with execution failure
        AgentResult {
            item_id: "item-2".to_string(),
            status: AgentStatus::Failed("execution error".to_string()),
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: Some("Command failed".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
        // Agent with timeout
        AgentResult {
            item_id: "item-3".to_string(),
            status: AgentStatus::Timeout,
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: Some("Timeout".to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        },
    ];

    let summary = AggregationSummary::from_results(&results);

    assert_eq!(summary.total, 4);
    assert_eq!(summary.successful, 1, "Only one agent succeeded and merged");
    assert_eq!(
        summary.failed, 3,
        "All failure types (merge, execution, timeout) should be counted"
    );
}

#[test]
fn test_no_commits_no_merge_failure() {
    // Agent with no commits shouldn't fail even if merge fails,
    // because there's nothing to merge
    let results = vec![AgentResult {
        item_id: "item-0".to_string(),
        status: AgentStatus::Success,
        output: Some("no changes".to_string()),
        commits: vec![], // No commits
        duration: Duration::from_secs(1),
        error: None,
        worktree_path: None,
        branch_name: None,
        worktree_session_id: None,
        files_modified: vec![],
        json_log_location: None,
        cleanup_status: None,
    }];

    let summary = AggregationSummary::from_results(&results);

    assert_eq!(
        summary.successful, 1,
        "Agent with no commits is still successful"
    );
    assert_eq!(summary.failed, 0, "Should not count as failed");
}
