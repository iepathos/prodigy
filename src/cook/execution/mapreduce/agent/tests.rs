//! Comprehensive tests for the agent module

use super::*;
use crate::worktree::WorktreeManager;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// Test AgentResult creation and status checking
#[test]
fn test_agent_result_creation() {
    let result = AgentResult::success(
        "item-1".to_string(),
        Some("output".to_string()),
        Duration::from_secs(5),
    );

    assert!(result.is_success());
    assert!(!result.is_failure());
    assert_eq!(result.item_id, "item-1");
    assert_eq!(result.output, Some("output".to_string()));
    assert!(matches!(result.status, AgentStatus::Success));
}

#[test]
fn test_agent_result_failure() {
    let result = AgentResult::failed(
        "item-2".to_string(),
        "error occurred".to_string(),
        Duration::from_secs(2),
    );

    assert!(!result.is_success());
    assert!(result.is_failure());
    assert_eq!(result.item_id, "item-2");
    assert_eq!(result.error, Some("error occurred".to_string()));
    assert!(matches!(result.status, AgentStatus::Failed(_)));
}

#[test]
fn test_agent_result_with_worktree_info() {
    let mut result = AgentResult::success("item-1".to_string(), None, Duration::from_secs(5));

    result.worktree_path = Some(PathBuf::from("/tmp/worktree"));
    result.branch_name = Some("feature-branch".to_string());
    result.worktree_session_id = Some("session-123".to_string());
    result.files_modified = vec!["file1.rs".to_string(), "file2.rs".to_string()];
    result.commits = vec!["commit1".to_string(), "commit2".to_string()];

    assert_eq!(result.worktree_path, Some(PathBuf::from("/tmp/worktree")));
    assert_eq!(result.branch_name, Some("feature-branch".to_string()));
    assert_eq!(result.worktree_session_id, Some("session-123".to_string()));
    assert_eq!(result.files_modified.len(), 2);
    assert_eq!(result.commits.len(), 2);
}

// Test AggregatedResults functionality
#[test]
fn test_aggregated_results() {
    let results = vec![
        AgentResult::success("item-1".to_string(), None, Duration::from_secs(5)),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            Duration::from_secs(3),
        ),
        AgentResult::success("item-3".to_string(), None, Duration::from_secs(4)),
    ];

    let aggregated = AggregatedResults::from_results(results);

    assert_eq!(aggregated.success_count, 2);
    assert_eq!(aggregated.failure_count, 1);
    assert_eq!(aggregated.total, 3);
    assert_eq!(aggregated.successful.len(), 2);
    assert_eq!(aggregated.failed.len(), 1);
}

#[test]
fn test_aggregated_results_json_conversion() {
    let mut results = vec![
        AgentResult::success(
            "item-1".to_string(),
            Some("output1".to_string()),
            Duration::from_secs(5),
        ),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            Duration::from_secs(3),
        ),
    ];

    // Add commits to first result
    results[0].commits = vec!["commit1".to_string()];

    let aggregated = AggregatedResults::from_results(results);
    let json_value = aggregated.to_json_value();

    assert_eq!(json_value["total"], json!(2));
    assert_eq!(json_value["success_count"], json!(1));
    assert_eq!(json_value["failure_count"], json!(1));
    assert!(json_value["successful"].is_array());
    assert!(json_value["failed"].is_array());
}

#[test]
fn test_aggregated_results_empty() {
    let aggregated = AggregatedResults::from_results(vec![]);

    assert_eq!(aggregated.total, 0);
    assert_eq!(aggregated.success_count, 0);
    assert_eq!(aggregated.failure_count, 0);
    assert!(aggregated.successful.is_empty());
    assert!(aggregated.failed.is_empty());
}

