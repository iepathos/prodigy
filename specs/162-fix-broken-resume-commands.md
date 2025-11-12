---
number: 162
title: Fix Broken Resume Commands
category: foundation
priority: critical
status: draft
dependencies: [134, 159, 160]
created: 2025-01-11
---

# Specification 162: Fix Broken Resume Commands

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 134 (MapReduce Checkpoint and Resume), Spec 159 (MapReduce Resume CLI), Spec 160 (Comprehensive Resume Test Coverage)

## Context

Both `prodigy resume` and `prodigy resume-job` commands are broken in production, preventing users from resuming interrupted workflows. This is a critical issue because:

1. **`prodigy resume-job` is completely non-functional** - it's a TODO stub (line 560-578 in `src/cli/commands/resume.rs`) that only prints "Next steps for resume implementation" instead of actually resuming execution
2. **`prodigy resume` fails for MapReduce workflows** - it attempts regular workflow resume first, then falls back to the broken `resume-job` stub
3. **Working implementation exists but is unused** - `MapReduceResumeManager.resume_job()` in `src/cook/execution/mapreduce_resume.rs` has all the resume logic but is never called from the CLI layer

This forces users to restart MapReduce workflows from scratch, losing all completed work and wasting compute resources. The investigation documented in `RESUME_ANALYSIS.md` identified these root causes and provides detailed solutions.

## Objective

Fix both resume commands to enable users to successfully resume interrupted workflows (both standard and MapReduce) from checkpoints, preserving all completed work and continuing from the appropriate phase.

## Requirements

### Functional Requirements

1. **Complete `run_resume_job_command()` Implementation**
   - Replace TODO stub with working implementation
   - Connect CLI to existing `MapReduceResumeManager.resume_job()`
   - Load checkpoint and reconstruct job state
   - Find and load associated session data
   - Parse workflow file and create execution environment
   - Call resume manager with appropriate options
   - Display progress and completion summary

2. **Fix Unified Resume Logic**
   - Check session type BEFORE attempting resume strategy
   - Route MapReduce sessions to MapReduce resume path
   - Route standard workflow sessions to workflow resume path
   - Provide clear error messages for invalid session types
   - Support both session IDs and job IDs as input

3. **Session-Job ID Mapping**
   - Implement `find_session_for_job()` helper function
   - Search session-job mapping files in `~/.prodigy/state/mappings/`
   - Extract session ID from job ID patterns as fallback
   - Provide helpful errors when mapping not found

4. **Error Handling and User Experience**
   - Display clear progress indicators during resume
   - Show which phase is being resumed (setup/map/reduce)
   - Report number of items remaining vs completed
   - Provide actionable error messages for common failures
   - Include checkpoint location and log paths in output

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing `prodigy resume` behavior for standard workflows must continue working
   - Old checkpoints without workflow_path should still work with fallback logic
   - Resume lock mechanism must work for both standard and MapReduce resumes

2. **Performance**
   - Resume should start execution within 5 seconds for jobs with <1000 work items
   - Checkpoint loading should not load entire event history
   - Memory usage should scale linearly with active work items

3. **Reliability**
   - Resume must be idempotent (can be interrupted and resumed again)
   - Checkpoint state must remain consistent if resume fails
   - No duplicate work item processing on resume

4. **Observability**
   - Log all resume attempts with correlation IDs
   - Track resume success/failure metrics
   - Include JSON log locations in output for debugging

## Acceptance Criteria

- [ ] `prodigy resume-job <job_id>` successfully resumes MapReduce workflows from checkpoints
- [ ] `prodigy resume <session_id>` works for both standard and MapReduce workflow sessions
- [ ] Resume from all three MapReduce phases works (setup, map, reduce)
- [ ] Session-job ID mapping resolves correctly for all cases
- [ ] Workflow file loading handles missing files with clear errors
- [ ] Resume displays progress summary showing phase, items processed, duration
- [ ] Failed items in DLQ are retried when `--force` flag is used
- [ ] Resume can be interrupted and resumed again without duplicate work
- [ ] All error scenarios provide helpful, actionable messages
- [ ] Existing integration tests continue to pass
- [ ] New CLI execution tests validate end-to-end resume flow
- [ ] Resume completes successfully for workflows with 100+ work items

