---
number: 135
title: Fix Merge Workflow Execution Context
category: foundation
priority: high
status: draft
dependencies: [127]
created: 2025-10-26
---

# Specification 135: Fix Merge Workflow Execution Context

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [127 - Worktree Isolation]

## Context

**Both custom merge workflows AND the default Claude merge** are currently executing commands in the wrong directory context. All merge operations should execute within the parent worktree directory to operate on the worktree changes being merged. However, they are currently executing in the original repository's main/master branch directory (`self.repo_path`).

This issue was discovered when running:
```bash
prodigy run workflows/implement.yml -y --args 130
```

The workflow creates a worktree at `/Users/user/.prodigy/worktrees/debtmap/session-9c92f6fd-e9c1-41b8-b15a-70beedc9e508` and executes the main workflow steps correctly in that worktree. However, when merge operations execute (whether custom merge workflow steps like `/prodigy-merge-master`, `/prodigy-ci`, or the default `/prodigy-merge-worktree`), those commands run in the original repository directory instead of the worktree.

This breaks the isolation guarantee that worktrees provide and causes merge operations to run against the wrong branch/context (main/master instead of the worktree branch).

## Objective

Fix the execution context for ALL merge operations (both custom merge workflows and default Claude merge) so they execute within the parent worktree directory, maintaining consistency with the main workflow execution and preserving worktree isolation.

## Requirements

### Functional Requirements

1. **All merge commands must execute in worktree context**
   - Default Claude merge (`execute_claude_merge`) must execute in worktree path
   - Shell commands in custom merge workflows must run with `current_dir` set to worktree path
   - Claude commands in custom merge workflows must execute with working directory set to worktree path
   - Methods affected: `execute_claude_merge`, `execute_merge_shell_command`, `execute_merge_claude_command`

2. **Worktree path must be determinable from session name**
   - Given a worktree session name (e.g., `session-9c92f6fd-e9c1-41b8-b15a-70beedc9e508`), compute the full worktree path
   - Worktree path follows pattern: `{base_dir}/{worktree_name}`
   - Must be computed before merge workflow execution begins

3. **Preserve existing merge workflow variable interpolation**
   - All existing merge variables must continue to work (`${merge.worktree}`, `${merge.source_branch}`, etc.)
   - Git information gathering must use worktree path (already correct at line 912-947)
   - No breaking changes to merge workflow YAML syntax

### Non-Functional Requirements

1. **Consistency**: Merge workflow execution context must match main workflow execution context
2. **Debuggability**: Logging should clearly show which directory commands execute in
3. **Backward Compatibility**: Existing merge workflows must continue to work without modification

## Acceptance Criteria

- [ ] Shell commands in custom merge workflows execute in worktree directory
- [ ] Claude commands in custom merge workflows execute in worktree directory
- [ ] Execution logs show correct working directory (worktree path, not repo path)
- [ ] Git operations in merge workflows operate on worktree branch
- [ ] Test with `workflows/implement.yml` confirms commands execute in worktree
- [ ] No breaking changes to existing merge workflow syntax or variables

## Technical Details

### Root Cause Analysis

The bug exists in THREE methods in `src/worktree/manager.rs`:

1. **`execute_claude_merge` (line 402-425)** - Default merge path:
   ```rust
   let result = claude_executor
       .execute_claude_command(
           &format!("/prodigy-merge-worktree {worktree_branch}"),
           &self.repo_path,  // ❌ WRONG - should be worktree path
           env_vars,
       )
       .await?;
   ```

2. **`execute_merge_shell_command` (line 1000-1038)** - Custom merge workflow:
   ```rust
   let shell_command = ProcessCommandBuilder::new("sh")
       .current_dir(&self.repo_path)  // ❌ WRONG - should be worktree path
       .args(["-c", &shell_cmd_interpolated])
       .build();
   ```

3. **`execute_merge_claude_command` (line 1040-1094)** - Custom merge workflow:
   ```rust
   let result = claude_executor
       .execute_claude_command(&claude_cmd_interpolated, &self.repo_path, env_vars)
       //                                                  ^^^^^^^^^^^^^^^^ ❌ WRONG
       .await?;
   ```