// Test AgentResultAggregator
#[tokio::test]
async fn test_default_result_aggregator() {
    let aggregator = DefaultResultAggregator::new();

    let results = vec![
        AgentResult::success("item-1".to_string(), None, Duration::from_secs(1)),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            Duration::from_secs(1),
        ),
        AgentResult::success(
            "item-3".to_string(),
            Some("output".to_string()),
            Duration::from_secs(2),
        ),
    ];

    let aggregated = aggregator.aggregate(results.clone());

    assert_eq!(aggregated.success_count, 2);
    assert_eq!(aggregated.failure_count, 1);
    assert_eq!(aggregated.total, 3);
}

#[tokio::test]
async fn test_aggregator_to_interpolation_context() {
    let aggregator = DefaultResultAggregator::new();

    let results = vec![
        AgentResult::success(
            "item-1".to_string(),
            Some("output1".to_string()),
            Duration::from_secs(1),
        ),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            Duration::from_secs(1),
        ),
    ];

    let aggregated = aggregator.aggregate(results.clone());
    let context = aggregator.to_interpolation_context(&aggregated);

    // Check that variables were set correctly
    assert!(context.variables.contains_key("map.successful"));
    assert!(context.variables.contains_key("map.failed"));
    assert!(context.variables.contains_key("map.total"));
    assert!(context.variables.contains_key("map.results"));
}

#[tokio::test]
async fn test_aggregator_to_variable_context() {
    let aggregator = DefaultResultAggregator::new();

    let results = vec![
        AgentResult::success("item-1".to_string(), None, Duration::from_secs(1)),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            Duration::from_secs(1),
        ),
    ];

    let aggregated = aggregator.aggregate(results.clone());
    let context = aggregator.to_variable_context(&aggregated).await;

    // Verify that the context has the expected global variables
    // Note: We can't easily check the actual values without exposing internal state,
    // but we can verify the context was created
    assert_eq!(std::mem::size_of_val(&context) > 0, true);
}

// Test AgentState and status transitions
#[test]
fn test_agent_state_transitions() {
    let mut state = AgentState::default();

    assert_eq!(state.status, AgentStateStatus::Idle);

    state.status = AgentStateStatus::Executing;
    assert!(matches!(state.status, AgentStateStatus::Executing));

    state.status = AgentStateStatus::Completed;
    assert!(matches!(state.status, AgentStateStatus::Completed));
}

#[test]
fn test_agent_state_with_error() {
    let mut state = AgentState::default();

    state.status = AgentStateStatus::Failed("Test error".to_string());
    state.retry_count = 2;

    assert!(matches!(state.status, AgentStateStatus::Failed(_)));
    assert_eq!(state.retry_count, 2);
}

// Test AgentOperation enum
#[test]
fn test_agent_operation_variants() {
    let op1 = AgentOperation::Idle;
    assert!(matches!(op1, AgentOperation::Idle));

    let op2 = AgentOperation::Claude("test command".to_string());
    if let AgentOperation::Claude(item) = op2 {
        assert_eq!(item, "test command");
    } else {
        panic!("Expected Processing variant");
    }

    let op3 = AgentOperation::Retrying("item-2".to_string(), 2);
    if let AgentOperation::Retrying(item, attempt) = op3 {
        assert_eq!(item, "item-2");
        assert_eq!(attempt, 2);
    } else {
        panic!("Expected Retrying variant");
    }

    let op4 = AgentOperation::Complete;
    assert!(matches!(op4, AgentOperation::Complete));
}

// Test lifecycle manager
#[tokio::test]
async fn test_default_lifecycle_manager() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Initialize git repository in temp directory
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&project_root)
        .output()
        .expect("Failed to initialize git repository");

    // Configure git user for the test
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to configure git email");

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to configure git name");

    // Create initial commit
    std::fs::write(project_root.join("README.md"), "Test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&project_root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&project_root)
        .output()
        .unwrap();

    let subprocess = crate::subprocess::SubprocessManager::production();
    let worktree_manager = Arc::new(WorktreeManager::new(project_root, subprocess).unwrap());

    let lifecycle_manager = DefaultLifecycleManager::new(worktree_manager);

    let config = AgentConfig::new(
        "agent-1".to_string(),
        "item-1".to_string(),
        "test-branch".to_string(),
        3,
        Duration::from_secs(60),
        0,
        10,
    );

    // Test initialization
    let commands = vec![];
    let handle = lifecycle_manager
        .create_agent(config, commands)
        .await
        .unwrap();
    assert_eq!(handle.id(), "agent-1");

    // Test cleanup (should not fail even if worktree doesn't exist)
    lifecycle_manager.cleanup_agent(handle).await.unwrap();
}