## Technical Details

### Implementation Approach

**Phase 1: Complete `run_resume_job_command()` Implementation**

Replace the stub at `src/cli/commands/resume.rs:481-578` with working implementation:

```rust
pub async fn run_resume_job_command(
    job_id: String,
    force: bool,
    max_retries: u32,
    path: Option<PathBuf>,
) -> Result<()> {
    use crate::cook::execution::mapreduce_resume::{
        MapReduceResumeManager, EnhancedResumeOptions
    };
    use crate::cook::orchestrator::ExecutionEnvironment;

    println!("🔄 Resuming MapReduce job: {}", job_id);

    // 1. Acquire resume lock to prevent concurrent resume
    let prodigy_home = crate::storage::get_default_storage_dir()?;
    let lock_manager = crate::cook::execution::ResumeLockManager::new(prodigy_home.clone())?;
    let _lock = lock_manager.acquire_lock(&job_id).await?;

    // 2. Find job directory in global storage
    let job_dir = find_job_directory(&job_id, &prodigy_home).await?;
    println!("📂 Found job at: {}", job_dir.display());

    // 3. Find and load associated session
    let session_id = find_session_for_job(&job_id).await?;
    let session = load_session(&session_id).await?;

    // 4. Load workflow file from session data
    let workflow_data = session.workflow_data
        .ok_or_else(|| anyhow!("Session has no workflow data"))?;
    let workflow_path = PathBuf::from(&workflow_data.workflow_path);

    if !workflow_path.exists() {
        return Err(anyhow!(
            "Workflow file not found: {}\n\
             The workflow file may have been moved or deleted.",
            workflow_path.display()
        ));
    }

    // 5. Parse workflow and create execution environment
    let workflow_content = std::fs::read_to_string(&workflow_path)?;
    let workflow: Playbook = serde_yaml::from_str(&workflow_content)?;

    let project_root = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let env = ExecutionEnvironment {
        repo_path: project_root,
        playbook: workflow,
        args: vec![],
        verbosity: 0,
    };

    // 6. Create resume manager components
    let event_logger = Arc::new(EventLogger::new(&job_id, &job_dir)?);
    let dlq = Arc::new(DeadLetterQueue::new(&job_id, &job_dir)?);
    let state_manager = JobStateManager::new(&job_dir)?;

    let resume_manager = MapReduceResumeManager::new(
        event_logger,
        dlq,
        state_manager,
    );

    // 7. Create resume options from CLI flags
    let options = EnhancedResumeOptions {
        force,
        max_additional_retries: max_retries,
        include_dlq_items: true,
        ..Default::default()
    };

    // 8. Resume execution
    println!("\n🔍 Loading checkpoint and resuming execution...\n");
    let result = resume_manager.resume_job(&job_id, options, &env).await?;

    // 9. Display summary
    display_resume_summary(&result);

    Ok(())
}
```

**Phase 2: Implement Session-Job Mapping Resolution**

Add `find_session_for_job()` helper function:

```rust
/// Find the session ID for a given job ID
async fn find_session_for_job(job_id: &str) -> Result<String> {
    let prodigy_home = crate::storage::get_default_storage_dir()?;
    let mappings_dir = prodigy_home.join("state").join("mappings");

    // Strategy 1: Check session-job mapping files
    if mappings_dir.exists() {
        let job_mapping_file = mappings_dir.join(format!("job-{}.json", job_id));
        if job_mapping_file.exists() {
            let content = fs::read_to_string(&job_mapping_file).await?;
            let mapping: serde_json::Value = serde_json::from_str(&content)?;
            if let Some(session_id) = mapping.get("session_id").and_then(|v| v.as_str()) {
                return Ok(session_id.to_string());
            }
        }
    }

    // Strategy 2: Extract from job_id pattern (mapreduce-{timestamp}_session-{uuid})
    if job_id.contains("session-") {
        if let Some(session_part) = job_id.split("session-").nth(1) {
            return Ok(format!("session-{}", session_part));
        }
    }

    // Strategy 3: Search for session with matching job_id in mapreduce_data
    let sessions_dir = prodigy_home.join("sessions");
    if sessions_dir.exists() {
        let mut entries = fs::read_dir(&sessions_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Ok(content) = fs::read_to_string(entry.path()).await {
                if let Ok(session) = serde_json::from_str::<UnifiedSession>(&content) {
                    if let Some(mr_data) = session.mapreduce_data {
                        if mr_data.job_id == job_id {
                            return Ok(session.id);
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!(
        "Could not find session ID for job: {}\n\
         The session may have been cleaned up or the mapping is missing.",
        job_id
    ))
}
```

