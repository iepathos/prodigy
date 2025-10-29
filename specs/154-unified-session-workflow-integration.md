---
number: 154
title: Integrate UnifiedSessionManager with Workflow Orchestrator
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-10-29
updated: 2025-10-29
---

# Specification 154: Integrate UnifiedSessionManager with Workflow Orchestrator

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Currently, regular workflow executions create checkpoint files and worktree metadata, but do not create UnifiedSession records in `~/.prodigy/sessions/`. This causes resume operations to fail because the resume command (`prodigy resume`) attempts to load sessions from the UnifiedSessionManager, which expects session records to exist in `~/.prodigy/sessions/{session-id}.json`.

### Current Behavior

When a workflow executes:
1. Orchestrator generates a session ID (via `generate_session_id()`)
2. Creates a worktree with metadata stored in `~/.prodigy/worktrees/{repo}/.metadata/{session-id}.json`
3. Creates checkpoints in `~/.prodigy/state/{session-id}/checkpoints/`
4. Saves legacy session state to `.prodigy/session_state.json` in the worktree
5. **Does NOT create a UnifiedSession record**

When attempting to resume:
1. `prodigy resume {session-id}` calls `try_resume_regular_workflow()`
2. Tries to load session via `session_manager.load_session()` (resume.rs:212)
3. UnifiedSessionManager looks in `~/.prodigy/sessions/{session-id}.json` (manager.rs:356-363)
4. **Session not found** → Error thrown before checkpoint loading even begins

### Problem Evidence

Session `session-16bb0e91-b98b-47f6-89b8-ccbe352ca058` has:
- ✅ Checkpoint: `~/.prodigy/state/session-16bb0e91-b98b-47f6-89b8-ccbe352ca058/checkpoints/workflow-1761712942647.checkpoint.json`
- ✅ Worktree metadata: `~/.prodigy/worktrees/debtmap/.metadata/session-16bb0e91-b98b-47f6-89b8-ccbe352ca058.json`
- ✅ Worktree directory: `~/.prodigy/worktrees/debtmap/session-16bb0e91-b98b-47f6-89b8-ccbe352ca058/`
- ❌ **Missing**: UnifiedSession at `~/.prodigy/sessions/session-16bb0e91-b98b-47f6-89b8-ccbe352ca058.json`

### Root Cause

The orchestrator uses the older `SessionManager` (from `crate::cook::session`) but never creates records in the newer `UnifiedSessionManager` (from `crate::unified_session`). This creates a gap where checkpoint-based resume expects UnifiedSession records that were never created.

**Code Location**: `/Users/glen/memento-mori/prodigy/src/cook/orchestrator/core.rs`
- Line 18: Uses `crate::cook::session::SessionManager` (legacy)
- Line 436: Generates session ID but doesn't create UnifiedSession
- Line 556-561: Creates checkpoints using `CheckpointStorage::Session`
- Missing: Call to `unified_session::SessionManager::create_session()`

## Objective

Ensure that all workflow executions create and maintain UnifiedSession records throughout their lifecycle, enabling checkpoint-based resume to work correctly while providing proper session status tracking.

## Requirements

### Functional Requirements

1. **Session Creation on Workflow Start**
   - Create UnifiedSession record when workflow execution begins
   - Store session in `~/.prodigy/sessions/{session-id}.json`
   - Associate session with workflow ID and type
   - Set initial status to Running

2. **Session Status Tracking**
   - Update session status as workflow progresses
   - Track execution milestones (started, paused, completed, failed)
   - Maintain timing information for operations
   - Preserve session metadata for debugging

3. **Session Updates During Execution**
   - Sync checkpoint saves with session updates
   - Update progress information as steps complete
   - Track errors and failure states
   - Maintain consistency between checkpoint and session state

4. **Session Completion Handling**
   - Mark session as Completed on successful workflow completion
   - Mark session as Failed on workflow failure
   - Preserve final execution state and metadata
   - Enable post-execution analysis and debugging

