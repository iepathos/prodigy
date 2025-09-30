---
number: 109
title: Remove Legacy Session System
category: refactor
priority: high
status: draft
dependencies: []
created: 2025-09-30
---

# Specification 109: Remove Legacy Session System

**Category**: refactor
**Priority**: high
**Status**: draft
**Dependencies**: None (UnifiedSessionManager already in use)

## Context

Prodigy currently has two session management systems that cause confusion and technical debt:

1. **Legacy System** (`src/cook/session/`):
   - `SessionTrackerImpl` - file-based session tracking in worktrees
   - Local `.prodigy/session_state.json` files
   - Tightly coupled to cook orchestrator
   - Limited to worktree-based storage

2. **Unified System** (`src/unified_session/`):
   - `UnifiedSessionManager` - global storage-based session management
   - Centralized in `~/.prodigy/state/{repo}/sessions/`
   - Used via `CookSessionAdapter` bridge
   - Supports both workflow and MapReduce sessions

The orchestrator already uses UnifiedSessionManager through CookSessionAdapter, making the legacy system redundant. The legacy code remains only for backward compatibility and causes:
- Code confusion and maintenance burden
- Duplicate session management logic
- Unclear ownership of session state
- Difficulty understanding the actual session flow

## Objective

Remove the legacy session system code while ensuring all functionality is preserved through UnifiedSessionManager, improving code clarity and reducing technical debt.

## Requirements

### Functional Requirements
- All session management must continue to work through UnifiedSessionManager
- Resume functionality must remain operational
- Session state tracking must be preserved
- Checkpoint integration must continue to function
- Test coverage must be maintained or improved

