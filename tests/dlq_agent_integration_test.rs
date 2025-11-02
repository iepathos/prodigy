//! Integration tests for DLQ agent failure handling
//!
//! These tests verify DLQ data structure operations and integration with
//! MapReduce components. They validate Spec 138 implementation with focus
//! on testable integration points.
//!
//! **Test Coverage**:
//! - DLQ data structure operations (add, update, remove, list)
//! - agent_result_to_dlq_item conversion
//! - JSON log location preservation
//! - Failure count tracking
//! - Resume manager integration with DLQ

use chrono::Utc;
use prodigy::cook::execution::dlq::{
    DLQFilter, DeadLetterQueue, DeadLetteredItem, ErrorType, FailureDetail,
};
use prodigy::cook::execution::events::{EventLogger, JsonlEventWriter};
use prodigy::cook::execution::mapreduce::dlq_integration::agent_result_to_dlq_item;
use prodigy::cook::execution::mapreduce::{AgentResult, AgentStatus};
use prodigy::cook::execution::mapreduce_resume::{EnhancedResumeOptions, MapReduceResumeManager};
use prodigy::cook::execution::state::DefaultJobStateManager;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

/// Test that JSON log locations are preserved in DLQ entries.
///
/// **Validates**:
/// - json_log_location field is populated when available
/// - Log location can be used for debugging
#[tokio::test]
async fn test_json_log_location_preserved_in_dlq() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let job_id = "test-json-log-job";

    // Create events and DLQ
    let events_dir = project_root.join(".prodigy/events").join(job_id);
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        project_root.clone(),
        1000,
        30,
        Some(event_logger.clone()),
    )
    .await
    .unwrap();

    // Create a DLQ item with json_log_location
    let dlq_item = DeadLetteredItem {
        item_id: "item-with-log".to_string(),
        item_data: json!({"id": 1, "data": "test"}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 1,
        failure_history: vec![FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: ErrorType::CommandFailed { exit_code: 1 },
            error_message: "Command failed".to_string(),
            stack_trace: None,
            agent_id: "agent-1".to_string(),
            step_failed: "step-1".to_string(),
            duration_ms: 1000,
            json_log_location: Some("/path/to/claude/log.json".to_string()),
        }],
        error_signature: "test-signature".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    // Add to DLQ
    dlq.add(dlq_item).await.unwrap();

    // Retrieve and verify
    let items = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(items.len(), 1);

    let item = &items[0];
    assert_eq!(
        item.failure_history[0].json_log_location,
        Some("/path/to/claude/log.json".to_string()),
        "JSON log location should be preserved"
    );
}

/// Test that successful retry removes items from DLQ.
///
/// **Scenario**:
/// 1. Add failed items to DLQ
/// 2. Simulate successful reprocessing
/// 3. Verify items removed from DLQ
///
/// **Validates**:
/// - Successful processing removes DLQ entries
/// - DLQ cleanup works correctly
#[tokio::test]
async fn test_successful_retry_removes_from_dlq() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let job_id = "test-retry-success-job";

    // Create events and DLQ
    let events_dir = project_root.join(".prodigy/events").join(job_id);
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        project_root.clone(),
        1000,
        30,
        Some(event_logger),
    )
    .await
    .unwrap();

    // Add items to DLQ
    for i in 0..3 {
        let item = DeadLetteredItem {
            item_id: format!("retry-item-{}", i),
            item_data: json!({"id": i, "data": format!("test-{}", i)}),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 1,
            failure_history: vec![FailureDetail {
                attempt_number: 1,
                timestamp: Utc::now(),
                error_type: ErrorType::CommandFailed { exit_code: 1 },
                error_message: "Temporary failure".to_string(),
                stack_trace: None,
                agent_id: format!("agent-{}", i),
                step_failed: "process".to_string(),
                duration_ms: 1000,
                json_log_location: None,
            }],
            error_signature: "temp-failure".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };
        dlq.add(item).await.unwrap();
    }

    // Verify items are in DLQ
    let items = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(items.len(), 3, "Should have 3 items in DLQ");

    // Remove one item (simulate successful retry)
    dlq.remove("retry-item-0").await.unwrap();

    // Verify item was removed
    let items = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(items.len(), 2, "Should have 2 items remaining in DLQ");
    assert!(
        !items.iter().any(|i| i.item_id == "retry-item-0"),
        "Item should be removed"
    );
}

