---
number: 142
title: Work Item Deduplication Integration Test Suite
category: testing
priority: critical
status: draft
dependencies: [139]
created: 2025-01-11
---

# Specification 142: Work Item Deduplication Integration Test Suite

**Category**: testing
**Priority**: critical
**Status**: draft
**Dependencies**: [139]

## Context

Spec 139 implemented work item deduplication for MapReduce resume with excellent unit test coverage (9 tests passing for the pure deduplication function, 7 tests for collection helpers). However, there are **no integration tests** verifying deduplication works in actual resume workflows.

**Current Gap**: While `deduplicate_work_items()` is well-tested in isolation, we cannot verify:
1. Deduplication actually runs during resume operations
2. Items from pending take precedence over failed/DLQ (priority order)
3. No duplicate agent execution occurs when resuming with overlapping sources
4. Warning logs are emitted when duplicates are detected

**Impact**: Without these tests, we risk scenarios where items appear in multiple sources (pending + failed + DLQ) and get processed multiple times, wasting resources and potentially corrupting results.

## Objective

Create a comprehensive integration test suite (`tests/resume_deduplication_integration_test.rs`) that validates work item deduplication in real resume workflows, ensuring Spec 139 works correctly when resuming MapReduce jobs with overlapping item sources.

## Requirements

### Functional Requirements

- **FR1**: Test that resume deduplicates items appearing in multiple sources
- **FR2**: Test that pending items take precedence over failed items (first occurrence rule)
- **FR3**: Test that pending items take precedence over DLQ items
- **FR4**: Test that failed items take precedence over DLQ items
- **FR5**: Verify no duplicate agent execution occurs
- **FR6**: Verify warning log is emitted when duplicates detected
- **FR7**: Verify metrics are emitted for duplicate count
- **FR8**: Test deduplication with all three sources (pending + failed + DLQ)

### Non-Functional Requirements

- **NFR1**: Tests must be deterministic (no race conditions)
- **NFR2**: Tests use real resume manager (minimal mocking)
- **NFR3**: Tests verify actual agent execution count (not just item count)
- **NFR4**: Tests complete in <20 seconds total
- **NFR5**: Tests follow existing MapReduce test patterns

## Acceptance Criteria

- [ ] Test file `tests/resume_deduplication_integration_test.rs` created
- [ ] Test: `test_resume_deduplicates_overlapping_sources()` - Verifies deduplication runs
- [ ] Test: `test_resume_pending_takes_precedence_over_failed()` - Verifies priority order
- [ ] Test: `test_resume_pending_takes_precedence_over_dlq()` - Verifies pending > DLQ
- [ ] Test: `test_no_duplicate_agent_execution()` - Verifies agents run once per item
- [ ] Test: `test_deduplication_logs_warning()` - Verifies observability
- [ ] Test: `test_deduplication_with_all_three_sources()` - Verifies complete flow
- [ ] Test: `test_deduplication_preserves_pending_data()` - Verifies data integrity
- [ ] All tests pass consistently (100+ runs)
- [ ] Tests follow patterns from `tests/mapreduce_resume_integration_test.rs`
- [ ] Agent execution tracking verified

## Technical Details

### Implementation Approach

**Test File Structure**:

```rust
// tests/resume_deduplication_integration_test.rs

use prodigy::cook::execution::mapreduce_resume::{
    MapReduceResumeManager, EnhancedResumeOptions
};
use prodigy::cook::execution::state::{DefaultJobStateManager, MapReduceJobState};
use prodigy::cook::execution::dlq::DeadLetterQueue;
use prodigy::storage::GlobalStorage;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tracing_subscriber;

/// Helper to create job state with overlapping items
fn create_state_with_overlapping_items(
    job_id: &str,
    pending: Vec<&str>,
    failed: Vec<&str>,
) -> MapReduceJobState {
    let mut state = create_base_state(job_id);

    // Add items to pending_items
    state.pending_items = pending.iter().map(|s| s.to_string()).collect();

    // Add same items to failed_agents
    for item_id in failed {
        state.failed_agents.insert(
            item_id.to_string(),
            FailureRecord {
                item_id: item_id.to_string(),
                attempts: 1,
                last_error: "test error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );
    }

    state
}

#[tokio::test]
async fn test_resume_deduplicates_overlapping_sources() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let job_id = "test-overlap";

    // Create state where "item-1" is in BOTH pending AND failed_agents
    let mut state = create_state_with_overlapping_items(
        job_id,
        vec!["item-1", "item-2"],      // pending
        vec!["item-1", "item-3"],      // failed (item-1 overlaps!)
    );

    // Create resume manager
    let storage = Arc::new(GlobalStorage::new().unwrap());
    let state_manager = Arc::new(DefaultJobStateManager::new(
        temp_dir.path().to_path_buf()
    ));
    let event_logger = Arc::new(EventLogger::new(vec![]));

    let manager = MapReduceResumeManager::new(
        job_id.to_string(),
        state_manager,
        event_logger,
        temp_dir.path().to_path_buf(),
    )
    .await
    .unwrap();

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
        .unwrap();

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
        .map(|item| item["id"].as_str().unwrap().to_string())
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

#[tokio::test]
async fn test_resume_pending_takes_precedence_over_failed() {
    // Setup state where item has different data in pending vs failed
    let mut state = create_base_state("test-precedence");

    // Add "item-1" to pending with data "pending-version"
    state.work_items.push(json!({
        "id": "item-1",
        "data": "pending-version"
    }));
    state.pending_items.push("item_0".to_string());

    // Add "item-1" to failed_agents (simulates same item ID)
    // In real scenario, this would reference same work_items index
    // but we'll test that pending version is chosen

    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .unwrap();

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
        .unwrap();

    assert_eq!(
        item_1["data"], "pending-version",
        "Should use pending version (first occurrence)"
    );
}

#[tokio::test]
async fn test_no_duplicate_agent_execution() {
    // This is the most important test - verify agents don't run twice

    // Track agent executions
    let execution_tracker = Arc::new(Mutex::new(Vec::new()));
    let tracker_clone = execution_tracker.clone();

    // Create job with overlapping items
    // Use mock executor that tracks which items are processed
    let mock_executor = create_tracking_executor(tracker_clone);

    // Run resume with overlapping sources
    // ... execute workflow ...

    // Verify each item ID was processed exactly once
    let executions = execution_tracker.lock().unwrap();
    let mut execution_counts = std::collections::HashMap::new();

    for item_id in executions.iter() {
        *execution_counts.entry(item_id).or_insert(0) += 1;
    }

    for (item_id, count) in execution_counts.iter() {
        assert_eq!(
            *count, 1,
            "Item {} should be executed exactly once, but was executed {} times",
            item_id, count
        );
    }
}

#[tokio::test]
async fn test_deduplication_logs_warning() {
    // Setup tracing subscriber to capture logs
    let (log_capture, _guard) = create_log_capture();

    // Create state with duplicates
    let mut state = create_state_with_overlapping_items(
        "test-log",
        vec!["item-1"],
        vec!["item-1"],  // Duplicate!
    );

    // Resume (triggers deduplication)
    let _ = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .unwrap();

    // Verify warning was logged
    let logs = log_capture.lock().unwrap();
    let warning_found = logs.iter().any(|log| {
        log.contains("duplicate work items") && log.contains("WARN")
    });

    assert!(warning_found, "Should log warning about duplicates");

    // Verify duplicate count in log
    let log_with_count = logs.iter().find(|log| log.contains("duplicate"));
    assert!(
        log_with_count.unwrap().contains("1"),
        "Should mention 1 duplicate"
    );
}

#[tokio::test]
async fn test_deduplication_with_all_three_sources() {
    // Test with pending + failed + DLQ all containing same item

    // Setup state
    let mut state = create_base_state("test-three-sources");

    // Add "item-1" to pending
    state.pending_items.push("item_0".to_string());

    // Add "item-1" to failed_agents
    state.failed_agents.insert("item_0".to_string(), create_failure_record());

    // Add "item-1" to DLQ
    let dlq = create_dlq_with_item("item-1");

    // Resume with all sources enabled
    let options = EnhancedResumeOptions {
        reset_failed_agents: true,
        include_dlq_items: true,
        ..Default::default()
    };

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .unwrap();

    // Should have exactly 1 item (deduplicated from 3 sources)
    assert_eq!(
        remaining.len(),
        1,
        "Should deduplicate item across all 3 sources"
    );

    // Should be pending version (highest priority)
    assert_eq!(remaining[0]["id"], "item-1");
}

#[tokio::test]
async fn test_deduplication_preserves_pending_data() {
    // Verify that when deduplicating, we keep the pending version's data

    // Setup with different data in each source
    let mut state = create_base_state("test-data-preservation");

    // Pending version has important metadata
    state.work_items[0] = json!({
        "id": "item-1",
        "data": "important-pending-data",
        "metadata": {"source": "pending"}
    });

    // Failed version has different data
    // (In real scenario, they'd share work_items reference)

    let remaining = manager
        .calculate_remaining_items(&mut state, &options)
        .await
        .unwrap();

    // Verify pending data is preserved
    assert_eq!(remaining[0]["data"], "important-pending-data");
    assert_eq!(remaining[0]["metadata"]["source"], "pending");
}
```

