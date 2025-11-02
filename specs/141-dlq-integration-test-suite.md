---
number: 141
title: DLQ Integration Test Suite for MapReduce Agent Failures
category: testing
priority: critical
status: draft
dependencies: [138]
created: 2025-01-11
---

# Specification 141: DLQ Integration Test Suite for MapReduce Agent Failures

**Category**: testing
**Priority**: critical
**Status**: draft
**Dependencies**: [138]

## Context

Spec 138 implemented DLQ integration for failed MapReduce agents with comprehensive unit tests (17 tests passing). However, there are **no integration tests** verifying the complete end-to-end flow from agent failure through DLQ population to retry.

**Current Gap**: While the pure conversion function `agent_result_to_dlq_item()` is well-tested, we cannot verify:
1. Failed agents actually populate DLQ in real workflow execution
2. `prodigy dlq retry` correctly reprocesses failed items
3. Resume with `include_dlq_items: true` properly loads and processes DLQ items
4. DLQ entries contain all required metadata (json_log_location, error details, etc.)

**Impact**: Without these tests, we risk deploying code where the orchestrator integration is broken despite having working pure functions.

## Objective

Create a comprehensive integration test suite (`tests/dlq_agent_integration_test.rs`) that validates the complete DLQ workflow from agent failure through retry, ensuring Spec 138 works correctly in production scenarios.

## Requirements

### Functional Requirements

- **FR1**: Test that failed MapReduce agents populate DLQ with correct metadata
- **FR2**: Test that timed-out agents populate DLQ with timeout error type
- **FR3**: Test that `prodigy dlq retry` reprocesses failed items
- **FR4**: Test that resume with `include_dlq_items: true` includes DLQ items in work queue
- **FR5**: Verify DLQ entries include json_log_location for debugging
- **FR6**: Verify DLQ entries include worktree_path when preserved
- **FR7**: Test that successful retry removes items from DLQ
- **FR8**: Test that failed retry updates DLQ failure count

### Non-Functional Requirements

- **NFR1**: Tests must be deterministic (no flakiness)
- **NFR2**: Tests use real MapReduce executor (no mocks for critical paths)
- **NFR3**: Test setup/teardown cleans up temp directories
- **NFR4**: Tests complete in <30 seconds total
- **NFR5**: Tests follow existing integration test patterns

## Acceptance Criteria

- [ ] Test file `tests/dlq_agent_integration_test.rs` created
- [ ] Test: `test_failed_agent_populates_dlq_end_to_end()` - Verifies agent failure → DLQ flow
- [ ] Test: `test_timeout_agent_populates_dlq()` - Verifies timeout → DLQ flow
- [ ] Test: `test_dlq_retry_reprocesses_failed_items()` - Verifies retry command
- [ ] Test: `test_resume_includes_dlq_items()` - Verifies resume integration
- [ ] Test: `test_json_log_location_preserved_in_dlq()` - Verifies metadata
- [ ] Test: `test_successful_retry_removes_from_dlq()` - Verifies DLQ cleanup
- [ ] Test: `test_failed_retry_updates_dlq_failure_count()` - Verifies retry tracking
- [ ] All tests pass without flakiness
- [ ] Tests follow existing patterns from `tests/mapreduce_*_test.rs`
- [ ] Test cleanup verified (no temp file leaks)

## Technical Details

### Implementation Approach

**Test File Structure**:

```rust
// tests/dlq_agent_integration_test.rs

use prodigy::cook::execution::dlq::{DeadLetterQueue, DLQFilter};
use prodigy::cook::execution::mapreduce::MapReduceConfig;
use prodigy::cook::execution::state::DefaultJobStateManager;
use prodigy::storage::GlobalStorage;
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;
use tokio;

/// Helper to create test workflow that will fail
fn create_failing_workflow(temp_dir: &TempDir) -> (PathBuf, String) {
    // Create workflow YAML with agent command that will fail
    let workflow = r#"
name: test-dlq-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 2

  agent_template:
    - shell: "exit 1"  # Always fails
"#;

    let workflow_path = temp_dir.path().join("workflow.yml");
    std::fs::write(&workflow_path, workflow).unwrap();

    // Create input items
    let items = json!({
        "items": [
            {"id": "item-1", "data": "test1"},
            {"id": "item-2", "data": "test2"},
        ]
    });

    let items_path = temp_dir.path().join("items.json");
    std::fs::write(&items_path, serde_json::to_string_pretty(&items).unwrap()).unwrap();

    (workflow_path, "test-job-id".to_string())
}

#[tokio::test]
async fn test_failed_agent_populates_dlq_end_to_end() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    let (workflow_path, job_id) = create_failing_workflow(&temp_dir);

    // Execute MapReduce workflow (expect failures)
    let storage = Arc::new(GlobalStorage::new().unwrap());
    let state_manager = Arc::new(DefaultJobStateManager::new(
        storage.get_state_dir().join(&job_id)
    ));

    // Execute workflow - agents will fail
    let result = execute_mapreduce_workflow(&workflow_path, &job_id, storage.clone()).await;

    // Verify workflow completed (even with failures)
    assert!(result.is_ok(), "Workflow should complete despite agent failures");

    // Load DLQ
    let repo_name = "test-repo";
    let dlq = DeadLetterQueue::new(
        storage.clone(),
        repo_name.to_string(),
        job_id.clone(),
        1000, // capacity
    ).await.unwrap();

    // Verify DLQ contains failed items
    let dlq_items = dlq.list_items(DLQFilter::default()).await.unwrap();

    assert_eq!(dlq_items.len(), 2, "Should have 2 failed items in DLQ");

    // Verify DLQ entry structure
    let first_item = &dlq_items[0];
    assert_eq!(first_item.item_id, "item-1");
    assert_eq!(first_item.failure_count, 1);
    assert!(first_item.reprocess_eligible);
    assert_eq!(first_item.failure_history.len(), 1);

    // Verify failure details
    let failure = &first_item.failure_history[0];
    assert_eq!(failure.attempt_number, 1);
    assert!(failure.error_message.contains("exit") || failure.error_message.contains("failed"));
    assert!(failure.json_log_location.is_some(), "Should have Claude JSON log location");
}

#[tokio::test]
async fn test_timeout_agent_populates_dlq() {
    // Similar to above but with timeout scenario
    // Use agent_timeout_secs: 1 and sleep command
}

#[tokio::test]
async fn test_dlq_retry_reprocesses_failed_items() {
    // 1. Create job with failures in DLQ
    // 2. Modify workflow to succeed on retry
    // 3. Run `prodigy dlq retry <job_id>`
    // 4. Verify items reprocessed
    // 5. Verify DLQ updated (removed on success)
}

#[tokio::test]
async fn test_resume_includes_dlq_items() {
    // 1. Create interrupted job with DLQ items
    // 2. Create checkpoint
    // 3. Resume with include_dlq_items: true
    // 4. Verify DLQ items in work queue
    // 5. Verify no duplicates with pending
}

#[tokio::test]
async fn test_json_log_location_preserved_in_dlq() {
    // Verify Claude JSON log path is captured
}

#[tokio::test]
async fn test_successful_retry_removes_from_dlq() {
    // Verify DLQ cleanup on successful retry
}

#[tokio::test]
async fn test_failed_retry_updates_dlq_failure_count() {
    // Verify retry attempts tracked
}
```

### Test Helpers

**Shared Test Utilities**:

```rust
/// Execute MapReduce workflow and return result
async fn execute_mapreduce_workflow(
    workflow_path: &Path,
    job_id: &str,
    storage: Arc<GlobalStorage>,
) -> Result<MapReduceResult> {
    // Load and parse workflow
    // Create execution environment
    // Run MapReduce coordinator
    // Return result
}

/// Create workflow that fails N times then succeeds
fn create_retry_workflow(temp_dir: &TempDir, fail_count: usize) -> PathBuf {
    // Use shell script with counter file
}

/// Verify DLQ entry has all required fields
fn assert_dlq_entry_complete(entry: &DeadLetteredItem) {
    assert!(!entry.item_id.is_empty());
    assert!(entry.failure_count > 0);
    assert!(!entry.failure_history.is_empty());
    assert!(!entry.error_signature.is_empty());
}
```

### Architecture Changes

**New Test File**: `tests/dlq_agent_integration_test.rs`
- No production code changes
- Uses existing DLQ and MapReduce infrastructure
- Tests integration points only

**Test Infrastructure**:
- Reuse existing `TempDir` pattern
- Reuse existing MapReduce test helpers
- Follow patterns from `tests/mapreduce_resume_integration_test.rs`