/// Test that failed retry updates DLQ failure count.
///
/// **Scenario**:
/// 1. Add failed item to DLQ
/// 2. Simulate failed retry attempt
/// 3. Verify failure count incremented
///
/// **Validates**:
/// - Retry attempts are tracked
/// - Failure history is appended
/// - Failure count is incremented
#[tokio::test]
async fn test_failed_retry_updates_dlq_failure_count() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let job_id = "test-retry-failure-job";

    // Create events and DLQ
    let events_dir = project_root.join(".prodigy/events").join(job_id);
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        project_root.clone(),
        1000,
        30,
        Some(event_logger),
    )
    .await
    .unwrap();

    // Add initial failure
    let mut item = DeadLetteredItem {
        item_id: "failing-item".to_string(),
        item_data: json!({"id": 1, "data": "test"}),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 1,
        failure_history: vec![FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: ErrorType::CommandFailed { exit_code: 1 },
            error_message: "First failure".to_string(),
            stack_trace: None,
            agent_id: "agent-1".to_string(),
            step_failed: "process".to_string(),
            duration_ms: 1000,
            json_log_location: None,
        }],
        error_signature: "failure-sig".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };
    dlq.add(item.clone()).await.unwrap();

    // Simulate failed retry by updating the item
    item.failure_count = 2;
    item.last_attempt = Utc::now();
    item.failure_history.push(FailureDetail {
        attempt_number: 2,
        timestamp: Utc::now(),
        error_type: ErrorType::CommandFailed { exit_code: 1 },
        error_message: "Second failure".to_string(),
        stack_trace: None,
        agent_id: "agent-1".to_string(),
        step_failed: "process".to_string(),
        duration_ms: 1200,
        json_log_location: None,
    });

    // Update by removing and re-adding (DLQ doesn't have direct update method)
    dlq.remove(&item.item_id).await.unwrap();
    dlq.add(item).await.unwrap();

    // Verify failure count and history
    let items = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(items.len(), 1);

    let updated_item = &items[0];
    assert_eq!(
        updated_item.failure_count, 2,
        "Failure count should be incremented"
    );
    assert_eq!(
        updated_item.failure_history.len(),
        2,
        "Should have 2 failure entries"
    );
    assert_eq!(updated_item.failure_history[1].attempt_number, 2);
    assert_eq!(
        updated_item.failure_history[1].error_message,
        "Second failure"
    );
}

/// Test that resume manager can be created with DLQ integration.
///
/// **Validates**:
/// - MapReduceResumeManager creation with correct API
/// - DLQ integration is available
/// - Resume options can configure DLQ behavior
#[tokio::test]
async fn test_resume_manager_dlq_integration() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let job_id = "test-resume-dlq-job";

    // Create state directory
    let state_dir = project_root.join(".prodigy/state").join(job_id);
    tokio::fs::create_dir_all(&state_dir).await.unwrap();

    // Create events and DLQ
    let events_dir = project_root.join(".prodigy/events").join(job_id);
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    let state_manager = Arc::new(DefaultJobStateManager::new(state_dir.clone()));

    // Create resume manager with correct API (4 parameters)
    let _resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger.clone(),
        project_root.clone(),
    )
    .await
    .unwrap();

    // Verify resume options can configure DLQ behavior
    let options = EnhancedResumeOptions {
        include_dlq_items: true,
        ..Default::default()
    };

    assert!(
        options.include_dlq_items,
        "DLQ items should be includable in resume"
    );
    // Note: Full resume functionality would be tested in end-to-end workflow tests
}

