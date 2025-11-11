---
number: 158
title: Fix Silent Merge Failure After Workflow Completion
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-11
---

# Specification 158: Fix Silent Merge Failure After Workflow Completion

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

When a MapReduce workflow completes successfully, users are prompted to merge the worktree changes back to the original branch:

```
✅ Cook session completed successfully!
Merge session-4c1dcbe8-722b-44fc-bd43-3e57802ad0d7 to mkdocs [Y/n]: Y
ℹ️ Session complete: 0 iterations, 22 files changed
```

However, the merge does not actually occur, even though the user answered "Y" to the prompt. The worktree remains with 59 commits that were never merged to the target branch, and no error message is displayed to the user.

### Current Behavior

1. Workflow completes successfully - "Cook session completed successfully!" is displayed
2. User is prompted: "Merge session-xxx to target-branch [Y/n]:"
3. User answers "Y"
4. Execution continues without error
5. "Session complete: 0 iterations, 22 files changed" is displayed
6. **BUG**: Merge never happens, no error shown, no success message displayed

### Expected Behavior

1. Workflow completes successfully
2. User is prompted to merge
3. User answers "Y"
4. Merge executes successfully
5. "Worktree changes merged successfully!" is displayed
6. Worktree is cleaned up (if auto-cleanup is enabled)
7. Changes are now in the target branch

### Root Cause Analysis

The execution flow is:

1. `DefaultCookOrchestrator::run()` → `execute_workflow()` → returns `Ok(())`
2. `finalize_session()` is called with `cleanup_fn` parameter (src/cook/orchestrator/core.rs:506)
3. `cleanup_fn.await?` is called (src/cook/orchestrator/execution_pipeline.rs:258)
4. Inside `cleanup()`:
   - User is prompted (src/cook/orchestrator/core.rs:771)
   - `should_merge` is set to `true` based on user's "Y" response
   - `worktree_manager.merge_session(worktree_name).await?` is called (line 796)
   - Either this call fails silently OR returns without merging

**Potential Issues**:

1. **Error Swallowing**: The `?` operator at line 796 propagates errors, but the error may be caught and swallowed somewhere in the call stack
2. **Silent Failure**: `merge_session()` may be returning `Ok(())` without actually performing the merge due to a validation check
3. **Missing Error Handling**: Errors from `cleanup_fn.await?` may not be properly displayed to the user

## Objective

Ensure that when a user approves a worktree merge after workflow completion:
1. The merge actually executes
2. Success or failure is clearly communicated to the user
3. Any errors are displayed with actionable information
4. The worktree state is properly updated

## Requirements

### Functional Requirements

- **FR1**: When user answers "Y" to merge prompt, the merge MUST execute
- **FR2**: Merge success MUST display: "Worktree changes merged successfully!"
- **FR3**: Merge failure MUST display clear error message with:
  - What failed (e.g., "Failed to merge worktree session-xxx to target-branch")
  - Why it failed (the actual error message)
  - How to retry (e.g., "Run: prodigy worktree merge session-xxx")
- **FR4**: If merge fails, the worktree MUST NOT be deleted
- **FR5**: Cleanup errors MUST NOT be silently swallowed
- **FR6**: Session completion summary MUST reflect actual merge status

### Non-Functional Requirements

- **NFR1**: Error messages must be user-friendly and actionable
- **NFR2**: Merge failures must not lose user's work
- **NFR3**: Debug logging must capture merge execution flow
- **NFR4**: Changes must not break existing workflows (standard, MapReduce, argument-based)

## Acceptance Criteria

- [ ] **AC1**: MapReduce workflow completion prompts for merge and executes when user confirms
- [ ] **AC2**: Successful merge displays "Worktree changes merged successfully!"
- [ ] **AC3**: Failed merge displays error with retry instructions
- [ ] **AC4**: Merge errors do not prevent session completion summary from displaying
- [ ] **AC5**: All merge attempts are logged at INFO level with outcome
- [ ] **AC6**: Test coverage for merge success and failure paths
- [ ] **AC7**: Integration test verifying end-to-end merge flow for MapReduce workflows
- [ ] **AC8**: No regression in standard workflow merge behavior
- [ ] **AC9**: Worktree is only cleaned up after successful merge (when auto-cleanup enabled)
- [ ] **AC10**: User can manually retry merge using `prodigy worktree merge <session-name>` after failure

## Technical Details

### Implementation Approach

1. **Add Error Visibility**:
   - Wrap `merge_session` call in `src/cook/orchestrator/core.rs:796` with explicit error handling
   - Log merge attempts and outcomes
   - Display errors to user before propagating

2. **Improve Error Context**:
   - Add `.context()` to `merge_session` call with session name and target branch
   - Include worktree path in error messages for debugging

3. **Separate Merge from Cleanup**:
   - Consider making merge failures non-fatal to cleanup flow
   - Allow session completion summary to display even if merge fails
   - Store merge status in session state

4. **Add Defensive Checks**:
   - Verify worktree exists before merge attempt
   - Check if target branch exists
   - Validate commit count before merge
   - Log validation results

### Code Locations

**Primary Issue**: `src/cook/orchestrator/core.rs:774-805` (cleanup function)

