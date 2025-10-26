---
number: 134
title: Fix MapReduce Worktree Architecture
category: parallel
priority: critical
status: draft
dependencies: []
created: 2025-10-26
---

# Specification 134: Fix MapReduce Worktree Architecture

**Category**: parallel
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The MapReduce workflow implementation currently has a critical architectural bug where an unnecessary intermediate worktree is created, and changes are auto-merged without user confirmation. This violates the intended architecture and user expectations.

### Current (Buggy) Behavior

```
master
  ↓
parent worktree (session-xxx) ← Created by orchestrator, but NEVER USED
  └─ (just a directory container, not a git worktree)
     ↓
session-mapreduce-xxx ← EXTRA worktree created incorrectly (line 1811-1818 in workflow/executor.rs)
  ├→ setup phase runs here
  ├→ agent-1, agent-2, agent-3... ← Agents branch from session-mapreduce
  ├→ reduce phase runs here
  └→ AUTO-MERGED to "parent" WITHOUT user confirmation (line 240 in executor.rs)
```

**Problems:**
1. Creates an unnecessary `session-mapreduce-xxx` worktree instead of using parent
2. Parent worktree created by orchestrator is never used
3. Merge from `session-mapreduce` to "parent" happens automatically without user confirmation
4. Parent detection logic fails (returns same path) because parent is not a git worktree
5. User sees merge happening before they can review changes

### Intended (Correct) Architecture

```
original_branch (e.g., master, feature-xyz, etc.)
  ↓
parent worktree (session-xxx) ← Single worktree for all MapReduce phases
  ├→ setup phase runs here
  ├→ agent-1, agent-2, agent-3... ← Multiple agent worktrees for map phase
  ├→ reduce phase runs here
  └→ Merge back to original_branch WITH user confirmation (orchestrator cleanup)
```

**Note**: The parent worktree branches from whatever branch the user was on when they started the workflow (`original_branch`). This is captured in `WorktreeState` and used as the merge target via `get_merge_target()`. The merge is NOT hardcoded to "master" - it merges back to the user's original branch.

## Objective

Remove the unnecessary intermediate `session-mapreduce-xxx` worktree creation and ensure MapReduce workflows execute in the parent session worktree with proper user confirmation before merging back to the original branch.

## Requirements

### Functional Requirements

1. **Use Existing Parent Worktree**
   - MapReduce workflows must execute in the parent worktree created by orchestrator
   - No intermediate `session-mapreduce-xxx` worktree should be created
   - Setup and reduce phases must run in parent worktree directory
   - Agent worktrees must branch from parent worktree

2. **Remove Automatic Merge Logic**
   - Remove the `merge_mapreduce_to_parent()` function call at line 240 in executor.rs
   - Remove the entire `merge_mapreduce_to_parent()` function (lines 246-426 in executor.rs)
   - No automatic merging should occur after reduce phase

3. **User Confirmation for Merge**
   - User must be prompted before merging worktree to original branch
   - Prompt should happen AFTER workflow completion
   - User can review changes before confirming merge
   - Orchestrator cleanup handles the merge with user confirmation (already implemented)
   - Merge target determined via `get_merge_target()` which returns `original_branch` from state

4. **Correct Execution Flow**
   - Setup phase → Executes in parent worktree
   - Map phase → Agents branch from parent, merge back to parent
   - Reduce phase → Executes in parent worktree
   - Completion → User prompted to merge parent to original branch

### Non-Functional Requirements

1. **Backward Compatibility**
   - Existing MapReduce workflows should continue to work
   - No changes to workflow YAML syntax required
   - Event logging and DLQ functionality preserved

2. **Data Integrity**
   - No loss of agent merge results
   - All changes properly tracked in git history
   - Clean git worktree state maintained

3. **User Experience**
   - Clear prompts before any destructive operations
   - Consistent with non-MapReduce workflow behavior
   - No surprising automatic merges

## Acceptance Criteria

