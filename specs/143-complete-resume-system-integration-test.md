---
number: 143
title: Complete Resume System Integration Test (DLQ + Deduplication + Locking)
category: testing
priority: critical
status: draft
dependencies: [138, 139, 140]
created: 2025-01-11
---

# Specification 143: Complete Resume System Integration Test

**Category**: testing
**Priority**: critical
**Status**: draft
**Dependencies**: [138, 139, 140]

## Context

Specs 138, 139, and 140 implemented three critical features for MapReduce resume:
- **Spec 138**: DLQ integration for failed agents
- **Spec 139**: Work item deduplication
- **Spec 140**: Concurrent resume protection with locking

Each spec has been tested individually (unit tests and some integration tests). However, there is **no test that verifies all three features working together** in a complete end-to-end workflow.

**Current Gap**: We cannot verify scenarios where:
1. A MapReduce job runs with some failures → DLQ populated (Spec 138)
2. Job interrupted → checkpoint created
3. Resume attempted concurrently → locking prevents conflicts (Spec 140)
4. Successful resume loads pending + failed + DLQ → deduplicates (Spec 139)
5. Work items processed without duplicates
6. Failed items retried successfully
7. DLQ cleaned up, lock released

**Impact**: Without this test, we risk integration bugs where individual components work but the complete system fails due to unexpected interactions between features.

## Objective

Create a comprehensive integration test (`tests/resume_dlq_lock_integration_test.rs`) that validates all three resume features working together in realistic end-to-end scenarios, ensuring the complete resume system works correctly in production.

## Requirements

### Functional Requirements

- **FR1**: Test complete resume workflow with DLQ, deduplication, and locking together
- **FR2**: Verify concurrent resume attempts are blocked while one is in progress
- **FR3**: Verify failed agents populate DLQ and are included on resume
- **FR4**: Verify deduplication works with DLQ items included
- **FR5**: Verify lock is released after resume completes
- **FR6**: Test successful retry removes items from DLQ
- **FR7**: Verify no duplicate processing across all sources
- **FR8**: Test complete failure → resume → retry → success lifecycle

### Non-Functional Requirements

- **NFR1**: Test must be deterministic and repeatable
- **NFR2**: Test uses real components (minimal mocking)
- **NFR3**: Test completes in <60 seconds
- **NFR4**: Test cleanup is thorough (no resource leaks)
- **NFR5**: Test follows existing integration test patterns

## Acceptance Criteria

- [ ] Test file `tests/resume_dlq_lock_integration_test.rs` created
- [ ] Test: `test_complete_resume_workflow_with_dlq_and_lock()` - Full end-to-end flow
- [ ] Test: `test_concurrent_resume_blocked_with_dlq()` - Locking + DLQ interaction
- [ ] Test: `test_deduplication_with_dlq_and_pending()` - All three sources
- [ ] Test: `test_retry_success_cleans_up_dlq_and_releases_lock()` - Complete cleanup
- [ ] Test: `test_resume_after_partial_failure()` - Realistic recovery scenario
- [ ] All tests pass consistently (50+ runs)
- [ ] Tests demonstrate all three specs working together
- [ ] Test execution time <60 seconds
- [ ] Resource cleanup verified (locks, temp dirs, DLQ)
- [ ] Documentation explains integration scenarios

## Technical Details

### Implementation Approach

**Test File Structure**:

