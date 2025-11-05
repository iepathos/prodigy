---
number: 154
title: Fix Merge Target Branch Tracking in Workflows
category: compatibility
priority: high
status: draft
dependencies: [110]
created: 2025-11-04
---

# Specification 154: Fix Merge Target Branch Tracking in Workflows

**Category**: compatibility
**Priority**: high
**Status**: draft
**Dependencies**: Spec 110 (Branch Tracking)

## Context

When running workflows with custom merge phases (e.g., `workflows/implement.yml`), users expect the merge to target the branch they started from. However, there's a critical bug where merges always target `main`/`master` instead of the original branch.

### Current Behavior

1. User runs `prodigy run workflows/implement.yml` from `feature/my-feature` branch
2. Prodigy correctly tracks `feature/my-feature` in `WorktreeState.original_branch`
3. Workflow completes and enters custom merge phase defined in `workflows/implement.yml`:
   ```yaml
   merge:
     - claude: "/prodigy-merge-master"
     - claude: "/prodigy-ci"
     - claude: "/prodigy-merge-worktree ${merge.source_branch}"
   ```
4. **BUG**: The `/prodigy-merge-worktree` command merges to `main`/`master` instead of `feature/my-feature`

### Root Cause Analysis

The bug occurs due to a disconnect between three components:

1. **WorktreeManager** (`src/worktree/manager.rs:349`):
   - Correctly uses `get_merge_target()` to retrieve `original_branch`
   - Works correctly for direct merge operations via CLI

2. **MergeOrchestrator** (`src/worktree/merge_orchestrator.rs:341`):
   - Sets `merge.target_branch` variable from the provided `target_branch` parameter
   - This variable IS available for interpolation

3. **Workflow Configuration** (`workflows/implement.yml:41`):
   - Uses `/prodigy-merge-worktree ${merge.source_branch}`
   - **Does NOT pass the target branch** to the command
   - The command determines target branch by inspecting current git branch

4. **Claude Command** (`.claude/commands/prodigy-merge-worktree.md:22-28`):
   - Determines target branch using `git rev-parse --abbrev-ref HEAD`
   - When run in worktree, this returns the worktree branch (e.g., `prodigy-session-xxx`)
   - Falls back to `main`/`master` when current branch is not a valid target

### Impact

- **Severity**: High - Data loss risk if user's feature branch work is merged to wrong branch
- **Frequency**: Affects all users running custom merge workflows from non-main branches
- **User Experience**: Silent failure - merge succeeds but goes to wrong place
- **Workaround**: None - users must manually fix the merge after the fact

## Objective

Fix the merge target branch tracking in custom merge workflows to ensure merges always target the original branch where the workflow was started, regardless of the current working directory or branch context.

## Requirements

### Functional Requirements

1. **Pass Target Branch to Claude Command**
   - Modify `/prodigy-merge-worktree` command to accept optional target branch parameter
   - Update command signature: `/prodigy-merge-worktree <source-branch> [target-branch]`
   - Maintain backward compatibility with existing single-argument usage

2. **Update Workflow Templates**
   - Update `workflows/implement.yml` to pass `${merge.target_branch}`
   - Update all other workflow files that use `/prodigy-merge-worktree`
   - Document the correct usage pattern

3. **Preserve Existing Behavior**
   - When target branch not specified, maintain fallback logic (current branch → main/master)
   - Ensure non-workflow merge operations (CLI) continue working
   - Maintain compatibility with existing workflows that don't specify target

### Non-Functional Requirements

- **Backward Compatibility**: Existing workflows must continue to work
- **Clear Error Messages**: If target branch doesn't exist, provide actionable error
- **Documentation**: Update all relevant docs to reflect correct usage
- **Testing**: Comprehensive tests covering all branch tracking scenarios

## Acceptance Criteria

- [ ] `/prodigy-merge-worktree` command accepts optional second argument for target branch
- [ ] When target branch specified, command merges to that branch (no fallback)
- [ ] When target branch NOT specified, command uses fallback logic (current behavior)
- [ ] `workflows/implement.yml` updated to pass `${merge.target_branch}`
- [ ] Test: Workflow started from `feature/xyz` merges back to `feature/xyz`
- [ ] Test: Workflow started from `main` merges back to `main`
- [ ] Test: Workflow with no target specified falls back to main/master
- [ ] Test: Invalid target branch produces clear error message
- [ ] All existing tests pass without modification
- [ ] Documentation updated (CLAUDE.md, workflow README, command docs)
- [ ] Backward compatibility verified with old workflow files

## Technical Details

### Implementation Approach

**Phase 1: Update Claude Command**

Modify `.claude/commands/prodigy-merge-worktree.md`:

```markdown
## Usage

```
/prodigy-merge-worktree <source-branch> [target-branch]
```

Arguments:
- `source-branch`: Branch to merge FROM (required)
- `target-branch`: Branch to merge TO (optional, defaults to current branch or main/master)

Examples:
- `/prodigy-merge-worktree prodigy-session-123` (uses current branch)
- `/prodigy-merge-worktree prodigy-session-123 feature/my-feature` (explicit target)
- `/prodigy-merge-worktree prodigy-session-123 main` (explicit main)
```