```rust
if should_merge {
    // Current code (line 796):
    worktree_manager.merge_session(worktree_name).await?;

    // Proposed change:
    match worktree_manager.merge_session(worktree_name).await {
        Ok(_) => {
            self.user_interaction
                .display_success("Worktree changes merged successfully!");
            log::info!("Successfully merged {} to {}", worktree_name, merge_target);
        }
        Err(e) => {
            let error_msg = format!(
                "Failed to merge worktree '{}' to '{}': {}",
                worktree_name, merge_target, e
            );
            self.user_interaction.display_error(&error_msg);
            self.user_interaction.display_info(&format!(
                "\n💡 To retry merge: prodigy worktree merge {}",
                worktree_name
            ));
            log::error!("{}", error_msg);
            // Don't propagate - allow cleanup to continue
        }
    }
}
```

**Secondary Locations**:
- `src/worktree/manager.rs:343-373` - `merge_session()` implementation
- `src/cook/orchestrator/execution_pipeline.rs:257-258` - cleanup_fn invocation
- `src/worktree/orchestrator.rs` - merge workflow execution

### Architecture Changes

None. This is a bug fix within existing architecture.

### Data Structures

Consider adding to `SessionState` or `WorktreeState`:
```rust
pub struct MergeStatus {
    pub attempted: bool,
    pub succeeded: bool,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}
```

### APIs and Interfaces

No public API changes. Internal error handling improvement only.

## Dependencies

**Prerequisites**: None - this is a bug fix

**Affected Components**:
- `DefaultCookOrchestrator` - cleanup function
- `WorktreeManager` - merge_session error reporting
- `UserInteraction` - error display

**External Dependencies**: None

## Testing Strategy

### Unit Tests

1. **Test: cleanup_handles_merge_failure_gracefully**
   - Mock `merge_session` to return error
   - Verify error is logged and displayed
   - Verify cleanup continues (doesn't panic/exit)
   - Verify session completes

2. **Test: cleanup_displays_success_on_merge_success**
   - Mock `merge_session` to return success
   - Verify "Worktree changes merged successfully!" is displayed
   - Verify info log is written

3. **Test: merge_session_failure_includes_retry_instructions**
   - Simulate merge failure
   - Verify error message includes `prodigy worktree merge` command
   - Verify includes session name

### Integration Tests

1. **Test: mapreduce_workflow_merge_on_completion**
   - Run actual MapReduce workflow in test
   - Answer "Y" to merge prompt
   - Verify merge executes
   - Verify target branch contains worktree commits
   - Verify success message displayed

2. **Test: mapreduce_workflow_merge_failure_recovery**
   - Run MapReduce workflow
   - Inject merge failure (e.g., target branch doesn't exist)
   - Verify error displayed
   - Verify retry instructions shown
   - Verify worktree not deleted
   - Verify manual merge works: `prodigy worktree merge <session>`

3. **Test: standard_workflow_merge_unchanged**
   - Run standard (non-MapReduce) workflow
   - Verify merge behavior is unchanged
   - Ensure no regression

### Performance Tests

Not applicable - this is error handling logic with negligible performance impact.

### User Acceptance

**Scenario 1**: Successful Merge
```
$ prodigy run workflows/mkdocs-drift.yml
...
✅ Cook session completed successfully!
Merge session-xxx to mkdocs [Y/n]: Y
✅ Worktree changes merged successfully!
ℹ️ Session complete: 0 iterations, 22 files changed
```

**Scenario 2**: Failed Merge
```
$ prodigy run workflows/mkdocs-drift.yml
...
✅ Cook session completed successfully!
Merge session-xxx to mkdocs [Y/n]: Y
❌ Failed to merge worktree 'session-xxx' to 'mkdocs': merge conflict in file.txt

💡 To retry merge: prodigy worktree merge session-xxx
ℹ️ Session complete: 0 iterations, 22 files changed
```

## Documentation Requirements

### Code Documentation

- Add doc comments to cleanup error handling explaining why merge failures are non-fatal
- Document merge retry workflow in `WorktreeManager::merge_session`

### User Documentation

- Update troubleshooting section with merge failure recovery steps
- Add section on manual merge retry: `prodigy worktree merge <session>`
- Document when worktrees are auto-cleaned vs preserved

### Architecture Updates

- No ARCHITECTURE.md changes needed - bug fix only

## Implementation Notes

### Gotchas

1. **Don't Make Merge Failures Fatal**: If merge fails, session should still complete gracefully
2. **Preserve Worktree on Failure**: Never delete worktree if merge failed - user's work must be preserved
3. **Log Everything**: Merge is critical operation - log attempts, outcomes, and errors
4. **Test Both Paths**: Ensure both success and failure paths work correctly

### Best Practices

- Use `match` instead of `?` for merge_session to handle both success and failure explicitly
- Include context in all error messages (session name, target branch, operation)
- Provide actionable next steps in error messages
- Log at appropriate levels (INFO for success, ERROR for failure)

## Migration and Compatibility

### Breaking Changes

None. This is a bug fix that makes existing functionality work correctly.

### Migration Requirements

None. Changes are backward compatible.

### Compatibility Considerations

- Existing workflows continue to work as before
- Merge behavior becomes more robust and visible
- Error messages improve user experience

## Related Issues

This bug was discovered when running:
```bash
prodigy run workflows/mkdocs-drift.yml
```

The workflow completed all 19 agents successfully (100% success rate), user approved merge with "Y", but merge never happened. Manual merge was required:
```bash
git merge --no-ff prodigy-session-4c1dcbe8-722b-44fc-bd43-3e57802ad0d7
```
