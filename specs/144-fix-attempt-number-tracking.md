---
number: 144
title: Fix Attempt Number Tracking for DLQ Integration
category: foundation
priority: critical
status: draft
dependencies: [138]
created: 2025-01-11
---

# Specification 144: Fix Attempt Number Tracking for DLQ Integration

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [138]

## Context

Spec 138 implemented DLQ integration for failed MapReduce agents, including an `attempt_number` parameter in the `agent_result_to_dlq_item()` function. However, the orchestrator integration **always passes `1` as the attempt number**, regardless of how many times an item has actually been retried.

**Current Issue** (`src/cook/execution/mapreduce/coordination/executor.rs:752`):

```rust
if let Some(dlq_item) =
    dlq_integration::agent_result_to_dlq_item(&agent_result, &item_for_dlq, 1)
//                                                                         ^
//                                                                   Always 1!
```

**Impact**:
- DLQ failure history is inaccurate
- Cannot distinguish first failure from subsequent retries
- `failure_count` in DLQ is always 1, even after multiple retry attempts
- Users cannot see retry progression
- DLQ retry strategies cannot use attempt count for backoff/limits

**Example Scenario**:
1. Item fails first time → DLQ entry with `attempt_number: 1`
2. Item retried, fails again → DLQ entry with `attempt_number: 1` (WRONG!)
3. Item retried again, fails → DLQ entry with `attempt_number: 1` (WRONG!)

**Expected Behavior**:
1. Item fails first time → DLQ entry with `attempt_number: 1` ✓
2. Item retried, fails again → DLQ entry with `attempt_number: 2` ✓
3. Item retried again, fails → DLQ entry with `attempt_number: 3` ✓

## Objective

Implement proper attempt number tracking so that DLQ entries accurately reflect the actual number of retry attempts for each work item, enabling accurate failure analysis and retry strategies.

## Requirements

### Functional Requirements

- **FR1**: Track actual retry count per work item across agent executions
- **FR2**: Pass correct attempt number to `agent_result_to_dlq_item()`
- **FR3**: Persist retry count in job state across interruptions
- **FR4**: Increment retry count when reprocessing failed items
- **FR5**: Reset retry count to 1 for fresh items (not retries)
- **FR6**: Expose retry count in DLQ failure history
- **FR7**: Support retry count in DLQ retry command

### Non-Functional Requirements

- **NFR1**: No breaking changes to existing DLQ API
- **NFR2**: Backward compatible with existing checkpoints (default to 1 if missing)
- **NFR3**: Pure function for retry count increment (testable)
- **NFR4**: Minimal performance impact (<1ms overhead)
- **NFR5**: Thread-safe retry count updates

## Acceptance Criteria

- [ ] `MapReduceJobState` extended with `item_retry_counts: HashMap<String, u32>`
- [ ] Pure function `get_item_attempt_number()` implemented
- [ ] Orchestrator integration updated to pass correct attempt number
- [ ] Retry count incremented when loading from failed_agents
- [ ] Retry count incremented when loading from DLQ
- [ ] Retry count persisted in checkpoints
- [ ] Backward compatibility tested (old checkpoints work)
- [ ] Unit test: Fresh item has attempt number 1
- [ ] Unit test: Retried item has attempt number incremented
- [ ] Integration test: DLQ failure history shows correct attempts
- [ ] All existing tests pass without modification

## Technical Details

### Implementation Approach

**Step 1: Extend MapReduceJobState**

```rust
// src/cook/execution/state/mod.rs

pub struct MapReduceJobState {
    // ... existing fields ...

    /// Track retry attempts per work item
    /// Key: item_id, Value: number of attempts so far
    #[serde(default)]
    pub item_retry_counts: HashMap<String, u32>,
}
```

**Step 2: Add Pure Helper Function**

