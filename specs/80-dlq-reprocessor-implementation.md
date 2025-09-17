---
number: 80
title: DLQ Reprocessor Implementation
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 80: DLQ Reprocessor Implementation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The Dead Letter Queue (DLQ) reprocessor is a critical component for recovering from failures in MapReduce workflows. Currently, the implementation in `src/cook/execution/dlq_reprocessor.rs` contains placeholder code that returns mock success responses without actually processing failed items. This completely breaks the DLQ functionality, leaving users unable to retry failed work items.

The DLQ is essential for production reliability as it allows:
- Recovery from transient failures
- Batch retry of failed work items
- Debugging and analysis of failure patterns
- Resilient MapReduce job execution

## Objective

Complete the implementation of the DLQ reprocessor to enable actual processing of failed work items, providing users with a robust mechanism to recover from failures in MapReduce workflows.

## Requirements

### Functional Requirements

1. **Single Item Processing**
   - Replace mock implementation at lines 444-448 with actual processing logic
   - Execute commands for individual work items
   - Track execution results accurately
   - Update DLQ state based on processing outcome

2. **Batch Processing**
   - Process multiple items with configurable parallelism
   - Stream items to avoid memory issues with large queues
   - Respect max_parallel configuration
   - Maintain progress tracking during batch operations

3. **State Management**
   - Remove successfully processed items from DLQ
   - Retain failed items with updated failure information
   - Preserve correlation IDs for tracking
   - Update checkpoint state for resumability

4. **Statistics and Reporting**
   - Calculate accurate statistics (line 544)
   - Track successful vs failed reprocessing attempts
   - Provide detailed error information for failures
   - Report processing duration and throughput

### Non-Functional Requirements

1. **Performance**
   - Handle large DLQs (10,000+ items) without memory issues
   - Process items in parallel for efficiency
   - Minimize overhead for small queues

2. **Reliability**
   - Ensure atomic updates to DLQ state
   - Support interruption and resumption
   - Maintain data integrity during failures

3. **Observability**
   - Provide clear progress indicators
   - Log detailed information for debugging
   - Emit events for monitoring systems

## Acceptance Criteria

- [ ] Single item processing executes actual commands and returns real results
- [ ] Batch processing successfully handles multiple items in parallel
- [ ] Successfully processed items are removed from the DLQ
- [ ] Failed items remain in DLQ with updated failure information
- [ ] Statistics accurately reflect processing outcomes
- [ ] Large DLQs (10,000+ items) can be processed without memory issues
- [ ] Processing can be interrupted and resumed without data loss
- [ ] All existing DLQ tests pass
- [ ] New integration tests validate end-to-end reprocessing
- [ ] Performance benchmarks show <5% overhead for reprocessing

## Technical Details

### Implementation Approach

1. **Replace Mock Processing**
   ```rust
   // Current (line 444-448)
   // For now, return placeholder success

   // New implementation
   let result = self.executor.execute_work_item(
       &work_item,
       &self.context
   ).await?;
   ```

2. **Implement Statistics Calculation**
   ```rust
   // Current (line 544)
   // For now, return stats for current DLQ

   // New implementation
   let stats = DlqStatistics {
       total_items: dlq.items.len(),
       failed_items: dlq.items.iter().filter(|i| i.failed).count(),
       success_rate: calculate_success_rate(&dlq),
       average_duration: calculate_avg_duration(&dlq),
   };
   ```

3. **Stream Processing for Large Queues**
   ```rust
   use futures::stream::{self, StreamExt};

   let item_stream = stream::iter(dlq.items.into_iter())
       .map(|item| self.process_item(item))
       .buffer_unordered(max_parallel);
   ```

### Architecture Changes

- Integrate with existing `CommandExecutor` for actual command execution
- Use streaming iterators to avoid loading entire DLQ into memory
- Implement proper error handling and recovery logic
- Add metrics collection for monitoring

### Data Structures

```rust
pub struct ProcessingResult {
    pub item_id: String,
    pub success: bool,
    pub error: Option<String>,
    pub duration: Duration,
    pub output: Option<String>,
}

pub struct DlqStatistics {
    pub total_items: usize,
    pub successful: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub average_duration: Duration,
    pub error_categories: HashMap<String, usize>,
}
```

### APIs and Interfaces

No changes to external APIs. Internal implementation changes only.

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `DlqReprocessor` struct
  - `DlqManager` for state updates
  - `CommandExecutor` for work item execution
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test single item processing with various outcomes
  - Validate batch processing with different parallelism levels
  - Test statistics calculation accuracy
  - Verify state updates are correct

- **Integration Tests**:
  - End-to-end test of DLQ reprocessing workflow
  - Test with large DLQs (1000+ items)
  - Validate resumption after interruption
  - Test error handling and recovery

- **Performance Tests**:
  - Benchmark processing throughput
  - Measure memory usage with large queues
  - Validate streaming behavior

- **User Acceptance**:
  - Manual testing of `prodigy dlq reprocess` command
  - Verify progress reporting and statistics
  - Test interruption and resumption scenarios

## Documentation Requirements

- **Code Documentation**:
  - Document all public methods in `DlqReprocessor`
  - Add inline comments for complex logic
  - Include examples in doc comments

- **User Documentation**:
  - Update CLI help for `dlq reprocess` command
  - Add DLQ reprocessing guide to user docs
  - Include troubleshooting section

- **Architecture Updates**:
  - Update ARCHITECTURE.md with DLQ processing details
  - Document state management approach

## Implementation Notes

- Start by implementing single item processing to establish the foundation
- Use existing `CommandExecutor` infrastructure for consistency
- Ensure backward compatibility with existing DLQ files
- Consider adding dry-run mode for testing without execution
- Implement comprehensive logging for debugging production issues

## Migration and Compatibility

- No breaking changes to existing DLQ format
- Existing DLQ files will work with new implementation
- Consider adding version field to DLQ format for future extensions