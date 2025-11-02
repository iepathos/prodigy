---
number: 139
title: Work Item Deduplication in MapReduce Resume
category: foundation
priority: critical
status: draft
dependencies: [138]
created: 2025-01-11
---

# Specification 139: Work Item Deduplication in MapReduce Resume

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [138]

## Context

When resuming a MapReduce job, work items can be added to the processing queue from multiple sources:
1. `state.pending_items` - Items not yet started
2. `state.failed_agents` - Items that failed and may be retried
3. DLQ items - Items in the Dead Letter Queue (after Spec 138)

Currently, `calculate_remaining_items()` in `mapreduce_resume.rs:356-389` doesn't deduplicate items across these sources. This causes:

- **Duplicate Processing**: Same item processed multiple times
- **Wasted Resources**: Redundant agent execution
- **State Corruption**: Multiple competing results for same item
- **Confusing Semantics**: Unclear which source takes precedence

The `reset_failed_agents` and `include_dlq_items` options can add the same item twice if it appears in both `failed_agents` and DLQ.

## Objective

Implement functional, pure deduplication logic to ensure each work item appears exactly once in the resume queue, regardless of source.

## Requirements

### Functional Requirements

- **FR1**: Deduplicate work items by item ID before processing
- **FR2**: Preserve work item order (pending → failed → DLQ)
- **FR3**: Keep first occurrence of duplicate items (stable deduplication)
- **FR4**: Pure function for deduplication (no I/O, fully testable)
- **FR5**: Log warning when duplicates are detected
- **FR6**: Emit metric for duplicate count (observability)

### Non-Functional Requirements

- **NFR1**: O(n) time complexity for deduplication (using HashSet)
- **NFR2**: No mutations of input collections
- **NFR3**: Functional composition with helper functions
- **NFR4**: Clear separation: collection → deduplication → result
- **NFR5**: Performance: <10ms for 10,000 items

## Acceptance Criteria

- [ ] Pure function `deduplicate_work_items()` implemented
- [ ] Function uses HashSet for O(n) deduplication
- [ ] Stable deduplication (first occurrence preserved)
- [ ] Unit test: Empty list returns empty list
- [ ] Unit test: List with no duplicates unchanged
- [ ] Unit test: List with duplicates removes later occurrences
- [ ] Unit test: Order preserved for unique items
- [ ] Unit test: Large dataset (10K items) completes quickly
- [ ] Integration test: Resume with overlapping sources deduplicates
- [ ] Integration test: Item from pending takes precedence over failed
- [ ] Integration test: No duplicate agent execution
- [ ] Refactored `calculate_remaining_items()` uses deduplication
- [ ] All existing tests pass without modification
- [ ] No unwrap() or panic!() in production code

## Technical Details

### Implementation Approach

**Step 1: Pure Deduplication Function**

Create pure function in new module:

```rust
// src/cook/execution/mapreduce/resume_deduplication.rs

use serde_json::Value;
use std::collections::HashSet;

/// Deduplicate work items by ID, keeping first occurrence of each unique ID.
///
/// Uses a HashSet for O(n) time complexity. Preserves order of first occurrences.
///
/// # Arguments
/// * `items` - Work items to deduplicate
///
/// # Returns
/// Deduplicated list with first occurrence of each unique item ID
///
/// # Examples
/// ```
/// let items = vec![
///     json!({"id": "1", "data": "a"}),
///     json!({"id": "2", "data": "b"}),
///     json!({"id": "1", "data": "c"}),  // Duplicate of first
/// ];
/// let deduped = deduplicate_work_items(items);
/// assert_eq!(deduped.len(), 2);
/// ```
pub fn deduplicate_work_items(items: Vec<Value>) -> Vec<Value> {
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut deduped = Vec::new();

    for item in items {
        let item_id = extract_item_id(&item);

        // Skip items without ID or with duplicate ID
        if !item_id.is_empty() && seen_ids.insert(item_id) {
            deduped.push(item);
        }
    }

    deduped
}