### Non-Functional Requirements
- No breaking changes to public APIs or CLI commands
- Existing sessions should gracefully degrade (old sessions won't be resumable)
- Code should be simpler and easier to understand
- Build time should not increase
- All existing tests must pass or be updated appropriately

## Acceptance Criteria

- [ ] Remove `src/cook/session/tracker.rs` (SessionTrackerImpl)
- [ ] Keep `src/cook/session/mod.rs` as trait-only module (SessionManager trait is used by CookSessionAdapter)
- [ ] Keep `src/cook/session/state.rs` (SessionState is used by CookSessionAdapter)
- [ ] Keep `src/cook/session/summary.rs` (SessionSummary is used by orchestrator)
- [ ] Remove `src/cook/session/resume_tests.rs` if tests are redundant with unified_session tests
- [ ] Update all imports to use only unified_session types where appropriate
- [ ] Verify CookSessionAdapter properly implements all SessionManager trait methods
- [ ] Update or remove tests that depend on SessionTrackerImpl
- [ ] All existing integration tests pass
- [ ] Resume command works with newly created sessions
- [ ] Documentation is updated to reflect single session system
- [ ] No compiler warnings related to removed code

## Technical Details

### Implementation Approach

1. **Phase 1: Analysis**
   - Identify all usages of SessionTrackerImpl
   - Map legacy functionality to UnifiedSessionManager equivalents
   - Identify tests that need updates
   - Document any gaps in UnifiedSessionManager functionality

2. **Phase 2: Preparation**
   - Ensure CookSessionAdapter fully implements SessionManager trait
   - Verify all SessionManager trait methods are tested
   - Update any mock implementations used in tests
   - Create migration notes for any breaking changes

3. **Phase 3: Removal**
   - Delete `src/cook/session/tracker.rs`
   - Remove SessionTrackerImpl from `src/cook/session/mod.rs`
   - Keep SessionManager trait (used by CookSessionAdapter)
   - Keep supporting types (SessionState, SessionSummary, SessionUpdate)
   - Remove resume_tests.rs if redundant

4. **Phase 4: Cleanup**
   - Update all imports and references
   - Remove unused mock implementations
   - Update test utilities
   - Clean up any dead code flagged by compiler

5. **Phase 5: Verification**
   - Run full test suite
   - Test resume functionality manually
   - Verify no regressions in workflow execution
   - Check that documentation is accurate

### Files to Keep (Used by CookSessionAdapter)

```
src/cook/session/
├── mod.rs          # SessionManager trait definition (KEEP)
├── state.rs        # SessionState, SessionStatus, etc (KEEP)
└── summary.rs      # SessionSummary (KEEP)
```

### Files to Remove

```
src/cook/session/
├── tracker.rs        # SessionTrackerImpl (REMOVE)
└── resume_tests.rs   # Legacy tests (REMOVE if redundant)
```

### Components Using Legacy Session Code

Based on grep analysis, these components use `cook::session`:
- `src/unified_session/cook_adapter.rs` - Uses trait and types (keep)
- `src/unified_session/tests.rs` - Uses trait for testing (keep)
- `src/testing/mocks/session.rs` - May need updates
- `src/cook/workflow/executor.rs` - Uses SessionManager trait
- `src/cook/workflow/resume.rs` - Uses SessionManager trait
- `src/cook/execution/mapreduce/*.rs` - Uses SessionManager trait
- Various test files - Need review

### CookSessionAdapter Verification

The adapter implements all required SessionManager trait methods:
- ✅ `start_session()` - Creates UnifiedSession
- ✅ `update_session()` - Updates UnifiedSession
- ✅ `complete_session()` - Completes UnifiedSession
- ✅ `get_state()` - Returns cached state (recently fixed)
- ✅ `save_state()` - Delegated to UnifiedSessionManager
- ✅ `load_state()` - Delegated to UnifiedSessionManager
- ✅ `load_session()` - Loads from UnifiedSessionManager
- ✅ `save_checkpoint()` - Saves to UnifiedSessionManager
- ✅ `list_resumable()` - Lists from UnifiedSessionManager
- ✅ `get_last_interrupted()` - Queries UnifiedSessionManager

## Dependencies

**Prerequisites**: None - UnifiedSessionManager is already in production use

**Affected Components**:
- Cook orchestrator (already uses CookSessionAdapter)
- Workflow executor (uses SessionManager trait)
- Resume functionality (uses SessionManager trait)
- MapReduce execution (uses SessionManager trait)
- Test infrastructure (mocks may need updates)

**External Dependencies**: None

## Testing Strategy

### Unit Tests
- Verify CookSessionAdapter implements all trait methods correctly
- Test session state caching behavior
- Test error handling in adapter methods
- Verify state conversions between unified and cook types

### Integration Tests
- Test workflow execution end-to-end
- Test resume functionality with new sessions
- Test MapReduce session management
- Test checkpoint creation and loading
- Verify session state persistence across restarts

### Regression Tests
- Run full test suite before and after changes
- Compare test results to ensure no regressions
- Test resume command manually
- Test workflow interruption and resume
- Verify session listing commands work

### Manual Testing
```bash
# Test basic workflow
prodigy run workflows/test.yml

# Test interruption and resume
prodigy run workflows/long-workflow.yml
# Interrupt with Ctrl+C
prodigy resume <session-id>

# Test session listing
prodigy sessions list
```

## Documentation Requirements

### Code Documentation
- Document that SessionManager trait is the adapter interface
- Add comments explaining why trait/types are kept
- Document CookSessionAdapter as the bridge implementation
- Update module-level documentation in cook/session/mod.rs

### User Documentation
- No user-facing changes (session management is internal)
- Update CLAUDE.md if it mentions session architecture
- Update any architecture diagrams showing session flow

### Architecture Updates
- Update ARCHITECTURE.md to show single session system
- Remove references to dual session systems
- Document UnifiedSessionManager as the authoritative source
- Explain CookSessionAdapter bridge pattern

## Implementation Notes

### Important Considerations

1. **Trait Preservation**: The SessionManager trait must be kept because it's the interface CookSessionAdapter implements. It provides abstraction between cook orchestrator and session management.

2. **Type Preservation**: SessionState, SessionStatus, SessionUpdate, and SessionSummary types must be kept because they're used throughout the cook codebase and by CookSessionAdapter.

3. **Mock Updates**: Test mocks that implement SessionManager trait may need updates to work with new unified-only approach.

4. **Backward Compatibility**: Old sessions created before unified session fix won't be resumable. This is acceptable as stated in recent commit.

5. **Gradual Approach**: Consider removing code in stages to catch issues early:
   - Stage 1: Remove SessionTrackerImpl implementation
   - Stage 2: Clean up resume_tests.rs
   - Stage 3: Update mocks and test utilities
   - Stage 4: Clean up any flagged dead code

### Potential Issues

1. **Test Mocks**: Tests using SessionTrackerImpl directly will need updates to use CookSessionAdapter or a mock implementation.

2. **Import Cleanup**: Many files import from cook::session and may need path updates.

3. **Hidden Dependencies**: There may be conditional compilation or test-only code paths using SessionTrackerImpl.

### Success Criteria

The refactoring is successful when:
- All tests pass
- Resume functionality works for new sessions
- Code is clearer and easier to understand
- No duplicate session management logic remains
- Documentation accurately reflects architecture
- No performance regressions

## Migration and Compatibility

### Breaking Changes
- Old sessions created before UnifiedSessionManager will not be resumable
- This is acceptable and documented in recent commits
- Users should be aware that interrupting old sessions won't allow resume

### Migration Path
1. Run new version to create sessions with unified storage
2. Old sessions will gracefully fail with "Session not found" error
3. Users can re-run workflows to create new resumable sessions
4. No data migration needed (old session files can remain)

### Compatibility Notes
- Public API (CLI commands) unchanged
- Session file format remains internal implementation detail
- UnifiedSessionManager storage format is stable and versioned
- Future sessions will work consistently across versions

## Timeline

**Estimated Effort**: 4-6 hours
- Analysis: 1 hour
- Code removal: 1-2 hours
- Test updates: 1-2 hours
- Documentation: 1 hour
- Verification: 1 hour

**Risk Level**: Medium
- Risk: Breaking tests or resume functionality
- Mitigation: Comprehensive testing, staged approach
- Rollback: Git revert if issues found

## Success Metrics

- ✅ Lines of code reduced by ~500-1000 (removing tracker.rs)
- ✅ Number of session-related files reduced from 5 to 3
- ✅ Test coverage maintained or improved
- ✅ Build time unchanged or improved
- ✅ Zero regression bugs reported
- ✅ Documentation reflects single session system