---
number: 134
title: MapReduce Checkpoint and Resume Functionality
category: parallel
priority: high
status: draft
dependencies: []
created: 2025-10-15
---

# Specification 134: MapReduce Checkpoint and Resume Functionality

**Category**: parallel
**Priority**: high
**Status**: draft

## Context

The `prodigy resume` command is currently broken for MapReduce workflows. When a MapReduce workflow fails (e.g., during the reduce phase), users cannot resume from the last checkpoint because:

1. **Storage Location Mismatch**: Regular workflows store checkpoints at `~/.prodigy/state/{session-id}/checkpoints/` while MapReduce workflows store checkpoints at `~/.prodigy/state/{repo}/mapreduce/jobs/{job-id}/`

2. **ID Format Incompatibility**: The resume command expects a session ID (e.g., `session-86af9ca1-f4b1-4c10-90a0-f1343a0d3ffe`) but MapReduce uses job IDs (e.g., `mapreduce-20251015_035918`)

3. **Missing Reduce Phase Checkpoints**: MapReduce workflows only checkpoint during the map phase. If a workflow fails during the reduce phase, there's no checkpoint to resume from, forcing users to re-run the entire workflow

4. **No Unified Resume Interface**: Users must know whether they're resuming a regular workflow or a MapReduce job and use different commands (`prodigy resume` vs `prodigy resume-job`)

This creates a poor user experience where:
- Long-running MapReduce workflows cannot be resumed after reduce phase failures
- Error messages are confusing ("No checkpoints found for session: X")
- Users must understand internal implementation details to resume workflows
- Valuable map phase work is lost when reduce phase fails

## Objective

Implement comprehensive checkpoint and resume functionality for MapReduce workflows that:
1. Creates checkpoints at all workflow phases (setup, map, and reduce)
2. Provides a unified resume interface that works for both regular and MapReduce workflows
3. Allows resumption from any phase failure
4. Maintains compatibility with existing checkpoint storage structure

## Requirements

### Functional Requirements

1. **Reduce Phase Checkpointing**
   - Create checkpoints after each reduce phase command execution
   - Store reduce phase state including completed steps and captured variables
   - Include map phase results in reduce checkpoints for context
   - Checkpoint before and after critical operations (write_file, claude commands, etc.)

2. **Setup Phase Checkpointing**
   - Create checkpoint after setup phase completes successfully
   - Store setup phase results and generated artifacts
   - Include setup output in map phase context

3. **Unified Resume Command**
   - `prodigy resume` command should handle both session IDs and job IDs
   - Auto-detect whether ID is a session or MapReduce job
   - Fall back to searching both storage locations if ID format is ambiguous
   - Provide clear error messages indicating what was searched and not found

4. **Session-to-Job Mapping**
   - Maintain mapping from session ID to MapReduce job ID
   - Store mapping in session metadata when MapReduce workflow starts
   - Allow resume command to use either ID interchangeably

5. **Checkpoint Granularity**
   - Checkpoint after each reduce command completes
   - Store sufficient state to resume from exact point of failure
   - Include variable context, file paths, and execution state
   - Maintain checkpoint history for debugging

### Non-Functional Requirements

- Resume operation should complete within 2 seconds for checkpoint loading
- Checkpoint size should not exceed 10MB per checkpoint
- Backward compatible with existing MapReduce checkpoint format
- No performance degradation during reduce phase execution
- Clear progress indicators during resume operation

## Acceptance Criteria

### Reduce Phase Checkpointing
- [ ] Reduce phase creates checkpoint after each command execution
- [ ] Reduce checkpoints include all necessary state to resume
- [ ] Reduce checkpoints store map phase results reference
- [ ] Failed reduce commands trigger checkpoint before retry
- [ ] Checkpoint includes captured variables and execution context

### Setup Phase Checkpointing
- [ ] Setup phase creates checkpoint after successful completion
- [ ] Setup checkpoint includes generated artifacts and outputs
- [ ] Setup state is available to map phase on resume