### Implementation Approach

**Step 1: Fix default Claude merge to use worktree path**

The `execute_merge_workflow` method (line 374-399) calls either `execute_custom_merge_workflow` or `execute_claude_merge`. We need to pass the worktree name to `execute_claude_merge`:

```rust
async fn execute_claude_merge(&self, name: &str, worktree_branch: &str) -> Result<String> {
    let worktree_path = self.base_dir.join(name);

    if !worktree_path.exists() {
        anyhow::bail!("Worktree path does not exist: {}", worktree_path.display());
    }

    // ... existing code ...

    let result = claude_executor
        .execute_claude_command(
            &format!("/prodigy-merge-worktree {worktree_branch}"),
            &worktree_path,  // ✅ FIXED
            env_vars,
        )
        .await?;
}
```

**Step 2: Add worktree path parameter to custom merge command executors**

Modify method signatures to accept worktree path:
```rust
async fn execute_merge_shell_command(
    &self,
    shell_cmd: &str,
    variables: &HashMap<String, String>,
    step_index: usize,
    total_steps: usize,
    worktree_path: &Path,  // ✅ NEW
) -> Result<String>
```

**Step 3: Compute worktree path in `execute_custom_merge_workflow`**

At the beginning of `execute_custom_merge_workflow` (line 1195):
```rust
async fn execute_custom_merge_workflow(
    &self,
    merge_workflow: &MergeWorkflow,
    worktree_name: &str,
    source_branch: &str,
    target_branch: &str,
) -> Result<String> {
    // Compute worktree path from session name
    let worktree_path = self.base_dir.join(worktree_name);

    // Verify worktree exists
    if !worktree_path.exists() {
        anyhow::bail!("Worktree path does not exist: {}", worktree_path.display());
    }

    // ... rest of function
}
```

**Step 4: Update command executors to use worktree path**

Change `current_dir` and execution directory:
```rust
// In execute_merge_shell_command:
let shell_command = ProcessCommandBuilder::new("sh")
    .current_dir(worktree_path)  // ✅ FIXED
    .args(["-c", &shell_cmd_interpolated])
    .build();

// In execute_merge_claude_command:
let result = claude_executor
    .execute_claude_command(&claude_cmd_interpolated, worktree_path, env_vars)
    //                                                  ^^^^^^^^^^^^^ ✅ FIXED
    .await?;
```

**Step 5: Update logging to reflect correct context**

Modify `log_execution_context` to accept and log worktree path:
```rust
fn log_execution_context(
    &self,
    step_name: &str,
    variables: &HashMap<String, String>,
    worktree_path: &Path,  // ✅ NEW
) {
    tracing::debug!("=== Step Execution Context ===");
    tracing::debug!("Step: {}", step_name);
    tracing::debug!("Working Directory: {}", worktree_path.display());  // ✅ FIXED
    tracing::debug!("Worktree Path: {}", worktree_path.display());
    tracing::debug!("Project Directory: {}", self.repo_path.display());
    // ... rest of logging
}
```

### Files to Modify

1. **`src/worktree/manager.rs`**:
   - Line 374-399: `execute_merge_workflow` - Pass `name` to `execute_claude_merge`
   - Line 402-425: `execute_claude_merge` - Add `name` parameter, compute worktree path, use it for execution
   - Line 1000-1038: `execute_merge_shell_command` - Add `worktree_path` parameter, use it for `current_dir`
   - Line 1040-1094: `execute_merge_claude_command` - Add `worktree_path` parameter, pass to Claude executor
   - Line 1110-1131: `log_execution_context` - Add `worktree_path` parameter, log it correctly
   - Line 1195-1296: `execute_custom_merge_workflow` - Compute worktree path, pass to command executors

### Verification Strategy

**Manual Testing**:
```bash
# Run workflow with custom merge
prodigy run workflows/implement.yml -y --args 130

# Verify in logs that merge commands execute in worktree:
# Expected: "Working directory: /Users/user/.prodigy/worktrees/debtmap/session-XXX"
# Not: "Working directory: /Users/user/project"
```

