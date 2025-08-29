---
number: 51
title: MapReduce Event Logging and Audit Trail
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-29
---

# Specification 51: MapReduce Event Logging and Audit Trail

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The current MapReduce implementation lacks comprehensive event logging, making it difficult to debug failures, track job progress, and maintain an audit trail of distributed execution. Without structured event logging, operators cannot understand what happened during job execution, especially when failures occur across multiple parallel agents.

## Objective

Implement a structured event logging system for MapReduce jobs that provides complete visibility into job execution, enables debugging of failures, and maintains an audit trail for compliance and analysis.

## Requirements

### Functional Requirements
- Log all significant events during MapReduce execution
- Structure events with consistent schema
- Include correlation IDs for tracing
- Capture event timestamps with microsecond precision
- Support multiple output targets (file, stdout, external systems)
- Enable event filtering and querying
- Maintain event order guarantees
- Provide event replay capability

### Non-Functional Requirements
- Event logging overhead < 1% of execution time
- Support 10,000+ events per second
- Event files rotated at 100MB
- Events persisted durably before acknowledgment
- Zero event loss during normal operation

## Acceptance Criteria

- [ ] MapReduceEvent enum covers all execution events
- [ ] Events written to `.mmm/mapreduce/events/` directory
- [ ] Each event includes job_id, timestamp, and correlation_id
- [ ] JSONL format for efficient streaming
- [ ] Event viewer tool implemented
- [ ] Events searchable by job_id, agent_id, or timestamp
- [ ] Failed agent events include error details
- [ ] Event retention policy configurable
- [ ] Real-time event streaming supported
- [ ] Event aggregation for metrics generation

## Technical Details

### Implementation Approach

1. **Event Types**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum MapReduceEvent {
    // Job lifecycle events
    JobStarted {
        job_id: String,
        config: MapReduceConfig,
        total_items: usize,
        timestamp: DateTime<Utc>,
    },
    JobCompleted {
        job_id: String,
        duration: Duration,
        success_count: usize,
        failure_count: usize,
    },
    JobFailed {
        job_id: String,
        error: String,
        partial_results: usize,
    },
    JobPaused {
        job_id: String,
        checkpoint_version: u32,
    },
    JobResumed {
        job_id: String,
        checkpoint_version: u32,
        pending_items: usize,
    },
    
    // Agent lifecycle events
    AgentStarted {
        job_id: String,
        agent_id: String,
        item_id: String,
        worktree: String,
        attempt: u32,
    },
    AgentProgress {
        job_id: String,
        agent_id: String,
        step: String,
        progress_pct: f32,
    },
    AgentCompleted {
        job_id: String,
        agent_id: String,
        duration: Duration,
        commits: Vec<String>,
    },
    AgentFailed {
        job_id: String,
        agent_id: String,
        error: String,
        retry_eligible: bool,
    },
    AgentRetrying {
        job_id: String,
        agent_id: String,
        attempt: u32,
        backoff_ms: u64,
    },
    
    // Checkpoint events
    CheckpointCreated {
        job_id: String,
        version: u32,
        agents_completed: usize,
    },
    CheckpointLoaded {
        job_id: String,
        version: u32,
    },
    CheckpointFailed {
        job_id: String,
        error: String,
    },
    
    // Worktree events
    WorktreeCreated {
        job_id: String,
        agent_id: String,
        worktree_name: String,
        branch: String,
    },
    WorktreeMerged {
        job_id: String,
        agent_id: String,
        target_branch: String,
    },
    WorktreeCleaned {
        job_id: String,
        agent_id: String,
        worktree_name: String,
    },
    
    // Performance events
    QueueDepthChanged {
        job_id: String,
        pending: usize,
        active: usize,
        completed: usize,
    },
    MemoryPressure {
        job_id: String,
        used_mb: usize,
        limit_mb: usize,
    },
}
```

2. **Event Logger**
```rust
pub struct EventLogger {
    writers: Vec<Box<dyn EventWriter>>,
    buffer: Arc<Mutex<Vec<EventRecord>>>,
    flush_interval: Duration,
}

