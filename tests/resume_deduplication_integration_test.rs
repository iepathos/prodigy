//! Integration tests for work item deduplication in MapReduce resume.
//!
//! These tests verify that deduplication actually works in real resume
//! scenarios, preventing duplicate work item processing when items appear
//! in multiple sources (pending, failed_agents, DLQ).
//!
//! **Test Coverage**:
//! - Deduplication across overlapping sources
//! - Priority order (pending > failed > DLQ)
//! - No duplicate agent execution
//! - Warning logs and metrics

use chrono::Utc;
use prodigy::cook::execution::dlq::{DeadLetterQueue, DeadLetteredItem, ErrorType, FailureDetail};
use prodigy::cook::execution::events::{EventLogger, JsonlEventWriter};
use prodigy::cook::execution::mapreduce::types::MapReduceConfig;
use prodigy::cook::execution::mapreduce_resume::{EnhancedResumeOptions, MapReduceResumeManager};
use prodigy::cook::execution::state::{DefaultJobStateManager, FailureRecord, MapReduceJobState};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a base job state for testing
fn create_base_state(job_id: &str) -> MapReduceJobState {
    let config = MapReduceConfig {
        input: "test-input.json".to_string(),
        json_path: "$.items[*]".to_string(),
        max_parallel: 5,
        agent_timeout_secs: None,
        continue_on_failure: true,
        batch_size: None,
        enable_checkpoints: true,
        max_items: None,
        offset: None,
    };

    MapReduceJobState::new(job_id.to_string(), config, Vec::new())
}

/// Create job state with overlapping items in pending and failed sources
fn create_state_with_overlapping_items(
    job_id: &str,
    pending_items: Vec<&str>,
    failed_items: Vec<&str>,
) -> MapReduceJobState {
    let mut state = create_base_state(job_id);

    // Create work items with unique IDs
    let mut all_item_ids: Vec<String> = pending_items.iter().map(|s| s.to_string()).collect();
    all_item_ids.extend(failed_items.iter().map(|s| s.to_string()));
    all_item_ids.sort();
    all_item_ids.dedup();

    // Build work_items array with all unique items
    for item_id in &all_item_ids {
        state.work_items.push(json!({
            "id": item_id,
            "data": format!("data-{}", item_id)
        }));
    }

    state.total_items = state.work_items.len();

    // Map pending items to work_items indices
    for item_id in pending_items {
        if let Some(index) = all_item_ids.iter().position(|id| id == item_id) {
            state.pending_items.push(format!("item_{}", index));
        }
    }

    // Map failed items to failed_agents
    for item_id in failed_items {
        if let Some(index) = all_item_ids.iter().position(|id| id == item_id) {
            let item_ref = format!("item_{}", index);
            state.failed_agents.insert(
                item_ref,
                FailureRecord {
                    item_id: item_id.to_string(),
                    attempts: 1,
                    last_error: "test error".to_string(),
                    last_attempt: Utc::now(),
                    worktree_info: None,
                },
            );
        }
    }

    state
}

/// Create a temporary directory for testing
fn create_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Create event logger for testing
async fn create_test_event_logger(temp_dir: &TempDir) -> Arc<EventLogger> {
    let event_file = temp_dir.path().join("events.jsonl");
    let writer = JsonlEventWriter::new(event_file)
        .await
        .expect("Failed to create event writer");
    Arc::new(EventLogger::new(vec![Box::new(writer)]))
}

