# Specification 25: Claude-Assisted Worktree Merge with Conflict Resolution

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: 24

## Context

Currently, `mmm worktree merge` uses standard git merge which fails when conflicts occur. This requires manual intervention and breaks the automated flow. For truly parallel improvement sessions, we need automatic conflict resolution to ensure worktrees can always be merged back successfully.

## Objective

Replace the simple git merge with a Claude-assisted merge process that automatically resolves conflicts, ensuring worktree merges never fail due to conflicts.

## Requirements

### Functional Requirements
- Create `/mmm-merge-worktree` Claude command that handles merging with conflict resolution
- Update `mmm worktree merge` to call Claude instead of direct git merge
- Preserve all commits from the worktree branch
- Generate clear merge commits explaining what was merged and how conflicts were resolved
- Support merging multiple worktrees in sequence

### Non-Functional Requirements
- Conflict resolution should prioritize code correctness over preserving all changes
- Clear documentation of conflict resolution decisions in commit messages
- Maintain audit trail of what conflicts were resolved and how

## Acceptance Criteria

- [ ] `/mmm-merge-worktree` command created in `.claude/commands/`
- [ ] `mmm worktree merge` calls Claude command instead of git merge
- [ ] Conflicts are automatically resolved without manual intervention
- [ ] Merge commits include details about resolved conflicts
- [ ] Multiple worktrees can be merged sequentially
- [ ] Test coverage for conflict scenarios

## Technical Details

### Claude Command Structure

The `/mmm-merge-worktree` command should:
1. Attempt a regular merge first
2. If conflicts occur:
   - Analyze both versions of conflicted files
   - Understand the intent of changes from both branches
   - Resolve conflicts intelligently, preserving functionality
   - Document resolution decisions
3. Create a detailed merge commit with:
   - Summary of what was merged
   - List of conflicts that were resolved
   - Rationale for resolution decisions

### Implementation Approach

1. **Create Claude Command**
   ```markdown
   # /mmm-merge-worktree Command
   
   Merges a worktree branch with intelligent conflict resolution.
   
   ## Usage
   /mmm-merge-worktree <branch-name> [--target <target-branch>]
   
   ## Process
   1. Attempt merge
   2. If conflicts, analyze and resolve
   3. Create detailed merge commit
   ```

2. **Update WorktreeManager**
   - Change `merge_session` to call Claude instead of git directly
   - Handle Claude command output and errors
   - Ensure proper cleanup even if merge is complex

3. **Conflict Resolution Strategy**
   - Prefer newer code that maintains functionality
   - Preserve test additions from both branches
   - Combine documentation updates
   - When in doubt, keep both versions with clear markers

### Example Workflow

```bash
# Multiple parallel sessions
mmm improve --worktree --focus "performance"
mmm improve -w --focus "security"
mmm improve --worktree --focus "testing"

# Merge all with automatic conflict resolution
mmm worktree merge mmm-performance-1234567890
mmm worktree merge mmm-security-1234567891  # Auto-resolves conflicts
mmm worktree merge mmm-testing-1234567892   # Auto-resolves conflicts

# Or merge all at once
mmm worktree merge --all  # Merges all worktrees with conflict resolution
```

## Dependencies

- **Prerequisites**: 
  - Spec 24 (Git worktree isolation) - Base worktree functionality
- **Affected Components**: 
  - `src/worktree/manager.rs` - Update merge_session method
  - `src/main.rs` - Potentially add --all flag for bulk merging
- **New Files**:
  - `.claude/commands/mmm-merge-worktree.md`

## Testing Strategy

- **Unit Tests**: 
  - Mock Claude responses for conflict scenarios
  - Test various conflict types
- **Integration Tests**: 
  - Create actual conflicts and verify resolution
  - Test sequential merging of multiple worktrees
- **Conflict Scenarios**:
  - Same function modified differently
  - File deleted in one branch, modified in another
  - New files with same names but different content
  - Import conflicts

## Documentation Requirements

- **Claude Command Documentation**: 
  - Detailed examples of conflict resolution
  - Best practices for merge strategies
- **User Documentation**: 
  - Update README with new merge behavior
  - Add examples of parallel workflow with merging
- **Architecture Updates**: 
  - Document the Claude-assisted merge flow

## Implementation Notes

1. **Graceful Degradation**: If Claude fails, fall back to marking conflicts for manual resolution
2. **Verification**: After merge, run tests to ensure nothing broke
3. **Rollback**: Keep ability to undo merge if needed
4. **Performance**: Batch multiple worktree merges when possible

## Success Metrics

- Zero failed merges due to conflicts
- All conflicts resolved maintain code functionality
- Clear audit trail of merge decisions
- Faster parallel workflow completion