---
number: 61
title: Dead Letter Queue Reprocessing Implementation
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 61: Dead Letter Queue Reprocessing Implementation

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The whitepaper prominently features Dead Letter Queue (DLQ) functionality for handling failed MapReduce items:
- Failed items are saved to DLQ for later analysis
- The `prodigy dlq retry workflow-id` command should reprocess failed items
- Currently, while DLQ storage works, the reprocessing command returns "DLQ reprocessing is not yet implemented"

This is a critical gap that prevents recovery from partial failures in large-scale operations, forcing users to restart entire workflows when some items fail.

## Objective

Implement complete DLQ reprocessing functionality to enable recovery and retry of failed MapReduce items with configurable retry strategies and filtering options.

## Requirements

### Functional Requirements
- `prodigy dlq retry <workflow-id>` reprocesses all failed items
- `prodigy dlq retry <workflow-id> --filter <expression>` for selective retry
- `prodigy dlq list <workflow-id>` shows failed items with details
- `prodigy dlq clear <workflow-id>` removes processed items
- Support custom retry strategies per reprocessing run
- Maintain failure history and retry counts
- Generate new job ID for reprocessing runs
- Support merging results with original job

### Non-Functional Requirements
- Efficient handling of large DLQs (10,000+ items)
- Atomic operations to prevent data loss
- Clear audit trail of reprocessing attempts
- Backwards compatible with existing DLQ format

## Acceptance Criteria

- [ ] `prodigy dlq list workflow-123` displays all failed items with error details
- [ ] `prodigy dlq retry workflow-123` successfully reprocesses failed items
- [ ] `--max-retries N` limits retry attempts during reprocessing
- [ ] `--filter "item.priority == 'high'"` processes subset of items
- [ ] Reprocessed items removed from DLQ on success
- [ ] Failed reprocessing adds items back to DLQ with updated metadata
- [ ] Progress tracking shows reprocessing status
- [ ] Original job results merged with reprocessing results
- [ ] `prodigy dlq stats` shows summary across all workflows
- [ ] Concurrent reprocessing runs prevented for same workflow

## Technical Details

### Implementation Approach

1. **DLQ Reprocessing Command**:
   ```rust
   pub struct DlqReprocessor {
       dlq: DeadLetterQueue,
       executor: MapReduceExecutor,
       state_manager: JobStateManager,
   }

   impl DlqReprocessor {
       pub async fn reprocess(
           &self,
           workflow_id: &str,
           options: ReprocessOptions,
       ) -> Result<ReprocessResult> {
           // Load failed items from DLQ
           let items = self.dlq.load_items(workflow_id).await?;

           // Apply filters if specified
           let filtered = self.apply_filters(items, &options.filter)?;

           // Create new job for reprocessing
           let reprocess_job_id = format!("{}-reprocess-{}",
               workflow_id,
               Utc::now().timestamp()
           );

           // Execute with retry strategy
           let results = self.executor.execute_items(
               filtered,
               &reprocess_job_id,
               options.max_retries,
           ).await?;

           // Update DLQ based on results
           self.update_dlq(workflow_id, &results).await?;

           // Merge with original job results
           self.merge_results(workflow_id, &reprocess_job_id).await?;

           Ok(ReprocessResult {
               total_items: filtered.len(),
               successful: results.successful_count(),
               failed: results.failed_count(),
               job_id: reprocess_job_id,
           })
       }
   }
   ```

2. **Enhanced DLQ Item Structure**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct EnhancedDlqItem {
       pub item_id: String,
       pub original_item: Value,
       pub failure_history: Vec<FailureRecord>,
       pub retry_count: u32,
       pub first_failed_at: DateTime<Utc>,
       pub last_failed_at: DateTime<Utc>,
       pub reprocess_attempts: u32,
       pub metadata: HashMap<String, Value>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct FailureRecord {
       pub timestamp: DateTime<Utc>,
       pub error_type: ErrorType,
       pub error_message: String,
       pub job_id: String,
       pub agent_id: Option<String>,
   }
   ```

3. **Filtering Support**:
   ```rust
   pub struct ItemFilter {
       expression: String,
       evaluator: FilterEvaluator,
   }

   impl ItemFilter {
       pub fn apply(&self, items: Vec<DlqItem>) -> Result<Vec<DlqItem>> {
           items.into_iter()
               .filter(|item| self.evaluator.matches(item))
               .collect()
       }
   }
   ```

### Architecture Changes
- Enhance `DeadLetterQueue` with reprocessing methods
- Add `DlqReprocessor` component
- Extend CLI with comprehensive DLQ commands
- Implement filter expression evaluator
- Add job result merging logic

### Data Structures
```yaml
# DLQ reprocessing configuration
reprocess:
  workflow_id: "original-job-123"
  options:
    max_retries: 3
    filter: "item.score >= 5"
    parallel: 10
    timeout_per_item: 300
    strategy: "exponential_backoff"
    merge_results: true
```

## Dependencies

- **Prerequisites**: None (builds on existing DLQ storage)
- **Affected Components**:
  - `src/cook/execution/dlq.rs` - Core DLQ functionality
  - `src/cli/dlq.rs` - CLI commands
  - `src/cook/execution/mapreduce.rs` - Integration with executor
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - DLQ item filtering logic
  - Retry strategy application
  - Result merging algorithms
  - Concurrent access prevention
- **Integration Tests**:
  - Full reprocessing cycle
  - Filter expression evaluation
  - Progress tracking during reprocessing
  - State persistence across runs
- **Failure Tests**:
  - Handling of re-failures during reprocessing
  - Corruption recovery
  - Concurrent reprocessing attempts

## Documentation Requirements

- **Code Documentation**: Document reprocessing algorithm and state machine
- **User Documentation**:
  - DLQ reprocessing guide with examples
  - Filter expression syntax reference
  - Best practices for failure recovery
- **Architecture Updates**: Add DLQ reprocessing flow to architecture docs

## Implementation Notes

- Store reprocessing history for audit trails
- Consider implementing exponential backoff between reprocess attempts
- Support dry-run mode to preview what would be reprocessed
- Implement DLQ compaction for old, resolved items
- Future: Support custom reprocessing strategies via plugins

## Migration and Compatibility

- Existing DLQ files automatically upgraded to new format
- Legacy DLQ items wrapped with default metadata
- No breaking changes to current DLQ storage
- Backwards compatible with workflows using DLQ