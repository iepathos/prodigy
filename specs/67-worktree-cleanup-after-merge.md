# Specification 67: Worktree Cleanup After Merge

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: Spec 24 (Git Worktree Isolation), Spec 25 (Claude-Assisted Worktree Merge), Spec 26 (Worktree CLI Flag), Spec 41 (Auto-Accept Flag)

## Context

When using worktree workflows with `mmm cook --worktree`, the system creates a separate git worktree for isolated development. After the cook session completes and changes are merged back to the main branch, the worktree remains on disk. Previously, MMM would prompt users to cleanup the worktree after merging, with automatic cleanup when using the `-y/--yes` flag. This functionality was lost during refactoring and needs to be restored.

Currently:
- The orchestrator merges the worktree when prompted
- The worktree remains on disk after merge
- Users must manually run `mmm worktree clean` to remove it
- The `-y/--yes` flag doesn't trigger automatic cleanup

The standalone `mmm worktree merge` command still prompts for cleanup, showing the expected behavior that should also exist in the cook workflow.

## Objective

Restore the worktree cleanup prompt after successful merge in the cook workflow, with automatic cleanup when using the `-y/--yes` flag for fully automated workflows.

## Requirements

### Functional Requirements
- After successful worktree merge in cook workflow, prompt user to cleanup the worktree
- When `-y/--yes` flag is used, automatically cleanup without prompting
- Display clear success/failure messages for cleanup operations
- Handle cleanup failures gracefully without disrupting the workflow
- Maintain consistency with standalone `mmm worktree merge` behavior

### Non-Functional Requirements
- No performance impact on existing workflows
- Maintain backward compatibility
- Clear and consistent user messaging
- Proper error handling and recovery

## Acceptance Criteria

- [ ] Cook workflow prompts "Would you like to clean up the worktree? (y/N)" after successful merge
- [ ] Answering "y" successfully removes the worktree and branch
- [ ] Answering "n" or pressing Enter leaves the worktree intact
- [ ] With `-y/--yes` flag, cleanup happens automatically without prompt
- [ ] Success message "✅ Worktree cleaned up" displayed after successful cleanup
- [ ] Warning message displayed if cleanup fails, but workflow continues
- [ ] Cleanup behavior matches standalone `mmm worktree merge` command
- [ ] Test coverage for both interactive and auto-accept scenarios

## Technical Details

### Implementation Approach
Modify the `cleanup` method in `DefaultCookOrchestrator` to add cleanup logic after successful merge:

1. After `worktree_manager.merge_session()` succeeds
2. Check for auto-accept flag or prompt user
3. Call `worktree_manager.cleanup_session()` if approved
4. Handle errors gracefully with warning messages

### Architecture Changes
No architectural changes required. This is a restoration of lost functionality within the existing orchestrator.

### Data Structures
No new data structures needed.

### APIs and Interfaces
No API changes. Uses existing:
- `WorktreeManager::cleanup_session(name: &str, force: bool)`
- `UserInteraction::prompt_yes_no(prompt: &str)`

## Dependencies

- **Prerequisites**: 
  - Spec 24: Git Worktree Isolation (completed)
  - Spec 25: Claude-Assisted Worktree Merge (completed)
  - Spec 41: Auto-Accept Flag (completed)
- **Affected Components**: 
  - `cook::orchestrator::DefaultCookOrchestrator::cleanup()`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Test cleanup prompt appears after merge
  - Test auto-accept bypasses prompt
  - Test cleanup success/failure handling
- **Integration Tests**: 
  - Full cook workflow with worktree and cleanup
  - Verify worktree removed from filesystem
  - Verify git branch removed
- **Manual Testing**: 
  - Interactive prompt flow
  - Auto-accept flow
  - Error scenarios (permission denied, etc.)

## Documentation Requirements

- **Code Documentation**: Document the cleanup behavior in orchestrator
- **User Documentation**: Already documented in README, ensure accuracy
- **Architecture Updates**: None required

## Implementation Notes

The implementation should follow the pattern in `main.rs::handle_merge_command()`:
```rust
// After successful merge
if config.command.auto_accept {
    // Auto cleanup
    if let Err(e) = worktree_manager.cleanup_session(worktree_name, true).await {
        eprintln!("⚠️ Warning: Failed to clean up worktree '{}': {}", worktree_name, e);
    } else {
        self.user_interaction.display_success("Worktree cleaned up");
    }
} else {
    // Prompt for cleanup
    let should_cleanup = self.user_interaction
        .prompt_yes_no("Would you like to clean up the worktree?")
        .await?;
    
    if should_cleanup {
        if let Err(e) = worktree_manager.cleanup_session(worktree_name, true).await {
            eprintln!("⚠️ Warning: Failed to clean up worktree '{}': {}", worktree_name, e);
        } else {
            self.user_interaction.display_success("Worktree cleaned up");
        }
    }
}
```

## Migration and Compatibility

No migration required. This restores previously existing functionality that users expect.