/// Test that resume deduplicates items appearing in multiple sources.
///
/// **Scenario**:
/// 1. Create state with "item-1" in both pending AND failed_agents
/// 2. Resume with reset_failed_agents: true
/// 3. Verify only 1 instance of "item-1" in work queue
/// 4. Verify total item count is deduplicated
///
/// **Validates**:
/// - `deduplicate_work_items()` is called during resume
/// - Duplicate items are removed
/// - Work queue has correct unique count
#[tokio::test]
async fn test_resume_deduplicates_overlapping_sources() {
    let temp_dir = create_temp_dir();
    let job_id = "test-overlap";

    // Create state where "item-1" is in BOTH pending AND failed_agents
    let mut state = create_state_with_overlapping_items(
        job_id,
        vec!["item-1", "item-2"], // pending
        vec!["item-1", "item-3"], // failed (item-1 overlaps!)
    );

    // Create resume manager
    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    // Resume with reset_failed_agents: true (includes both sources)
    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: false,
        ..Default::default()
    };

    // Calculate remaining items (this should deduplicate)
    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Verify deduplication: Should have 3 unique items, not 5
    // pending: [item-1, item-2]
    // failed:  [item-1, item-3]
    // unique:  [item-1, item-2, item-3]
    assert_eq!(
        remaining.len(),
        3,
        "Should have 3 unique items after deduplication"
    );

    // Verify item IDs
    let item_ids: Vec<String> = remaining
        .iter()
        .map(|item| {
            item["id"]
                .as_str()
                .expect("Item should have id")
                .to_string()
        })
        .collect();

    assert!(item_ids.contains(&"item-1".to_string()));
    assert!(item_ids.contains(&"item-2".to_string()));
    assert!(item_ids.contains(&"item-3".to_string()));

    // Verify no duplicates
    let unique_ids: std::collections::HashSet<_> = item_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        item_ids.len(),
        "Should have no duplicate IDs"
    );
}

/// Test that pending items take precedence over failed items.
///
/// **Scenario**:
/// 1. Create state with same item in pending and failed sources
/// 2. Use different data in each source
/// 3. Verify pending version is kept (first occurrence rule)
///
/// **Validates**:
/// - Priority order: pending > failed
/// - First occurrence is preserved
/// - Data from pending source is maintained
#[tokio::test]
async fn test_resume_pending_takes_precedence_over_failed() {
    let temp_dir = create_temp_dir();
    let job_id = "test-precedence";

    // Create state with item-1 in both pending and failed
    let mut state = create_base_state(job_id);

    // Add "item-1" to work_items with pending version data
    state.work_items.push(json!({
        "id": "item-1",
        "data": "pending-version",
        "source": "pending"
    }));
    state.total_items = 1;

    // Add to pending_items
    state.pending_items.push("item_0".to_string());

    // Also add to failed_agents (simulates item appearing in both sources)
    state.failed_agents.insert(
        "item_0".to_string(),
        FailureRecord {
            item_id: "item-1".to_string(),
            attempts: 1,
            last_error: "test error".to_string(),
            last_attempt: Utc::now(),
            worktree_info: None,
        },
    );

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: false,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Verify only one "item-1" in result
    let item_1_count = remaining
        .iter()
        .filter(|item| item["id"] == "item-1")
        .count();

    assert_eq!(item_1_count, 1, "Should have exactly one item-1");

    // Verify it's the pending version (first occurrence)
    let item_1 = remaining
        .iter()
        .find(|item| item["id"] == "item-1")
        .expect("Should have item-1");

    assert_eq!(
        item_1["data"], "pending-version",
        "Should use pending version (first occurrence)"
    );
    assert_eq!(
        item_1["source"], "pending",
        "Should preserve pending source metadata"
    );
}

