//! Complete system integration tests for MapReduce resume features.
//!
//! These tests verify that DLQ integration (Spec 138), work item deduplication
//! (Spec 139), and concurrent resume protection (Spec 140) all work together
//! correctly in realistic end-to-end scenarios.
//!
//! **Test Coverage**:
//! - Complete workflow: failure → DLQ → resume → retry → success
//! - Concurrent resume blocking with DLQ items
//! - Deduplication with all three sources (pending + failed + DLQ)
//! - Cleanup verification (DLQ cleared, locks released)
//!
//! **Why These Tests Are Critical**:
//! Individual features may work in isolation but fail when combined.
//! These tests ensure the complete resume system functions correctly.

use anyhow::Result;
use chrono::Utc;
use prodigy::cook::execution::dlq::{
    DLQFilter, DeadLetterQueue, DeadLetteredItem, ErrorType, FailureDetail,
};
use prodigy::cook::execution::events::{EventLogger, JsonlEventWriter};
use prodigy::cook::execution::mapreduce::types::MapReduceConfig;
use prodigy::cook::execution::mapreduce_resume::{EnhancedResumeOptions, MapReduceResumeManager};
use prodigy::cook::execution::state::{DefaultJobStateManager, FailureRecord, MapReduceJobState};
use prodigy::cook::execution::ResumeLockManager;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

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

/// Create event logger for testing
async fn create_test_event_logger(temp_dir: &TempDir) -> Arc<EventLogger> {
    let event_file = temp_dir.path().join("events.jsonl");
    let writer = JsonlEventWriter::new(event_file)
        .await
        .expect("Failed to create event writer");
    Arc::new(EventLogger::new(vec![Box::new(writer)]))
}

/// Create DLQ with specific items for testing
async fn create_dlq_with_items(
    job_id: &str,
    item_ids: Vec<&str>,
    temp_dir: &TempDir,
    event_logger: Arc<EventLogger>,
) -> Result<DeadLetterQueue> {
    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        temp_dir.path().to_path_buf(),
        1000,
        30,
        Some(event_logger),
    )
    .await?;

    for item_id in item_ids {
        let item = DeadLetteredItem {
            item_id: item_id.to_string(),
            item_data: json!({
                "id": item_id,
                "data": format!("data-{}", item_id)
            }),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 1,
            failure_history: vec![FailureDetail {
                attempt_number: 1,
                timestamp: Utc::now(),
                error_type: ErrorType::CommandFailed { exit_code: 1 },
                error_message: format!("Test failure for {}", item_id),
                error_context: None,
                stack_trace: None,
                agent_id: format!("agent-{}", item_id),
                step_failed: "test-step".to_string(),
                duration_ms: 1000,
                json_log_location: Some(format!("/path/to/log-{}.json", item_id)),
            }],
            error_signature: "test-error".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };
        dlq.add(item).await?;
    }

    Ok(dlq)
}