```rust
// tests/resume_dlq_lock_integration_test.rs

use prodigy::cook::execution::dlq::{DeadLetterQueue, DLQFilter};
use prodigy::cook::execution::mapreduce_resume::{
    MapReduceResumeManager, EnhancedResumeOptions
};
use prodigy::cook::execution::ResumeLockManager;
use prodigy::cook::execution::state::DefaultJobStateManager;
use prodigy::storage::GlobalStorage;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_complete_resume_workflow_with_dlq_and_lock() {
    // This is the "golden path" test that exercises everything

    // PHASE 1: Setup
    let temp_dir = TempDir::new().unwrap();
    let job_id = "complete-workflow-test";
    let storage = Arc::new(GlobalStorage::new().unwrap());

    // Create workflow that will have:
    // - 10 items total
    // - Items 1-3: succeed
    // - Items 4-6: fail (go to DLQ)
    // - Items 7-10: not processed yet (pending)
    let workflow = create_mixed_success_workflow(&temp_dir, 10, vec![4, 5, 6]);

    // PHASE 2: Initial Execution (with failures)
    let result = execute_mapreduce_workflow(&workflow, job_id, storage.clone())
        .await
        .unwrap();

    // Verify initial state
    assert_eq!(result.successful, 3, "Should have 3 successful items");
    assert_eq!(result.failed, 3, "Should have 3 failed items");

    // Verify DLQ populated (Spec 138)
    let dlq = DeadLetterQueue::new(
        storage.clone(),
        "test-repo".to_string(),
        job_id.to_string(),
        1000,
    )
    .await
    .unwrap();

    let dlq_items = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(dlq_items.len(), 3, "DLQ should have 3 failed items");

    // PHASE 3: Simulate Interruption
    // Job interrupted with 4 items pending (7-10)
    let checkpoint = create_checkpoint_with_pending(
        job_id,
        vec!["item-7", "item-8", "item-9", "item-10"],
        &temp_dir,
    )
    .await;

    // PHASE 4: Attempt Concurrent Resume (Spec 140)
    let lock_manager = ResumeLockManager::new(storage.get_state_dir())?;

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
        sleep(Duration::from_millis(10)).await; // Ensure first acquires lock
        lock_manager2.acquire_lock(&job_id2).await
    });

    let result1 = handle1.await.unwrap();
    let result2 = handle2.await.unwrap();

    // Verify locking works (Spec 140)
    assert!(
        (result1.is_ok() && result2.is_err()) || (result1.is_err() && result2.is_ok()),
        "Exactly one resume should be blocked by lock"
    );

    // PHASE 5: Successful Resume with Deduplication (Spec 139)
    let resume_manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager.clone(),
        event_logger.clone(),
        temp_dir.path().to_path_buf(),
    )
    .await
    .unwrap();

    // Acquire lock for resume
    let _lock = lock_manager.acquire_lock(job_id).await.unwrap();

    // Load state
    let mut state = state_manager.load(job_id).await.unwrap();

    // Manually create overlapping scenario for testing:
    // - Pending: items 7-10
    // - Failed: items 4-6
    // - DLQ: items 4-6 (same as failed)
    // This tests deduplication across all sources

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: true,
        max_additional_retries: 3,
        ..Default::default()
    };

    // Calculate remaining items (should deduplicate)
    let remaining = resume_manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .unwrap();

    // Verify deduplication (Spec 139)
    // Should have: 4 pending (7-10) + 3 failed/DLQ (4-6, deduplicated) = 7 unique
    assert_eq!(
        remaining.len(),
        7,
        "Should have 7 unique items after deduplication"
    );

    // Verify no duplicates
    let item_ids: Vec<String> = remaining
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect();
    let unique_ids: std::collections::HashSet<_> = item_ids.iter().collect();
    assert_eq!(unique_ids.len(), 7, "Should have no duplicate IDs");

    // PHASE 6: Execute Resume (items should succeed this time)
    // Modify workflow to succeed for previously failed items
    update_workflow_to_succeed(&workflow, vec![4, 5, 6]);

    let resume_result = execute_resume(&resume_manager, &state, &env, &options)
        .await
        .unwrap();

    // Verify all items processed
    assert_eq!(
        resume_result.total_processed, 7,
        "Should process all 7 remaining items"
    );
    assert_eq!(
        resume_result.successful, 7,
        "All retried items should succeed"
    );

    // PHASE 7: Verify Cleanup
    // DLQ should be cleared (successful retry removes items)
    let dlq_after = dlq.list_items(DLQFilter::default()).await.unwrap();
    assert_eq!(
        dlq_after.len(),
        0,
        "DLQ should be empty after successful retry"
    );

    // Lock should be released (implicit via RAII, but verify file gone)
    let lock_path = storage
        .get_state_dir()
        .join("resume_locks")
        .join(format!("{}.lock", job_id));
    drop(_lock); // Explicit drop to release
    assert!(!lock_path.exists(), "Lock file should be removed");

    // PHASE 8: Verify Final State
    let final_state = state_manager.load(job_id).await.unwrap();
    assert_eq!(
        final_state.successful_count, 10,
        "All 10 items should be successful"
    );
    assert_eq!(final_state.failed_count, 0, "No failed items remaining");
    assert!(final_state.is_complete, "Job should be complete");
}

#[tokio::test]
async fn test_concurrent_resume_blocked_with_dlq() {
    // Test that concurrent resumes are blocked even when DLQ is involved

    // Setup job with DLQ items
    // Attempt two concurrent resumes
    // Verify both try to load DLQ
    // Verify only one proceeds (lock blocking works)
}

#[tokio::test]
async fn test_deduplication_with_dlq_and_pending() {
    // Test specific scenario: same item in pending AND DLQ

    // Setup
    let job_id = "dedup-dlq-test";
    let mut state = create_base_state(job_id);

    // Add "item-5" to pending
    state.pending_items.push("item_5".to_string());

    // Add "item-5" to DLQ
    let dlq = create_dlq_with_items(vec!["item-5"]).await;

    // Resume with both sources
    let options = EnhancedResumeOptions {
        include_dlq_items: true,
        ..Default::default()
    };

    let remaining = resume_manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .unwrap();

    // Should have only 1 "item-5" (deduplicated)
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["id"], "item-5");

    // Verify it's pending version (higher priority)
    assert_eq!(
        remaining[0]["source"], "pending",
        "Should use pending version, not DLQ"
    );
}

#[tokio::test]
async fn test_retry_success_cleans_up_dlq_and_releases_lock() {
    // Test complete cleanup lifecycle

    // 1. Job fails → DLQ populated
    // 2. Acquire lock
    // 3. Resume with DLQ retry
    // 4. Items succeed
    // 5. Verify DLQ cleared
    // 6. Verify lock released
}

#[tokio::test]
async fn test_resume_after_partial_failure() {
    // Realistic scenario: Some items succeed, some fail, some pending

    // Initial run:
    // - Items 1-5: succeed
    // - Items 6-8: fail → DLQ
    // - Items 9-10: not started → pending

    // Resume:
    // - Load pending (9-10)
    // - Load DLQ (6-8)
    // - Deduplicate (no overlap)
    // - Process all 5 items
    // - Items 6-8 succeed on retry → remove from DLQ
    // - Items 9-10 succeed
}
```