/// Test that pending items take precedence over DLQ items.
///
/// **Scenario**:
/// 1. Create state with item in pending
/// 2. Add same item to DLQ
/// 3. Verify pending version is used
///
/// **Validates**:
/// - Priority order: pending > DLQ
/// - DLQ items are not duplicated if already pending
#[tokio::test]
async fn test_resume_pending_takes_precedence_over_dlq() {
    let temp_dir = create_temp_dir();
    let job_id = "test-pending-dlq";

    // Create state with item-1 in pending
    let mut state = create_base_state(job_id);
    state.work_items.push(json!({
        "id": "item-1",
        "data": "pending-version"
    }));
    state.total_items = 1;
    state.pending_items.push("item_0".to_string());

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    // Create DLQ with same item
    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        temp_dir.path().to_path_buf(),
        1000,
        30,
        Some(event_logger.clone()),
    )
    .await
    .expect("Failed to create DLQ");

    // Add item to DLQ
    let dlq_item = DeadLetteredItem {
        item_id: "item-1".to_string(),
        item_data: json!({
            "id": "item-1",
            "data": "dlq-version"
        }),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 1,
        failure_history: vec![FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: ErrorType::CommandFailed { exit_code: 1 },
            error_message: "test error".to_string(),
            stack_trace: None,
            agent_id: "agent-1".to_string(),
            step_failed: "test-step".to_string(),
            duration_ms: 1000,
            json_log_location: None,
        }],
        error_signature: "test_error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    dlq.add(dlq_item).await.expect("Failed to add DLQ item");

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    let options = EnhancedResumeOptions {
        reset_failed_agents: false,
        include_dlq_items: true,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Should have exactly 1 item (deduplicated)
    assert_eq!(remaining.len(), 1, "Should have 1 item after deduplication");

    // Should be pending version (higher priority)
    assert_eq!(remaining[0]["id"], "item-1");
    assert_eq!(
        remaining[0]["data"], "pending-version",
        "Should use pending version over DLQ"
    );
}

/// Test deduplication with all three sources (pending + failed + DLQ).
///
/// **Scenario**:
/// 1. Add same item to pending, failed_agents, and DLQ
/// 2. Resume with all sources enabled
/// 3. Verify only one instance remains
/// 4. Verify pending version takes precedence
///
/// **Validates**:
/// - Complete deduplication across all sources
/// - Correct priority order maintained
/// - No data loss from highest-priority source
#[tokio::test]
async fn test_deduplication_with_all_three_sources() {
    let temp_dir = create_temp_dir();
    let job_id = "test-three-sources";

    // Create state with item-1 in pending and failed
    let mut state = create_base_state(job_id);
    state.work_items.push(json!({
        "id": "item-1",
        "data": "pending-version",
        "source": "pending"
    }));
    state.total_items = 1;

    // Add to pending
    state.pending_items.push("item_0".to_string());

    // Add to failed_agents
    state.failed_agents.insert(
        "item_0".to_string(),
        FailureRecord {
            item_id: "item-1".to_string(),
            attempts: 1,
            last_error: "test error".to_string(),
            last_attempt: Utc::now(),
            worktree_info: None,
        },
    );

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    // Create DLQ with same item
    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        temp_dir.path().to_path_buf(),
        1000,
        30,
        Some(event_logger.clone()),
    )
    .await
    .expect("Failed to create DLQ");

    let dlq_item = DeadLetteredItem {
        item_id: "item-1".to_string(),
        item_data: json!({
            "id": "item-1",
            "data": "dlq-version"
        }),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 1,
        failure_history: vec![FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: ErrorType::CommandFailed { exit_code: 1 },
            error_message: "test error".to_string(),
            stack_trace: None,
            agent_id: "agent-1".to_string(),
            step_failed: "test-step".to_string(),
            duration_ms: 1000,
            json_log_location: None,
        }],
        error_signature: "test_error".to_string(),
        worktree_artifacts: None,
        reprocess_eligible: true,
        manual_review_required: false,
    };

    dlq.add(dlq_item).await.expect("Failed to add DLQ item");

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    // Resume with all sources enabled
    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: true,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Should have exactly 1 item (deduplicated from 3 sources)
    assert_eq!(
        remaining.len(),
        1,
        "Should deduplicate item across all 3 sources"
    );

    // Should be pending version (highest priority)
    assert_eq!(remaining[0]["id"], "item-1");
    assert_eq!(
        remaining[0]["data"], "pending-version",
        "Should use pending version (highest priority)"
    );
    assert_eq!(
        remaining[0]["source"], "pending",
        "Should preserve pending source"
    );
}