- [ ] Remove worktree creation code from workflow/executor.rs (lines 1803-1818)
- [ ] Update MapReduce executor to use `env.working_dir` from parent worktree
- [ ] Remove `merge_mapreduce_to_parent()` call from executor.rs line 240
- [ ] Delete entire `merge_mapreduce_to_parent()` function (lines 246-426)
- [ ] Setup phase executes in parent worktree (verify with integration test)
- [ ] Agent worktrees branch from parent worktree (verify branch parent)
- [ ] Reduce phase executes in parent worktree (verify working directory)
- [ ] No automatic merge happens after reduce phase
- [ ] User is prompted to merge parent worktree to original branch
- [ ] Prompt displays correct target branch (not hardcoded "master")
- [ ] Prompt happens AFTER reduce phase completes
- [ ] User can decline merge and review changes
- [ ] Existing MapReduce tests pass without modification
- [ ] New test validates correct worktree hierarchy
- [ ] Documentation updated to reflect correct architecture

## Technical Details

### Implementation Approach

**Phase 1: Remove Intermediate Worktree Creation**

In `src/cook/workflow/executor.rs` (lines 1803-1818):

```rust
// REMOVE THIS CODE BLOCK:
// Create worktree for MapReduce execution BEFORE setup phase
// This ensures all phases (setup, map, reduce) execute in the isolated worktree
let worktree_manager = Arc::new(WorktreeManager::new(
    env.working_dir.to_path_buf(),
    self.subprocess.clone(),
)?);

// Create session for the MapReduce workflow
let session_id = format!(
    "session-mapreduce-{}",
    chrono::Utc::now().format("%Y%m%d_%H%M%S")
);
let worktree_result = worktree_manager
    .create_session_with_id(&session_id)
    .await
    .context("Failed to create worktree for MapReduce workflow")?;

tracing::info!(
    "Created worktree for MapReduce at: {}",
    worktree_result.path.display()
);

// Update environment to use the worktree directory
let worktree_env = ExecutionEnvironment { ... };
```

**REPLACE WITH:**
```rust
// MapReduce executes in the parent worktree (already created by orchestrator)
// No need to create an additional intermediate worktree
tracing::info!(
    "Executing MapReduce in parent worktree: {}",
    env.working_dir.display()
);

// Use the existing environment directly - it already points to parent worktree
let worktree_env = env.clone();
```

**Phase 2: Remove Automatic Merge Logic**

In `src/cook/execution/mapreduce/coordination/executor.rs`:

**Line 240 - REMOVE:**
```rust
self.merge_mapreduce_to_parent(env).await?;
```

**Lines 246-426 - DELETE ENTIRE FUNCTION:**
```rust
async fn merge_mapreduce_to_parent(&self, env: &ExecutionEnvironment) -> MapReduceResult<()> {
    // ... entire function body ...
}
```

**REPLACE LINE 240 WITH:**
```rust
// Merge to master is handled by orchestrator cleanup with user confirmation
// No automatic merge happens here
tracing::info!("MapReduce reduce phase completed. Changes are in parent worktree.");
```

**Phase 3: Update Agent Worktree Creation**

Verify that agent worktrees correctly branch from parent worktree:

In `src/cook/execution/mapreduce/agent/lifecycle.rs` (around line where agents are created):
- Ensure agents use `env.working_dir` as their parent
- Verify branch creation uses parent worktree as base
- Confirm merge target is parent worktree

**Phase 4: Update Documentation**

Update `CLAUDE.md` MapReduce architecture section to reflect correct structure:

```markdown
## MapReduce Worktree Architecture

MapReduce workflows execute in a single parent worktree with agent worktrees branching from it:

```
original_branch (e.g., master, feature-xyz, develop, etc.)
  ↓