5. **Resume Compatibility**
   - Ensure resume command can load UnifiedSession records
   - Verify checkpoint and session state consistency
   - Support both new and legacy resume paths
   - Provide clear error messages for missing sessions

### Non-Functional Requirements

1. **Performance**
   - Session creation must not significantly impact workflow startup time (<100ms)
   - Session updates must not block workflow execution
   - Async I/O for all session persistence operations
   - Minimal memory overhead for session tracking

2. **Reliability**
   - Session creation must succeed or fail workflow startup
   - Session updates must not cause workflow failures
   - Graceful handling of session persistence errors
   - Session state must survive process crashes (via checkpoints)

3. **Consistency**
   - Session state must match checkpoint state
   - No duplicate session records
   - Atomic updates to session files
   - Clear separation between legacy and unified session systems

4. **Maintainability**
   - Clear code organization for session lifecycle
   - Well-documented session state transitions
   - Consistent error handling patterns
   - Easy to test session management logic

## Acceptance Criteria

- [ ] **AC1**: Workflow execution creates UnifiedSession in `~/.prodigy/sessions/{session-id}.json`
- [ ] **AC2**: UnifiedSession includes workflow_id, session_type (Workflow), and initial status (Running)
- [ ] **AC3**: Session status updates to Completed on successful workflow completion
- [ ] **AC4**: Session status updates to Failed on workflow failure with error details
- [ ] **AC5**: `prodigy resume {session-id}` successfully loads UnifiedSession and continues execution
- [ ] **AC6**: Session metadata includes execution timing, progress, and workflow type
- [ ] **AC7**: All existing workflow tests pass with UnifiedSession integration
- [ ] **AC8**: MapReduce workflows continue to work correctly (no regression)
- [ ] **AC9**: Session creation failure causes workflow to exit with clear error message
- [ ] **AC10**: Session state remains consistent with checkpoint state throughout execution

## Technical Details

### Implementation Approach

#### Phase 1: Session Creation in `setup_environment()`

**File**: `src/cook/orchestrator/core.rs`
**Location**: Line 436 (after session_id generation)

```rust
// After: let session_id = Arc::from(self.generate_session_id().as_str());

// Create UnifiedSession for this workflow
let storage = crate::storage::GlobalStorage::new()
    .context("Failed to create global storage")?;
let unified_session_manager = crate::unified_session::SessionManager::new(storage)
    .await
    .context("Failed to create unified session manager")?;

let workflow_id = format!("workflow-{}", chrono::Utc::now().timestamp_millis());
let session_config = crate::unified_session::SessionConfig {
    session_type: crate::unified_session::SessionType::Workflow,
    workflow_id: Some(workflow_id.clone()),
    job_id: None,
    metadata: std::collections::HashMap::new(),
};

let unified_session_id = unified_session_manager
    .create_session(session_config)
    .await
    .context("Failed to create unified session")?;

// Start the session
unified_session_manager
    .start_session(&unified_session_id)
    .await
    .context("Failed to start unified session")?;

// Store session manager in orchestrator for later updates
// (requires adding field to CookOrchestratorImpl struct)
```

#### Phase 2: Session Updates in `execute_workflow()`

**File**: `src/cook/orchestrator/core.rs`
**Location**: Line 600 (during workflow execution)

```rust
// After workflow execution completes or fails
let session_id_obj = crate::unified_session::SessionId::from_string(
    env.session_id.to_string()
);

if execution_succeeded {
    self.unified_session_manager
        .complete_session(&session_id_obj, true)
        .await
        .context("Failed to mark session as completed")?;
} else {
    self.unified_session_manager
        .complete_session(&session_id_obj, false)
        .await
        .context("Failed to mark session as failed")?;
}
```

#### Phase 3: Session Cleanup in `cleanup()`

**File**: `src/cook/orchestrator/core.rs`
**Location**: Line 606 (during cleanup phase)

