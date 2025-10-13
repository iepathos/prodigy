# MapReduce Worktree Architecture

This document explains how worktrees and git branches work in Prodigy's MapReduce workflow, including the merge flow that propagates changes from agent worktrees → MapReduce worktree → parent worktree → master branch.

## Overview

MapReduce workflows use a **nested worktree hierarchy** to enable massive parallel processing while maintaining git isolation. Each level in the hierarchy has its own worktree and git branch, and changes flow upward through a series of merges.

## Worktree Hierarchy

```
main repository (master/main branch)
    ↓
parent worktree (session-xxx)
  Branch: prodigy-session-xxx
  Location: ~/.prodigy/worktrees/{project}/session-xxx
    ↓
mapreduce worktree (session-mapreduce-xxx)
  Branch: prodigy-session-mapreduce-xxx  ← Setup and reduce phases execute here
  Location: ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx
    ↓
agent worktrees (one per work item)
  Branch: agent-{job_id}_agent_{index}-item_{index}
  Location: ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx/mapreduce-agent-{job_id}_agent_{index}
```

## Branch Naming Convention

**Critical for merge success:** Branch names follow consistent patterns.

| Worktree Type | Branch Pattern | Example |
|---------------|---------------|---------|
| Parent Session | `prodigy-session-{session_id}` | `prodigy-session-ea460deb-9946-4583-8a3b-ab804850c25a` |
| MapReduce | `prodigy-session-mapreduce-{timestamp}` | `prodigy-session-mapreduce-20251012_223630` |
| Agent | `agent-{job_id}_agent_{index}-item_{index}` | `agent-mapreduce-20251012_224808_agent_0-item_0` |

**Important:** The `prodigy-` prefix is essential. The MapReduce-to-parent merge bug occurred because the code was creating a new branch with a `merge-` prefix instead of using the existing `prodigy-` branch.

## Merge Flow

Changes propagate upward through three merge operations:

### 1. Agent → MapReduce Merge

**When:** After each agent completes successfully
**What:** Agent's branch merges to MapReduce worktree
**How:** Serialized through merge queue to prevent conflicts
**Location:** `src/cook/execution/mapreduce/coordination/executor.rs:1086-1145`

```rust
// Submit merge to queue (serialized processing)
merge_queue.submit_merge(
    agent_id.to_string(),
    config.branch_name.clone(),  // agent-{job_id}_agent_{index}-item_{index}
    item_id.to_string(),
    env.clone(),
).await
```

**Result:** MapReduce worktree accumulates all agent changes on its branch (`prodigy-session-mapreduce-xxx`)

### 2. MapReduce → Parent Worktree Merge

**When:** After reduce phase completes
**What:** MapReduce worktree branch merges to parent session worktree
**How:** Claude-assisted merge using `/prodigy-merge-worktree` command
**Location:** `src/cook/execution/mapreduce/coordination/executor.rs:199-365`

```rust
// THE CRITICAL FIX: Use existing branch, don't create new one
let mapreduce_branch = format!("prodigy-{}", worktree_name);
// mapreduce_branch = "prodigy-session-mapreduce-20251012_223630"

// Verify branch exists before attempting merge
let branch_exists = subprocess
    .runner()
    .run(ProcessCommandBuilder::new("git")
        .args(["rev-parse", "--verify", &mapreduce_branch])
        .current_dir(&env.working_dir)
        .build())
    .await;

// Execute Claude merge in parent worktree
claude_executor.execute_claude_command(
    &format!("/prodigy-merge-worktree {}", mapreduce_branch),
    &parent_worktree,
    env_vars,
).await
```

**Result:** Parent worktree contains all MapReduce changes

### 3. Parent Worktree → Master Merge

**When:** After workflow completes and user confirms
**What:** Parent worktree branch merges to master/main
**How:** Standard worktree merge (also Claude-assisted if custom merge workflow configured)
**Location:** `src/worktree/manager.rs`