```rust
// src/cook/execution/mapreduce/retry_tracking.rs

/// Get the attempt number for a work item.
///
/// Returns the current attempt count + 1 (since this is the next attempt).
/// If item has never been attempted, returns 1.
///
/// # Arguments
/// * `item_id` - The work item identifier
/// * `retry_counts` - Map of item IDs to retry counts
///
/// # Returns
/// The attempt number for this execution (1-indexed)
///
/// # Examples
/// ```
/// let mut retry_counts = HashMap::new();
///
/// // First attempt
/// assert_eq!(get_item_attempt_number("item-1", &retry_counts), 1);
///
/// // After one retry
/// retry_counts.insert("item-1".to_string(), 1);
/// assert_eq!(get_item_attempt_number("item-1", &retry_counts), 2);
/// ```
pub fn get_item_attempt_number(
    item_id: &str,
    retry_counts: &HashMap<String, u32>,
) -> u32 {
    retry_counts.get(item_id).map(|count| count + 1).unwrap_or(1)
}

/// Increment retry count for an item.
///
/// Pure function - returns new HashMap with incremented count.
///
/// # Arguments
/// * `item_id` - The work item identifier
/// * `retry_counts` - Current retry counts map
///
/// # Returns
/// New HashMap with incremented count for the item
pub fn increment_retry_count(
    item_id: &str,
    mut retry_counts: HashMap<String, u32>,
) -> HashMap<String, u32> {
    *retry_counts.entry(item_id.to_string()).or_insert(0) += 1;
    retry_counts
}

/// Get retry counts from multiple sources (failed_agents, DLQ).
///
/// Pure function that merges retry information.
///
/// # Arguments
/// * `failed_items` - Items from failed_agents with their failure counts
/// * `dlq_items` - Items from DLQ with their failure counts
///
/// # Returns
/// Merged retry counts map
pub fn merge_retry_counts(
    failed_items: &HashMap<String, FailureRecord>,
    dlq_items: &[DeadLetteredItem],
) -> HashMap<String, u32> {
    let mut counts = HashMap::new();

    // Add counts from failed_agents
    for (item_id, failure) in failed_items {
        counts.insert(item_id.clone(), failure.attempts);
    }

    // Merge with DLQ counts (use max if both exist)
    for dlq_item in dlq_items {
        counts
            .entry(dlq_item.item_id.clone())
            .and_modify(|count| *count = (*count).max(dlq_item.failure_count))
            .or_insert(dlq_item.failure_count);
    }

    counts
}
```

**Step 3: Update Orchestrator Integration**

```rust
// src/cook/execution/mapreduce/coordination/executor.rs

// Line ~752 (in agent execution loop)

// Get current attempt number for this item
let attempt_number = retry_tracking::get_item_attempt_number(
    &item_id,
    &state.item_retry_counts,
);

// Add failed items to DLQ with correct attempt number
if let Some(dlq_item) =
    dlq_integration::agent_result_to_dlq_item(&agent_result, &item_for_dlq, attempt_number)
{
    if let Err(e) = dlq.add(dlq_item).await {
        warn!(
            "Failed to add item {} to DLQ: {}. Item tracking may be incomplete.",
            agent_result.item_id, e
        );
    } else {
        info!(
            "Added failed item {} to DLQ (attempt {})",
            agent_result.item_id, attempt_number
        );
    }

    // Increment retry count in state
    state.item_retry_counts = retry_tracking::increment_retry_count(
        &item_id,
        state.item_retry_counts.clone(),
    );
}
```

**Step 4: Update Resume Logic**

```rust
// src/cook/execution/mapreduce_resume.rs