### Test Helpers

```rust
/// Create workflow that has mixed success/failure
fn create_mixed_success_workflow(
    temp_dir: &TempDir,
    total_items: usize,
    fail_items: Vec<usize>,
) -> PathBuf {
    // Create workflow where specific items fail
    // Use shell script with item ID check
}

/// Update workflow to make previously failing items succeed
fn update_workflow_to_succeed(workflow_path: &Path, item_ids: Vec<usize>) {
    // Modify workflow to skip failure condition
}

/// Create checkpoint with specific pending items
async fn create_checkpoint_with_pending(
    job_id: &str,
    pending: Vec<&str>,
    temp_dir: &TempDir,
) -> WorkflowCheckpoint {
    // Helper to create realistic checkpoint state
}

/// Create DLQ with specific items
async fn create_dlq_with_items(item_ids: Vec<&str>) -> DeadLetterQueue {
    // Populate DLQ for testing
}

/// Execute resume operation
async fn execute_resume(
    manager: &MapReduceResumeManager,
    state: &MapReduceJobState,
    env: &ExecutionEnvironment,
    options: &EnhancedResumeOptions,
) -> Result<ResumeResult> {
    // Wrapper for resume execution
}
```

### Architecture Changes

**New Test File**: `tests/resume_dlq_lock_integration_test.rs`
- No production code changes
- Uses all three feature implementations together
- Tests realistic end-to-end scenarios