### Unified Resume
- [ ] `prodigy resume <session-id>` works for MapReduce workflows
- [ ] `prodigy resume <job-id>` works for MapReduce workflows
- [ ] Resume command auto-detects ID type and searches appropriate locations
- [ ] Error messages clearly indicate what locations were searched
- [ ] Resume command suggests `prodigy sessions list` and `prodigy resume-job list` on failure

### Session-Job Mapping
- [ ] Session metadata includes MapReduce job ID when workflow starts
- [ ] Job metadata includes parent session ID
- [ ] Either ID can be used to resume workflow
- [ ] Mapping persists across workflow lifecycle

### Resume Behavior
- [ ] Resume from reduce phase failure continues at exact failed step
- [ ] Resume from map phase failure reprocesses incomplete items only
- [ ] Resume from setup phase failure reruns setup phase
- [ ] Resume displays progress information about what will be resumed
- [ ] Resume asks for confirmation before continuing expensive operations

### Compatibility
- [ ] Existing MapReduce checkpoints remain loadable
- [ ] Regular workflow resume still works unchanged
- [ ] No breaking changes to checkpoint file format
- [ ] Legacy `prodigy resume-job` command still works

## Technical Details

### Implementation Approach

#### 1. Extend Reduce Phase Executor with Checkpointing

**File**: `src/cook/execution/mapreduce/phases/reduce.rs`

```rust
pub struct ReducePhaseExecutor {
    reduce_phase: ReducePhase,
    checkpoint_manager: Arc<CheckpointManager>,
}

impl ReducePhaseExecutor {
    async fn execute_reduce_commands(&self, context: &mut PhaseContext) -> Result<(), PhaseError> {
        for (step_index, step) in self.reduce_phase.commands.iter().enumerate() {
            // Execute step
            let step_result = self.execute_single_step(step, context).await?;

            // Checkpoint after successful step
            self.checkpoint_after_step(step_index, &step_result, context).await?;

            // Handle step failure...
        }
        Ok(())
    }

    async fn checkpoint_after_step(
        &self,
        step_index: usize,
        result: &StepResult,
        context: &PhaseContext,
    ) -> Result<()> {
        let checkpoint = ReducePhaseCheckpoint {
            completed_steps: step_index + 1,
            total_steps: self.reduce_phase.commands.len(),
            step_results: context.step_results.clone(),
            variables: context.variables.clone(),
            timestamp: Utc::now(),
        };

        self.checkpoint_manager
            .save_reduce_checkpoint(&checkpoint)
            .await
    }
}
```

#### 2. Create Reduce Phase Checkpoint Structure

**File**: `src/cook/execution/mapreduce/checkpoint/reduce.rs` (new)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhaseCheckpoint {
    /// Number of reduce commands completed
    pub completed_steps: usize,

    /// Total number of reduce commands
    pub total_steps: usize,

    /// Results from each completed step
    pub step_results: Vec<StepResult>,

    /// Variable context at this point
    pub variables: HashMap<String, String>,

    /// Map phase results reference
    pub map_results: Vec<AgentResult>,

    /// Timestamp of checkpoint
    pub timestamp: DateTime<Utc>,

    /// Checkpoint version for compatibility
    pub version: u32,
}

impl ReducePhaseCheckpoint {
    pub fn can_resume(&self) -> bool {
        self.completed_steps < self.total_steps
    }

    pub fn next_step_index(&self) -> usize {
        self.completed_steps
    }
}
```

#### 3. Add Session-to-Job ID Mapping

**File**: `src/cook/execution/mapreduce/coordination/executor.rs`

```rust
impl MapReduceCoordinator {
    pub async fn execute(&mut self) -> Result<MapReduceResult> {
        // Store job ID in session metadata
        self.store_job_mapping().await?;

        // Execute workflow phases...
    }