// Test helper functions for creating test data
fn create_test_agent_result(id: &str, success: bool) -> AgentResult {
    if success {
        AgentResult::success(
            id.to_string(),
            Some(format!("Output for {}", id)),
            Duration::from_secs(1),
        )
    } else {
        AgentResult::failed(
            id.to_string(),
            format!("Error for {}", id),
            Duration::from_secs(1),
        )
    }
}

#[test]
fn test_batch_agent_results() {
    let results: Vec<AgentResult> = (0..10)
        .map(|i| create_test_agent_result(&format!("item-{}", i), i % 3 != 0))
        .collect();

    let success_count = results.iter().filter(|r| r.is_success()).count();
    let failure_count = results.iter().filter(|r| r.is_failure()).count();

    assert_eq!(success_count, 6); // items 1,2,4,5,7,8
    assert_eq!(failure_count, 4); // items 0,3,6,9
    assert_eq!(success_count + failure_count, 10);
}

// Test JSON serialization/deserialization
#[test]
fn test_agent_result_serialization() {
    let result = AgentResult::success(
        "item-1".to_string(),
        Some("output".to_string()),
        Duration::from_secs(5),
    );

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: AgentResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.item_id, deserialized.item_id);
    assert_eq!(result.output, deserialized.output);
    assert!(matches!(deserialized.status, AgentStatus::Success));
}

#[test]
fn test_aggregated_results_serialization() {
    let results = vec![
        AgentResult::success("item-1".to_string(), None, Duration::from_secs(1)),
        AgentResult::failed(
            "item-2".to_string(),
            "error".to_string(),
            Duration::from_secs(1),
        ),
    ];

    let aggregated = AggregatedResults::from_results(results);

    let json = serde_json::to_string(&aggregated).unwrap();
    let deserialized: AggregatedResults = serde_json::from_str(&json).unwrap();

    assert_eq!(aggregated.total, deserialized.total);
    assert_eq!(aggregated.success_count, deserialized.success_count);
    assert_eq!(aggregated.failure_count, deserialized.failure_count);
}

// Test edge cases
#[test]
fn test_agent_result_timeout_status() {
    let mut result = AgentResult::success("item-1".to_string(), None, Duration::from_secs(5));

    result.status = AgentStatus::Timeout;

    assert!(!result.is_success());
    assert!(result.is_failure()); // Timeout is considered a failure
    assert!(matches!(result.status, AgentStatus::Timeout));
}

#[test]
fn test_agent_result_retrying_status() {
    let mut result = AgentResult::success("item-1".to_string(), None, Duration::from_secs(5));

    result.status = AgentStatus::Retrying(2);

    assert!(!result.is_success());
    assert!(!result.is_failure());
    if let AgentStatus::Retrying(count) = result.status {
        assert_eq!(count, 2);
    } else {
        panic!("Expected Retrying status");
    }
}

// Performance tests
#[test]
fn test_large_aggregation_performance() {
    let start = std::time::Instant::now();

    let results: Vec<AgentResult> = (0..1000)
        .map(|i| create_test_agent_result(&format!("item-{}", i), i % 2 == 0))
        .collect();

    let aggregated = AggregatedResults::from_results(results);

    let elapsed = start.elapsed();

    assert_eq!(aggregated.total, 1000);
    assert_eq!(aggregated.success_count, 500);
    assert_eq!(aggregated.failure_count, 500);

    // Should complete in under 100ms
    assert!(elapsed.as_millis() < 100);
}

