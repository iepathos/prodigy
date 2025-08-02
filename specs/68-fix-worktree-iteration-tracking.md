---
number: 68
title: Fix Worktree Iteration Tracking Synchronization
category: testing
priority: high
status: draft
dependencies: []
created: 2025-08-02
---

# Specification 68: Fix Worktree Iteration Tracking Synchronization

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The worktree list command shows `(0/10)` for all worktree sessions, indicating 0 completed iterations out of 10 maximum, even for successfully merged sessions that completed multiple iterations. This misleading display occurs because there are two separate session tracking systems that are not synchronized:

1. **Cook Session Tracking** (`src/cook/session/`) - Properly tracks and increments iterations via `SessionUpdate::IncrementIteration`
2. **Worktree State Tracking** (`src/worktree/state.rs`) - Has its own `iterations.completed` field that is never updated

The cook orchestrator correctly calls `session_manager.update_session(SessionUpdate::IncrementIteration)` during workflow execution, but this only updates the internal cook session state. There is no mechanism to propagate iteration updates to the worktree state that is displayed by `mmm worktree list`.

## Objective

Synchronize iteration tracking between the cook session management system and worktree state management so that `mmm worktree list` displays accurate iteration counts that reflect the actual progress made during cooking sessions.

## Requirements

### Functional Requirements
- Worktree state `iterations.completed` field must be updated when cook session iterations are incremented
- `mmm worktree list` must display accurate iteration counts for all sessions (active, completed, merged)
- Synchronization must work for both regular and worktree-isolated cooking sessions
- Historical worktree sessions should maintain accurate iteration counts after completion
- No breaking changes to existing APIs or data structures

### Non-Functional Requirements
- Synchronization should be efficient with minimal performance overhead
- Updates should be atomic to prevent inconsistent states
- Error handling must gracefully handle synchronization failures without breaking cooking workflows
- Backward compatibility with existing worktree state files

## Acceptance Criteria

- [ ] `mmm worktree list` shows accurate iteration counts (e.g., `(3/10)` instead of `(0/10)`)
- [ ] Merged sessions display the total iterations completed before merge
- [ ] Active sessions show real-time iteration progress during cooking
- [ ] Failed sessions preserve iteration count at point of failure
- [ ] All tests pass with correct iteration tracking behavior
- [ ] No regression in cooking session performance or reliability
- [ ] Existing worktree state files continue to work (backward compatibility)

## Technical Details

### Implementation Approach

1. **Create Synchronization Bridge**: Add mechanism to propagate iteration updates from cook session to worktree state
2. **Update Cook Orchestrator**: Modify cooking workflow to update both session tracking systems
3. **Centralize Update Logic**: Ensure all iteration increments go through a unified update path
4. **Add Error Handling**: Handle synchronization failures gracefully without breaking workflows

### Architecture Changes

1. **Cook Session Manager Interface**: Extend to notify worktree manager of iteration updates
2. **Worktree Manager Integration**: Add method to receive and apply iteration updates from cook sessions
3. **Orchestrator Updates**: Modify workflow execution to update both tracking systems atomically

### Data Structures

No changes to existing data structures required - both `cook::session::SessionState.iterations_completed` and `worktree::WorktreeState.iterations.completed` already exist.

### APIs and Interfaces

1. **New Method**: `WorktreeManager::update_iteration_count(session_name: &str, iterations: u32)`
2. **Enhanced Cook Orchestrator**: Update iteration logic to synchronize both systems
3. **Session Update Integration**: Connect cook session updates to worktree state updates

## Dependencies

- **Prerequisites**: None - uses existing session and worktree management infrastructure
- **Affected Components**: 
  - `src/cook/orchestrator.rs` - cooking workflow execution
  - `src/cook/session/` - session state management  
  - `src/worktree/manager.rs` - worktree state management
  - `src/main.rs` - worktree list display logic
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test synchronization logic between cook and worktree sessions
- **Integration Tests**: Test full cooking workflow with accurate iteration tracking
- **Regression Tests**: Ensure existing functionality continues to work
- **Edge Case Tests**: Test error scenarios and recovery from synchronization failures

## Documentation Requirements

- **Code Documentation**: Add inline documentation for synchronization logic
- **User Documentation**: No user-facing documentation changes needed (improvement is transparent)
- **Architecture Updates**: Update `ARCHITECTURE.md` to document synchronization between tracking systems

## Implementation Notes

The root cause is architectural - two independent tracking systems evolved without coordination. The fix requires bridging these systems at the point where iterations are incremented in the cooking workflow.

Key implementation considerations:
- Use existing `WorktreeManager::update_session_state()` method for atomic worktree updates
- Add iteration synchronization to existing `SessionUpdate::IncrementIteration` handling
- Ensure synchronization works for both worktree and non-worktree sessions
- Handle cases where worktree state files might not exist (graceful degradation)

## Migration and Compatibility

No migration required - this is a bug fix that improves accuracy of existing functionality. Existing worktree state files will start showing accurate iteration counts as soon as sessions resume activity.

The fix maintains full backward compatibility:
- Existing state file formats remain unchanged
- No changes to CLI interfaces or user workflows  
- No breaking changes to internal APIs