**Phase 3: Fix Unified Resume Logic**

Update `try_unified_resume()` at `src/cli/commands/resume.rs:82-135`:

```rust
async fn try_unified_resume(id: &str, from_checkpoint: Option<String>) -> Result<()> {
    let id_type = detect_id_type(id);

    match id_type {
        IdType::SessionId => {
            // Check session type FIRST before attempting resume
            match check_session_type(id).await {
                Ok(SessionType::Workflow) => {
                    try_resume_regular_workflow(id, from_checkpoint).await
                }
                Ok(SessionType::MapReduce) => {
                    // Find job ID from session and resume
                    try_resume_mapreduce_from_session(id).await
                }
                Err(_) => {
                    // Session not found in UnifiedSessionManager
                    // Try workflow resume as fallback (may have old checkpoint)
                    try_resume_regular_workflow(id, from_checkpoint).await
                }
            }
        }
        IdType::MapReduceJobId => {
            // Direct MapReduce job resume
            try_resume_mapreduce_job(id).await
        }
        IdType::Ambiguous => {
            // Check session type first for ambiguous IDs
            match check_session_type(id).await {
                Ok(SessionType::MapReduce) => {
                    try_resume_mapreduce_job(id).await
                }
                Ok(SessionType::Workflow) => {
                    try_resume_regular_workflow(id, from_checkpoint).await
                }
                Err(_) => {
                    // Try workflow first, then MapReduce
                    try_resume_regular_workflow(id, from_checkpoint.clone())
                        .await
                        .or_else(|_| try_resume_mapreduce_job(id).await)
                }
            }
        }
    }
}
```

### Architecture Changes

**No new modules required** - this is a bug fix connecting existing components:

1. **CLI Layer** (`src/cli/commands/resume.rs`)
   - Complete `run_resume_job_command()` stub
   - Fix `try_unified_resume()` logic
   - Add `find_session_for_job()` helper

2. **Resume Manager** (`src/cook/execution/mapreduce_resume.rs`)
   - Already has working `resume_job()` implementation
   - No changes needed, just needs to be called from CLI

3. **Session Management** (`src/unified_session/`)
   - Already tracks session type and mapreduce data
   - No changes needed

4. **Storage** (`src/storage/`)
   - Already has session-job mappings infrastructure
   - No changes needed

### Data Structures

**No new data structures** - use existing ones:

- `EnhancedResumeOptions` - Already defined in `mapreduce_resume.rs`
- `EnhancedResumeResult` - Already defined in `mapreduce_resume.rs`
- `UnifiedSession` - Already defined in `unified_session/`
- `MapReduceData` - Already part of UnifiedSession

### Integration Points

1. **Resume Lock** (Spec 140)
   - Use `ResumeLockManager` to prevent concurrent resume
   - Lock acquisition already implemented

2. **Checkpoint System** (Spec 134)
   - Checkpoint loading handled by `MapReduceResumeManager`
   - No changes needed to checkpoint infrastructure

3. **Session Management**
   - Load session via `SessionManager`
   - Check session type via `check_session_type()`

4. **MapReduce Executor**
   - Execution delegated to `MapReduceResumeManager.resume_job()`
   - No direct executor changes needed

## Dependencies

### Prerequisites
- **Spec 134**: MapReduce Checkpoint and Resume (provides checkpoint infrastructure)
- **Spec 159**: MapReduce Resume CLI Implementation (documents TODO that needs fixing)
- **Spec 140**: Concurrent Resume Protection (provides locking mechanism)

### Affected Components
- `src/cli/commands/resume.rs` - CLI command implementation (main changes)
- `src/cook/execution/mapreduce_resume.rs` - Resume manager (already works, just needs CLI integration)
- `src/unified_session/manager.rs` - Session loading (already works)

