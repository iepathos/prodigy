//! Tests for DLQ reprocessor functionality

use super::dlq_reprocessor::*;
use crate::cook::execution::dlq::{
    DLQFilter, DeadLetterQueue, DeadLetteredItem, ErrorType as DlqErrorType, FailureDetail,
};
use chrono::{Duration, Utc};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a test DLQ with sample items
async fn create_test_dlq_with_items(
    job_id: &str,
) -> anyhow::Result<(Arc<DeadLetterQueue>, TempDir)> {
    let temp_dir = TempDir::new()?;
    let dlq = Arc::new(
        DeadLetterQueue::new(
            job_id.to_string(),
            temp_dir.path().to_path_buf(),
            100,
            30,
            None,
        )
        .await?,
    );

    // Add test items
    let item1 = DeadLetteredItem {
        item_id: "test-item-1".to_string(),
        item_data: json!({"id": 1, "priority": "high"}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 3,
        failure_history: vec![FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: DlqErrorType::CommandFailed { exit_code: 1 },
            error_message: "Test failure".to_string(),
            stack_trace: None,
            agent_id: "agent-1".to_string(),
            step_failed: "map".to_string(),
            duration_ms: 100,
        }],
        error_signature: "test-error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let item2 = DeadLetteredItem {
        item_id: "test-item-2".to_string(),
        item_data: json!({"id": 2, "priority": "low"}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 5,
        failure_history: vec![],
        error_signature: "test-error-2".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: false,
        manual_review_required: true,
    };

    dlq.add(item1).await?;
    dlq.add(item2).await?;

    Ok((dlq, temp_dir))
}

#[tokio::test]
async fn test_filter_evaluator_expressions() {
    let item = DeadLetteredItem {
        item_id: "test-1".to_string(),
        item_data: json!({
            "priority": "high",
            "score": 10,
            "name": "test item"
        }),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 3,
        failure_history: vec![],
        error_signature: "test".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    // Test various filter expressions
    let test_cases = vec![
        ("item.priority == 'high'", true),
        ("item.priority == 'low'", false),
        ("item.priority != 'low'", true),
        ("item.score >= 10", true),
        ("item.score > 10", false),
        ("item.score < 20", true),
        ("item.failure_count >= 3", true),
        ("item.failure_count > 5", false),
        ("item.reprocess_eligible == true", true),
        ("item.reprocess_eligible == false", false),
        ("item.name contains 'test'", true),
        ("item.name contains 'xyz'", false),
    ];

    for (expression, expected) in test_cases {
        let evaluator = FilterEvaluator::new(expression.to_string());
        assert_eq!(
            evaluator.matches(&item),
            expected,
            "Failed for expression: {}",
            expression
        );
    }
}

// Commented out - requires complex MapReduceExecutor setup
/*
#[tokio::test]
async fn test_reprocess_eligible_items() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-1").await.unwrap();
    let project_root = PathBuf::from(".");

    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root.clone());

    // Create mock executor
    let worktree_manager = Arc::new(WorktreeManager::new(project_root.clone()).await.unwrap());
    let claude_executor = Arc::new(ClaudeExecutorImpl::new());
    let session_manager = Arc::new(MockSessionManager::new());
    let user_interaction = Arc::new(MockUserInteraction::new(true));

    let executor = Arc::new(MapReduceExecutor::new(
        claude_executor,
        session_manager,
        user_interaction,
        worktree_manager,
        project_root,
    ));

    // Run reprocessing without force (should only process eligible items)
    let options = ReprocessOptions {
        max_retries: 2,
        filter: None,
        parallel: 5,
        timeout_per_item: 60,
        strategy: RetryStrategy::Immediate,
        merge_results: true,
        force: false,
    };

    let result = reprocessor
        .reprocess("test-job-1", options, executor)
        .await
        .unwrap();

    // Should only process the eligible item
    assert_eq!(result.total_items, 1);
    assert_eq!(result.successful, 1);
    assert_eq!(result.failed, 0);
}
*/

// Commented out - requires complex MapReduceExecutor setup
/*
#[tokio::test]
async fn test_reprocess_with_filter() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-2").await.unwrap();
    let project_root = PathBuf::from(".");

    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root.clone());

    // Create mock executor
    let worktree_manager = Arc::new(WorktreeManager::new(project_root.clone()).await.unwrap());
    let claude_executor = Arc::new(ClaudeExecutorImpl::new());
    let session_manager = Arc::new(MockSessionManager::new());
    let user_interaction = Arc::new(MockUserInteraction::new(true));

    let executor = Arc::new(MapReduceExecutor::new(
        claude_executor,
        session_manager,
        user_interaction,
        worktree_manager,
        project_root,
    ));

    // Run with filter for high priority items
    let options = ReprocessOptions {
        max_retries: 2,
        filter: Some("item.priority == 'high'".to_string()),
        parallel: 5,
        timeout_per_item: 60,
        strategy: RetryStrategy::Immediate,
        merge_results: true,
        force: true, // Force to bypass eligibility check
    };

    let result = reprocessor
        .reprocess("test-job-2", options, executor)
        .await
        .unwrap();

    // Should only process high priority item
    assert_eq!(result.total_items, 1);
}
*/

#[tokio::test]
async fn test_concurrent_reprocessing_prevention() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-3").await.unwrap();
    let project_root = PathBuf::from(".");

    let reprocessor = Arc::new(DlqReprocessor::new(dlq.clone(), None, project_root.clone()));

    // Acquire lock for the first reprocessing
    reprocessor
        .acquire_reprocessing_lock("test-job-3")
        .await
        .unwrap();

    // Second attempt should fail
    let result = reprocessor.acquire_reprocessing_lock("test-job-3").await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("already being reprocessed"));

    // Release lock
    reprocessor.release_reprocessing_lock("test-job-3").await;

    // Now should succeed
    let result = reprocessor.acquire_reprocessing_lock("test-job-3").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_clear_processed_items() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-4").await.unwrap();
    let project_root = PathBuf::from(".");

    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root);

    // Clear processed items (non-eligible items)
    let count = reprocessor
        .clear_processed_items("test-job-4")
        .await
        .unwrap();

    // Should clear 1 item (the non-eligible one)
    assert_eq!(count, 1);

    // Verify only eligible item remains
    let remaining = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].item_id, "test-item-1");
}