#[async_trait]
pub trait EventWriter: Send + Sync {
    async fn write(&mut self, events: &[EventRecord]) -> Result<()>;
    async fn flush(&mut self) -> Result<()>;
}

pub struct EventRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: String,
    pub event: MapReduceEvent,
    pub metadata: HashMap<String, Value>,
}

impl EventLogger {
    pub async fn log(&self, event: MapReduceEvent) -> Result<()> {
        let record = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: self.current_correlation_id(),
            event,
            metadata: self.collect_metadata(),
        };
        
        self.buffer.lock().await.push(record);
        
        if self.should_flush() {
            self.flush().await?;
        }
        
        Ok(())
    }
}
```

3. **File Layout**
```
.mmm/mapreduce/events/
├── {job_id}/
│   ├── events-{timestamp}.jsonl     # Primary event log
│   ├── events-{timestamp}.jsonl.gz  # Compressed archives
│   └── index.json                   # Event index for quick lookup
├── global/
│   ├── events-{date}.jsonl         # Global event stream
│   └── metrics-{date}.json         # Aggregated metrics
```

### Architecture Changes
- Add `EventLogger` to `MapReduceExecutor`
- Instrument all state transitions
- Add event correlation through execution
- Implement event replay system

### Data Structures
```rust
pub struct EventIndex {
    pub job_id: String,
    pub event_counts: HashMap<String, usize>,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub file_offsets: Vec<FileOffset>,
}

pub struct FileOffset {
    pub file_path: PathBuf,
    pub byte_offset: u64,
    pub line_number: usize,
    pub event_id: Uuid,
}
```

### APIs and Interfaces
```rust
pub trait EventStore {
    async fn append(&self, event: MapReduceEvent) -> Result<()>;
    async fn query(&self, filter: EventFilter) -> Result<Vec<EventRecord>>;
    async fn replay(&self, job_id: &str, handler: EventHandler) -> Result<()>;
    async fn aggregate(&self, job_id: &str) -> Result<EventStats>;
}

pub struct EventFilter {
    pub job_id: Option<String>,
    pub event_types: Vec<String>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub correlation_id: Option<String>,
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/workflow/executor.rs`
  - All agent execution paths
- **External Dependencies**: 
  - `uuid` crate for event IDs
  - Existing `chrono` for timestamps

## Testing Strategy

- **Unit Tests**: 
  - Test event serialization/deserialization
  - Verify event buffer management
  - Test file rotation logic
  - Validate event ordering
  
- **Integration Tests**: 
  - Test complete job event sequence
  - Verify event persistence
  - Test event replay functionality
  - Validate concurrent event writing
  
- **Performance Tests**: 
  - Benchmark event logging overhead
  - Test with 10,000 events/second
  - Measure file I/O impact
  
- **User Acceptance**: 
  - View job execution timeline
  - Debug failed agents via events
  - Generate execution reports

## Documentation Requirements

- **Code Documentation**: 
  - Document event schema
  - Explain correlation ID propagation
  - Document event writer interface
  
- **User Documentation**: 
  - Add event viewer CLI docs
  - Create event analysis guide
  - Document event retention
  
- **Architecture Updates**: 
  - Add event flow diagram
  - Document event store design

## Implementation Notes

- Use buffered writes for efficiency
- Implement async event flushing
- Consider using Apache Arrow for event storage
- Add event sampling for high-volume scenarios
- Implement event deduplication
- Use structured logging via tracing crate
- Consider event sourcing for state reconstruction

## Migration and Compatibility

- Events are additive - no breaking changes
- Optional event logging initially
- Gradual rollout via feature flag
- Backward compatible event schema
- Clear migration path for existing jobs