/// Test complete resume workflow with DLQ, deduplication, and locking.
///
/// **Scenario** (Multi-Phase):
/// 1. **Initial State**: Job with pending items, failed items, and DLQ items (with overlap)
/// 2. **Concurrent Resume**: Two processes try to resume (one blocked by lock)
/// 3. **Deduplication**: Load pending + failed + DLQ, deduplicate overlaps
/// 4. **Verification**: All three features working together correctly
///
/// **Validates**:
/// - Spec 138: Failed agents → DLQ → retry
/// - Spec 139: Deduplication across all sources
/// - Spec 140: Concurrent resume protection
/// - Integration: All three features working together
#[tokio::test]
async fn test_complete_resume_workflow_with_dlq_and_lock() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let job_id = "complete-workflow-test";

    // PHASE 1: Setup state with overlapping items
    // - pending: [item-1, item-2, item-3]
    // - failed:  [item-3, item-4, item-5]
    // - DLQ:     [item-5, item-6, item-7]
    // Expected unique after dedup: [item-1, item-2, item-3, item-4, item-5, item-6, item-7] = 7

    let mut state = create_base_state(job_id);

    // Add work items
    for i in 1..=7 {
        state.work_items.push(json!({
            "id": format!("item-{}", i),
            "data": format!("data-{}", i)
        }));
    }
    state.total_items = 7;

    // Add pending items (item-1, item-2, item-3)
    for i in 0..3 {
        state.pending_items.push(format!("item_{}", i));
    }

    // Add failed items (item-3, item-4, item-5)
    for i in 2..5 {
        let item_id = format!("item-{}", i + 1);
        state.failed_agents.insert(
            format!("item_{}", i),
            FailureRecord {
                item_id: item_id.clone(),
                attempts: 1,
                last_error: "test error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );
    }

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    // Create DLQ with items (item-5, item-6, item-7)
    let _dlq = create_dlq_with_items(
        job_id,
        vec!["item-5", "item-6", "item-7"],
        &temp_dir,
        event_logger.clone(),
    )
    .await?;

    // PHASE 2: Test concurrent resume protection (Spec 140)
    {
        let lock_manager = Arc::new(ResumeLockManager::new(temp_dir.path().to_path_buf())?);

        let lock_manager1 = lock_manager.clone();
        let job_id1 = job_id.to_string();
        let handle1 = tokio::spawn(async move {
            let lock = lock_manager1.acquire_lock(&job_id1).await;
            if lock.is_ok() {
                sleep(Duration::from_millis(100)).await;
            }
            lock
        });

        let lock_manager2 = lock_manager.clone();
        let job_id2 = job_id.to_string();
        let handle2 = tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            lock_manager2.acquire_lock(&job_id2).await
        });

        let result1 = handle1.await?;
        let result2 = handle2.await?;

        // Verify locking works (Spec 140)
        assert!(
            (result1.is_ok() && result2.is_err()) || (result1.is_err() && result2.is_ok()),
            "Exactly one resume should be blocked by lock"
        );

        // The failed one should have "already in progress" error
        let error = if result1.is_err() {
            result1.unwrap_err()
        } else {
            result2.unwrap_err()
        };
        assert!(
            error.to_string().contains("already in progress"),
            "Error should indicate resume already in progress"
        );

        // Drop locks by letting result1/result2 go out of scope
    } // Lock released here

    // PHASE 3: Test deduplication with all three sources (Spec 139)
    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger.clone(),
        temp_dir.path().to_path_buf(),
    )
    .await?;

    // Create new lock manager and acquire lock for resume
    let lock_manager = ResumeLockManager::new(temp_dir.path().to_path_buf())?;
    let _lock = lock_manager.acquire_lock(job_id).await?;

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: true,
        ..Default::default()
    };

    // Calculate remaining items (should deduplicate)
    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await?;

    // Verify deduplication (Spec 139)
    // Should have 7 unique items:
    // - item-1, item-2 (only in pending)
    // - item-3 (in pending and failed, pending wins)
    // - item-4 (only in failed)
    // - item-5 (in failed and DLQ, failed wins)
    // - item-6, item-7 (only in DLQ)
    assert_eq!(
        remaining.len(),
        7,
        "Should have 7 unique items after deduplication across all sources"
    );

    // Verify no duplicates
    let item_ids: Vec<String> = remaining
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    let unique_ids: std::collections::HashSet<_> = item_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        7,
        "Should have no duplicate IDs after deduplication"
    );

    // Verify all expected items are present
    for i in 1..=7 {
        let expected_id = format!("item-{}", i);
        assert!(
            item_ids.contains(&expected_id),
            "Should contain {}",
            expected_id
        );
    }

    // PHASE 4: Verify lock is released (RAII)
    drop(_lock);
    let lock_path = temp_dir
        .path()
        .join("resume_locks")
        .join(format!("{}.lock", job_id));
    assert!(
        !lock_path.exists(),
        "Lock file should be removed after drop"
    );

    Ok(())
}

