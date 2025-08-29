---
number: 49
title: MapReduce Persistent State and Checkpointing
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-29
---

# Specification 49: MapReduce Persistent State and Checkpointing

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The current MapReduce implementation in MMM maintains all job state in memory, making it vulnerable to failures and preventing recovery from partial completion. When a MapReduce job fails or is interrupted, all progress is lost and the entire job must be restarted from the beginning. This is particularly problematic for long-running jobs with hundreds or thousands of work items.

## Objective

Implement persistent state management and checkpointing for MapReduce jobs to enable recovery from failures, support job resumption, and provide durability guarantees for distributed execution.

## Requirements

### Functional Requirements
- Persist MapReduce job state to disk at regular checkpoints
- Store completed agent results durably
- Track job progress and status persistently
- Support atomic state updates to prevent corruption
- Enable job recovery from latest checkpoint
- Provide job state querying capabilities
- Maintain backward compatibility with existing workflows

### Non-Functional Requirements
- Checkpoint operations must complete within 100ms
- State files must use atomic write patterns (write-temp-rename)
- Support jobs with up to 10,000 work items
- Minimize storage overhead (< 1MB per 100 agents)
- Ensure thread-safe concurrent access to state

## Acceptance Criteria

- [ ] MapReduceJobState struct implemented with all required fields
- [ ] Checkpoint files written atomically to `.mmm/mapreduce/jobs/{job_id}/`
- [ ] State persisted after each agent completion
- [ ] Job can be resumed from checkpoint after failure
- [ ] Checkpoint versioning prevents data loss
- [ ] State files use efficient JSON serialization
- [ ] Concurrent agent updates handled safely
- [ ] Old checkpoint files cleaned up automatically
- [ ] Job state queryable via new API methods
- [ ] Existing workflows continue to function without modification

## Technical Details

### Implementation Approach

1. **State Structure**
```rust
pub struct MapReduceJobState {
    pub job_id: String,
    pub config: MapReduceConfig,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub work_items: Vec<Value>,
    pub agent_results: HashMap<String, AgentResult>,
    pub completed_agents: HashSet<String>,
    pub failed_agents: HashMap<String, FailureRecord>,
    pub pending_items: Vec<String>,
    pub checkpoint_version: u32,
    pub parent_worktree: Option<String>,
    pub reduce_phase_state: Option<ReducePhaseState>,
}

pub struct FailureRecord {
    pub item_id: String,
    pub attempts: u32,
    pub last_error: String,
    pub last_attempt: DateTime<Utc>,
    pub worktree_info: Option<WorktreeInfo>,
}
```

2. **Checkpoint Manager**
```rust
impl CheckpointManager {
    async fn save_checkpoint(&self, state: &MapReduceJobState) -> Result<()>;
    async fn load_checkpoint(&self, job_id: &str) -> Result<MapReduceJobState>;
    async fn list_checkpoints(&self, job_id: &str) -> Result<Vec<CheckpointInfo>>;
    async fn cleanup_old_checkpoints(&self, job_id: &str, keep: usize) -> Result<()>;
}
```

3. **File Layout**
```
.mmm/mapreduce/
├── jobs/
│   ├── {job_id}/
│   │   ├── checkpoint-v{N}.json       # Latest checkpoint
│   │   ├── checkpoint-v{N-1}.json     # Previous checkpoint
│   │   ├── metadata.json              # Job metadata
│   │   └── agents/                    # Agent-specific data
│   │       ├── {agent_id}/
│   │       │   ├── result.json
│   │       │   └── logs.txt
└── index.json                         # Active jobs index
```

### Architecture Changes
- Add `CheckpointManager` to `MapReduceExecutor`
- Integrate checkpointing into agent completion flow
- Add state recovery in job initialization
- Implement atomic file operations utility

### Data Structures
- Implement custom serialization for large state objects
- Use compression for work items if > 100KB
- Store agent results separately if > 10KB each

### APIs and Interfaces
```rust
pub trait JobStateManager {
    async fn create_job(&self, config: MapReduceConfig) -> Result<String>;
    async fn update_agent_result(&self, job_id: &str, result: AgentResult) -> Result<()>;
    async fn get_job_state(&self, job_id: &str) -> Result<MapReduceJobState>;
    async fn resume_job(&self, job_id: &str) -> Result<Vec<AgentResult>>;
    async fn cleanup_job(&self, job_id: &str) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/orchestrator.rs`
  - `src/cook/session/`
- **External Dependencies**: None (uses existing serde_json, tokio::fs)

## Testing Strategy

- **Unit Tests**: 
  - Test atomic write operations
  - Verify checkpoint serialization/deserialization
  - Test concurrent state updates
  - Validate checkpoint versioning
  
- **Integration Tests**: 
  - Test job recovery after simulated crash
  - Verify state consistency across checkpoints
  - Test cleanup of old checkpoints
  - Validate resume with partial completion
  
- **Performance Tests**: 
  - Benchmark checkpoint write times
  - Test with 1000+ agent states
  - Measure storage overhead
  
- **User Acceptance**: 
  - Resume interrupted MapReduce job
  - Query job progress during execution
  - Verify no data loss on failure

## Documentation Requirements

- **Code Documentation**: 
  - Document checkpoint file format
  - Explain atomic write pattern
  - Document state machine transitions
  
- **User Documentation**: 
  - Add job recovery section to README
  - Document `mmm resume-job` command
  - Explain checkpoint retention policy
  
- **Architecture Updates**: 
  - Update ARCHITECTURE.md with state persistence layer
  - Document checkpoint manager component

## Implementation Notes

- Use tokio::fs for async file operations
- Implement exponential backoff for checkpoint retries
- Consider using bincode for more efficient serialization if JSON becomes too large
- Keep maximum of 3 checkpoints per job by default
- Use file locking to prevent concurrent checkpoint corruption
- Consider memory-mapped files for very large state objects

## Migration and Compatibility

- Existing workflows continue to work without checkpointing
- Checkpointing is opt-in via config flag initially
- No breaking changes to public APIs
- Graceful degradation if checkpoint directory not writable
- Future migration path to mandatory checkpointing