async fn calculate_remaining_items(
    &self,
    state: &mut MapReduceJobState,
    options: &EnhancedResumeOptions,
) -> MRResult<Vec<Value>> {
    // ... existing collection code ...

    // Initialize retry counts from state sources
    if state.item_retry_counts.is_empty() {
        // Load DLQ items to get their failure counts
        let dlq_items = if options.include_dlq_items {
            self.load_all_dlq_items(&state.job_id).await?
        } else {
            vec![]
        };

        // Merge retry counts from failed_agents and DLQ
        state.item_retry_counts = retry_tracking::merge_retry_counts(
            &state.failed_agents,
            &dlq_items,
        );
    }

    // ... rest of resume logic ...
}
```

### Architecture Changes

**New Module**: `src/cook/execution/mapreduce/retry_tracking.rs`
- Pure functions for retry count management
- No I/O, fully testable
- Exports public API for retry tracking

**Modified Modules**:
- `src/cook/execution/state/mod.rs` - Add `item_retry_counts` field
- `src/cook/execution/mapreduce/coordination/executor.rs` - Use retry tracking
- `src/cook/execution/mapreduce_resume.rs` - Initialize retry counts

**Checkpoint Compatibility**:
- `item_retry_counts` uses `#[serde(default)]` - old checkpoints work
- Missing field defaults to empty HashMap
- First load after upgrade initializes from failed_agents

### Data Structures

```rust
// In MapReduceJobState
pub struct MapReduceJobState {
    // ... existing fields ...

    /// Track retry attempts per work item ID
    #[serde(default)]
    pub item_retry_counts: HashMap<String, u32>,
}

// No changes to existing structures
```

### APIs and Interfaces

**New Public Functions**:

```rust
pub fn get_item_attempt_number(item_id: &str, retry_counts: &HashMap<String, u32>) -> u32;
pub fn increment_retry_count(item_id: &str, retry_counts: HashMap<String, u32>) -> HashMap<String, u32>;
pub fn merge_retry_counts(failed_items: &HashMap<String, FailureRecord>, dlq_items: &[DeadLetteredItem]) -> HashMap<String, u32>;
```

**Modified Behavior**:
- DLQ entries now have accurate `attempt_number`
- `failure_count` in DLQ accurately reflects retries
- Resume loads retry counts from checkpoint

## Dependencies

- **Prerequisites**: [138] DLQ Integration (implementation complete)
- **Affected Components**:
  - MapReduce job state (add field)
  - Orchestrator (use retry tracking)
  - Resume manager (initialize counts)
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

**Test File**: `src/cook/execution/mapreduce/retry_tracking_tests.rs`

```rust
#[test]
fn test_get_item_attempt_number_first_attempt() {
    let retry_counts = HashMap::new();
    assert_eq!(get_item_attempt_number("item-1", &retry_counts), 1);
}

#[test]
fn test_get_item_attempt_number_after_retry() {
    let mut retry_counts = HashMap::new();
    retry_counts.insert("item-1".to_string(), 1);

    assert_eq!(get_item_attempt_number("item-1", &retry_counts), 2);
}

#[test]
fn test_increment_retry_count() {
    let counts = HashMap::new();
    let updated = increment_retry_count("item-1", counts);

    assert_eq!(updated.get("item-1"), Some(&1));

    let updated2 = increment_retry_count("item-1", updated);
    assert_eq!(updated2.get("item-1"), Some(&2));
}

#[test]
fn test_merge_retry_counts_from_failed_and_dlq() {
    let mut failed_items = HashMap::new();
    failed_items.insert(
        "item-1".to_string(),
        FailureRecord { attempts: 2, /* ... */ },
    );

    let dlq_items = vec![
        DeadLetteredItem {
            item_id: "item-2".to_string(),
            failure_count: 3,
            // ...
        },
    ];

    let merged = merge_retry_counts(&failed_items, &dlq_items);

    assert_eq!(merged.get("item-1"), Some(&2));
    assert_eq!(merged.get("item-2"), Some(&3));
}

#[test]
fn test_merge_retry_counts_uses_max() {
    // Item in both failed_agents (2 attempts) and DLQ (3 attempts)
    // Should use max (3)

    let mut failed_items = HashMap::new();
    failed_items.insert(
        "item-1".to_string(),
        FailureRecord { attempts: 2, /* ... */ },
    );

    let dlq_items = vec![
        DeadLetteredItem {
            item_id: "item-1".to_string(),
            failure_count: 3,
            // ...
        },
    ];

    let merged = merge_retry_counts(&failed_items, &dlq_items);
    assert_eq!(merged.get("item-1"), Some(&3), "Should use max count");
}
```