    async fn store_job_mapping(&self) -> Result<()> {
        let mapping = SessionJobMapping {
            session_id: self.session_id.clone(),
            job_id: self.job_id.clone(),
            workflow_name: self.config.name.clone(),
            created_at: Utc::now(),
        };

        self.storage
            .write_session_job_mapping(&mapping)
            .await
    }
}
```

#### 4. Enhance Resume Command

**File**: `src/cli/commands/resume.rs`

```rust
pub async fn resume_workflow(session_id: String) -> Result<()> {
    // Try to detect ID type
    let resume_target = detect_resume_target(&session_id).await?;

    match resume_target {
        ResumeTarget::RegularWorkflow(session) => {
            resume_regular_workflow(session).await
        }
        ResumeTarget::MapReduceJob(job_id) => {
            resume_mapreduce_job(job_id).await
        }
        ResumeTarget::Ambiguous => {
            // Try both
            if let Ok(()) = try_resume_regular(&session_id).await {
                return Ok(());
            }
            try_resume_mapreduce(&session_id).await
        }
    }
}

async fn detect_resume_target(id: &str) -> Result<ResumeTarget> {
    // Check format patterns
    if id.starts_with("session-") {
        // Could be session or legacy job ID
        ResumeTarget::Ambiguous
    } else if id.starts_with("mapreduce-") {
        ResumeTarget::MapReduceJob(id.to_string())
    } else {
        // Unknown format, try both
        ResumeTarget::Ambiguous
    }
}
```

#### 5. Checkpoint Manager Enhancements

**File**: `src/cook/execution/mapreduce/checkpoint/manager.rs`

```rust
impl CheckpointManager {
    /// Save reduce phase checkpoint
    pub async fn save_reduce_checkpoint(&self, checkpoint: &ReducePhaseCheckpoint) -> Result<()> {
        let checkpoint_file = self.job_dir
            .join(format!("reduce-checkpoint-v{}.json", checkpoint.version));

        self.storage
            .write_json(&checkpoint_file, checkpoint)
            .await
    }

    /// Load latest reduce checkpoint
    pub async fn load_reduce_checkpoint(&self) -> Result<Option<ReducePhaseCheckpoint>> {
        let checkpoints = self.list_reduce_checkpoints().await?;
        Ok(checkpoints.into_iter().max_by_key(|c| c.timestamp))
    }

    /// Check if reduce phase can be resumed
    pub async fn can_resume_reduce(&self) -> Result<bool> {
        if let Some(checkpoint) = self.load_reduce_checkpoint().await? {
            Ok(checkpoint.can_resume())
        } else {
            Ok(false)
        }
    }
}
```

### Architecture Changes

1. **New Module**: `src/cook/execution/mapreduce/checkpoint/reduce.rs`
   - Contains `ReducePhaseCheckpoint` struct
   - Implements checkpoint serialization/deserialization
   - Provides resume logic for reduce phase

2. **Enhanced**: `src/cook/execution/mapreduce/phases/reduce.rs`
   - Add checkpoint manager dependency
   - Checkpoint after each command execution
   - Resume from checkpoint on startup

3. **Enhanced**: `src/cli/commands/resume.rs`
   - Auto-detect session vs job ID
   - Unified resume interface
   - Better error messages

4. **New Module**: `src/storage/session_job_mapping.rs`
   - Stores bidirectional mapping between sessions and jobs
   - Enables resume using either ID

### Data Structures

```rust
// Session to Job ID mapping
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionJobMapping {
    pub session_id: String,
    pub job_id: String,
    pub workflow_name: String,
    pub created_at: DateTime<Utc>,
}

// Resume target type
pub enum ResumeTarget {
    RegularWorkflow(String),
    MapReduceJob(String),
    Ambiguous,
}
```

### Storage Structure

```
~/.prodigy/
├── state/
│   └── {repo}/
│       ├── mapreduce/
│       │   └── jobs/
│       │       └── {job-id}/
│       │           ├── checkpoint-v1.json          # Map phase checkpoints
│       │           ├── reduce-checkpoint-v1.json   # NEW: Reduce checkpoints
│       │           ├── setup-checkpoint.json       # NEW: Setup checkpoint
│       │           └── metadata.json
│       └── sessions/
│           └── {session-id}/
│               └── job-mapping.json                # NEW: Session→Job mapping
```

## Dependencies

- None - this is a foundational improvement to existing MapReduce functionality

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_reduce_checkpoint_serialization() {
        // Test checkpoint can be serialized and deserialized
    }

    #[test]
    fn test_resume_target_detection() {
        // Test ID format detection works correctly
    }

    #[test]
    fn test_session_job_mapping_storage() {
        // Test mapping can be stored and retrieved
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_resume_from_reduce_failure() {
    // Setup: Run MapReduce workflow that fails in reduce phase
    // Assert: Can resume using session ID
    // Assert: Resume continues from exact failure point
    // Assert: Workflow completes successfully
}

#[tokio::test]
async fn test_unified_resume_with_session_id() {
    // Run MapReduce workflow
    // Kill during reduce phase
    // Resume using session ID (not job ID)
    // Verify workflow completes
}

#[tokio::test]
async fn test_unified_resume_with_job_id() {
    // Run MapReduce workflow
    // Kill during reduce phase
    // Resume using job ID (not session ID)
    // Verify workflow completes
}
```

