---
number: 159
title: MapReduce Resume CLI Implementation
category: foundation
priority: high
status: draft
dependencies: [134]
created: 2025-01-11
---

# Specification 159: MapReduce Resume CLI Implementation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 134 (MapReduce Checkpoint and Resume)

## Context

The MapReduce resume CLI command (`prodigy resume-job`) currently displays checkpoint information but does not actually resume execution. This was documented as a TODO at line 560 in `src/cli/commands/resume.rs`. Users who experience MapReduce workflow failures or interruptions cannot resume their work via the CLI, forcing them to restart workflows from scratch and losing all completed work.

Phase 1 fixed the standard workflow resume bug (hardcoded workflow.yml path), but MapReduce resume remains incomplete. This spec addresses the MapReduce-specific resume implementation.

## Objective

Complete the MapReduce resume CLI implementation to enable users to resume interrupted or failed MapReduce workflows from checkpoints, preserving all completed work and continuing from the appropriate phase (setup/map/reduce).

## Requirements

### Functional Requirements

1. **CLI Command Execution**
   - `prodigy resume-job <job_id>` must actually resume execution, not just display information
   - Support both session ID and job ID for resume (auto-detection)
   - Maintain backward compatibility with existing `prodigy resume` command for standard workflows

2. **Checkpoint Loading and Validation**
   - Load the latest checkpoint for the specified job ID
   - Validate checkpoint integrity and compatibility
   - Detect checkpoint corruption and provide clear error messages
   - Support global storage checkpoint location (`~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`)

3. **Phase Detection and Resumption**
   - Automatically determine which phase to resume from (setup/map/reduce)
   - Handle partial setup completion (restart setup phase if incomplete)
   - Resume map phase with remaining work items from checkpoint
   - Resume reduce phase from last completed step

4. **Work Item Management**
   - Collect pending items from checkpoint state
   - Collect failed items for retry (respecting max_retries configuration)
   - Load DLQ items if retry is configured
   - Deduplicate work items across all sources
   - Track and preserve retry counts

5. **State Restoration**
   - Restore workflow variables and context from checkpoint
   - Restore map phase results for reduce phase execution
   - Restore environment configuration
   - Maintain correlation IDs and tracking metadata

### Non-Functional Requirements

1. **Performance**
   - Resume should start within 5 seconds for jobs with <1000 work items
   - Checkpoint loading should not load entire event history
   - Memory usage should scale linearly with active work items, not total items

2. **Reliability**
   - Resume must be idempotent (can be interrupted and resumed again)
   - Checkpoint state must remain consistent if resume fails
   - No duplicate work item processing on resume

3. **Observability**
   - Display clear progress indicators during resume
   - Show which phase is being resumed
   - Report number of items remaining vs. completed
   - Display expected duration estimate based on previous execution

4. **User Experience**
   - Provide helpful error messages for common failure scenarios
   - Display resume summary upon completion
   - Show location of JSON logs for debugging
   - Maintain verbosity level control (-v, -vv, -vvv)

## Acceptance Criteria

- [ ] `prodigy resume-job <job_id>` successfully resumes MapReduce workflows from checkpoints
- [ ] Resume works from all three phases: setup, map, and reduce
- [ ] Work items are correctly deduplicated across pending, failed, and DLQ sources
- [ ] Retry counts are preserved and respected during resume
- [ ] Workflow variables and map results are correctly restored
- [ ] Resume can be interrupted and resumed again without duplicate work
- [ ] CLI displays progress and completion summary
- [ ] All existing MapReduce resume integration tests continue to pass
- [ ] New CLI execution test validates end-to-end resume flow
- [ ] Resume completes successfully for workflows with 100+ work items

## Technical Details

### Implementation Approach

1. **Refactor Resume Logic** (`src/cli/commands/resume.rs:560-578`)
   - Replace TODO stub with actual implementation
   - Extract resume execution logic from integration tests into library code
   - Create `execute_mapreduce_resume()` function in resume module