### Integration Tests

**Test File**: `tests/dlq_agent_integration_test.rs` (add to existing)

```rust
#[tokio::test]
async fn test_dlq_failure_history_shows_correct_attempts() {
    // 1. Run job where item fails
    // 2. Verify DLQ has attempt_number: 1
    // 3. Retry item, fails again
    // 4. Verify DLQ has TWO entries with attempt_number: 1 and 2
    // 5. Verify failure_count incremented
}

#[tokio::test]
async fn test_retry_count_persisted_in_checkpoint() {
    // 1. Run job with failure
    // 2. Create checkpoint
    // 3. Load checkpoint
    // 4. Verify item_retry_counts preserved
}

#[tokio::test]
async fn test_backward_compatibility_with_old_checkpoints() {
    // 1. Load checkpoint without item_retry_counts field
    // 2. Verify defaults to empty HashMap
    // 3. Verify resume initializes from failed_agents
}
```

## Documentation Requirements

### Code Documentation

```rust
//! Retry tracking for MapReduce work items.
//!
//! This module provides pure functions to track retry attempts across
//! MapReduce executions, enabling accurate DLQ failure history and
//! retry strategies.
//!
//! **Key Functions**:
//! - `get_item_attempt_number()` - Get current attempt number
//! - `increment_retry_count()` - Update retry count
//! - `merge_retry_counts()` - Combine counts from multiple sources
```

### User Documentation

Update `CLAUDE.md`:

```markdown
## DLQ Retry Tracking

DLQ entries now accurately track retry attempts:

**Attempt Numbers**:
- First failure: `attempt_number: 1`
- First retry failure: `attempt_number: 2`
- Second retry failure: `attempt_number: 3`
- And so on...

**Viewing Retry History**:
```bash
prodigy dlq show <job_id> | jq '.items[].failure_history[].attempt_number'
```

**Retry Count in Failure Details**:
Each DLQ entry shows:
- `failure_count` - Total number of failures
- `failure_history` - Array of attempts with attempt_number
```

## Implementation Notes

### Pure Function Design

All retry tracking logic is pure:
- `get_item_attempt_number()` - No side effects
- `increment_retry_count()` - Returns new HashMap
- `merge_retry_counts()` - Combines inputs, no mutation

### Backward Compatibility Strategy

1. **Checkpoint Migration**: `#[serde(default)]` handles missing field
2. **First Load**: Initialize from `failed_agents.attempts`
3. **Ongoing**: Track normally from that point

### Performance Considerations

- HashMap lookups: O(1) average
- Increment operation: O(1)
- Minimal memory overhead (~24 bytes per tracked item)
- No performance impact on hot path

## Migration and Compatibility

### Breaking Changes

None. This is backward compatible.

### Compatibility Considerations

- Old checkpoints without `item_retry_counts` work fine
- Field defaults to empty HashMap
- Resume initializes from existing failure data
- No data loss during upgrade

### Migration Steps

1. Deploy new code
2. Existing jobs continue working
3. New jobs track retry counts
4. Old checkpoints auto-upgrade on load

### Rollback Plan

If issues arise:
1. Revert code changes
2. Old code ignores `item_retry_counts` field (extra field OK)
3. No data corruption
4. DLQ still works (just with attempt_number: 1)

## Success Metrics

- [ ] All unit tests passing (10+ tests)
- [ ] Integration tests verify correct attempt numbers
- [ ] Backward compatibility verified
- [ ] No performance regression
- [ ] DLQ failure history accurate
- [ ] Code review approved

## Validation Checklist

- [ ] All acceptance criteria met
- [ ] Tests pass on CI
- [ ] Backward compatibility tested with old checkpoints
- [ ] Performance benchmarked (no regression)
- [ ] Documentation updated
- [ ] Code review completed