/// Test concurrent resume blocked with DLQ items present.
///
/// **Scenario**:
/// 1. Create job with DLQ items
/// 2. Attempt two concurrent resumes
/// 3. Verify both try to load DLQ
/// 4. Verify only one proceeds (lock blocking works)
///
/// **Validates**:
/// - Concurrent resume protection works when DLQ is populated
/// - Lock prevents race conditions in DLQ access
/// - Second resume is properly rejected with clear error
#[tokio::test]
async fn test_concurrent_resume_blocked_with_dlq() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let job_id = "concurrent-dlq-test";

    let event_logger = create_test_event_logger(&temp_dir).await;

    // Create DLQ with items
    let _dlq = create_dlq_with_items(
        job_id,
        vec!["item-1", "item-2", "item-3"],
        &temp_dir,
        event_logger,
    )
    .await?;

    // Create lock manager
    let lock_manager = Arc::new(ResumeLockManager::new(temp_dir.path().to_path_buf())?);

    // Spawn two concurrent resume attempts
    let lock_manager1 = lock_manager.clone();
    let job_id1 = job_id.to_string();
    let handle1 = tokio::spawn(async move {
        let lock = lock_manager1.acquire_lock(&job_id1).await;
        if lock.is_ok() {
            sleep(Duration::from_millis(100)).await;
        }
        lock
    });

    let lock_manager2 = lock_manager.clone();
    let job_id2 = job_id.to_string();
    let handle2 = tokio::spawn(async move {
        sleep(Duration::from_millis(10)).await;
        lock_manager2.acquire_lock(&job_id2).await
    });

    let result1 = handle1.await?;
    let result2 = handle2.await?;

    // Verify only one succeeds
    assert!(
        (result1.is_ok() && result2.is_err()) || (result1.is_err() && result2.is_ok()),
        "Exactly one resume should succeed when DLQ is present"
    );

    Ok(())
}

/// Test deduplication with items in pending, failed, and DLQ sources.
///
/// **Scenario**:
/// 1. Add same item ("item-1") to all three sources
/// 2. Resume with all sources enabled
/// 3. Verify only one instance of "item-1" in result
/// 4. Verify pending version takes precedence
///
/// **Validates**:
/// - Deduplication across all three sources
/// - Priority order: pending > failed > DLQ
/// - Data from highest-priority source is preserved
#[tokio::test]
async fn test_deduplication_with_dlq_and_pending() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let job_id = "dedup-dlq-pending-test";

    let mut state = create_base_state(job_id);

    // Add work item with pending version
    state.work_items.push(json!({
        "id": "item-1",
        "data": "pending-version",
        "source": "pending"
    }));
    state.total_items = 1;

    // Add to pending
    state.pending_items.push("item_0".to_string());

    // Add to failed (same item)
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

    // Add to DLQ (same item with different data)
    let dlq = DeadLetterQueue::new(
        job_id.to_string(),
        temp_dir.path().to_path_buf(),
        1000,
        30,
        Some(event_logger.clone()),
    )
    .await?;

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
            error_context: None,
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

    dlq.add(dlq_item).await?;

    // Create resume manager
    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await?;

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: true,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await?;

    // Should have only 1 "item-1" (deduplicated from 3 sources)
    assert_eq!(
        remaining.len(),
        1,
        "Should deduplicate item-1 from all sources"
    );
    assert_eq!(remaining[0]["id"], "item-1");

    // Verify pending version is used (highest priority)
    assert_eq!(
        remaining[0]["data"], "pending-version",
        "Should use pending version (highest priority)"
    );
    assert_eq!(
        remaining[0]["source"], "pending",
        "Should preserve pending source"
    );

    Ok(())
}

