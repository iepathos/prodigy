---
number: 82
title: MapReduce Job Resume Functionality
category: parallel
priority: critical
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 82: MapReduce Job Resume Functionality

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

MapReduce workflows are designed to handle large-scale parallel processing, often running for hours or days. The ability to resume interrupted jobs is crucial for production reliability. Currently, the implementation in `src/cook/execution/mapreduce.rs` has a critical gap where `list_resumable_jobs()` returns an empty list (line 1936), completely breaking the resume functionality.

Additionally, command duration tracking is hardcoded to 0 (line 4091), preventing accurate performance monitoring and debugging. Without proper resume functionality, users must restart entire MapReduce jobs from the beginning when interruptions occur, leading to:
- Wasted computational resources
- Extended processing times
- Increased operational costs
- Poor user experience

## Objective

Implement complete MapReduce job resume functionality, including proper job listing, state restoration, and accurate duration tracking.

## Requirements

### Functional Requirements

1. **Job Discovery**
   - Implement `list_resumable_jobs()` to find interrupted jobs
   - Scan checkpoint directories for resumable states
   - Validate checkpoint integrity
   - Return jobs with sufficient metadata for resumption

2. **State Restoration**
   - Restore job progress from checkpoints
   - Recover processed item list
   - Restore DLQ state
   - Resume from last successful position

3. **Duration Tracking**
   - Track actual command execution duration (fix line 4091)
   - Aggregate durations across parallel agents
   - Persist timing information in checkpoints
   - Calculate total job duration including resumed portions

4. **Progress Management**
   - Track items completed vs remaining
   - Support partial completion states
   - Maintain agent assignment history
   - Handle duplicate processing prevention

### Non-Functional Requirements

1. **Reliability**
   - Atomic checkpoint updates
   - Corruption detection and recovery
   - Graceful handling of partial states

2. **Performance**
   - Efficient checkpoint scanning
   - Minimal overhead for tracking
   - Fast resume operations

3. **Usability**
   - Clear status reporting
   - Intuitive resume commands
   - Detailed progress information

## Acceptance Criteria

- [ ] `list_resumable_jobs()` returns all resumable MapReduce jobs
- [ ] Jobs can be resumed from their last checkpoint
- [ ] Command durations are accurately tracked and persisted
- [ ] Progress is correctly restored including completed items
- [ ] DLQ state is properly restored on resume
- [ ] Duplicate processing is prevented for completed items
- [ ] Resume command provides clear feedback about restoration
- [ ] Integration tests validate end-to-end resume scenarios
- [ ] Performance tests show <1s overhead for checkpoint operations
- [ ] Documentation includes comprehensive resume guide

## Technical Details

### Implementation Approach

1. **Implement Job Discovery**
   ```rust
   // Replace empty return at line 1936
   pub async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>> {
       let checkpoint_dir = self.get_checkpoint_directory()?;
       let mut jobs = Vec::new();

       for entry in fs::read_dir(checkpoint_dir)? {
           if let Ok(checkpoint) = self.load_checkpoint(&entry.path()) {
               if checkpoint.is_resumable() {
                   jobs.push(ResumableJob::from_checkpoint(checkpoint)?);
               }
           }
       }

       Ok(jobs)
   }
   ```

2. **Fix Duration Tracking**
   ```rust
   // Replace hardcoded duration at line 4091
   let start_time = Instant::now();
   let result = self.execute_command(&command, &context).await?;
   let duration = start_time.elapsed();

   CommandResult {
       output: result.output,
       duration, // Use actual duration
       exit_code: result.exit_code,
   }
   ```

3. **State Restoration**
   ```rust
   pub async fn resume_job(&self, job_id: &str) -> Result<MapReduceExecution> {
       let checkpoint = self.load_checkpoint(job_id)?;

       // Restore state
       let mut state = MapReduceState {
           completed_items: checkpoint.completed_items,
           remaining_items: checkpoint.remaining_items,
           dlq_items: checkpoint.dlq_items,
           agent_states: checkpoint.agent_states,
           total_duration: checkpoint.total_duration,
       };

       // Continue execution
       self.execute_with_state(state).await
   }
   ```

### Architecture Changes

- Enhance checkpoint format to include all necessary state
- Add checkpoint validation and repair mechanisms
- Implement efficient checkpoint indexing
- Add metrics collection for resume operations

### Data Structures

```rust
pub struct ResumableJob {
    pub job_id: String,
    pub workflow_name: String,
    pub started_at: DateTime<Utc>,
    pub last_checkpoint: DateTime<Utc>,
    pub progress: JobProgress,
    pub estimated_remaining: Duration,
}

pub struct JobProgress {
    pub total_items: usize,
    pub completed_items: usize,
    pub failed_items: usize,
    pub completion_percentage: f64,
}

pub struct MapReduceCheckpoint {
    pub job_id: String,
    pub version: u32,
    pub completed_items: HashSet<String>,
    pub remaining_items: Vec<WorkItem>,
    pub dlq_items: Vec<FailedItem>,
    pub agent_states: HashMap<String, AgentState>,
    pub total_duration: Duration,
    pub last_updated: DateTime<Utc>,
}

pub struct AgentState {
    pub agent_id: String,
    pub assigned_items: Vec<String>,
    pub completed_items: Vec<String>,
    pub status: AgentStatus,
}
```

### APIs and Interfaces

```rust
pub trait MapReduceResume {
    async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>>;

    async fn resume_job(
        &self,
        job_id: &str,
        options: ResumeOptions,
    ) -> Result<MapReduceExecution>;

    async fn get_job_progress(
        &self,
        job_id: &str,
    ) -> Result<JobProgress>;

    async fn repair_checkpoint(
        &self,
        job_id: &str,
    ) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `MapReduceExecutor`
  - `CheckpointManager`
  - `DlqManager`
  - CLI resume commands
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test job discovery with various checkpoint states
  - Validate duration tracking accuracy
  - Test state restoration completeness
  - Verify checkpoint corruption handling

- **Integration Tests**:
  - End-to-end resume scenarios
  - Test resume after various failure points
  - Validate no duplicate processing
  - Test concurrent resume attempts

- **Performance Tests**:
  - Checkpoint operation overhead
  - Resume time for large jobs
  - Memory usage during restoration

- **User Acceptance**:
  - Resume interrupted MapReduce job
  - Verify progress continuation
  - Check timing information accuracy

## Documentation Requirements

- **Code Documentation**:
  - Document checkpoint format
  - Add examples for resume operations
  - Include troubleshooting guide

- **User Documentation**:
  - MapReduce resume guide
  - Best practices for checkpointing
  - Troubleshooting resume issues

- **Architecture Updates**:
  - Update ARCHITECTURE.md with resume flow
  - Document checkpoint lifecycle
  - Include state diagrams

## Implementation Notes

- Ensure backward compatibility with existing checkpoints
- Consider adding checkpoint versioning for future extensions
- Implement checkpoint compression for large jobs
- Add checkpoint validation on write to prevent corruption
- Consider implementing incremental checkpoints for efficiency
- Add option to resume from specific checkpoint (not just latest)

## Migration and Compatibility

- Existing jobs without proper checkpoints cannot be resumed
- New checkpoint format is backward compatible
- Add migration tool for old checkpoint formats
- Consider checkpoint format versioning for future changes