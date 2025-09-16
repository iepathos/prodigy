---
number: 58
title: DLQ Reprocessing Implementation
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-15
---

# Specification 58: DLQ Reprocessing Implementation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The Dead Letter Queue (DLQ) system is fully implemented for capturing failed MapReduce work items, but the critical capability to reprocess these failed items is missing. Currently, the `prodigy dlq retry` command exists but returns "DLQ reprocessing is not yet implemented", leaving failed items stranded without an automated recovery mechanism. This severely limits the production readiness of the MapReduce system.

## Objective

Implement complete DLQ reprocessing functionality to enable automatic retry of failed MapReduce work items with configurable strategies, filtering, and parallel execution support.

## Requirements

### Functional Requirements

1. **Core Reprocessing Engine**
   - Complete implementation of `DlqReprocessor::reprocess_items()` method
   - Support for selective item reprocessing based on filters
   - Parallel reprocessing of multiple failed items
   - Maintain item processing history and retry counts

2. **Retry Strategies**
   - Exponential backoff with configurable base and max delay
   - Linear retry with fixed delays
   - Custom retry strategies via configuration
   - Per-item retry limit tracking

3. **Filtering Capabilities**
   - Filter by error type (timeout, validation, command failure)
   - Filter by date range (items failed within time window)
   - Filter by item properties using JSONPath expressions
   - Filter by failure count thresholds

4. **Workflow Generation**
   - Generate new MapReduce workflow from DLQ items
   - Preserve original agent templates and configurations
   - Apply modifications for retry (reduced parallelism, increased timeouts)
   - Support for partial reprocessing workflows

5. **Progress Tracking**
   - Real-time progress display during reprocessing
   - Success/failure metrics per retry attempt
   - Detailed logs for each reprocessed item
   - Summary report after completion

### Non-Functional Requirements

1. **Performance**
   - Process DLQ items without loading entire queue into memory
   - Stream processing for large DLQ queues (>10,000 items)
   - Efficient parallel execution with resource limits

2. **Reliability**
   - Atomic updates to DLQ state
   - Crash recovery for interrupted reprocessing
   - Prevent duplicate reprocessing of items

3. **Observability**
   - Detailed event logging for audit trail
   - Metrics on reprocessing success rates
   - Integration with existing event system

## Acceptance Criteria

- [ ] `prodigy dlq retry <workflow-id>` successfully reprocesses all eligible items
- [ ] Failed items are retried with exponential backoff by default
- [ ] `--filter` flag allows selective reprocessing (e.g., `--filter "error_type=timeout"`)
- [ ] `--parallel` flag controls concurrent reprocessing (default: 5)
- [ ] `--max-retries` flag limits retry attempts per item
- [ ] Successfully reprocessed items are removed from DLQ
- [ ] Items that fail after max retries remain in DLQ with updated metadata
- [ ] Progress bar shows real-time reprocessing status
- [ ] Summary report displays success/failure counts and patterns
- [ ] Reprocessing can be interrupted and resumed
- [ ] Event logs capture complete reprocessing history

## Technical Details

### Implementation Approach

```rust
// Complete the DlqReprocessor implementation
impl DlqReprocessor {
    pub async fn reprocess_items(&self, options: ReprocessOptions) -> Result<ReprocessResult> {
        // 1. Load and filter DLQ items
        let items = self.load_filtered_items(&options.filter).await?;

        // 2. Create reprocessing workflow
        let workflow = self.generate_retry_workflow(items, &options)?;

        // 3. Initialize progress tracking
        let progress = ProgressTracker::new(items.len());

        // 4. Execute parallel reprocessing
        let results = self.execute_parallel_retry(workflow, &progress).await?;

        // 5. Update DLQ state
        self.update_dlq_state(results).await?;

        // 6. Generate summary report
        Ok(self.generate_report(results))
    }
}
```

### Architecture Changes

1. **DLQ Storage Enhancement**
   - Add index for efficient filtering
   - Implement streaming API for large queues
   - Add versioning for state updates

2. **Workflow Generation Module**
   - Template-based workflow generation
   - Dynamic configuration adjustment
   - Validation of generated workflows

3. **Progress Integration**
   - Extend ProgressTracker for DLQ operations
   - Add DLQ-specific progress events
   - Real-time UI updates

### Data Structures

```rust
pub struct ReprocessOptions {
    pub max_retries: u32,
    pub filter: Option<DlqFilter>,
    pub parallel: usize,
    pub timeout_per_item: u64,
    pub retry_strategy: RetryStrategy,
    pub force: bool,
}

pub struct DlqFilter {
    pub error_types: Option<Vec<ErrorType>>,
    pub date_range: Option<DateRange>,
    pub item_filter: Option<String>, // JSONPath expression
    pub max_failure_count: Option<u32>,
}

pub struct ReprocessResult {
    pub total_items: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration: Duration,
    pub error_patterns: HashMap<String, usize>,
}
```

## Dependencies

- **Prerequisites**: None (DLQ system already implemented)
- **Affected Components**:
  - `src/cook/execution/dlq_reprocessor.rs`
  - `src/cook/execution/dlq.rs`
  - `src/main.rs` (CLI command handlers)
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Filter expression evaluation
  - Workflow generation from DLQ items
  - Retry strategy implementations

- **Integration Tests**:
  - End-to-end DLQ retry with mock failures
  - Parallel reprocessing with resource limits
  - Interrupted and resumed reprocessing

- **Performance Tests**:
  - Large DLQ queue processing (10,000+ items)
  - Memory usage during streaming
  - Parallel execution scaling

- **User Acceptance**:
  - Manual testing of various failure scenarios
  - Verification of progress display and reporting
  - Testing filter combinations

## Documentation Requirements

- **Code Documentation**:
  - Complete rustdoc for all public APIs
  - Examples in DlqReprocessor documentation

- **User Documentation**:
  - Update CLAUDE.md with DLQ retry examples
  - Add troubleshooting guide for common patterns
  - Document filter syntax and expressions

- **Architecture Updates**:
  - Update whitepaper with implemented retry strategies
  - Document DLQ lifecycle and state transitions

## Implementation Notes

1. **Backward Compatibility**: Ensure existing DLQ data can be reprocessed
2. **Resource Management**: Implement semaphore-based limiting for parallel execution
3. **Error Handling**: Distinguish between transient and permanent failures
4. **Monitoring**: Add metrics for DLQ queue depth and reprocessing rates
5. **Safety**: Prevent infinite retry loops with circuit breaker pattern

## Migration and Compatibility

- No breaking changes to existing DLQ format
- Automatic migration of old DLQ items to support new metadata
- Graceful handling of DLQ items from previous versions
- Optional cleanup of successfully reprocessed items