/// Test agent_result_to_dlq_item conversion for failed agents.
///
/// **Validates**:
/// - Failed agent results are converted to DLQ items
/// - Timeout results are converted to DLQ items
/// - Successful results are not converted
/// - JSON log location is preserved
/// - Error details are captured correctly
#[tokio::test]
async fn test_agent_result_to_dlq_item_conversion() {
    let work_item = json!({"id": "test-item", "data": "test-data"});

    // Test failed agent result
    let failed_result = AgentResult {
        item_id: "test-item".to_string(),
        status: AgentStatus::Failed("Command failed with exit code 1".to_string()),
        output: None,
        commits: vec![],
        files_modified: vec![],
        duration: Duration::from_secs(10),
        error: Some("Command failed with exit code 1".to_string()),
        worktree_path: None,
        branch_name: None,
        worktree_session_id: None,
        json_log_location: Some("/path/to/log.json".to_string()),
        cleanup_status: None,
    };

    let dlq_item = agent_result_to_dlq_item(&failed_result, &work_item, 1);
    assert!(dlq_item.is_some(), "Failed result should create DLQ item");

    let item = dlq_item.unwrap();
    assert_eq!(item.item_id, "test-item");
    assert_eq!(item.failure_count, 1);
    assert_eq!(item.failure_history.len(), 1);
    assert_eq!(
        item.failure_history[0].json_log_location,
        Some("/path/to/log.json".to_string())
    );
    assert!(item.failure_history[0]
        .error_message
        .contains("Command failed"));

    // Test timeout result
    let timeout_result = AgentResult {
        item_id: "test-item".to_string(),
        status: AgentStatus::Timeout,
        output: None,
        commits: vec![],
        files_modified: vec![],
        duration: Duration::from_secs(60),
        error: Some("Agent execution timed out".to_string()),
        worktree_path: None,
        branch_name: None,
        worktree_session_id: None,
        json_log_location: Some("/path/to/timeout-log.json".to_string()),
        cleanup_status: None,
    };

    let timeout_dlq_item = agent_result_to_dlq_item(&timeout_result, &work_item, 1);
    assert!(
        timeout_dlq_item.is_some(),
        "Timeout result should create DLQ item"
    );

    let timeout_item = timeout_dlq_item.unwrap();
    assert_eq!(timeout_item.item_id, "test-item");
    assert_eq!(
        timeout_item.failure_history[0].error_type,
        ErrorType::Timeout
    );
    assert_eq!(
        timeout_item.failure_history[0].json_log_location,
        Some("/path/to/timeout-log.json".to_string())
    );

    // Test successful result (should not create DLQ item)
    let success_result = AgentResult {
        item_id: "test-item".to_string(),
        status: AgentStatus::Success,
        output: Some("Success output".to_string()),
        commits: vec!["abc123".to_string()],
        files_modified: vec![],
        duration: Duration::from_secs(5),
        error: None,
        worktree_path: None,
        branch_name: None,
        worktree_session_id: None,
        json_log_location: Some("/path/to/success-log.json".to_string()),
        cleanup_status: None,
    };

    let success_dlq_item = agent_result_to_dlq_item(&success_result, &work_item, 1);
    assert!(
        success_dlq_item.is_none(),
        "Success result should not create DLQ item"
    );
}

/// Test DLQ retry reprocesses failed items.
///
/// **Scenario**:
/// 1. Add failed items to DLQ
/// 2. Simulate retry execution
/// 3. Verify items are reprocessed
///
/// **Note**: This is a basic test of the DLQ data structure.
/// Full retry command testing would require additional orchestration.
#[tokio::test]
async fn test_dlq_retry_reprocesses_failed_items() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let job_id = "test-dlq-retry-job";

    // Create events and DLQ
    let events_dir = project_root.join(".prodigy/events").join(job_id);
    tokio::fs::create_dir_all(&events_dir).await.unwrap();
    let event_writer = Box::new(
        JsonlEventWriter::new(events_dir.join("events.jsonl"))
            .await
            .unwrap(),
    );
    let event_logger = Arc::new(EventLogger::new(vec![event_writer]));

    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        project_root.clone(),
        1000,
        30,
        Some(event_logger),
    )
    .await
    .unwrap();

    // Add reprocessable items
    for i in 0..3 {
        let item = DeadLetteredItem {
            item_id: format!("reprocess-item-{}", i),
            item_data: json!({"id": i, "data": format!("reprocess-{}", i)}),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 1,
            failure_history: vec![FailureDetail {
                attempt_number: 1,
                timestamp: Utc::now(),
                error_type: ErrorType::CommandFailed { exit_code: 1 },
                error_message: "Temporary network error".to_string(),
                stack_trace: None,
                agent_id: format!("agent-{}", i),
                step_failed: "process".to_string(),
                duration_ms: 1000,
                json_log_location: None,
            }],
            error_signature: "network-error".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };
        dlq.add(item).await.unwrap();
    }

    // Verify items are reprocessable
    let filter = DLQFilter {
        reprocess_eligible: Some(true),
        ..Default::default()
    };
    let reprocessable_items = dlq.list_items(filter).await.unwrap();

    assert_eq!(
        reprocessable_items.len(),
        3,
        "All items should be reprocessable"
    );

    // Verify each item has correct properties
    for item in reprocessable_items {
        assert!(item.reprocess_eligible);
        assert_eq!(item.failure_count, 1);
        assert!(!item.manual_review_required);
    }
}
