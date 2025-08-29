---
number: 52
title: MapReduce Dead Letter Queue
category: parallel
priority: high
status: draft
dependencies: [49, 51]
created: 2025-01-29
---

# Specification 52: MapReduce Dead Letter Queue

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [49 - Persistent State, 51 - Event Logging]

## Context

When MapReduce agents fail repeatedly, the current implementation either retries indefinitely or abandons the work item entirely. This leads to either infinite loops or data loss. A Dead Letter Queue (DLQ) pattern is needed to capture persistently failing items for later analysis and potential manual intervention, while allowing the job to continue processing other items.

## Objective

Implement a Dead Letter Queue system for MapReduce jobs that captures items that fail beyond the retry threshold, provides mechanisms for analyzing failures, and enables reprocessing of dead-lettered items after issues are resolved.

## Requirements

### Functional Requirements
- Capture items that exceed retry threshold
- Store failure context and history
- Support manual inspection of failed items
- Enable reprocessing from DLQ
- Provide failure analysis tools
- Group similar failures for pattern detection
- Support DLQ size limits and policies
- Enable selective reprocessing

### Non-Functional Requirements
- DLQ operations must not block main execution
- Support up to 10,000 items in DLQ
- Persist DLQ across job restarts
- Query DLQ items in < 100ms
- Maintain full failure history per item

## Acceptance Criteria

- [ ] Items moved to DLQ after max retries exceeded
- [ ] DLQ persisted to `.mmm/mapreduce/dlq/` directory
- [ ] Failed items include complete error history
- [ ] `mmm dlq list` command shows dead-lettered items
- [ ] `mmm dlq inspect <item-id>` shows failure details
- [ ] `mmm dlq reprocess` moves items back to queue
- [ ] Similar failures grouped automatically
- [ ] DLQ metrics included in job summary
- [ ] DLQ size limits enforced with FIFO eviction
- [ ] Integration with event logging system

## Technical Details

### Implementation Approach

1. **DLQ Structure**
```rust
pub struct DeadLetterQueue {
    job_id: String,
    items: Arc<RwLock<HashMap<String, DeadLetteredItem>>>,
    storage: Arc<DLQStorage>,
    max_items: usize,
    retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetteredItem {
    pub item_id: String,
    pub item_data: Value,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: DateTime<Utc>,
    pub failure_count: u32,
    pub failure_history: Vec<FailureDetail>,
    pub error_signature: String,
    pub worktree_artifacts: Option<WorktreeArtifacts>,
    pub reprocess_eligible: bool,
    pub manual_review_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDetail {
    pub attempt_number: u32,
    pub timestamp: DateTime<Utc>,
    pub error_type: ErrorType,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub agent_id: String,
    pub step_failed: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorType {
    Timeout,
    CommandFailed { exit_code: i32 },
    WorktreeError,
    MergeConflict,
    ValidationFailed,
    ResourceExhausted,
    Unknown,
}
```

2. **DLQ Operations**
```rust
impl DeadLetterQueue {
    pub async fn add(&self, item: DeadLetteredItem) -> Result<()> {
        // Check capacity
        if self.items.read().await.len() >= self.max_items {
            self.evict_oldest().await?;
        }
        
        // Store to disk first
        self.storage.persist(&item).await?;
        
        // Update in-memory cache
        self.items.write().await.insert(item.item_id.clone(), item.clone());
        
        // Log event
        self.log_dlq_event(DLQEvent::ItemAdded { item }).await?;
        
        Ok(())
    }
    
    pub async fn reprocess(&self, item_ids: Vec<String>) -> Result<Vec<Value>> {
        let mut reprocessable = Vec::new();
        
        for item_id in item_ids {
            if let Some(item) = self.items.read().await.get(&item_id) {
                if item.reprocess_eligible {
                    reprocessable.push(item.item_data.clone());
                    self.remove(&item_id).await?;
                }
            }
        }
        
        Ok(reprocessable)
    }
    
    pub async fn analyze_patterns(&self) -> Result<FailureAnalysis> {
        let items = self.items.read().await;
        
        // Group by error signature
        let mut patterns: HashMap<String, Vec<&DeadLetteredItem>> = HashMap::new();
        for item in items.values() {
            patterns.entry(item.error_signature.clone())
                .or_default()
                .push(item);
        }
        
        // Build analysis
        Ok(FailureAnalysis {
            total_items: items.len(),
            pattern_groups: patterns.into_iter().map(|(sig, items)| {
                PatternGroup {
                    signature: sig,
                    count: items.len(),
                    first_occurrence: items.iter().map(|i| i.first_attempt).min(),
                    last_occurrence: items.iter().map(|i| i.last_attempt).max(),
                    sample_items: items.into_iter().take(3).cloned().collect(),
                }
            }).collect(),
        })
    }
}
```