### External Dependencies
- No new external dependencies required
- Uses existing tokio, anyhow, tracing infrastructure

## Testing Strategy

### Unit Tests

Add to `src/cli/commands/resume.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_session_for_job_with_mapping() {
        // Test session-job mapping resolution
    }

    #[tokio::test]
    async fn test_find_session_for_job_from_pattern() {
        // Test extracting session from job ID pattern
    }

    #[tokio::test]
    async fn test_find_session_for_job_not_found() {
        // Test error handling when session not found
    }
}
```

### Integration Tests

Create `tests/cli_integration/resume_mapreduce_cli_test.rs`:

```rust
/// Test that prodigy resume-job actually resumes execution
#[tokio::test]
async fn test_resume_job_executes_successfully() {
    let temp_dir = TempDir::new()?;
    let workflow_path = create_test_mapreduce_workflow(&temp_dir, 10)?;

    // Start workflow and interrupt after 3 items
    let job_id = start_and_interrupt_workflow(&workflow_path, 3).await?;

    // Resume via CLI
    let output = run_cli(&["prodigy", "resume-job", &job_id]).await?;

    // Verify success
    assert!(output.status.success());
    assert!(output.stdout.contains("Resume complete"));

    // Verify all items completed
    let result = verify_all_items_completed(&job_id).await?;
    assert_eq!(result.completed, 10);
}

/// Test that prodigy resume works with MapReduce session IDs
#[tokio::test]
async fn test_resume_with_mapreduce_session_id() {
    // Test auto-detection of MapReduce sessions
}

/// Test resume with failed items in DLQ
#[tokio::test]
async fn test_resume_with_dlq_items() {
    // Test DLQ retry functionality
}

/// Test resume from each phase
#[tokio::test]
async fn test_resume_from_each_phase() {
    // Test resume from setup, map, and reduce phases
}

/// Test error handling
#[tokio::test]
async fn test_resume_error_scenarios() {
    // Test missing job, corrupted checkpoint, missing workflow
}
```

### Error Scenario Tests

```rust
#[tokio::test]
async fn test_resume_nonexistent_job() {
    let output = run_cli(&["prodigy", "resume-job", "nonexistent-123"]).await?;
    assert!(!output.status.success());
    assert!(output.stderr.contains("not found"));
}

#[tokio::test]
async fn test_resume_with_corrupted_checkpoint() {
    // Create job, corrupt checkpoint, verify error
}

#[tokio::test]
async fn test_resume_with_missing_workflow() {
    // Create job, delete workflow file, verify error
}

#[tokio::test]
async fn test_concurrent_resume_blocked() {
    // Test resume lock prevents concurrent resume
}
```

### Performance Tests

```rust
#[tokio::test]
async fn test_resume_performance_large_job() {
    // Test resume with 1000+ work items completes in reasonable time
    let start = Instant::now();
    let result = resume_job_with_items(1000).await?;
    let duration = start.elapsed();

    assert!(duration < Duration::from_secs(600)); // < 10 minutes
}
```

### User Acceptance

Manual testing checklist:
- [ ] Interrupt real MapReduce workflow and resume successfully
- [ ] Verify resume summary displays correct information
- [ ] Confirm error messages are helpful and actionable
- [ ] Test with various workflow sizes (small, medium, large)
- [ ] Verify DLQ retry works correctly
- [ ] Test resume from different phases

## Documentation Requirements

### Code Documentation

1. **Function Documentation**
   - Document `run_resume_job_command()` with usage examples
   - Document `find_session_for_job()` with search strategies
   - Add inline comments explaining complex logic

2. **Error Documentation**
   - Document all error scenarios and their causes
   - Include troubleshooting steps in error messages

### User Documentation

Update `CLAUDE.md`:

1. **Remove TODO Language**
   - Replace "TODO: Implement full resume logic" with actual implementation details
   - Update MapReduce resume section with working commands