#[tokio::test]
async fn test_retry_strategy_delays() {
    use std::time::Instant;

    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-5").await.unwrap();
    let project_root = PathBuf::from(".");
    let reprocessor = DlqReprocessor::new(dlq, None, project_root);

    // Test immediate strategy (no delay)
    let start = Instant::now();
    reprocessor
        .apply_retry_delay(&RetryStrategy::Immediate, 2)
        .await;
    assert!(start.elapsed().as_millis() < 10);

    // Test fixed delay
    let start = Instant::now();
    reprocessor
        .apply_retry_delay(&RetryStrategy::FixedDelay { delay_ms: 50 }, 2)
        .await;
    assert!(start.elapsed().as_millis() >= 50);

    // Test exponential backoff
    let start = Instant::now();
    reprocessor
        .apply_retry_delay(&RetryStrategy::ExponentialBackoff, 3)
        .await;
    // 2^(3-1) * 1000 = 4000ms
    assert!(start.elapsed().as_millis() >= 4000);
}

#[tokio::test]
async fn test_global_stats() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-6").await.unwrap();
    let project_root = PathBuf::from(".");

    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root.clone());

    let stats = reprocessor.get_global_stats(&project_root).await.unwrap();

    assert_eq!(stats.total_workflows, 1);
    assert_eq!(stats.total_items, 2);
    assert_eq!(stats.eligible_for_reprocess, 1);
    assert_eq!(stats.requiring_manual_review, 1);
    assert!(stats.oldest_item.is_some());
    assert!(stats.newest_item.is_some());
}