```rust
// Ensure final session state is saved
let session_id_obj = crate::unified_session::SessionId::from_string(
    env.session_id.to_string()
);

// Final status update if not already done
// (In case cleanup is called without execute_workflow completing)
if let Ok(session) = self.unified_session_manager.load_session(&session_id_obj).await {
    if session.status == crate::unified_session::SessionStatus::Running {
        // Workflow was interrupted, mark as paused or failed
        self.unified_session_manager
            .pause_session(&session_id_obj)
            .await
            .ok(); // Don't fail cleanup on session update failure
    }
}
```

### Architecture Changes

1. **Add UnifiedSessionManager to CookOrchestratorImpl**
   ```rust
   pub struct CookOrchestratorImpl<E, G, W, C> {
       // ... existing fields ...
       unified_session_manager: Arc<crate::unified_session::SessionManager>,
   }
   ```

2. **Initialize in Constructor**
   - Add UnifiedSessionManager initialization to orchestrator builder
   - Pass GlobalStorage instance to SessionManager
   - Store Arc reference in orchestrator struct

3. **Session Lifecycle Integration**
   ```
   setup_environment() → create_session() → start_session()
   execute_workflow()  → update progress via checkpoint sync
   cleanup()          → complete_session() or pause_session()
   ```

### Data Structures

**UnifiedSession** (already exists, no changes needed):
```rust
pub struct UnifiedSession {
    pub id: SessionId,
    pub session_type: SessionType,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub checkpoints: Vec<Checkpoint>,
    // ... other fields
}
```