2. **Add Usage Examples**
   ```markdown
   ## Resuming MapReduce Workflows

   Resume using job ID:
   ```bash
   prodigy resume-job mapreduce-20251112_190130
   ```

   Resume using session ID (auto-detects MapReduce):
   ```bash
   prodigy resume session-93c8f475-17d2-4163-bf2e-c24095dd5254
   ```

   Resume with DLQ retry:
   ```bash
   prodigy resume-job mapreduce-20251112_190130 --force --max-retries 3
   ```
   ```

3. **Add Troubleshooting Section**
   - Common resume errors and solutions
   - How to find job IDs and session IDs
   - Checkpoint location and log paths

### Architecture Updates

Update `RESUME_ANALYSIS.md`:
- Mark issues as "FIXED" with implementation references
- Document final solution approach
- Add link to this specification

## Implementation Notes

### Critical Considerations

1. **Session-Job Mapping Reliability**
   - Primary strategy: Check mapping files in `~/.prodigy/state/mappings/`
   - Secondary strategy: Extract from job ID pattern
   - Tertiary strategy: Search all sessions for matching job ID
   - Always provide clear error if all strategies fail

2. **Workflow File Loading**
   - Must handle missing workflow files gracefully
   - Error message should explain file may have been moved/deleted
   - Include workflow path in error for user reference

3. **Backward Compatibility**
   - Old checkpoints may not have all fields
   - Use fallback values for missing data
   - Maintain compatibility with session format changes

4. **Error Recovery**
   - If resume fails, checkpoint state must remain valid
   - Resume lock must be released on error (RAII pattern)
   - Provide clear path forward in error messages

### Code Organization

All changes in existing files:
- `src/cli/commands/resume.rs` - Main implementation
- Tests in `tests/cli_integration/` - New test files

No new modules or major refactoring required.

### Testing Strategy for Implementation

1. **Fix implementation first**
   - Replace stub with working code
   - Test manually with real workflow
   - Verify basic functionality works

2. **Add error handling**
   - Test each error scenario
   - Verify error messages are helpful
   - Ensure graceful degradation

3. **Add comprehensive tests**
   - Implement all test cases
   - Verify edge cases
   - Check coverage metrics

4. **Performance validation**
   - Test with large workflows (100+ items)
   - Verify resume time is reasonable
   - Check memory usage is acceptable

## Migration and Compatibility

### Breaking Changes
- None - this is a bug fix that enables previously broken functionality

### Migration Requirements
- No migration needed
- Existing workflows and checkpoints remain compatible
- Old sessions without mappings will use fallback strategies

### Compatibility Considerations
- Resume must work with checkpoints created by older versions
- Session-job ID mapping may not exist for old jobs (use fallback)
- Workflow file path must be validated (may have been moved)

## Success Metrics

- Users can successfully resume 100% of interrupted MapReduce workflows (up from 0%)
- Resume time is <10% of original workflow execution time
- Zero duplicate work item processing on resume
- Error messages result in <5 support requests per 100 resumes
- Resume success rate >95% for valid checkpoints
- All existing tests continue to pass
- New tests achieve >90% coverage of resume code paths

## Implementation Phases

### Phase 1: Core Fix (Priority 1 - Days 1-2)
- [ ] Complete `run_resume_job_command()` implementation
- [ ] Add `find_session_for_job()` helper
- [ ] Test with real MapReduce workflow
- [ ] Verify basic resume works end-to-end

### Phase 2: Integration (Priority 1 - Day 3)
- [ ] Fix `try_unified_resume()` logic
- [ ] Add comprehensive error handling
- [ ] Test auto-detection of MapReduce sessions
- [ ] Verify error messages are helpful

### Phase 3: Testing (Priority 2 - Days 4-5)
- [ ] Add CLI integration tests
- [ ] Add error scenario tests
- [ ] Add performance tests
- [ ] Verify coverage >90%

### Phase 4: Documentation (Priority 2 - Day 6)
- [ ] Update CLAUDE.md
- [ ] Add inline code documentation
- [ ] Update RESUME_ANALYSIS.md
- [ ] Create user guide examples

## Future Enhancements (Out of Scope)

- Interactive resume mode (select which phase to resume from)
- Resume point selection (resume from specific checkpoint, not just latest)
- Resume with modified workflow file (detect and handle changes)
- Parallel resume of multiple jobs
- Resume progress estimation with time remaining
- Automatic retry of failed items without `--force` flag