**Result:** All changes propagate to main repository

## The Bug That Was Fixed

### What Was Wrong

In `executor.rs`, the MapReduce-to-parent merge was creating a **new branch** instead of using the **existing branch**:

```rust
// BUG: Created new branch that didn't exist in parent
let mapreduce_branch = format!("merge-{}", worktree_name);
// Result: "merge-session-mapreduce-20251012_223630"

// Created this new branch in MapReduce worktree
Command::new("git")
    .args(["checkout", "-b", &mapreduce_branch])
    .await;

// Tried to merge branch that doesn't exist in parent!
claude_executor.execute_claude_command(
    &format!("/prodigy-merge-worktree {}", mapreduce_branch),
    &parent_worktree,
    env_vars,
).await
```

### Why It Failed

1. MapReduce worktree has branch: `prodigy-session-mapreduce-20251012_223630` (with all agent merges)
2. Code created new branch: `merge-session-mapreduce-20251012_223630` (empty, no changes)
3. Passed new branch name to Claude in parent worktree context
4. Claude's `/prodigy-merge-worktree` tried to merge `merge-session-mapreduce-20251012_223630`
5. Branch didn't exist in parent worktree's repository
6. Merge commit ended up on orphaned branch instead of parent worktree
7. Changes never propagated to master

### The Fix

```rust
// FIX: Use the existing branch that contains all agent merges
let mapreduce_branch = format!("prodigy-{}", worktree_name);
// Result: "prodigy-session-mapreduce-20251012_223630" ✓

// Verify it exists
let branch_exists = subprocess
    .runner()
    .run(ProcessCommandBuilder::new("git")
        .args(["rev-parse", "--verify", &mapreduce_branch])
        .current_dir(&env.working_dir)
        .build())
    .await;

if branch_exists.is_err() {
    return Err(MapReduceError::ProcessingError(format!(
        "MapReduce branch '{}' does not exist. Cannot merge to parent.",
        mapreduce_branch
    )));
}

// Pass existing branch to Claude (it exists in parent's repo)
claude_executor.execute_claude_command(
    &format!("/prodigy-merge-worktree {}", mapreduce_branch),
    &parent_worktree,
    env_vars,
).await
```

## Execution Flow with Git Operations

### Phase 1: Setup (in MapReduce Worktree)

```bash
# Prodigy creates parent worktree
cd main-repo
git worktree add -b prodigy-session-xxx ~/.prodigy/worktrees/{project}/session-xxx

# Prodigy creates MapReduce worktree
cd ~/.prodigy/worktrees/{project}/session-xxx
git worktree add -b prodigy-session-mapreduce-xxx ./session-mapreduce-xxx

# Setup phase executes in MapReduce worktree
cd ./session-mapreduce-xxx
# Setup commands run here, commits to prodigy-session-mapreduce-xxx
```

### Phase 2: Map (in Agent Worktrees)

```bash
# For each work item, create agent worktree
cd ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx
git worktree add -b agent-{job}_agent_{i}-item_{i} ./mapreduce-agent-{job}_agent_{i}

# Agent executes commands in its worktree
cd ./mapreduce-agent-{job}_agent_{i}
# Agent commands run here, commits to agent branch

# Agent merges to MapReduce worktree (serialized via merge queue)
cd ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx
git merge --no-ff agent-{job}_agent_{i}-item_{i}
# Changes now on prodigy-session-mapreduce-xxx branch
```

### Phase 3: Reduce (in MapReduce Worktree)

```bash
# Reduce phase executes in MapReduce worktree
cd ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx
# Reduce commands run here, commits to prodigy-session-mapreduce-xxx
```

### Phase 4: MapReduce → Parent Merge

```bash
# Merge MapReduce worktree to parent using Claude
cd ~/.prodigy/worktrees/{project}/session-xxx
claude /prodigy-merge-worktree prodigy-session-mapreduce-xxx

# Claude performs intelligent merge:
# - Switches to default branch (main/master) in parent worktree
# - Runs: git merge --no-ff prodigy-session-mapreduce-xxx
# - Resolves any conflicts intelligently
# - Creates merge commit
# - Changes now on prodigy-session-xxx branch in parent worktree
```