### Data Structures

No new data structures. Tests use existing:
- `DeadLetteredItem`
- `FailureDetail`
- `MapReduceConfig`
- `MapReduceResult`

### APIs and Interfaces

Tests validate these existing APIs:
- `DeadLetterQueue::list_items()`
- `DeadLetterQueue::add()`
- `dlq_integration::agent_result_to_dlq_item()`
- MapReduce executor integration

## Dependencies

- **Prerequisites**: [138] DLQ Integration (implementation complete)
- **Affected Components**: None (testing only)
- **External Dependencies**: None (uses existing test infrastructure)

## Testing Strategy

### Test Execution

```bash
# Run DLQ integration tests
cargo test --test dlq_agent_integration_test

# Run with output
cargo test --test dlq_agent_integration_test -- --nocapture

# Run specific test
cargo test --test dlq_agent_integration_test test_failed_agent_populates_dlq_end_to_end
```

### Test Coverage Goals

- **End-to-End Flow**: ✅ Agent failure → DLQ → retry → success
- **Edge Cases**: ✅ Timeout, resume, retry failures
- **Metadata Validation**: ✅ All DLQ fields populated correctly
- **Cleanup**: ✅ Successful operations remove from DLQ

### Performance Targets

- Total test suite: <30 seconds
- Individual test: <10 seconds
- Temp directory cleanup: 100% (no leaks)

## Documentation Requirements

### Code Documentation

```rust
//! Integration tests for DLQ agent failure handling
//!
//! These tests verify the complete flow from agent failure through DLQ
//! population, retry, and resume. They validate Spec 138 implementation
//! in real workflow scenarios.
//!
//! **Test Coverage**:
//! - Agent failure → DLQ population
//! - Timeout → DLQ population
//! - DLQ retry command
//! - Resume with DLQ items
//! - Metadata preservation (json_log_location, etc.)
```

### Test Documentation

Each test should have clear doc comments:

```rust
/// Test that failed MapReduce agents populate the DLQ with correct metadata.
///
/// **Scenario**:
/// 1. Create MapReduce workflow with agents that will fail
/// 2. Execute workflow (agents fail with exit code 1)
/// 3. Verify DLQ contains failed items
/// 4. Verify DLQ entries have complete metadata
///
/// **Validates**:
/// - Agent failure triggers DLQ insertion
/// - DLQ entries include json_log_location
/// - Error details captured correctly
/// - Failure count initialized to 1
#[tokio::test]
async fn test_failed_agent_populates_dlq_end_to_end() { ... }
```

## Implementation Notes

### Test Design Principles

1. **Deterministic**: No random data, fixed sleep times
2. **Isolated**: Each test uses own temp directory
3. **Complete**: Test full end-to-end flows, not just happy paths
4. **Clear**: Assertion messages explain what's being validated

### Common Pitfalls to Avoid

1. **Flaky Tests**: Don't rely on timing assumptions
2. **Resource Leaks**: Always use TempDir, clean up in test end
3. **Over-Mocking**: Use real components where possible
4. **Unclear Assertions**: Always include descriptive messages

### Test Data Strategy

Use minimal, focused test data:
- 2-3 work items per test
- Simple JSON structures
- Predictable failure conditions
- Clear success/failure outcomes

## Migration and Compatibility

### No Breaking Changes

This is test-only, no production code changes.

### Test Infrastructure Requirements

- Existing `tempfile` dependency
- Existing `tokio::test` framework
- Existing MapReduce test helpers
- No new dependencies needed

### CI/CD Integration

Tests should run in CI:
- Add to `cargo test --all` suite
- Run in parallel with other integration tests
- No special environment requirements
- Should pass on all platforms

## Success Metrics

- [ ] All 7+ integration tests passing
- [ ] Tests cover 100% of critical DLQ workflows
- [ ] Tests complete in <30 seconds
- [ ] Zero flakiness (100 consecutive runs pass)
- [ ] Test code reviewed and follows patterns
- [ ] Documentation complete and clear

## Validation Checklist

Before marking spec complete:
- [ ] All acceptance criteria met
- [ ] Tests pass on CI
- [ ] No resource leaks detected
- [ ] Test coverage verified with `cargo tarpaulin` or `cargo llvm-cov`
- [ ] Code review completed
- [ ] Documentation updated
