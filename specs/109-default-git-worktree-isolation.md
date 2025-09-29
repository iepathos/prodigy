---
number: 109
title: Default Git Worktree Isolation
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-09-29
---

# Specification 109: Default Git Worktree Isolation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Currently, Prodigy workflows can run in two modes:
1. **With `--worktree` flag**: Creates isolated git worktree for the workflow session
2. **Without flag**: Runs directly in the current repository directory

The worktree mode provides critical benefits:
- **Isolation**: Changes don't affect the main working directory
- **Safety**: Can abort without leaving repository in dirty state
- **Parallelism**: Multiple workflows can run simultaneously
- **Auditability**: Clear separation of workflow changes via git commits

Running workflows directly in the current directory without isolation:
- Risks leaving repository in inconsistent state on failure
- Cannot support parallel workflow execution
- Makes it difficult to review and revert changes
- Breaks the MapReduce architecture which requires parent/child worktrees

The `--worktree` flag adds cognitive overhead and makes the safer option opt-in rather than default. Since git worktree isolation is fundamental to Prodigy's design philosophy, it should be the default and only mode of operation.

## Objective

Remove the `--worktree` command-line flag and make git worktree isolation the default and only behavior for all Prodigy workflow executions, simplifying the user experience and ensuring consistent isolation semantics.

## Requirements

### Functional Requirements

1. **Remove CLI Flag**
   - Remove `--worktree` / `-w` flag from all CLI commands
   - Remove worktree-related boolean from configuration structures
   - All workflows execute in isolated git worktrees by default

2. **Default Worktree Creation**
   - Automatically create worktree for every workflow execution
   - Use consistent naming: `prodigy-session-{uuid}`
   - Location: `~/.prodigy/worktrees/{project-name}/session-{uuid}/`
   - Branch: `prodigy-session-{uuid}`

3. **Workflow Execution**
   - All workflow steps execute in the worktree directory
   - Environment variables point to worktree path
   - Git operations reference worktree context

4. **MapReduce Workflows**
   - Parent worktree created for MapReduce session
   - Setup phase runs in parent worktree
   - Agent worktrees branch from parent worktree
   - Agents merge back to parent worktree
   - Reduce phase runs in parent worktree

5. **Session Cleanup**
   - Prompt user to merge changes back to original branch
   - Support `-y` flag for auto-merge
   - Clean up worktree after merge or abort
   - Handle interrupted sessions gracefully

6. **Backward Compatibility**
   - Existing workflows continue to work without modification
   - YAML files don't need updates
   - Session state files remain compatible

### Non-Functional Requirements

1. **Performance**
   - Worktree creation should be fast (< 1 second)
   - No significant overhead compared to current worktree mode
   - Efficient cleanup of completed worktrees

2. **User Experience**
   - Seamless transition for users
   - Clear messaging about worktree location
   - Helpful error messages if git operations fail

3. **Reliability**
   - Handle git errors gracefully
   - Recover from incomplete worktree cleanup
   - Detect and report worktree conflicts

## Acceptance Criteria

- [ ] `--worktree` / `-w` flag removed from CLI parser
- [ ] `worktree: bool` field removed from `CommandConfig` and related structures
- [ ] All workflow executions create worktree automatically
- [ ] Worktree path logged at start of workflow execution
- [ ] Setup phase runs in worktree (not main repo)
- [ ] MapReduce parent/agent worktree hierarchy works correctly
- [ ] Final merge prompt shows worktree → original branch
- [ ] `-y` flag auto-accepts final merge
- [ ] Cleanup removes worktree after merge or abort
- [ ] All existing tests pass without modification
- [ ] Integration tests validate worktree isolation
- [ ] Documentation updated to reflect default behavior
- [ ] Help text and examples don't reference `--worktree` flag

## Technical Details

### Implementation Approach

1. **CLI Changes** (`src/cli/mod.rs`, `src/config/mod.rs`)
   - Remove `worktree` field from `CommandConfig`
   - Remove `-w` / `--worktree` flag from clap parser
   - Remove conditional logic checking `config.command.worktree`

2. **Orchestrator Changes** (`src/cook/orchestrator.rs`)
   - Remove condition in `setup_environment()` (line ~1250)
   - Always call `worktree_manager.create_session_with_id()`
   - Always set `env.worktree_name = Some(session_id)`
   - Update logging to always show worktree path

3. **MapReduce Changes** (`src/cook/execution/mapreduce/`)
   - Remove fallback logic that uses `project_dir` when no worktree
   - Simplify `merge_agent_to_parent()` logic (line 147-153 in lifecycle.rs)
   - Always use `env.working_dir` as parent worktree path

4. **Cleanup Changes** (`src/cook/orchestrator.rs`)
   - Remove condition checking `env.worktree_name` (line ~1412)
   - Always prompt for merge
   - Always cleanup worktree after completion

### Architecture Changes

**Before:**
```
if config.command.worktree {
    // Create worktree
    env.worktree_name = Some(session_id)
} else {
    // Use current directory
    env.worktree_name = None
}
```

**After:**
```
// Always create worktree
worktree_manager.create_session_with_id(&session_id).await?
env.worktree_name = Some(session_id)
```

**Simplified MapReduce Merge Logic:**
```rust
// Before: Conditional logic
let parent_worktree_path = if env.worktree_name.is_some() {
    &env.working_dir
} else {
    &env.project_dir
};

// After: Always use working_dir (it's always a worktree)
let parent_worktree_path = &env.working_dir;
```

### Data Structures