2. **Checkpoint Manager Integration**
   - Use existing `CheckpointManager` to load checkpoints
   - Leverage `load_latest_mapreduce_checkpoint()` method
   - Integrate with global storage backend

3. **MapReduce Executor Integration**
   - Create `MapReduceResumeManager` public API for CLI usage
   - Expose `resume_from_checkpoint()` method
   - Pass resume options (force retry, max parallel, etc.)

4. **Session Management**
   - Load `UnifiedSession` to get session context
   - Map between session IDs and job IDs using session-job mapping
   - Update session status during resume

### Architecture Changes

**New Public API in `MapReduceResumeManager`**:
```rust
impl MapReduceResumeManager {
    /// Resume MapReduce job from checkpoint (public API for CLI)
    pub async fn resume_from_checkpoint(
        &self,
        job_id: &str,
        checkpoint: &MapReduceCheckpoint,
        options: ResumeOptions,
    ) -> Result<ResumeResult> {
        // Implementation extracted from integration tests
    }
}
```

**Enhanced Resume Command** (`src/cli/commands/resume.rs`):
```rust
async fn execute_mapreduce_resume(
    job_id: &str,
    resume_options: ResumeOptions,
) -> Result<()> {
    // 1. Load checkpoint
    let checkpoint = load_checkpoint(job_id).await?;

    // 2. Validate checkpoint
    validate_checkpoint(&checkpoint)?;

    // 3. Load session context
    let session = load_session_for_job(job_id).await?;

    // 4. Create resume manager
    let resume_manager = create_resume_manager()?;

    // 5. Resume execution
    let result = resume_manager
        .resume_from_checkpoint(job_id, &checkpoint, resume_options)
        .await?;

    // 6. Display summary
    display_resume_summary(&result);

    Ok(())
}
```

### Data Structures

**ResumeResult** (extend existing):
```rust
pub struct ResumeResult {
    pub items_processed: usize,
    pub items_failed: usize,
    pub duration: Duration,
    pub phase_resumed_from: MapReducePhase,
    pub final_status: JobStatus,
}
```

**ResumeOptions** (extend existing):
```rust
pub struct ResumeOptions {
    pub force_retry: bool,
    pub max_parallel: Option<usize>,
    pub include_dlq: bool,
    pub timeout: Option<Duration>,
    pub verbosity: u8,
}
```

### Integration Points

1. **Checkpoint System** (Spec 134)
   - Uses existing checkpoint loading infrastructure
   - Relies on checkpoint validation and integrity checks
   - Leverages checkpoint-based work item tracking

2. **MapReduce Executor**
   - Integrates with existing map/reduce execution logic
   - Reuses agent orchestration and worktree management
   - Maintains event streaming and DLQ functionality

3. **Session Management**
   - Updates `UnifiedSession` status during resume
   - Preserves session-job ID mappings
   - Tracks resume attempts and timestamps

4. **Resume Lock** (Spec 140)
   - Acquires exclusive lock before resume
   - Prevents concurrent resume attempts
   - Handles stale lock cleanup

## Dependencies

### Prerequisites
- **Spec 134**: MapReduce Checkpoint and Resume (provides checkpoint infrastructure)
- **Spec 140**: Concurrent Resume Protection (provides locking mechanism)

### Affected Components
- `src/cli/commands/resume.rs` - CLI command implementation
- `src/cook/execution/mapreduce_resume.rs` - Resume manager public API
- `src/cook/execution/mapreduce/checkpoint/manager.rs` - Checkpoint loading
- `src/unified_session/manager.rs` - Session context loading

### External Dependencies
- No new external dependencies required
- Uses existing tokio, anyhow, tracing infrastructure

## Testing Strategy

### Unit Tests
- Test checkpoint loading and validation
- Test phase detection logic
- Test work item collection and deduplication
- Test resume options parsing and application

### Integration Tests
- **End-to-end CLI resume test** (new):
  - Start MapReduce job and interrupt after N items
  - Resume via CLI command
  - Verify all items processed and no duplicates