**Phase 2: Update Command Logic**

```markdown
1. **Parse Arguments**
   - Split $ARGUMENTS on whitespace
   - First argument: source_branch (required)
   - Second argument: target_branch (optional)
   - If no source branch provided, fail with usage error

2. **Determine Target Branch**
   - If target_branch argument provided:
     - Verify target branch exists using `git rev-parse --verify refs/heads/$target_branch`
     - If exists, use it; otherwise fail with error
   - Otherwise (for backward compatibility):
     - Get current branch using `git rev-parse --abbrev-ref HEAD`
     - If valid branch name (not HEAD), use it
     - Otherwise fall back to main/master

3. **Switch to Target Branch**
   - If not already on target branch: `git checkout $target_branch`

4. **Execute Merge**
   - `git merge --no-ff $source_branch`
   - Handle conflicts as per existing logic
```

**Phase 3: Update Workflow Files**

Update `workflows/implement.yml`:

```yaml
merge:
  # Step 1: Merge master into worktree
  - claude: "/prodigy-merge-master"

  # Step 2: Run CI checks and fix any issues
  - claude: "/prodigy-ci"

  # Step 3: Merge worktree back to original branch
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

Update all other workflow files similarly:
- `workflows/implement-goal.yml`
- Any example workflows in documentation

**Phase 4: Update Documentation**

Files to update:
- `CLAUDE.md` - Custom Merge Workflows section
- `workflows/README.md` - Example usage
- `.claude/commands/prodigy-merge-worktree.md` - Command documentation
- `book/src/workflow-basics.md` - Workflow guide
- `book/src/configuration.md` - Configuration reference

### Architecture Changes

No architectural changes required. This is a usage pattern fix:

```
Before:
workflows/implement.yml → /prodigy-merge-worktree ${source}
                          ↓
                    Command determines target via git
                          ↓
                    ❌ Falls back to main/master

After:
workflows/implement.yml → /prodigy-merge-worktree ${source} ${target}
                          ↓
                    Command uses explicit target
                          ↓
                    ✅ Merges to correct branch
```

### Data Structures

No data structure changes. The `merge.target_branch` variable already exists and is populated by `MergeOrchestrator::init_merge_variables()` (line 342 in merge_orchestrator.rs).

### APIs and Interfaces

**Modified Interface**:

```markdown
Command: /prodigy-merge-worktree
Old: /prodigy-merge-worktree <source-branch>
New: /prodigy-merge-worktree <source-branch> [target-branch]
```

**Backward Compatibility**:
- Old usage (single argument) continues to work with fallback logic
- New usage (two arguments) provides explicit control

## Dependencies

- **Spec 110**: Branch Tracking - This spec builds on the branch tracking infrastructure
- **WorktreeState**: Relies on `original_branch` field being correctly populated
- **MergeOrchestrator**: Relies on `merge.target_branch` variable being set

### Affected Components

1. **`.claude/commands/prodigy-merge-worktree.md`**
   - Modified: Argument parsing and target branch determination

2. **`workflows/implement.yml`**
   - Modified: Merge command invocation

3. **`workflows/implement-goal.yml`**
   - Modified: Merge command invocation (if applicable)

4. **Documentation Files**
   - Modified: All references to `/prodigy-merge-worktree` usage

### External Dependencies

None - this is a pure workflow configuration and command usage fix.

## Testing Strategy

### Unit Tests

Not applicable - this is a usage pattern fix in workflow configuration and Claude command.

### Integration Tests

Create new test file: `tests/merge_target_branch_integration.rs`

```rust
#[tokio::test]
async fn test_merge_workflow_respects_original_branch() -> Result<()> {
    // Setup: Create feature branch and start workflow from it
    let temp_dir = setup_test_repo_with_feature_branch().await?;

    // Create a minimal workflow with merge phase
    let workflow = r#"
commands:
  - shell: "echo 'test' > test.txt"
    commit_required: true

merge:
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
"#;

    // Run workflow from feature branch
    checkout_branch(&temp_dir, "feature/test-branch").await?;
    let result = run_workflow(&temp_dir, workflow).await?;

    // Verify: Merge targeted feature branch, not main
    let current_branch = get_current_branch(&temp_dir).await?;
    assert_eq!(current_branch, "feature/test-branch");

    // Verify: Changes are present in feature branch
    let file_content = read_file(&temp_dir.join("test.txt")).await?;
    assert_eq!(file_content, "test\n");

    Ok(())
}

#[tokio::test]
async fn test_merge_workflow_backward_compatibility() -> Result<()> {
    // Verify old workflows still work (without target branch argument)
    let workflow = r#"
merge:
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
"#;

    // Should fall back to main/master
    let result = run_workflow_from_main(&workflow).await?;
    assert!(result.success);

    Ok(())
}

#[tokio::test]
async fn test_merge_workflow_invalid_target_branch() -> Result<()> {
    // Verify explicit invalid target produces clear error
    let workflow = r#"
merge:
  - claude: "/prodigy-merge-worktree ${merge.source_branch} nonexistent-branch"
"#;

    let result = run_workflow(&workflow).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Branch 'nonexistent-branch' does not exist"));

    Ok(())
}
```

### Manual Testing

**Test Case 1: Feature Branch Workflow**
```bash
# Setup
git checkout -b feature/test-154
echo "test" > test-file.txt
git add test-file.txt
git commit -m "test commit"