**Remove from `CommandConfig`:**
```rust
pub struct CommandConfig {
    // ... other fields ...
    pub worktree: bool,  // DELETE THIS
    // ... other fields ...
}
```

**ExecutionEnvironment remains same** (always has worktree info):
```rust
pub struct ExecutionEnvironment {
    pub working_dir: Arc<PathBuf>,      // Always points to worktree
    pub worktree_name: Some(String),    // Always Some
    pub session_id: String,
    pub project_dir: Arc<PathBuf>,      // Original repo path
}
```

### APIs and Interfaces

No public API changes - internal implementation only.

**CLI Interface Change:**
```bash
# Before
prodigy run workflow.yml              # Runs in current directory
prodigy run workflow.yml --worktree   # Runs in isolated worktree

# After
prodigy run workflow.yml              # Always runs in isolated worktree
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - CLI parser (`src/cli/mod.rs`)
  - Configuration (`src/config/mod.rs`)
  - Orchestrator (`src/cook/orchestrator.rs`)
  - MapReduce executor (`src/cook/execution/mapreduce/`)
  - Agent lifecycle (`src/cook/execution/mapreduce/agent/lifecycle.rs`)
- **External Dependencies**: None (uses existing git worktree functionality)

## Testing Strategy

### Unit Tests

- **Config Tests**: Verify `worktree` field removed from structures
- **Orchestrator Tests**: Verify worktree always created
- **MapReduce Tests**: Verify simplified merge logic works

### Integration Tests

- **Workflow Execution**: Test complete workflow with automatic worktree
- **MapReduce Flow**: Test parent/agent worktree hierarchy
- **Cleanup**: Test merge and worktree cleanup
- **Error Handling**: Test git operation failures

### Manual Testing

1. Run simple workflow: `prodigy run simple.yml`
   - Verify worktree created automatically
   - Verify changes isolated from main repo
   - Verify merge prompt at end

2. Run MapReduce workflow: `prodigy run mapreduce.yml`
   - Verify parent worktree created
   - Verify agent worktrees branch from parent
   - Verify reduce phase has all agent changes

3. Test with `-y` flag: `prodigy run workflow.yml -y`
   - Verify auto-merge without prompt
   - Verify worktree cleaned up

4. Test interrupted workflow: `Ctrl+C` during execution
   - Verify worktree state preserved
   - Verify can resume or cleanup

### Performance Tests

- Measure worktree creation overhead
- Verify no regression in workflow execution time
- Test cleanup performance with many worktrees

## Documentation Requirements

### Code Documentation

- Update doc comments for `ExecutionEnvironment`
- Document worktree lifecycle in orchestrator
- Add examples showing automatic worktree usage

### User Documentation

- Update CLAUDE.md to remove `--worktree` flag references
- Update README with simplified workflow execution examples
- Add FAQ entry explaining worktree behavior
- Document worktree cleanup commands

### Architecture Updates

- Update ARCHITECTURE.md to reflect default worktree isolation
- Document worktree directory structure
- Explain MapReduce worktree hierarchy

## Implementation Notes

### Code Removal Locations

1. **src/cli/mod.rs** (~line 50-60): Remove `--worktree` flag
2. **src/config/mod.rs** (~line 30): Remove `worktree: bool` field
3. **src/cook/orchestrator.rs** (~line 1250): Remove `if config.command.worktree`
4. **src/cook/orchestrator.rs** (~line 1412): Remove `if env.worktree_name.is_some()`
5. **src/cook/execution/mapreduce/agent/lifecycle.rs** (line 147-153): Simplify merge logic

### Migration Notes

- No data migration required
- Existing workflows work without changes
- Users will see worktree creation message for all executions

### Gotchas

- Git repository must be clean before creating worktree
- Cannot create worktree if branch already exists
- Must handle orphaned worktrees from previous crashes

## Migration and Compatibility

### Breaking Changes

**CLI Change:**
- Removing `--worktree` flag is technically breaking
- However, behavior becomes simpler (always isolated)
- Users who never used `--worktree` get better isolation by default
- Users who always used `--worktree` see no change (flag just becomes no-op before removal)

### Migration Strategy

**Phase 1: Deprecation Warning (Optional)**
- Add deprecation warning for `--worktree` flag
- Document that worktrees will be default in next version
- Run for one release cycle

**Phase 2: Make Default (This Spec)**
- Remove flag and always use worktrees
- Update all documentation
- Communicate change in release notes

### Compatibility Considerations

- **Workflows**: No changes needed to YAML files
- **Scripts**: Remove `--worktree` flag from automation scripts
- **CI/CD**: Update CI scripts to remove flag
- **Session State**: Existing sessions continue to work

### Rollback Plan

If issues arise:
1. Revert commits removing `--worktree` flag
2. Make worktree default but keep flag for backward compat
3. Document issues and plan resolution
4. Retry removal in future release

## Success Metrics

- [ ] All Prodigy workflow executions use git worktrees
- [ ] Zero regressions in existing functionality
- [ ] Simplified user experience (no flag to remember)
- [ ] Consistent isolation semantics across all workflow types
- [ ] MapReduce workflows work reliably with parent/agent hierarchy
- [ ] Documentation is clear and accurate
- [ ] User feedback is positive or neutral

## Related Specifications

- **Spec 101**: Error Handling Guidelines (affects error messages for git operations)
- **MapReduce Architecture**: Depends on worktree isolation for parent/agent hierarchy
- **Session Management**: Worktree lifecycle tied to session lifecycle