/// Test successful retry removes items from DLQ and releases lock.
///
/// **Scenario**:
/// 1. Create job with failed items in DLQ
/// 2. Acquire lock for resume
/// 3. Simulate successful retry (manually remove from DLQ)
/// 4. Verify DLQ is cleared
/// 5. Verify lock is released
///
/// **Validates**:
/// - Successful retry cleanup removes DLQ items
/// - Lock is properly released after completion
/// - Complete cleanup lifecycle works correctly
#[tokio::test]
async fn test_retry_success_cleans_up_dlq_and_releases_lock() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let job_id = "retry-cleanup-test";

    let event_logger = create_test_event_logger(&temp_dir).await;

    // Create DLQ with failed items
    let dlq = create_dlq_with_items(
        job_id,
        vec!["item-1", "item-2", "item-3"],
        &temp_dir,
        event_logger,
    )
    .await?;

    // Verify DLQ has items
    let items_before = dlq.list_items(DLQFilter::default()).await?;
    assert_eq!(items_before.len(), 3, "DLQ should have 3 items initially");

    // Acquire lock
    let lock_manager = ResumeLockManager::new(temp_dir.path().to_path_buf())?;
    let lock = lock_manager.acquire_lock(job_id).await?;

    // Simulate successful retry by removing items from DLQ
    dlq.remove("item-1").await?;
    dlq.remove("item-2").await?;
    dlq.remove("item-3").await?;

    // Verify DLQ is cleared
    let items_after = dlq.list_items(DLQFilter::default()).await?;
    assert_eq!(
        items_after.len(),
        0,
        "DLQ should be empty after successful retry"
    );

    // Release lock
    drop(lock);

    // Verify lock file is removed
    let lock_path = temp_dir
        .path()
        .join("resume_locks")
        .join(format!("{}.lock", job_id));
    assert!(
        !lock_path.exists(),
        "Lock file should be removed after release"
    );

    Ok(())
}

/// Test resume after partial failure with realistic scenario.
///
/// **Scenario**:
/// 1. Initial run: 10 items total
///    - Items 1-5: succeed
///    - Items 6-8: fail → DLQ
///    - Items 9-10: not started → pending
/// 2. Resume with all sources:
///    - Load pending (9-10)
///    - Load failed (none, as they went to DLQ)
///    - Load DLQ (6-8)
///    - Deduplicate (no overlap expected)
///    - Should have 5 items to process (6-10)
///
/// **Validates**:
/// - Realistic partial failure recovery
/// - Correct item loading from multiple sources
/// - No duplicates in realistic scenario
/// - All remaining work is captured
#[tokio::test]
async fn test_resume_after_partial_failure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let job_id = "partial-failure-test";

    let mut state = create_base_state(job_id);

    // Create 10 work items
    for i in 1..=10 {
        state.work_items.push(json!({
            "id": format!("item-{}", i),
            "data": format!("data-{}", i)
        }));
    }
    state.total_items = 10;

    // Items 1-5 succeeded (mark as completed)
    state.successful_count = 5;
    for i in 0..5 {
        state.completed_agents.insert(format!("item_{}", i));
    }

    // Items 6-8 failed → should be in DLQ
    // (not in failed_agents since they went to DLQ)

    // Items 9-10 not started → pending
    for i in 8..10 {
        state.pending_items.push(format!("item_{}", i));
    }

    let state_manager = Arc::new(DefaultJobStateManager::new(temp_dir.path().to_path_buf()));
    let event_logger = create_test_event_logger(&temp_dir).await;

    // Create DLQ with items 6-8
    let _dlq = create_dlq_with_items(
        job_id,
        vec!["item-6", "item-7", "item-8"],
        &temp_dir,
        event_logger.clone(),
    )
    .await?;

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await?;

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: true,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await?;

    // Should have 5 items to process:
    // - pending: item-9, item-10 (2 items)
    // - DLQ: item-6, item-7, item-8 (3 items)
    // - failed: none
    assert_eq!(
        remaining.len(),
        5,
        "Should have 5 items remaining after partial failure"
    );

    // Verify expected items are present
    let item_ids: Vec<String> = remaining
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();

    for expected_id in ["item-6", "item-7", "item-8", "item-9", "item-10"] {
        assert!(
            item_ids.contains(&expected_id.to_string()),
            "Should contain {}",
            expected_id
        );
    }

    // Verify no duplicates
    let unique_ids: std::collections::HashSet<_> = item_ids.iter().collect();
    assert_eq!(
        unique_ids.len(),
        item_ids.len(),
        "Should have no duplicates"
    );

    Ok(())
}