### Performance Tests

- Benchmark checkpoint save time (should be <100ms)
- Benchmark checkpoint load time (should be <500ms)
- Measure reduce phase overhead from checkpointing (should be <5%)
- Test with large variable contexts (>1MB)

### User Acceptance

```bash
# Scenario 1: Resume with session ID
prodigy run workflows/debtmap-reduce.yml
# ... workflow fails in reduce phase ...
prodigy resume session-86af9ca1-f4b1-4c10-90a0-f1343a0d3ffe
# Expected: Workflow resumes from failure point

# Scenario 2: Resume with job ID
prodigy run workflows/debtmap-reduce.yml
# ... workflow fails in reduce phase ...
prodigy resume mapreduce-20251015_035918
# Expected: Workflow resumes from failure point

# Scenario 3: List resumable workflows
prodigy sessions list
# Expected: Shows both session ID and job ID for MapReduce workflows
```

## Documentation Requirements

### Code Documentation

- Document `ReducePhaseCheckpoint` struct fields and usage
- Add examples to `CheckpointManager` methods
- Document resume target detection algorithm
- Explain session-job mapping lifecycle

### User Documentation

Update `CLAUDE.md` with:
- Resume behavior for MapReduce workflows
- How to find session/job IDs for resumption
- What state is preserved across resume
- Limitations and known issues

Add to project README:
- Section on MapReduce checkpoint and resume
- Examples of resuming failed workflows
- Troubleshooting guide for resume failures

### Architecture Updates

Update `ARCHITECTURE.md`:
- Document MapReduce checkpoint lifecycle
- Explain session-job ID mapping
- Describe storage structure for checkpoints
- Add sequence diagrams for resume flow

## Implementation Notes

### Checkpoint Frequency

Balance between checkpoint frequency and performance:
- **Too frequent**: Slows down reduce phase with I/O overhead
- **Too infrequent**: Lose too much work on failure

Recommended: Checkpoint after each reduce command (typically 3-6 commands per workflow)

### Checkpoint Retention

- Keep last 3 reduce checkpoints for debugging
- Clean up old checkpoints after successful workflow completion
- Provide command to manually clean old checkpoints

### Error Recovery

If checkpoint save fails:
- Log error but continue execution
- Don't fail entire workflow due to checkpoint failure
- Warn user that resume may not be possible

### Compatibility

Maintain backward compatibility with existing checkpoints:
- Check version field in checkpoint
- Support loading old format checkpoints
- Gracefully handle missing fields

## Migration and Compatibility

### Breaking Changes

None - this is additive functionality

### Migration Requirements

No migration needed - new functionality only

### Compatibility Considerations

- Existing MapReduce jobs without reduce checkpoints can still run
- Old checkpoint format can still be loaded
- `prodigy resume-job` command remains for backward compatibility
- Users can gradually adopt new unified resume command

## Success Metrics

- Reduce phase failures can be resumed 100% of the time
- Resume time is <5 seconds for 99% of workflows
- User errors related to resume decrease by 90%
- Checkpoint storage overhead is <10% of execution time
- Zero data loss on resume from any phase

## Future Enhancements

1. **Interactive Resume**: Show user what will be resumed and ask for confirmation
2. **Partial Resume**: Allow resuming only specific reduce steps
3. **Checkpoint Compression**: Compress large checkpoint files
4. **Cloud Checkpoint Storage**: Store checkpoints remotely for team sharing
5. **Checkpoint Diff**: Show what changed between checkpoints
