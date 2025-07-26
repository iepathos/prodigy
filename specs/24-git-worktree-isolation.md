# Specification 24: Git Worktree Isolation for Parallel MMM Sessions

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: 14, 19

## Context

Currently, when multiple `mmm improve` sessions run in the same git repository, they can clash with each other's changes. Each session creates commits in the same branch, potentially causing conflicts and making it difficult to manage parallel improvement efforts. This becomes problematic when users want to run multiple improvement sessions with different focus areas or workflows simultaneously.

Git worktrees provide an elegant solution by allowing multiple working directories for the same repository, each with its own branch and checkout state. This enables true parallel execution without conflicts.

## Objective

Enable multiple MMM improvement sessions to run concurrently in the same repository without interfering with each other by isolating each session in its own git worktree.

## Requirements

### Functional Requirements
- Each `mmm improve` session creates and operates in its own git worktree
- Worktrees are uniquely named (e.g., `mmm-session-{timestamp}` or `mmm-{focus}-{timestamp}`)
- Sessions automatically clean up their worktrees on completion or failure
- Support for listing active MMM worktrees
- Ability to merge completed worktrees back to the main branch

### Non-Functional Requirements
- Minimal performance overhead from worktree operations
- Graceful handling of worktree creation failures
- Clear error messages for git-related issues
- Backward compatibility for repositories without worktree support

## Acceptance Criteria

- [ ] Running `mmm improve` creates a new worktree with a unique branch
- [ ] Multiple concurrent `mmm improve` sessions work without conflicts
- [ ] Each session's commits are isolated to its own branch
- [ ] Worktrees are automatically cleaned up after session completion
- [ ] A new command exists to list active MMM worktrees
- [ ] A new command exists to merge a worktree's changes to master
- [ ] Error handling for scenarios where worktrees cannot be created
- [ ] Documentation updated with new workflow patterns

## Technical Details

### Implementation Approach

1. **Worktree Creation**
   - Before starting improvements, create a new worktree
   - Use naming pattern: `mmm-{focus}-{timestamp}` or `mmm-session-{timestamp}`
   - Create in `.mmm/worktrees/` directory (gitignored)
   - Create a new branch from current HEAD

2. **Session Execution**
   - Change working directory to the new worktree
   - Run all improvement commands in worktree context
   - All git operations happen in the isolated branch

3. **Cleanup Strategy**
   - On successful completion, optionally merge to original branch
   - On failure or interruption, preserve worktree for debugging
   - Provide command to clean up abandoned worktrees

### Architecture Changes

1. **New Module: `src/worktree/`**
   - `manager.rs`: Worktree lifecycle management
   - `mod.rs`: Module exports and types

2. **Updates to `src/improve/mod.rs`**
   - Add worktree creation before improvement loop
   - Execute all operations in worktree context
   - Handle cleanup on exit

3. **New Commands**
   - `mmm worktree list`: Show active MMM worktrees
   - `mmm worktree merge <name>`: Merge a worktree to current branch
   - `mmm worktree clean`: Remove completed/abandoned worktrees

### Data Structures

```rust
pub struct WorktreeSession {
    pub name: String,
    pub branch: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub focus: Option<String>,
}

pub struct WorktreeManager {
    pub base_dir: PathBuf,
    pub repo_path: PathBuf,
}
```

### APIs and Interfaces

```rust
impl WorktreeManager {
    pub fn create_session(&self, focus: Option<&str>) -> Result<WorktreeSession>;
    pub fn list_sessions(&self) -> Result<Vec<WorktreeSession>>;
    pub fn merge_session(&self, name: &str) -> Result<()>;
    pub fn cleanup_session(&self, name: &str) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 14 (Real Claude CLI integration) - Core improvement loop
  - Spec 19 (Git-native improvement flow) - Git commit-based workflow
- **Affected Components**: 
  - `improve/mod.rs` - Will execute in worktree context
  - `main.rs` - New commands for worktree management
- **External Dependencies**: 
  - Git 2.5+ (worktree support)

## Testing Strategy

- **Unit Tests**: 
  - Worktree creation and naming
  - Session tracking and listing
  - Error handling for git operations
- **Integration Tests**: 
  - Full improvement cycle in worktree
  - Concurrent session execution
  - Merge operations
- **Performance Tests**: 
  - Overhead of worktree operations
  - Scalability with multiple worktrees
- **User Acceptance**: 
  - Clear workflow for parallel improvements
  - Intuitive merge process

## Documentation Requirements

- **Code Documentation**: 
  - Document worktree lifecycle
  - Examples of concurrent usage patterns
- **User Documentation**: 
  - Update README with parallel execution examples
  - Add section on worktree management commands
  - Best practices for merging improvements
- **Architecture Updates**: 
  - Add worktree module to ARCHITECTURE.md
  - Update data flow diagrams

## Implementation Notes

1. **Worktree Location**: Store in `.mmm/worktrees/` to keep them organized and hidden
2. **Branch Naming**: Use descriptive names that include timestamp and optional focus
3. **Atomic Operations**: Ensure worktree creation/deletion is atomic
4. **Git Version Check**: Detect git version and provide fallback for older versions
5. **Progress Preservation**: Keep worktree on failure for debugging

## Migration and Compatibility

- No breaking changes to existing functionality
- Single-session mode continues to work as before
- Worktree mode is opt-in initially (could become default later)
- Graceful degradation for older git versions
- Clear migration path for existing users