### Phase 5: Parent → Master Merge

```bash
# User confirms merge
cd main-repo
git merge --no-ff prodigy-session-xxx
# All changes now on master
```

## Verification Commands

### Check Current Branch and Structure

```bash
# List all worktrees
git worktree list

# Check current branch in MapReduce worktree
cd ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx
git branch --show-current
# Should show: prodigy-session-mapreduce-xxx

# Check what's merged into MapReduce branch
git log --oneline --graph --all | head -30
# Should show agent merges

# Verify branch exists in parent worktree's repo
cd ~/.prodigy/worktrees/{project}/session-xxx
git branch -a | grep prodigy-session-mapreduce
# Should show: prodigy-session-mapreduce-xxx
```

### After MapReduce-to-Parent Merge

```bash
# Check merge succeeded
cd ~/.prodigy/worktrees/{project}/session-xxx
git log --oneline | head -5
# Should show: "Merge worktree 'prodigy-session-mapreduce-xxx' into master"

# Verify files from agents are present
ls -la book/src/
# Should include all files created/modified by agents
```

## Common Issues and Debugging

### Issue: "Branch does not exist" during MapReduce merge

**Cause:** Wrong branch name passed to Claude (the original bug)

**Check:**
```bash
cd ~/.prodigy/worktrees/session-xxx/session-mapreduce-xxx
git branch
# Look for branch starting with "prodigy-"
```

**Fix:** Ensure code uses `prodigy-{worktree_name}` pattern, not `merge-{worktree_name}`

### Issue: Changes not in parent worktree after merge

**Cause:** Merge went to wrong branch or orphaned branch

**Check:**
```bash
cd ~/.prodigy/worktrees/{project}/session-xxx
git log --oneline --all | grep "Merge worktree"
# Should see merge commit on current branch
```

**Debug:**
```bash
# Check if merge is on orphaned branch
git branch -a | grep merge-
# If you see "merge-session-mapreduce-xxx", that's the bug
```

### Issue: Parent worktree not merging to master

**Cause:** MapReduce-to-parent merge didn't actually update parent worktree

**Check:**
```bash
# Compare commits
git log master..prodigy-session-xxx
# Should show commits from MapReduce workflow
# If empty, MapReduce merge didn't work
```

## Testing

See `tests/mapreduce_to_parent_merge_test.rs` for integration tests covering:

- Branch name pattern verification
- Branch existence check before merge
- Full MapReduce-to-parent merge flow
- Regression test for the bug

Run tests:
```bash
# Quick branch pattern test
cargo test test_mapreduce_branch_name_pattern

# Branch existence verification
cargo test test_mapreduce_branch_existence_check

# Full integration test (requires Claude CLI)
cargo test test_mapreduce_merges_to_parent_worktree --ignored
```

## References

- **Implementation:** `src/cook/execution/mapreduce/coordination/executor.rs:199-365`
- **Tests:** `tests/mapreduce_to_parent_merge_test.rs`
- **Merge Queue:** `src/cook/execution/mapreduce/merge_queue.rs`
- **Worktree Manager:** `src/worktree/manager.rs`
- **Bug Fix Commit:** [Reference the commit that fixed the branch name bug]

## Summary

The MapReduce worktree architecture enables massive parallel processing through:

1. **Isolation:** Each agent has its own worktree (no conflicts)
2. **Accumulation:** Changes merge upward through the hierarchy
3. **Correct branch names:** Using existing `prodigy-` branches instead of creating new ones
4. **Claude-assisted merging:** Intelligent conflict resolution for complex merges

The critical insight is that the MapReduce worktree's branch **already exists** and contains all the agent work. The merge step must pass this **existing branch name** to Claude, not create a new branch.
