# Specification 51: Validate Git Commits After Workflow Commands

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [19 (Git-Native Improvement Flow)]

## Context

Currently, MMM assumes that a successful command execution (exit code 0) means changes were made. This assumption is incorrect and leads to several problems:

1. Commands may succeed without making any changes (e.g., when a spec is already implemented)
2. The workflow continues even when no actual work was done
3. Worktree merges report success even when no commits exist to merge
4. Users get misleading feedback about what actually happened

As demonstrated in a recent workflow run, `/mmm-implement-spec 50` reported "made changes" but no git commits were created, resulting in a confusing "successful" merge with no actual changes.

## Objective

Replace the current assumption-based change detection with actual git commit validation. When a command completes, verify that git commits were created. If no commits were made, stop the workflow with a clear message explaining why.

## Requirements

### Functional Requirements
- After each workflow command execution, check if new git commits were created
- Track the git HEAD before and after each command
- If no commits were created after a command that's expected to make changes:
  - Stop the workflow immediately
  - Display a clear message explaining why the workflow stopped
  - Indicate which command failed to produce commits
- Commands should be categorized by whether they're expected to create commits
- Provide option to continue workflow even without commits (for special cases)

### Non-Functional Requirements
- Minimal performance impact (git operations are fast)
- Clear and actionable error messages
- Maintain backward compatibility with existing workflows
- Respect test mode behavior (don't check commits in test mode)

## Acceptance Criteria

- [ ] Workflow executor tracks git HEAD before each command execution
- [ ] After command execution, verify if HEAD changed or new commits exist
- [ ] Commands that should create commits are properly identified
- [ ] Workflow stops with clear message when expected commits aren't created
- [ ] Error message includes command name and suggestions for resolution
- [ ] Test mode continues to work without git validation
- [ ] Option exists to bypass commit validation if needed
- [ ] Existing workflows continue to function with new validation

## Technical Details

### Implementation Approach

1. Modify `WorkflowExecutor::execute_structured_command` to:
   - Capture git HEAD before execution
   - After execution, check if HEAD changed
   - Return tuple: `(success: bool, changes_made: bool, output: Option<String>)`

2. Update `execute_iteration` to:
   - Check the `changes_made` flag
   - Stop iteration if no changes when expected
   - Provide clear user feedback

3. Command categorization:
   - Commands expected to create commits: `mmm-implement-spec`, `mmm-cleanup-tech-debt`, `mmm-lint` (when fixes are made)
   - Commands that may not create commits: `mmm-code-review` (only creates specs), analysis commands

### Architecture Changes

```rust
// Before
async fn execute_structured_command(&self, command: &Command) -> Result<(bool, Option<String>)>

// After  
async fn execute_structured_command(&self, command: &Command) -> Result<(bool, bool, Option<String>)>
//                                                                    (success, changes_made, output)
```

### Git Operations Required

```rust
// Before command execution
let head_before = git_ops::git_command(&["rev-parse", "HEAD"], "get HEAD before command").await?;

// After command execution
let head_after = git_ops::git_command(&["rev-parse", "HEAD"], "get HEAD after command").await?;
let changes_made = head_before != head_after;

// Alternative: Check for uncommitted changes
let status = git_ops::check_git_status().await?;
let has_changes = !status.contains("nothing to commit");
```

### Error Messages

When no commits are detected:
```
❌ Workflow stopped: No changes were committed by /mmm-implement-spec

The command executed successfully but did not create any git commits.
Possible reasons:
- The specification may already be implemented
- The command may have encountered an issue without reporting an error
- No changes were needed

To investigate:
- Check if the spec is already implemented
- Review the command output above for any warnings
- Run 'git status' to check for uncommitted changes

To continue anyway, run with --no-commit-validation
```

## Dependencies

- **Prerequisites**: 
  - Spec 19: Git-Native Improvement Flow (established git-based workflow)
- **Affected Components**: 
  - `workflow.rs` - Main execution logic changes
  - `cook/mod.rs` - May need to pass validation flags
  - All workflow commands - Need to handle new validation
- **External Dependencies**: Git must be available (already required)

## Testing Strategy

- **Unit Tests**: 
  - Mock git operations to test commit detection
  - Test both commit and no-commit scenarios
  - Verify error message generation
- **Integration Tests**: 
  - Create actual git repos in tests
  - Test full workflow with and without commits
  - Verify workflow stops appropriately
- **Performance Tests**: 
  - Measure overhead of git HEAD checks
  - Ensure minimal impact on workflow execution time
- **User Acceptance**: 
  - Test with real workflows that both do and don't make changes
  - Verify error messages are helpful and actionable

## Documentation Requirements

- **Code Documentation**: 
  - Document the new return tuple format
  - Explain commit validation logic
  - Document which commands expect commits
- **User Documentation**: 
  - Update workflow documentation with new behavior
  - Document --no-commit-validation flag
  - Add troubleshooting section for "no commits" errors
- **Architecture Updates**: 
  - Update workflow execution diagrams
  - Document the new validation step

## Implementation Notes

- Consider caching HEAD checks if multiple commands run in quick succession
- Some commands might create commits asynchronously - may need a small delay
- Consider checking for both committed and staged changes
- The validation should be smart enough to handle:
  - Commands that only create files (specs) without committing
  - Commands that commit previously staged changes
  - Commands that make no changes legitimately

## Migration and Compatibility

- Existing workflows will get new validation by default
- Add `--no-commit-validation` flag for backward compatibility
- Consider making validation opt-in initially, then default later
- Log warnings before making it a hard stop in first release