/// Extract item ID from work item JSON.
///
/// Tries multiple field names for compatibility.
///
/// # Arguments
/// * `item` - Work item JSON
///
/// # Returns
/// Item ID string, or empty string if not found
fn extract_item_id(item: &Value) -> String {
    // Try common ID field names
    item.get("id")
        .or_else(|| item.get("item_id"))
        .or_else(|| item.get("_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Count duplicate items in a list.
///
/// Pure function for observability metrics.
///
/// # Arguments
/// * `items` - Work items to analyze
///
/// # Returns
/// Number of duplicate items found
pub fn count_duplicates(items: &[Value]) -> usize {
    let total = items.len();
    let unique = deduplicate_work_items(items.to_vec()).len();
    total.saturating_sub(unique)
}
```

**Step 2: Helper Collection Functions**

Refactor existing collection logic into pure functions:

```rust
// src/cook/execution/mapreduce/resume_collection.rs

use super::state::MapReduceJobState;
use serde_json::Value;

/// Collect pending items from job state.
///
/// Pure function - no state mutation.
pub fn collect_pending_items(state: &MapReduceJobState) -> Vec<Value> {
    state
        .pending_items
        .iter()
        .filter_map(|item_id| state.find_work_item(item_id))
        .collect()
}

/// Collect failed items eligible for retry.
///
/// Pure function - no state mutation.
///
/// # Arguments
/// * `state` - Job state
/// * `max_retries` - Maximum retry attempts
pub fn collect_failed_items(
    state: &MapReduceJobState,
    max_retries: u32,
) -> Vec<Value> {
    state
        .failed_agents
        .iter()
        .filter(|(_, failure)| failure.attempts < max_retries)
        .filter_map(|(item_id, _)| state.find_work_item(item_id))
        .collect()
}

/// Combine items from multiple sources in priority order.
///
/// Pure function - functional composition.
///
/// Priority: pending → failed → dlq
pub fn combine_work_items(
    pending: Vec<Value>,
    failed: Vec<Value>,
    dlq: Vec<Value>,
) -> Vec<Value> {
    pending
        .into_iter()
        .chain(failed)
        .chain(dlq)
        .collect()
}
```

**Step 3: Refactor Resume Logic**

Update `mapreduce_resume.rs` to use pure functions:

```rust
// src/cook/execution/mapreduce/mapreduce_resume.rs

use super::resume_deduplication::{deduplicate_work_items, count_duplicates};
use super::resume_collection::{
    collect_pending_items, collect_failed_items, combine_work_items
};

async fn calculate_remaining_items(
    &self,
    state: &mut MapReduceJobState,
    options: &EnhancedResumeOptions,
) -> MRResult<Vec<Value>> {
    // Collect from all sources (pure functions)
    let pending = collect_pending_items(state);

    let failed = if options.reset_failed_agents {
        collect_failed_items(state, options.max_additional_retries)
    } else {
        Vec::new()
    };

    let dlq = if options.include_dlq_items {
        self.load_dlq_items(&state.job_id).await?
    } else {
        Vec::new()
    };

    // Combine in priority order
    let combined = combine_work_items(pending, failed, dlq);

    // Check for duplicates before deduplication (observability)
    let duplicate_count = count_duplicates(&combined);
    if duplicate_count > 0 {
        warn!(
            "Found {} duplicate work items across resume sources (pending: {}, failed: {}, dlq: {})",
            duplicate_count,
            pending.len(),
            failed.len(),
            dlq.len()
        );

        // Emit metric
        metrics::counter!("resume.work_items.duplicates", duplicate_count as u64);
    }

    // Deduplicate (pure function)
    let deduped = deduplicate_work_items(combined);

    info!(
        "Resume work items: {} total, {} unique after deduplication",
        pending.len() + failed.len() + dlq.len(),
        deduped.len()
    );

    Ok(deduped)
}
```

### Architecture Changes

**New Modules**:
- `src/cook/execution/mapreduce/resume_deduplication.rs` - Pure deduplication logic
- `src/cook/execution/mapreduce/resume_collection.rs` - Pure collection helpers

**Modified Modules**:
- `src/cook/execution/mapreduce/mapreduce_resume.rs` - Use pure functions

**Benefits**:
- Clear separation of concerns
- All logic is testable in isolation
- No I/O in pure functions
- Functional composition

### Data Structures

No new data structures. Uses:
- `Vec<Value>` - Work item lists
- `HashSet<String>` - For deduplication
- `MapReduceJobState` - Read-only access

### APIs and Interfaces

**New Public Functions**:

```rust
pub fn deduplicate_work_items(items: Vec<Value>) -> Vec<Value>
pub fn count_duplicates(items: &[Value]) -> usize
pub fn collect_pending_items(state: &MapReduceJobState) -> Vec<Value>
pub fn collect_failed_items(state: &MapReduceJobState, max_retries: u32) -> Vec<Value>
pub fn combine_work_items(pending: Vec<Value>, failed: Vec<Value>, dlq: Vec<Value>) -> Vec<Value>
```

**Modified Behavior**:
- `calculate_remaining_items()` now deduplicates all sources
- Resume with multiple sources enabled won't process duplicates
- Warning logged when duplicates detected

## Dependencies

- **Prerequisites**: [138] DLQ Integration (for dlq items source)
- **Affected Components**:
  - MapReduce resume manager
  - Work item collection logic
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

**Test File**: `src/cook/execution/mapreduce/resume_deduplication_tests.rs`

```rust
#[test]
fn test_deduplicate_empty_list() {
    let items: Vec<Value> = vec![];
    let result = deduplicate_work_items(items);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_deduplicate_no_duplicates() {
    let items = vec![
        json!({"id": "1", "data": "a"}),
        json!({"id": "2", "data": "b"}),
        json!({"id": "3", "data": "c"}),
    ];
    let result = deduplicate_work_items(items.clone());
    assert_eq!(result.len(), 3);
    assert_eq!(result, items);
}

#[test]
fn test_deduplicate_with_duplicates() {
    let items = vec![
        json!({"id": "1", "data": "first"}),
        json!({"id": "2", "data": "second"}),
        json!({"id": "1", "data": "duplicate"}),  // Should be removed
        json!({"id": "3", "data": "third"}),
    ];
    let result = deduplicate_work_items(items);

    assert_eq!(result.len(), 3);
    assert_eq!(result[0]["id"], "1");
    assert_eq!(result[0]["data"], "first");  // First occurrence kept
    assert_eq!(result[1]["id"], "2");
    assert_eq!(result[2]["id"], "3");
}

#[test]
fn test_deduplicate_preserves_order() {
    let items = vec![
        json!({"id": "3", "data": "third"}),
        json!({"id": "1", "data": "first"}),
        json!({"id": "2", "data": "second"}),
    ];
    let result = deduplicate_work_items(items.clone());

    assert_eq!(result.len(), 3);
    assert_eq!(result[0]["id"], "3");
    assert_eq!(result[1]["id"], "1");
    assert_eq!(result[2]["id"], "2");
}

#[test]
fn test_deduplicate_missing_ids_skipped() {
    let items = vec![
        json!({"id": "1", "data": "a"}),
        json!({"data": "no_id"}),  // No ID field
        json!({"id": "2", "data": "b"}),
    ];
    let result = deduplicate_work_items(items);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0]["id"], "1");
    assert_eq!(result[1]["id"], "2");
}

#[test]
fn test_deduplicate_large_dataset() {
    use std::time::Instant;

    // Create 10,000 items with 50% duplicates
    let mut items = Vec::new();
    for i in 0..5000 {
        items.push(json!({"id": i.to_string(), "data": "test"}));
        items.push(json!({"id": i.to_string(), "data": "duplicate"}));
    }

    let start = Instant::now();
    let result = deduplicate_work_items(items);
    let duration = start.elapsed();

    assert_eq!(result.len(), 5000);
    assert!(duration.as_millis() < 10, "Should complete in <10ms");
}

#[test]
fn test_count_duplicates() {
    let items = vec![
        json!({"id": "1"}),
        json!({"id": "2"}),
        json!({"id": "1"}),  // Duplicate
        json!({"id": "3"}),
        json!({"id": "2"}),  // Duplicate
    ];

    assert_eq!(count_duplicates(&items), 2);
}

#[test]
fn test_collect_pending_items() {
    let state = create_test_state_with_pending(vec!["item-1", "item-2"]);
    let pending = collect_pending_items(&state);

    assert_eq!(pending.len(), 2);
}

#[test]
fn test_collect_failed_items_respects_max_retries() {
    let mut state = create_test_state();
    state.failed_agents.insert("item-1".to_string(), Failure { attempts: 1 });
    state.failed_agents.insert("item-2".to_string(), Failure { attempts: 5 });

    let failed = collect_failed_items(&state, 3);

    assert_eq!(failed.len(), 1);  // Only item-1 (attempts < 3)
}

#[test]
fn test_combine_work_items_preserves_priority() {
    let pending = vec![json!({"id": "p1"})];
    let failed = vec![json!({"id": "f1"})];
    let dlq = vec![json!({"id": "d1"})];

    let combined = combine_work_items(pending, failed, dlq);

    assert_eq!(combined.len(), 3);
    assert_eq!(combined[0]["id"], "p1");  // Pending first
    assert_eq!(combined[1]["id"], "f1");  // Failed second
    assert_eq!(combined[2]["id"], "d1");  // DLQ last
}
```

### Integration Tests

**Test File**: `tests/resume_deduplication_integration_test.rs`

```rust
#[tokio::test]
async fn test_resume_deduplicates_overlapping_sources() {
    // Create job state with same item in pending AND failed_agents
    // Resume with reset_failed_agents: true
    // Verify item processed only once
}

#[tokio::test]
async fn test_resume_pending_takes_precedence() {
    // Item exists in both pending and DLQ
    // Resume
    // Verify pending version is used (first occurrence)
}

#[tokio::test]
async fn test_resume_logs_duplicate_warning() {
    // Create state with duplicates
    // Capture logs
    // Verify warning about duplicates
}

#[tokio::test]
async fn test_no_duplicate_agent_execution() {
    // Resume with overlapping sources
    // Track agent executions
    // Verify each item ID executed exactly once
}
```

## Documentation Requirements

### Code Documentation

- Module-level docs for `resume_deduplication.rs`
- Inline examples for `deduplicate_work_items()`
- Performance characteristics documented

### User Documentation

Update `CLAUDE.md`:

```markdown
## Work Item Deduplication on Resume

When resuming MapReduce jobs, work items are automatically deduplicated across sources:

**Sources (in priority order):**
1. Pending items (not yet started)
2. Failed items (if `reset_failed_agents: true`)
3. DLQ items (if `include_dlq_items: true`)

**Deduplication Behavior:**
- Items matched by ID field
- First occurrence kept, duplicates removed
- Priority order preserved (pending → failed → DLQ)
- Warning logged if duplicates detected

**Example:**
```bash
# Resume with multiple sources
prodigy resume <job_id>

# Log output:
# WARN: Found 5 duplicate work items across resume sources
# INFO: Resume work items: 105 total, 100 unique after deduplication
```
```

## Implementation Notes

### Performance Characteristics

- **Time Complexity**: O(n) using HashSet
- **Space Complexity**: O(n) for HashSet
- **Benchmark Target**: <10ms for 10,000 items

### Testing Checklist

- [ ] Empty list
- [ ] No duplicates
- [ ] With duplicates
- [ ] Order preservation
- [ ] Missing IDs
- [ ] Large dataset performance
- [ ] Integration with resume

### Gotchas

- **ID Field Names**: Try `id`, `item_id`, `_id` for compatibility
- **Empty IDs**: Skip items without valid ID
- **First Occurrence**: Stable sort - earlier sources take precedence
- **Logging**: Only warn on duplicates, don't fail

## Migration and Compatibility

### Breaking Changes

None. This is a bug fix that prevents duplicate processing.

### Compatibility Considerations

- Existing resume behavior enhanced
- No API changes
- Backward compatible with all workflows

### Migration Steps

1. Deploy new code
2. No data migration required
3. Resume operations automatically use deduplication
4. Existing checkpoints remain valid

### Rollback Plan

If issues arise:
1. Revert to original `calculate_remaining_items()`
2. No data corruption (pure functions)
3. May process duplicates again (original behavior)