**SessionConfig** (already exists, used for creation):
```rust
pub struct SessionConfig {
    pub session_type: SessionType,
    pub workflow_id: Option<String>,
    pub job_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### APIs and Interfaces

**New Methods** (none - all APIs already exist in UnifiedSessionManager):
- `create_session(config)` - Create new session
- `start_session(id)` - Mark session as running
- `complete_session(id, success)` - Mark session as completed/failed
- `pause_session(id)` - Mark session as paused
- `load_session(id)` - Load session for resume

**Modified Components**:
- `CookOrchestratorImpl`: Add unified_session_manager field
- `OrchestratorBuilder`: Add UnifiedSessionManager initialization
- Session lifecycle methods: Integrate session management calls

## Dependencies

### Prerequisites
- None (UnifiedSessionManager already exists and is functional)

### Affected Components
- `src/cook/orchestrator/core.rs` (primary changes)
- `src/cook/orchestrator/builder.rs` (add UnifiedSessionManager to builder)
- `src/cook/orchestrator/construction.rs` (may need updates to new_orchestrator())

### External Dependencies
- No new external dependencies required
- Uses existing `unified_session` module
- Uses existing `storage::GlobalStorage`

## Testing Strategy

### Unit Tests

1. **Session Creation Tests**
   - Test session creation during setup_environment()
   - Verify session fields (workflow_id, session_type, status)
   - Test session creation failure handling
   - Verify session stored in correct location

2. **Session Update Tests**
   - Test status transitions (Running → Completed)
   - Test status transitions (Running → Failed)
   - Test metadata updates during execution
   - Test checkpoint sync with session updates

3. **Session Lifecycle Tests**
   - Test complete workflow lifecycle with session tracking
   - Test interrupted workflow (paused state)
   - Test failed workflow (failed state with error details)
   - Test cleanup with various session states

### Integration Tests

1. **Resume Integration Tests**
   - Execute workflow, interrupt, verify session created
   - Resume workflow using `prodigy resume {session-id}`
   - Verify session loads correctly from UnifiedSessionManager
   - Verify workflow continues from checkpoint

2. **End-to-End Workflow Tests**
   - Run complete workflow with session tracking
   - Verify session created at start
   - Verify session updated during execution
   - Verify session completed at end
   - Verify session file exists with correct data

3. **MapReduce Compatibility Tests**
   - Run MapReduce workflow (should still work)
   - Verify MapReduce sessions continue to use existing path
   - Verify no regression in MapReduce functionality
   - Test MapReduce resume (should use existing MapReduce path)

### Performance Tests

1. **Session Creation Overhead**
   - Measure workflow startup time before/after changes
   - Verify session creation adds <100ms overhead
   - Test with multiple concurrent workflow starts

2. **Session Update Performance**
   - Measure session update time during workflow execution
   - Verify updates don't block workflow progress
   - Test async I/O performance for session persistence

### User Acceptance Tests

1. **Happy Path**
   - User runs workflow: `prodigy run workflow.yml`
   - Workflow executes successfully
   - User can see session in `prodigy sessions list`
   - Session marked as Completed

2. **Resume Path**
   - User runs workflow: `prodigy run workflow.yml`
   - User interrupts (Ctrl+C)
   - User resumes: `prodigy resume {session-id}`
   - Workflow continues from checkpoint
   - Session status transitions correctly

3. **Failure Path**
   - User runs workflow with intentional failure
   - Workflow fails with clear error message
   - Session marked as Failed
   - User can inspect session state for debugging

## Documentation Requirements

### Code Documentation

1. **Session Lifecycle Documentation**
   - Document session creation in setup_environment()
   - Document session updates in execute_workflow()
   - Document session completion in cleanup()
   - Add rustdoc comments explaining integration

2. **Architecture Documentation**
   - Update ARCHITECTURE.md with session management flow
   - Document UnifiedSession vs legacy SessionManager
   - Explain session lifecycle state diagram
   - Document storage locations for session files

### User Documentation

1. **User Guide Updates**
   - Document that workflows now create session records
   - Explain `prodigy sessions list` command
   - Document session lifecycle and states
   - Add troubleshooting section for session issues

2. **Developer Documentation**
   - Document how to access session information in code
   - Explain session state transitions
   - Provide examples of session metadata usage
   - Document testing patterns for session integration

### CLAUDE.md Updates

1. **Add Session Management Section**
   - Explain UnifiedSession creation during workflows
   - Document session storage location (`~/.prodigy/sessions/`)
   - Explain session lifecycle (Running → Completed/Failed/Paused)
   - Document resume behavior with UnifiedSessions

2. **Update Resume Section**
   - Clarify that resume now uses UnifiedSessionManager
   - Document that sessions must exist for resume to work
   - Explain relationship between checkpoints and sessions
   - Add troubleshooting for missing session errors

## Implementation Notes

### Critical Considerations

1. **Backward Compatibility**
   - This change only affects NEW workflow executions
   - Old sessions without UnifiedSession records will still fail resume
   - Consider implementing Spec 155 (checkpoint-first resume) for backward compat
   - Document migration path for users with old sessions

2. **Error Handling**
   - Session creation failure MUST fail workflow startup (fail-fast)
   - Session update failures should be logged but not block workflow
   - Session cleanup failures should be logged but not block cleanup
   - Provide clear error messages for session-related failures

3. **MapReduce Non-Regression**
   - MapReduce workflows already create UnifiedSessions (different code path)
   - Verify no changes to MapReduce session creation logic
   - Test that MapReduce resume still works correctly
   - Maintain separation between workflow and MapReduce session paths

4. **Timing and Atomicity**
   - Session creation must complete before workflow starts
   - Session updates should be async to avoid blocking
   - Use Arc<SessionManager> for thread-safe access
   - Consider using channels for async session updates if needed

### Gotchas

1. **Session ID Consistency**
   - Ensure same session_id used for worktree, checkpoint, and UnifiedSession
   - Avoid generating multiple session IDs in different places
   - Use env.session_id as single source of truth

2. **Storage Backend**
   - UnifiedSessionManager uses GlobalStorage (requires initialization)
   - GlobalStorage must be available before session creation
   - Handle storage initialization failures gracefully

3. **Testing Challenges**
   - Mock GlobalStorage in unit tests
   - Use temporary directories for integration tests
   - Clean up test session files after each test
   - Verify session files don't leak in test runs

### Best Practices

1. **Separation of Concerns**
   - Keep session management logic separate from workflow execution
   - Use dependency injection for SessionManager
   - Avoid tight coupling between orchestrator and session internals

2. **Logging and Observability**
   - Log session creation with session_id
   - Log session status transitions
   - Log session update failures (with context)
   - Include session_id in all related log messages

3. **Idempotency**
   - Session creation should be idempotent where possible
   - Handle cases where session already exists
   - Verify session state before updates
   - Clean up partial session state on failures

## Migration and Compatibility

### Breaking Changes

**None for new workflows**. This is a purely additive change that creates additional state (UnifiedSession records) without removing or modifying existing functionality.

### Compatibility Considerations

1. **Existing Sessions**
   - Old sessions without UnifiedSession records will still fail resume
   - No automatic migration of old sessions
   - Users must use checkpoints directly or re-run workflows
   - Consider implementing Spec 155 for backward compatibility

2. **Resume Command**
   - Resume command already expects UnifiedSession records
   - This change makes resume work for NEW workflows
   - Old workflows still have broken resume (existing bug)
   - Document workaround: use `prodigy run workflow.yml --resume {session-id}`

3. **MapReduce Workflows**
   - MapReduce already creates UnifiedSessions (no change)
   - Verify no regression in MapReduce functionality
   - Test MapReduce resume (should continue to work)
   - Document that MapReduce uses different session path

### Migration Path

**For Developers**:
1. Implement Spec 154 (this spec) - fixes resume for new workflows
2. Optionally implement Spec 155 (checkpoint-first resume) - fixes old workflows
3. Update documentation to reflect new behavior
4. Test thoroughly with both new and old sessions

**For Users**:
1. Update to version with Spec 154 implemented
2. New workflows will create UnifiedSessions automatically
3. Old sessions may need to be re-run (or use workaround)
4. No manual migration required

## Related Specifications

### Future Specifications

**Spec 155: Checkpoint-First Resume (Backward Compatibility)**
- Make resume work without UnifiedSession records
- Load checkpoint directly when session not found
- Auto-create UnifiedSession from checkpoint on resume
- Fixes broken resume for old sessions

**Spec 156: Session Migration Tool**
- Create tool to migrate old sessions to UnifiedSessions
- Scan for checkpoint files without corresponding sessions
- Generate UnifiedSession records from checkpoints
- Validate and repair inconsistent session state

### Related Issues

**Issue: Resume fails for workflows run in different repository**
- Problem: Session `session-16bb0e91-b98b-47f6-89b8-ccbe352ca058` from debtmap repo
- Resume attempted from prodigy repo
- Session not found because lookup is repo-specific
- Solution: This spec fixes for same-repo resumes, cross-repo needs separate fix

## Success Metrics

### Quantitative Metrics
- 100% of new workflows create UnifiedSession records
- 0% increase in workflow startup time (within measurement error)
- 100% of resume operations succeed for new workflows
- 0 regressions in MapReduce functionality

### Qualitative Metrics
- Clear error messages when session operations fail
- Easy to debug session-related issues via session files
- Improved user experience for workflow resume
- Better session visibility via `prodigy sessions list`

## Rollout Plan

### Phase 1: Implementation (Week 1)
- Implement session creation in setup_environment()
- Add UnifiedSessionManager to orchestrator
- Implement basic session lifecycle (create, start, complete)

### Phase 2: Integration (Week 1)
- Integrate session updates throughout workflow execution
- Add session cleanup logic
- Implement error handling and logging

### Phase 3: Testing (Week 2)
- Write unit tests for session creation and updates
- Write integration tests for resume functionality
- Verify MapReduce non-regression
- Performance testing for session overhead

### Phase 4: Documentation (Week 2)
- Update ARCHITECTURE.md with session management
- Update CLAUDE.md with session lifecycle
- Add user documentation for session commands
- Document troubleshooting procedures

### Phase 5: Deployment (Week 3)
- Code review and refinement
- Merge to main branch
- Monitor for issues in production use
- Gather user feedback on resume functionality