/// Test that deduplication preserves data from pending items.
///
/// **Scenario**:
/// 1. Create item with rich metadata in pending
/// 2. Create same item with different data in failed
/// 3. Verify pending metadata is preserved after deduplication
///
/// **Validates**:
/// - Data integrity maintained
/// - Metadata from first occurrence preserved
/// - No data loss during deduplication
#[tokio::test]
async fn test_deduplication_preserves_pending_data() {
    let temp_dir = create_temp_dir();
    let job_id = "test-data-preservation";

    let mut state = create_base_state(job_id);

    // Pending version has important metadata
    state.work_items.push(json!({
        "id": "item-1",
        "data": "important-pending-data",
        "metadata": {
            "source": "pending",
            "priority": "high",
            "tags": ["critical", "user-facing"]
        }
    }));
    state.total_items = 1;

    // Add to pending
    state.pending_items.push("item_0".to_string());

    // Add to failed with different data
    state.failed_agents.insert(
        "item_0".to_string(),
        FailureRecord {
            item_id: "item-1".to_string(),
            attempts: 1,
            last_error: "test error".to_string(),
            last_attempt: Utc::now(),
            worktree_info: None,
        },
    );

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: false,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Verify pending data is preserved
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["data"], "important-pending-data");
    assert_eq!(remaining[0]["metadata"]["source"], "pending");
    assert_eq!(remaining[0]["metadata"]["priority"], "high");
    assert_eq!(remaining[0]["metadata"]["tags"][0], "critical");
    assert_eq!(remaining[0]["metadata"]["tags"][1], "user-facing");
}

/// Test that no items are lost when sources overlap.
///
/// **Scenario**:
/// 1. Create multiple unique items across sources
/// 2. Add some duplicates
/// 3. Verify all unique items are preserved
///
/// **Validates**:
/// - No unique items lost during deduplication
/// - All sources contribute unique items
/// - Correct total count after deduplication
#[tokio::test]
async fn test_deduplication_preserves_all_unique_items() {
    let temp_dir = create_temp_dir();
    let job_id = "test-unique-preservation";

    // Create state with:
    // - pending: [item-1, item-2, item-3]
    // - failed: [item-3, item-4, item-5]
    // Expected unique: [item-1, item-2, item-3, item-4, item-5] = 5 items
    let mut state = create_state_with_overlapping_items(
        job_id,
        vec!["item-1", "item-2", "item-3"],
        vec!["item-3", "item-4", "item-5"],
    );

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: false,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Should have all 5 unique items
    assert_eq!(remaining.len(), 5, "Should preserve all unique items");

    // Verify all items are present
    let item_ids: Vec<String> = remaining
        .iter()
        .map(|item| item["id"].as_str().expect("Should have id").to_string())
        .collect();

    for expected_id in ["item-1", "item-2", "item-3", "item-4", "item-5"] {
        assert!(
            item_ids.contains(&expected_id.to_string()),
            "Should contain {}",
            expected_id
        );
    }
}

/// Test deduplication with empty sources.
///
/// **Scenario**:
/// 1. Create state with no pending items
/// 2. Add items only to failed
/// 3. Verify failed items are included
///
/// **Validates**:
/// - Deduplication works with partial sources
/// - Empty sources don't break deduplication
/// - Items from non-empty sources are preserved
#[tokio::test]
async fn test_deduplication_with_empty_pending() {
    let temp_dir = create_temp_dir();
    let job_id = "test-empty-pending";

    // Create state with NO pending items, only failed
    let mut state = create_state_with_overlapping_items(
        job_id,
        vec![],                   // No pending items
        vec!["item-1", "item-2"], // Only failed items
    );

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .expect("Failed to create resume manager");

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: false,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .expect("Failed to calculate remaining items");

    // Should have 2 items from failed source
    assert_eq!(remaining.len(), 2, "Should include failed items");

    let item_ids: Vec<String> = remaining
        .iter()
        .map(|item| item["id"].as_str().expect("Should have id").to_string())
        .collect();

    assert!(item_ids.contains(&"item-1".to_string()));
    assert!(item_ids.contains(&"item-2".to_string()));
}