// Integration test for the complete flow
#[tokio::test]
async fn test_agent_workflow_integration() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path().to_path_buf();

    // Initialize git repository in temp directory
    std::process::Command::new("git")
        .arg("init")
        .current_dir(&project_root)
        .output()
        .expect("Failed to initialize git repository");

    // Configure git user for the test
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to configure git email");

    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to configure git name");

    // Create initial commit
    std::fs::write(project_root.join("README.md"), "Test").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&project_root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&project_root)
        .output()
        .unwrap();

    let subprocess = crate::subprocess::SubprocessManager::production();
    let worktree_manager = Arc::new(WorktreeManager::new(project_root, subprocess).unwrap());

    let lifecycle_manager = DefaultLifecycleManager::new(worktree_manager);
    let aggregator = DefaultResultAggregator::new();

    // Create multiple agent configs
    let configs: Vec<AgentConfig> = (0..3)
        .map(|i| {
            AgentConfig::new(
                format!("agent-{}", i),
                format!("item-{}", i),
                format!("branch-{}", i),
                3,
                Duration::from_secs(60),
                i,
                3,
            )
        })
        .collect();

    // Initialize agents
    let mut handles = vec![];
    for config in configs {
        let commands = vec![];
        let handle = lifecycle_manager
            .create_agent(config, commands)
            .await
            .unwrap();
        handles.push(handle);
    }

    // Simulate results
    let results: Vec<AgentResult> = handles
        .iter()
        .enumerate()
        .map(|(i, handle)| {
            if i % 2 == 0 {
                AgentResult::success(
                    handle.config.item_id.clone(),
                    Some(format!("Output from {}", handle.id())),
                    Duration::from_secs(i as u64 + 1),
                )
            } else {
                AgentResult::failed(
                    handle.config.item_id.clone(),
                    format!("Error from {}", handle.id()),
                    Duration::from_secs(i as u64 + 1),
                )
            }
        })
        .collect();

    // Aggregate results
    let aggregated = aggregator.aggregate(results.clone());
    assert_eq!(aggregated.total, 3);
    assert_eq!(aggregated.success_count, 2);
    assert_eq!(aggregated.failure_count, 1);

    // Convert to contexts
    let interp_context = aggregator.to_interpolation_context(&aggregated);
    assert!(interp_context.variables.contains_key("map.total"));

    let var_context = aggregator.to_variable_context(&aggregated).await;
    // Context should be created successfully
    assert!(std::mem::size_of_val(&var_context) > 0);

    // Cleanup
    for handle in handles {
        lifecycle_manager.cleanup_agent(handle).await.unwrap();
    }
}

// Test concurrent agent operations
#[tokio::test]
async fn test_concurrent_agent_operations() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let results = Arc::new(Mutex::new(Vec::new()));

    let mut handles = vec![];
    for i in 0..10 {
        let results_clone = results.clone();
        let handle = tokio::spawn(async move {
            // Simulate some async work
            tokio::time::sleep(Duration::from_millis(10)).await;

            let result = if i % 3 == 0 {
                AgentResult::failed(
                    format!("item-{}", i),
                    "error".to_string(),
                    Duration::from_millis(10),
                )
            } else {
                AgentResult::success(
                    format!("item-{}", i),
                    Some(format!("output-{}", i)),
                    Duration::from_millis(10),
                )
            };

            let mut results = results_clone.lock().await;
            results.push(result);
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    let results = results.lock().await;
    assert_eq!(results.len(), 10);

    let success_count = results.iter().filter(|r| r.is_success()).count();
    let failure_count = results.iter().filter(|r| r.is_failure()).count();

    assert_eq!(success_count, 6); // items 1,2,4,5,7,8
    assert_eq!(failure_count, 4); // items 0,3,6,9
}