parent worktree (session-xxx)
  ├→ Setup phase executes here
  ├→ Agent worktrees branch from parent
  │  ├→ agent-1 → processes item, merges back to parent
  │  ├→ agent-2 → processes item, merges back to parent
  │  └→ agent-N → processes item, merges back to parent
  ├→ Reduce phase executes here (aggregates agent results)
  └→ User prompt: Merge to {original_branch}? [Y/n]
```

**Branch Tracking**: The parent worktree is created from whatever branch the user was on when they started the workflow. This branch is stored as `original_branch` in `WorktreeState` and is used as the merge target. The system uses `get_merge_target()` to retrieve this branch, so merges always go back to where the user started, not hardcoded to "master".

**Isolation Guarantees:**
- Setup and reduce phases execute in parent worktree
- Each agent runs in isolated child worktree
- Agent changes merge back to parent worktree
- Final merge to master requires user confirmation
```

### Architecture Changes

**Modified Components:**
- `src/cook/workflow/executor.rs` - Remove intermediate worktree creation
- `src/cook/execution/mapreduce/coordination/executor.rs` - Remove auto-merge logic
- `CLAUDE.md` - Update architecture documentation
- `book/src/mapreduce-worktree-architecture.md` - Update book documentation

**Removed Code:**
- Intermediate worktree creation (workflow/executor.rs:1803-1832)
- `merge_mapreduce_to_parent()` function (executor.rs:246-426)
- Auto-merge call (executor.rs:240)

**No Breaking Changes:**
- Workflow YAML syntax unchanged
- Agent execution logic unchanged
- Event logging unchanged
- DLQ functionality unchanged

### Data Structures

No new data structures needed. Using existing:
- `ExecutionEnvironment` - Already contains parent worktree path
- `WorktreeSession` - Agent worktree sessions
- `WorktreeManager` - Already used by orchestrator for parent worktree

### APIs and Interfaces

**No API Changes Required**

The fix simplifies the architecture by removing code:
- `env.working_dir` already points to parent worktree
- `env.worktree_name` already has parent worktree name
- Orchestrator cleanup already handles merge with user confirmation

## Dependencies

**Prerequisites**: None - This is a bug fix for existing functionality

**Affected Components:**
- MapReduce workflow executor
- MapReduce coordination executor
- Documentation files

**External Dependencies**: None

## Testing Strategy

### Unit Tests

1. **Test Worktree Environment**
   ```rust
   #[test]
   fn test_mapreduce_uses_parent_worktree() {
       // Verify env.working_dir is used directly
       // Verify no intermediate worktree created
   }
   ```

2. **Test Agent Branching**
   ```rust
   #[test]
   fn test_agents_branch_from_parent() {
       // Create parent worktree
       // Execute map phase
       // Verify agent worktrees branch from parent
   }
   ```

3. **Test No Auto-Merge**
   ```rust
   #[test]
   fn test_no_automatic_merge_after_reduce() {
       // Execute reduce phase
       // Verify merge_mapreduce_to_parent not called
       // Verify no git merge commands executed
   }
   ```

### Integration Tests

1. **End-to-End MapReduce Workflow**
   ```rust
   #[tokio::test]
   async fn test_mapreduce_correct_worktree_hierarchy() {
       // Create parent worktree
       // Run MapReduce workflow with 3 agents
       // Verify setup runs in parent worktree
       // Verify agents branch from parent
       // Verify reduce runs in parent worktree
       // Verify no automatic merge
       // Verify user prompted for merge
   }
   ```

2. **Worktree Git Structure Validation**
   ```rust
   #[tokio::test]
   async fn test_worktree_git_structure() {
       // Run MapReduce workflow
       // Use `git worktree list` to verify structure
       // Verify only parent + agent worktrees exist
       // Verify no session-mapreduce worktree
       // Verify agent branches point to parent
   }
   ```

3. **User Confirmation Flow**
   ```rust
   #[tokio::test]
   async fn test_user_merge_confirmation() {
       // Run MapReduce workflow
       // Complete reduce phase
       // Verify user prompted (not automatic)
       // Test accepting merge
       // Test declining merge
       // Verify changes preserved on decline
   }
   ```