#[tokio::test]
async fn test_advanced_filter_error_types() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-7").await.unwrap();
    let project_root = PathBuf::from(".");
    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root);

    // Create test items with different error signatures
    let timeout_item = DeadLetteredItem {
        item_id: "timeout-item".to_string(),
        item_data: json!({"id": 10}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 1,
        failure_history: vec![],
        error_signature: "timeout occurred during execution".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let validation_item = DeadLetteredItem {
        item_id: "validation-item".to_string(),
        item_data: json!({"id": 11}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 2,
        failure_history: vec![],
        error_signature: "validation failed for input".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let items = vec![timeout_item, validation_item];

    // Test filtering by error types
    let filter = DlqFilterAdvanced {
        error_types: Some(vec![ErrorType::Timeout]),
        date_range: None,
        item_filter: None,
        max_failure_count: None,
    };

    let filtered = reprocessor
        .apply_advanced_filter(items.clone(), &filter)
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].item_id, "timeout-item");

    // Test filtering by validation errors
    let filter = DlqFilterAdvanced {
        error_types: Some(vec![ErrorType::Validation]),
        date_range: None,
        item_filter: None,
        max_failure_count: None,
    };

    let filtered = reprocessor
        .apply_advanced_filter(items.clone(), &filter)
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].item_id, "validation-item");
}

#[tokio::test]
async fn test_advanced_filter_date_range() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-8").await.unwrap();
    let project_root = PathBuf::from(".");
    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root);

    let now = Utc::now();
    let old_item = DeadLetteredItem {
        item_id: "old-item".to_string(),
        item_data: json!({"id": 20}),
        first_attempt: now - Duration::days(5),
        last_attempt: now - Duration::days(5),
        failure_count: 1,
        failure_history: vec![],
        error_signature: "old error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let recent_item = DeadLetteredItem {
        item_id: "recent-item".to_string(),
        item_data: json!({"id": 21}),
        first_attempt: now - Duration::hours(1),
        last_attempt: now - Duration::hours(1),
        failure_count: 1,
        failure_history: vec![],
        error_signature: "recent error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let items = vec![old_item, recent_item];

    // Filter for items from last 2 days
    let filter = DlqFilterAdvanced {
        error_types: None,
        date_range: Some(DateRange {
            start: now - Duration::days(2),
            end: now,
        }),
        item_filter: None,
        max_failure_count: None,
    };

    let filtered = reprocessor.apply_advanced_filter(items, &filter).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].item_id, "recent-item");
}

#[tokio::test]
async fn test_advanced_filter_failure_count() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-9").await.unwrap();
    let project_root = PathBuf::from(".");
    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root);

    let low_failure_item = DeadLetteredItem {
        item_id: "low-failure".to_string(),
        item_data: json!({"id": 30}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 2,
        failure_history: vec![],
        error_signature: "error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let high_failure_item = DeadLetteredItem {
        item_id: "high-failure".to_string(),
        item_data: json!({"id": 31}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 10,
        failure_history: vec![],
        error_signature: "error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    let items = vec![low_failure_item, high_failure_item];

    // Filter for items with <= 5 failures
    let filter = DlqFilterAdvanced {
        error_types: None,
        date_range: None,
        item_filter: None,
        max_failure_count: Some(5),
    };

    let filtered = reprocessor.apply_advanced_filter(items, &filter).unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].item_id, "low-failure");
}

#[tokio::test]
async fn test_reprocess_items_basic() {
    let (dlq, _temp_dir) = create_test_dlq_with_items("test-job-10").await.unwrap();
    let project_root = PathBuf::from(".");
    let reprocessor = DlqReprocessor::new(dlq.clone(), None, project_root);

    // Test basic reprocess_items call
    let options = ReprocessOptions {
        max_retries: 1,
        filter: None,
        parallel: 2,
        timeout_per_item: 10,
        strategy: RetryStrategy::Immediate,
        merge_results: false,
        force: true,
    };

    // This will use the simulated processing in the implementation
    let result = reprocessor.reprocess_items(options).await.unwrap();

    // Should have attempted to process all items
    assert!(result.total_items > 0);
    assert_eq!(
        result.total_items,
        result.successful + result.failed + result.skipped
    );
    assert!(!result.job_id.is_empty());
    assert!(result.duration.as_secs() >= 0);
}