- **Resume from each phase** (extend existing):
  - Resume from setup phase (partial completion)
  - Resume from map phase (with pending items)
  - Resume from reduce phase (mid-execution)

- **Error scenario tests**:
  - Resume with corrupted checkpoint
  - Resume with missing workflow files
  - Resume with all items failed
  - Resume with no checkpoint found

### Performance Tests
- Resume with 1000+ work items should complete in <10 minutes
- Checkpoint loading should complete in <5 seconds
- Memory usage should not exceed 500MB for 10,000 items

### User Acceptance
- Manual test: Interrupt real workflow and resume successfully
- Verify resume summary displays correct information
- Confirm verbosity flags control output appropriately
- Validate error messages are helpful and actionable

## Documentation Requirements

### Code Documentation
- Document `resume_from_checkpoint()` public API with examples
- Add inline comments explaining phase detection logic
- Document resume options and their effects
- Include error recovery strategies in comments

### User Documentation
- Update `CLAUDE.md` with completed MapReduce resume CLI section (remove TODO)
- Add examples of `prodigy resume-job` usage
- Document resume options and flags
- Add troubleshooting section for common resume errors

### Architecture Updates
- Document MapReduce resume flow in architecture diagrams
- Update sequence diagrams to show resume execution
- Document checkpoint-to-execution state transition

## Implementation Notes

### Critical Considerations

1. **Idempotency**
   - Resume must be safe to run multiple times
   - Work items should not be duplicated on repeated resume
   - Use checkpoint timestamps to detect re-resume scenarios

2. **Error Recovery**
   - If resume fails, checkpoint state should remain valid
   - Failed resume attempts should be logged for debugging
   - Provide clear path forward if resume cannot proceed

3. **Backward Compatibility**
   - Existing `prodigy resume` for standard workflows must continue working
   - Resume lock mechanism must work for both standard and MapReduce resumes
   - Old checkpoints without workflow_path should still work (fallback logic)

4. **Performance Optimization**
   - Load checkpoint incrementally (don't load entire event history)
   - Stream work items rather than loading all into memory
   - Use async I/O for checkpoint loading

### Code Organization

Extract resume logic from tests into library:
```
src/cook/execution/mapreduce_resume.rs
  ├── pub fn resume_from_checkpoint()     // Public API for CLI
  ├── fn load_checkpoint_state()          // Load and validate checkpoint
  ├── fn determine_resume_phase()         // Detect which phase to resume
  ├── fn collect_resume_work_items()      // Gather pending/failed items
  └── fn execute_resume()                 // Run resume execution
```

### Testing Strategy for Implementation

1. **Extract from integration tests first**
   - Identify resume logic in `tests/mapreduce_resume_integration_test.rs`
   - Move to library with minimal changes
   - Verify integration tests still pass

2. **Create public API**
   - Design clean API for CLI usage
   - Add error handling and validation
   - Document with examples

3. **Implement CLI command**
   - Replace TODO stub with actual implementation
   - Add progress display and summary
   - Implement error handling

4. **Add CLI execution test**
   - Test end-to-end resume via CLI
   - Verify correct output and exit codes
   - Test error scenarios

## Migration and Compatibility

### Breaking Changes
- None - this is new functionality

### Migration Requirements
- No migration needed
- Existing workflows and checkpoints remain compatible

### Compatibility Considerations
- Resume must work with checkpoints created by older versions (within version tolerance)
- CLI command must maintain backward compatibility with `prodigy resume` for standard workflows
- Session-job ID mapping must handle legacy sessions without mappings

## Success Metrics

- Users can successfully resume 100% of interrupted MapReduce workflows (up from 0%)
- Resume time is <10% of original workflow execution time
- Zero duplicate work item processing on resume
- Error messages result in <5 support requests per 100 resumes
- Resume success rate >95% for valid checkpoints

## Future Enhancements (Out of Scope)

- Interactive resume mode (select which phase to resume from)
- Resume point selection (resume from specific checkpoint, not just latest)
- Resume with modified workflow file (detect and handle changes)
- Parallel resume of multiple jobs
- Resume progress estimation with time remaining