**Test Complexity**:
- Most complex integration test in the suite
- Multiple phases (setup, fail, checkpoint, resume, cleanup)
- Verifies interactions between features

## Dependencies

- **Prerequisites**:
  - [138] DLQ Integration (implementation + tests)
  - [139] Work Item Deduplication (implementation + tests)
  - [140] Concurrent Resume Protection (implementation + tests)
- **Affected Components**: None (testing only)
- **External Dependencies**: Existing test infrastructure

## Testing Strategy

### Test Execution

```bash
# Run complete system integration test
cargo test --test resume_dlq_lock_integration_test

# Run with full output
RUST_LOG=debug cargo test --test resume_dlq_lock_integration_test -- --nocapture

# Run specific scenario
cargo test --test resume_dlq_lock_integration_test test_complete_resume_workflow_with_dlq_and_lock
```

### Test Coverage Goals

- **End-to-End Flow**: ✅ Fail → DLQ → Resume → Dedup → Retry → Success
- **Feature Interaction**: ✅ All three specs working together
- **Cleanup Verification**: ✅ DLQ cleared, locks released
- **Realistic Scenarios**: ✅ Partial failures, concurrent resume

### Performance Targets

- Total test suite: <60 seconds
- Main test: <30 seconds
- Concurrent test: <10 seconds

## Documentation Requirements

### Code Documentation

```rust
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
```

### Test Documentation

```rust
/// Test complete resume workflow with DLQ, deduplication, and locking.
///
/// **Scenario** (Multi-Phase):
/// 1. **Initial Run**: 10 items, 3 succeed, 3 fail → DLQ, 4 pending
/// 2. **Concurrent Resume**: Two processes try to resume (one blocked by lock)
/// 3. **Deduplication**: Load pending + failed + DLQ, deduplicate overlaps
/// 4. **Retry**: Previously failed items succeed, DLQ cleared
/// 5. **Cleanup**: Locks released, state marked complete
///
/// **Validates**:
/// - Spec 138: Failed agents → DLQ → retry
/// - Spec 139: Deduplication across all sources
/// - Spec 140: Concurrent resume protection
/// - Integration: All three features working together
#[tokio::test]
async fn test_complete_resume_workflow_with_dlq_and_lock() { ... }
```

## Implementation Notes

### Test Design Philosophy

1. **Realistic Scenarios**: Use patterns that would occur in production
2. **Multi-Phase Testing**: Break complex scenarios into clear phases
3. **Verification at Each Step**: Assert state after each phase
4. **Complete Lifecycle**: Test from start to finish, including cleanup

### Key Integration Points to Test

1. **DLQ + Deduplication**: Same item in DLQ and pending
2. **Lock + DLQ**: Lock prevents concurrent DLQ access
3. **Lock + Deduplication**: Lock held while deduplication runs
4. **All Three Together**: Complete resume with all features active

### Avoiding Test Complexity Traps

- Keep helper functions focused and well-documented
- Use clear phase markers in test code
- Assert intermediate state, not just final state
- Make test data minimal but realistic

## Migration and Compatibility

### No Breaking Changes

Testing only, no production code changes.

### Test Infrastructure

- Uses existing test dependencies
- Follows existing integration test patterns
- May take longer than unit tests (acceptable for integration)

## Success Metrics

- [ ] All integration tests passing
- [ ] Tests demonstrate all three specs working together
- [ ] Zero flakiness (50+ consecutive runs)
- [ ] Tests complete in <60 seconds
- [ ] Clear documentation of integration scenarios
- [ ] Code review approved

## Validation Checklist

- [ ] All acceptance criteria met
- [ ] Tests pass on CI
- [ ] Resource cleanup verified (no leaks)
- [ ] Test scenarios realistic and valuable
- [ ] Documentation explains integration points
- [ ] Performance acceptable (<60s total)