3. **Storage Layout**
```
.mmm/mapreduce/dlq/
├── {job_id}/
│   ├── items/
│   │   ├── {item_id}.json          # Individual item details
│   │   └── ...
│   ├── index.json                  # DLQ index
│   ├── patterns.json               # Failure pattern analysis
│   └── metrics.json                # DLQ statistics
├── global/
│   ├── all-items.jsonl            # Global DLQ stream
│   └── retention-policy.json      # Retention settings
```

### Architecture Changes
- Add `DeadLetterQueue` to `MapReduceExecutor`
- Integrate DLQ checks in retry logic
- Add DLQ management commands to CLI
- Implement background cleanup task

### Data Structures
```rust
pub struct DLQStorage {
    base_path: PathBuf,
    compression: bool,
}

pub struct FailureAnalysis {
    pub total_items: usize,
    pub pattern_groups: Vec<PatternGroup>,
    pub error_distribution: HashMap<ErrorType, usize>,
    pub temporal_distribution: Vec<(DateTime<Utc>, usize)>,
}

pub struct ReprocessRequest {
    pub item_ids: Vec<String>,
    pub max_retries: u32,
    pub delay_ms: u64,
    pub force: bool,
}
```

### APIs and Interfaces
```rust
#[async_trait]
pub trait DLQManager {
    async fn add_failed_item(&self, item: WorkItem, failure: FailureDetail) -> Result<()>;
    async fn list_items(&self, filter: DLQFilter) -> Result<Vec<DeadLetteredItem>>;
    async fn get_item(&self, item_id: &str) -> Result<Option<DeadLetteredItem>>;
    async fn reprocess_items(&self, request: ReprocessRequest) -> Result<usize>;
    async fn purge_old_items(&self, older_than: DateTime<Utc>) -> Result<usize>;
    async fn export_items(&self, path: &Path) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: 
  - [49 - Persistent State and Checkpointing]
  - [51 - Event Logging]
- **Affected Components**: 
  - `src/cook/execution/mapreduce.rs`
  - `src/main.rs` (CLI commands)
  - Agent retry logic
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Test DLQ addition and eviction
  - Verify failure pattern detection
  - Test reprocessing logic
  - Validate storage operations
  
- **Integration Tests**: 
  - Test DLQ with actual failures
  - Verify persistence across restarts
  - Test capacity limits
  - Validate reprocessing flow
  
- **Performance Tests**: 
  - Test with 1000+ DLQ items
  - Measure query performance
  - Test concurrent DLQ operations
  
- **User Acceptance**: 
  - View and analyze failed items
  - Reprocess items from DLQ
  - Export DLQ for analysis

## Documentation Requirements

- **Code Documentation**: 
  - Document DLQ policies
  - Explain failure pattern detection
  - Document reprocessing criteria
  
- **User Documentation**: 
  - Add DLQ management guide
  - Document CLI commands
  - Create troubleshooting guide
  
- **Architecture Updates**: 
  - Add DLQ component diagram
  - Document failure flow

## Implementation Notes

- Use bloom filters for duplicate detection
- Implement exponential backoff for reprocessing
- Consider circuit breaker for toxic items
- Add metrics for DLQ health monitoring
- Implement automatic pattern detection
- Use error signatures for grouping
- Consider S3/blob storage for large DLQs

## Migration and Compatibility

- DLQ is optional and backward compatible
- Existing jobs continue without DLQ
- Gradual adoption via configuration
- No breaking changes to APIs
- Clear upgrade path documented