### Regression Tests

Run all existing MapReduce tests to ensure:
- Existing workflows continue to work
- Agent execution unchanged
- DLQ functionality preserved
- Event logging still works
- Checkpoint/resume still works

## Documentation Requirements

### Code Documentation

1. **Update workflow/executor.rs**
   - Add comment explaining why we use parent worktree directly
   - Document the correct architecture flow
   - Reference this spec for historical context

2. **Update executor.rs**
   - Add comment where auto-merge was removed
   - Explain that orchestrator handles merge with user confirmation
   - Document the correct merge flow

### User Documentation

1. **CLAUDE.md Updates**
   - Correct the MapReduce worktree architecture diagram
   - Update "Worktree Isolation" section
   - Fix examples to show correct structure
   - Remove references to session-mapreduce worktrees

2. **Book Documentation**
   - Update `book/src/mapreduce-worktree-architecture.md`
   - Correct all architecture diagrams
   - Update merge workflow examples
   - Add section on user confirmation

### Architecture Documentation

1. **Architecture Diagrams**
   - Update any diagrams showing MapReduce flow
   - Correct worktree hierarchy illustrations
   - Show user confirmation in flow charts

2. **Implementation Notes**
   - Document why intermediate worktree was removed
   - Explain correct execution model
   - Provide examples of correct usage

## Implementation Notes

### Why This Bug Existed

The intermediate worktree creation was likely added with good intentions to ensure isolation, but it created unnecessary complexity and broke the intended architecture where the orchestrator creates a single parent worktree for the entire workflow.

### Migration Path

**No Migration Required** - This is a bug fix that corrects behavior to match the original intent. Users will see:
- One less worktree in filesystem
- Merge prompt at the correct time (after review)
- Cleaner worktree structure

### Common Pitfalls

1. **Don't modify agent worktree creation** - Agents should continue to work as-is, just with parent worktree as their base
2. **Don't change orchestrator cleanup** - It already handles merge correctly with user confirmation
3. **Preserve all event logging** - Ensure events still track the same information

### Debugging

If issues arise after implementation:

1. **Check worktree list**: `git worktree list` should show parent + agents only
2. **Verify working directory**: Logs should show setup/reduce run in parent path
3. **Check git branches**: Agent branches should have parent worktree as base
4. **Verify merge timing**: User prompt should appear after reduce, not before

## Migration and Compatibility

### Breaking Changes

**None** - This is a bug fix that restores intended behavior

### Migration Path

**Automatic** - No user action required. Next MapReduce run will use correct architecture.

### Compatibility Guarantees

- All existing MapReduce workflows will work without modification
- YAML syntax unchanged
- Event format unchanged
- DLQ format unchanged
- Checkpoint format unchanged

### Version Compatibility

- **Previous versions**: Created extra intermediate worktree, auto-merged
- **This version**: Uses parent worktree directly, prompts for merge
- **Future versions**: Will build on this correct foundation

## Success Metrics

- [ ] No `session-mapreduce-xxx` worktrees created after implementation
- [ ] `git worktree list` shows only parent + agent worktrees
- [ ] User prompt appears after reduce phase completes
- [ ] User can decline merge and review changes
- [ ] All existing MapReduce tests pass
- [ ] New tests validate correct worktree hierarchy
- [ ] Documentation accurately reflects implementation
- [ ] No user complaints about automatic merges

## References

- **User Bug Report**: "ran debtmap reduce workflow and again it merged before prompting"
- **Root Cause Analysis**: Session in this document's Context section
- **Affected Files**:
  - `src/cook/workflow/executor.rs:1803-1832`
  - `src/cook/execution/mapreduce/coordination/executor.rs:240, 246-426`
- **Git Worktree Docs**: https://git-scm.com/docs/git-worktree
- **Related Specs**: Spec 127 (Worktree Isolation), Spec 117 (Custom Merge Workflows)
