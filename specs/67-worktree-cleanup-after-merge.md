# Spec 67: Worktree Cleanup After Merge

## Overview

Implement automatic cleanup of Git worktrees after successful merge operations to prevent accumulation of stale worktrees and maintain a clean development environment.

## Context

Currently, MMM creates Git worktrees for isolated development sessions but doesn't automatically clean them up after merge operations. This leads to:

- Accumulation of stale worktree directories
- Potential disk space issues
- Confusion about which worktrees are active
- Manual cleanup burden on developers

## Requirements

### Core Functionality

1. **Automatic Detection**: Detect when a worktree branch has been successfully merged into the main branch
2. **Safe Cleanup**: Remove worktree directory and Git worktree references after merge
3. **Confirmation**: Provide confirmation before cleanup with option to skip
4. **Logging**: Log cleanup operations for audit trail

### Implementation Details

1. **Merge Detection Logic**:
   - Check if worktree branch exists in remote
   - Verify merge commit exists in main branch history
   - Confirm no pending changes in worktree

2. **Cleanup Process**:
   - Remove worktree directory from filesystem
   - Remove Git worktree reference (`git worktree remove`)
   - Update MMM session state to mark as cleaned up
   - Log cleanup operation

3. **Safety Checks**:
   - Verify no uncommitted changes exist
   - Confirm branch has been merged (not just deleted)
   - Check that worktree is not currently active

### Integration Points

- **Post-merge hooks**: Trigger cleanup after successful merge operations
- **Session management**: Update session state to reflect cleanup status  
- **Command interface**: Provide `mmm worktree cleanup` command for manual cleanup
- **Status reporting**: Include cleanup status in worktree status commands

## Technical Implementation

### Files to Modify

1. `src/worktree/manager.rs` - Add cleanup functionality
2. `src/worktree/mod.rs` - Expose cleanup interface
3. `src/git/mod.rs` - Add merge detection utilities
4. `src/session/state.rs` - Track cleanup status
5. `src/cook/git_ops.rs` - Integrate cleanup into merge workflow

### New Components

1. **WorktreeCleanup struct**: Handle cleanup logic and safety checks
2. **MergeDetector trait**: Detect successful merge operations
3. **CleanupPolicy enum**: Define cleanup strategies (automatic, manual, disabled)

### Configuration

Add configuration options to control cleanup behavior:

```yaml
worktree:
  cleanup:
    auto_cleanup: true
    confirm_before_cleanup: true
    retention_days: 7  # Keep worktrees for N days after merge
    dry_run: false
```

## Test Requirements

1. **Unit Tests**:
   - Test merge detection logic
   - Test cleanup safety checks
   - Test configuration handling

2. **Integration Tests**:
   - Test full cleanup workflow
   - Test cleanup after successful merge
   - Test skip cleanup when uncommitted changes exist

3. **Edge Cases**:
   - Handle worktree with uncommitted changes
   - Handle missing or corrupted worktree
   - Handle network issues during merge detection

## Success Criteria

1. Worktrees are automatically cleaned up after successful merges
2. Safety checks prevent accidental data loss
3. Configuration allows customization of cleanup behavior
4. Comprehensive logging provides audit trail
5. Manual cleanup command works for edge cases

## Risk Mitigation

1. **Data Loss Prevention**: Multiple safety checks before cleanup
2. **Rollback Capability**: Maintain cleanup logs for potential recovery
3. **Gradual Rollout**: Start with confirmation required, move to automatic
4. **Testing**: Comprehensive test coverage for all cleanup scenarios

## Dependencies

- Git worktree commands (`git worktree list`, `git worktree remove`)
- Git merge detection (`git merge-base`, `git branch --merged`)
- Filesystem operations for directory cleanup
- MMM session state management

## Estimated Effort

- Implementation: 2-3 days
- Testing: 1-2 days  
- Documentation: 0.5 days
- **Total**: 3.5-5.5 days

## Acceptance Criteria

- [ ] Automatic cleanup after merge is implemented and working
- [ ] Safety checks prevent accidental cleanup of active worktrees
- [ ] Configuration options control cleanup behavior
- [ ] Manual cleanup command is available
- [ ] Comprehensive test coverage exists
- [ ] Documentation is updated
- [ ] Cleanup operations are properly logged