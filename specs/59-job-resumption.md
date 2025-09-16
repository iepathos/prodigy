---
number: 59
title: MapReduce Job Resumption Capability
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-15
---

# Specification 59: MapReduce Job Resumption Capability

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Long-running MapReduce workflows can be interrupted by system failures, network issues, or manual stops. Currently, Prodigy saves checkpoint data and maintains job state, but the `prodigy resume-job` command only displays status without actually resuming execution. This forces users to restart failed workflows from the beginning, wasting compute resources and time on already-processed items.

## Objective

Implement complete job resumption capability that can recover MapReduce workflows from their last checkpoint, skip already-processed items, and continue execution from the point of interruption.

## Requirements

### Functional Requirements

1. **Checkpoint Recovery**
   - Load job state from checkpoint files
   - Reconstruct workflow configuration and context
   - Identify processed vs. unprocessed items
   - Restore variable state and intermediate results

2. **State Reconstruction**
   - Rebuild work item queue from checkpoint
   - Restore map phase progress and results
   - Recover reduce phase state if applicable
   - Maintain execution order guarantees

3. **Smart Resume Logic**
   - Skip successfully processed items
   - Retry items that were in-progress during interruption
   - Handle partial results from interrupted agents
   - Merge previous results with new execution

4. **Resume Strategies**
   - Resume from last checkpoint (default)
   - Resume from specific checkpoint by timestamp
   - Force re-execution of failed items only
   - Resume with modified configuration (timeouts, parallelism)

5. **Conflict Resolution**
   - Handle git conflicts from interrupted worktrees
   - Clean up orphaned worktrees from previous run
   - Resolve state inconsistencies
   - Validate checkpoint integrity

### Non-Functional Requirements

1. **Reliability**
   - Atomic checkpoint operations
   - Corruption detection and recovery
   - Graceful handling of missing state files

2. **Performance**
   - Minimal overhead for checkpoint operations
   - Efficient state serialization/deserialization
   - Quick startup time for resumed jobs

3. **Compatibility**
   - Support resuming jobs from older Prodigy versions
   - Handle schema migrations in checkpoint format
   - Backward compatibility for state files

## Acceptance Criteria

- [ ] `prodigy resume-job <job-id>` successfully resumes interrupted workflows
- [ ] Previously completed items are skipped during resume
- [ ] In-progress items are automatically retried
- [ ] Resume preserves original workflow configuration by default
- [ ] `--from-checkpoint <timestamp>` allows resuming from specific point
- [ ] `--force-retry` flag re-executes failed items only
- [ ] `--modified-config` allows updating parallelism and timeouts
- [ ] Progress shows "Resuming from checkpoint" with accurate counts
- [ ] Orphaned worktrees are cleaned up during resume
- [ ] Event logs show clear resume timeline
- [ ] Interrupting and resuming multiple times works correctly

## Technical Details

### Implementation Approach

```rust
impl JobStateManager {
    pub async fn resume_job(&self, job_id: &str, options: ResumeOptions) -> Result<()> {
        // 1. Load checkpoint and validate
        let checkpoint = self.load_checkpoint(job_id).await?;
        self.validate_checkpoint(&checkpoint)?;

        // 2. Reconstruct job state
        let job_state = self.reconstruct_state(&checkpoint).await?;

        // 3. Identify remaining work
        let remaining_items = self.calculate_remaining_work(&job_state)?;

        // 4. Clean up previous execution artifacts
        self.cleanup_orphaned_resources(&job_state).await?;

        // 5. Resume execution with remaining items
        let executor = MapReduceExecutor::from_checkpoint(checkpoint, remaining_items);
        executor.resume(options).await
    }
}
```

### Architecture Changes

1. **Enhanced Checkpoint System**
   - Versioned checkpoint format
   - Incremental checkpointing for large jobs
   - Checkpoint validation and repair

2. **State Recovery Module**
   - State reconstruction from events
   - Partial result aggregation
   - Work item deduplication

3. **Resume Coordinator**
   - Orchestrates resume workflow
   - Handles state conflicts
   - Manages resource cleanup

### Data Structures

```rust
pub struct Checkpoint {
    pub version: u32,
    pub job_id: String,
    pub timestamp: DateTime<Utc>,
    pub workflow_config: WorkflowConfig,
    pub processed_items: HashSet<String>,
    pub failed_items: Vec<FailedItem>,
    pub in_progress: Vec<InProgressItem>,
    pub map_results: Vec<MapResult>,
    pub variables: HashMap<String, Value>,
    pub phase: ExecutionPhase,
}

pub struct ResumeOptions {
    pub from_checkpoint: Option<DateTime<Utc>>,
    pub force_retry_failed: bool,
    pub modified_config: Option<ConfigOverrides>,
    pub skip_cleanup: bool,
}

pub struct ResumeState {
    pub total_items: usize,
    pub already_processed: usize,
    pub to_process: usize,
    pub failed_to_retry: usize,
    pub checkpoint_age: Duration,
}
```

### APIs and Interfaces

```rust
pub trait Resumable {
    async fn can_resume(&self) -> bool;
    async fn prepare_resume(&self) -> Result<ResumeState>;
    async fn resume(&self, options: ResumeOptions) -> Result<()>;
    async fn validate_checkpoint(&self) -> Result<CheckpointStatus>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/execution/state.rs`
  - `src/cook/workflow/checkpoint.rs`
  - `src/cook/workflow/resume.rs`
  - `src/main.rs` (CLI handlers)
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Checkpoint serialization/deserialization
  - State reconstruction logic
  - Work item deduplication

- **Integration Tests**:
  - Interrupt and resume scenarios
  - Multiple resume cycles
  - Resume with configuration changes

- **Fault Injection Tests**:
  - Corrupt checkpoint recovery
  - Missing state file handling
  - Concurrent resume attempts

- **User Acceptance**:
  - Large job resume performance
  - UI/UX of resume progress
  - Error message clarity

## Documentation Requirements

- **Code Documentation**:
  - Document checkpoint format and versioning
  - Resume algorithm documentation
  - Error recovery procedures

- **User Documentation**:
  - Resume command usage guide
  - Troubleshooting resume failures
  - Best practices for checkpointing

- **Architecture Updates**:
  - Document checkpoint lifecycle
  - State management architecture
  - Resume flow diagrams

## Implementation Notes

1. **Checkpoint Frequency**: Balance between overhead and recovery granularity
2. **State Size**: Implement compression for large checkpoint files
3. **Atomic Operations**: Use temporary files with atomic rename for checkpoints
4. **Version Migration**: Support at least 2 previous checkpoint versions
5. **Monitoring**: Add metrics for checkpoint size and resume success rates

## Migration and Compatibility

- Automatic migration of old checkpoint formats
- Graceful degradation for missing checkpoint features
- Clear error messages for incompatible checkpoints
- Optional checkpoint format upgrade utility