# Run workflow
prodigy run workflows/implement.yml 110

# Verify
git branch --show-current  # Should be: feature/test-154
git log --oneline -5       # Should show merge commit to feature/test-154
```

**Test Case 2: Main Branch Workflow**
```bash
# Setup
git checkout main

# Run workflow
prodigy run workflows/implement.yml 110

# Verify
git branch --show-current  # Should be: main
```

**Test Case 3: Detached HEAD**
```bash
# Setup
git checkout HEAD~1  # Detached HEAD

# Run workflow
prodigy run workflows/implement.yml 110

# Verify
# Should merge to main/master (fallback behavior)
```

### Performance Tests

Not applicable - no performance impact expected.

### User Acceptance

**Success Criteria**:
- User starts workflow from `feature/xyz`
- User completes workflow without thinking about merge target
- Changes are merged back to `feature/xyz` automatically
- User confirms this matches their expectation

## Documentation Requirements

### Code Documentation

Update inline documentation in:
- `.claude/commands/prodigy-merge-worktree.md` - Command usage and examples
- Add comments explaining argument parsing logic

### User Documentation

**CLAUDE.md Updates**:
```markdown
## Custom Merge Workflows

When defining custom merge workflows, always pass both source and target branches
to the `/prodigy-merge-worktree` command:

```yaml
merge:
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

The `${merge.target_branch}` variable is automatically set to the branch you were
on when you started the workflow.
```

**workflows/README.md Updates**:
```markdown
## Merge Workflow Variables

The following variables are available in merge workflows:

- `${merge.source_branch}` - The worktree branch to merge FROM
- `${merge.target_branch}` - The branch to merge TO (your original branch)
- `${merge.worktree}` - The worktree name
- `${merge.session_id}` - The session ID

Example:
```yaml
merge:
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```
```

### Architecture Updates

Update `ARCHITECTURE.md` - Worktree Management section:

```markdown
### Merge Target Tracking

When a worktree is created, the original branch is captured in `WorktreeState.original_branch`.
During merge operations:

1. `get_merge_target()` retrieves the original branch
2. `MergeOrchestrator` sets `${merge.target_branch}` variable
3. Workflow passes target to `/prodigy-merge-worktree` command
4. Command merges to the specified target branch

This ensures merges always target the branch where the workflow started,
regardless of worktree location or current branch context.
```

## Implementation Notes

### Important Considerations

1. **Argument Parsing**: Use proper shell argument parsing (respect quotes, spaces)
2. **Error Messages**: Provide actionable errors when target branch doesn't exist
3. **Working Directory**: Command may run in worktree; ensure git operations work
4. **Branch Verification**: Always verify target branch exists before attempting merge

### Potential Gotchas

1. **Variable Interpolation Order**: Ensure `${merge.target_branch}` is interpolated before command execution
2. **Escaping**: Branch names with special characters need proper escaping
3. **Detached HEAD**: Ensure fallback logic still works when target not specified
4. **Race Conditions**: Branch could be deleted between verification and merge

### Best Practices

1. **Always Pass Target**: In workflow files, always explicitly pass target branch
2. **Verify Variables**: Log merge variables when verbosity >= 1 for debugging
3. **Clear Errors**: Distinguish between "branch doesn't exist" and "merge conflict"
4. **Test Edge Cases**: Test with unusual branch names (slashes, dashes, etc.)

## Migration and Compatibility

### Breaking Changes

None - this is a backward compatible enhancement.

### Migration Path

**For Users**:
- No action required - existing workflows continue to work
- Recommended: Update custom workflows to pass explicit target branch
- Update workflow files when convenient

**For Developers**:
- Review any custom merge workflows
- Update to use two-argument form: `/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}`

### Compatibility Matrix

| Workflow Version | Command Version | Behavior |
|-----------------|-----------------|----------|
| Old (1 arg)     | Old             | ✅ Works (fallback) |
| Old (1 arg)     | New             | ✅ Works (fallback) |
| New (2 args)    | Old             | ❌ Breaks (ignores 2nd arg) |
| New (2 args)    | New             | ✅ Works (explicit target) |

### Rollback Plan

If issues arise:
1. Revert workflow file changes
2. Revert command documentation changes
3. Keep command backward compatible changes (no harm)
4. Document workaround in known issues

## Success Metrics

### Completion Criteria

- [ ] All acceptance criteria met
- [ ] All tests passing
- [ ] Documentation updated
- [ ] No regressions in existing functionality

### Validation

- [ ] Manual test from feature branch succeeds
- [ ] Manual test from main branch succeeds
- [ ] Manual test with old workflow format succeeds
- [ ] Integration tests pass
- [ ] User confirmation that bug is fixed

### Post-Implementation

- Monitor for user reports of merge targeting wrong branch
- Collect feedback on new workflow syntax clarity
- Update examples based on user confusion patterns