**Integration Test** (add to `tests/merge_workflow_integration.rs`):
```rust
#[tokio::test]
async fn test_merge_workflow_executes_in_worktree() {
    // Create worktree with changes
    // Define custom merge workflow with shell command that prints pwd
    // Execute merge workflow
    // Verify output contains worktree path, not repo path
}
```

## Dependencies

- **Spec 127 (Worktree Isolation)**: This fix enforces the isolation guarantees promised in spec 127
- Requires no new dependencies
- No breaking changes to workflow YAML format

## Testing Strategy

### Unit Tests

Add tests to `src/worktree/manager.rs::tests`:
```rust
#[tokio::test]
async fn test_merge_commands_use_worktree_path() {
    // Verify execute_merge_shell_command uses worktree path
}

#[tokio::test]
async fn test_merge_claude_commands_use_worktree_path() {
    // Verify execute_merge_claude_command uses worktree path
}
```

### Integration Tests

Add tests to `tests/merge_workflow_integration.rs`:
```rust
#[tokio::test]
async fn test_custom_merge_workflow_execution_context() {
    // Create workflow with custom merge containing shell command "pwd"
    // Execute workflow
    // Verify pwd output is worktree path, not repo path
}
```

### Manual Testing

1. Run `prodigy run workflows/implement.yml -y --args 130`
2. Check logs for "Working Directory: " lines during merge phase
3. Verify they show worktree path, not main repo path
4. Verify merge commands operate on correct branch (worktree branch, not master)

## Documentation Requirements

### Code Documentation

- Add doc comments to `execute_merge_shell_command` explaining `worktree_path` parameter
- Add doc comments to `execute_merge_claude_command` explaining execution context
- Document the worktree path computation logic

### User Documentation

Update `CLAUDE.md` section "Custom Merge Workflows" to clarify:
- Merge workflow commands execute in the **parent worktree**, not the main repository
- This maintains isolation and ensures merge operations work on the correct branch
- Example showing how to reference worktree-relative paths in merge commands

## Implementation Notes

### Why This Matters

1. **Correctness**: Commands must operate on the worktree branch, not master/main
2. **Isolation**: Merge workflows should respect the worktree isolation boundary
3. **User Expectations**: Users expect merge workflows to operate on the work being merged
4. **Debugging**: When CI checks fail in merge workflows, they should run against the worktree changes

### Gotchas

1. **Git operations**: Git commands in merge workflows already use correct context because `git_service.get_merge_git_info(&worktree_path, ...)` is called correctly (line 925)
2. **Variable interpolation**: No changes needed - merge variables already contain correct values
3. **Checkpoint directory**: Checkpoints are stored in main repo `.prodigy/` directory, not worktree `.prodigy/` - this is correct and should not change

## Migration and Compatibility

### Breaking Changes

None - this is a bug fix that makes behavior match documented expectations.

### Backward Compatibility

All existing merge workflows will continue to work. In fact, any workflows that were mysteriously failing because commands ran in the wrong directory will now start working correctly.

### Deprecation

No deprecations required.

## Related Issues

This bug affects **ALL merge operations** - both custom merge workflows and the default Claude merge. The bug was present from the initial implementation where merge commands were set up to use `self.repo_path` (the main repository) instead of the worktree path.

**Affected operations**:
- Default merge (no custom merge workflow) - `/prodigy-merge-worktree` executes in main repo
- Custom merge workflows - All commands execute in main repo
- Example: `workflows/implement.yml` - Uses custom merge with `/prodigy-merge-master`, `/prodigy-ci`, `/prodigy-merge-worktree`

**Impact**:
- Merge commands run against main/master branch instead of worktree branch
- CI checks in merge phase test wrong code
- Git operations may commit to wrong branch
- Breaks worktree isolation guarantee from spec 127

## Success Metrics

- [ ] All existing tests pass
- [ ] New tests verify correct execution context
- [ ] Manual testing with `workflows/implement.yml` shows correct directory in logs
- [ ] Git operations in merge workflows work against correct branch
- [ ] No regressions in merge workflow functionality