### Test Helpers

```rust
/// Create tracking executor that records which items are processed
fn create_tracking_executor(
    tracker: Arc<Mutex<Vec<String>>>
) -> MockMapReduceExecutor {
    // Mock executor that logs item IDs to tracker
}

/// Create log capture for testing warning messages
fn create_log_capture() -> (Arc<Mutex<Vec<String>>>, TracingGuard) {
    // Setup tracing subscriber that captures logs
}

/// Create DLQ with specific item
async fn create_dlq_with_item(item_id: &str) -> DeadLetterQueue {
    // Helper to populate DLQ for testing
}
```

### Architecture Changes

**New Test File**: `tests/resume_deduplication_integration_test.rs`
- No production code changes
- Uses existing MapReduceResumeManager
- Tests integration points only

**Test Patterns**:
- Follow `tests/mapreduce_resume_integration_test.rs` patterns
- Use real components where possible
- Minimal mocking (only for tracking)

## Dependencies

- **Prerequisites**: [139] Work Item Deduplication (implementation complete)
- **Affected Components**: None (testing only)
- **External Dependencies**:
  - `tracing-subscriber` for log capture (already in dev-dependencies)
  - Existing test infrastructure

## Testing Strategy

### Test Execution

```bash
# Run deduplication integration tests
cargo test --test resume_deduplication_integration_test

# Run specific test
cargo test --test resume_deduplication_integration_test test_no_duplicate_agent_execution

# Run with log output
RUST_LOG=debug cargo test --test resume_deduplication_integration_test -- --nocapture
```

### Test Coverage Goals

- **Deduplication Verification**: ✅ Runs in real resume
- **Priority Order**: ✅ Pending > Failed > DLQ
- **Execution Tracking**: ✅ No duplicate agents
- **Observability**: ✅ Logs and metrics work

### Performance Targets

- Total test suite: <20 seconds
- Individual test: <5 seconds
- No resource leaks

## Documentation Requirements

### Code Documentation

```rust
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
```

### Test Documentation

```rust
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
async fn test_resume_deduplicates_overlapping_sources() { ... }
```

## Implementation Notes

### Key Testing Challenges

1. **Tracking Agent Execution**: Need to verify agents run once, not just that items are deduplicated
2. **Log Capture**: Must capture warning logs without interfering with other tests
3. **State Setup**: Creating realistic overlapping state is tricky
4. **Priority Verification**: Must verify correct version is kept (not just count)

### Test Data Strategy

- Use 2-3 items per test (minimal but realistic)
- Create clear overlap scenarios
- Use distinct data in each source for verification
- Predictable item IDs ("item-1", "item-2", etc.)

### Avoiding Flakiness

- No timing-dependent assertions
- Use deterministic data (no random IDs)
- Proper cleanup with TempDir
- Isolated log capture per test

## Migration and Compatibility

### No Breaking Changes

Testing only, no production code changes.

### Test Infrastructure

- Uses existing `tempfile` dependency
- Uses existing `tokio::test` framework
- May need `tracing-subscriber` in dev-dependencies (already present)

## Success Metrics

- [ ] All 7+ integration tests passing
- [ ] Tests cover priority order verification
- [ ] Agent execution tracking validated
- [ ] Zero flakiness (100+ consecutive runs)
- [ ] Tests complete in <20 seconds
- [ ] Code review approved

## Validation Checklist

- [ ] All acceptance criteria met
- [ ] Tests pass on CI
- [ ] No resource leaks
- [ ] Log capture doesn't interfere with other tests
- [ ] Agent tracking mechanism verified
- [ ] Documentation complete
