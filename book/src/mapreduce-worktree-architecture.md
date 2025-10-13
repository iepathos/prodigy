# MapReduce Worktree Architecture

MapReduce workflows in Prodigy use an isolated git worktree architecture that ensures the main repository remains untouched during workflow execution. This chapter explains the worktree hierarchy, branch naming conventions, merge flows, and debugging strategies.

## Overview

When you run a MapReduce workflow, Prodigy creates a hierarchical worktree structure:

```
Main Repository (untouched during execution)
    ↓
Parent Worktree (session-mapreduce-{id})
    ├── Setup Phase → Executes here
    ├── Reduce Phase → Executes here
    └── Map Phase → Each agent in child worktree
        ├── Child Worktree (agent-{id}-item-{n})
        ├── Child Worktree (agent-{id}-item-{n})
        └── Child Worktree (agent-{id}-item-{n})
```

This architecture provides complete isolation, allowing parallel agents to work independently while preserving a clean main repository.

## Worktree Hierarchy

### Parent Worktree

Created at the start of MapReduce workflow execution:

**Location**: `~/.prodigy/worktrees/{project}/session-mapreduce-{timestamp}`

**Purpose**:
- Isolates all workflow execution from main repository
- Hosts setup phase execution
- Hosts reduce phase execution
- Serves as merge target for agent results

**Branch**: `prodigy-session-mapreduce-{timestamp}`

### Child Worktrees

Created for each map agent:

**Location**: `~/.prodigy/worktrees/{project}/agent-{job_id}-{agent_id}-item-{n}`

**Purpose**:
- Complete isolation per agent
- Independent failure handling
- Parallel execution safety

**Branch**: `agent-{job_id}-{agent_id}-item-{n}` (branched from parent worktree)

## Branch Naming Conventions

Prodigy uses consistent branch naming to track worktree relationships:

### Parent Worktree Branch

Format: `prodigy-session-mapreduce-YYYYMMDD_HHMMSS`

Example: `prodigy-session-mapreduce-20250112_143052`

### Agent Worktree Branch

Format: `agent-{job_id}-{agent_id}-item-{item_index}`

Example: `agent-mapreduce-20250112_143052_agent_5-item_3`

**Components**:
- `job_id`: MapReduce job identifier
- `agent_id`: Unique agent identifier
- `item_index`: Work item index in the job

## Merge Flow

MapReduce workflows involve multiple merge operations to aggregate results:

### 1. Agent Merge (Child → Parent)

When an agent completes successfully:

```
Child Worktree (agent branch)
    ↓ merge
Parent Worktree (session branch)
```

**Process**:
1. Agent completes all commands successfully
2. Agent commits changes to its branch
3. Merge coordinator adds agent to merge queue
4. Sequential merge into parent worktree branch
5. Child worktree cleanup

### 2. MapReduce to Parent Merge

After all map agents complete and reduce phase finishes:

```
Parent Worktree (session branch)
    ↓ merge
Main Repository (original branch)
```

**Process**:
1. All agents merged into parent worktree
2. Reduce phase executes in parent worktree
3. User confirms merge to main repository
4. Sequential merge with conflict detection
5. Parent worktree cleanup

### Merge Strategies

**Fast-Forward When Possible**: If no divergence, use fast-forward merge

**Three-Way Merge**: When branches have diverged, perform three-way merge

**Conflict Handling**: Stop and report conflicts for manual resolution

## Agent Merge Details

### Merge Queue

Agents are added to a merge queue as they complete:

**Queue Location**: `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/merge_queue.json`

**Queue Format**:
```json
{
  "pending": [
    {
      "agent_id": "agent-1",
      "branch": "agent-mapreduce-20250112_143052_agent_1-item_0",
      "worktree_path": "/path/to/worktree",
      "completed_at": "2025-01-12T14:35:42Z"
    }
  ],
  "merged": [],
  "failed": []
}
```

### Sequential Merge Processing

Merges are processed sequentially to prevent conflicts:

1. Lock merge queue
2. Take next agent from pending queue
3. Perform merge into parent worktree
4. Update queue (move to merged or failed)
5. Release lock

## Parent to Master Merge

### Merge Confirmation

After reduce phase completes, Prodigy prompts for merge confirmation:

```
✓ MapReduce workflow completed successfully

Merge session-mapreduce-20250112_143052 to master? [y/N]
```

### Custom Merge Workflows

Configure custom merge validation:

```yaml
merge:
  - shell: "git fetch origin"
  - shell: "cargo test"
  - shell: "cargo clippy"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

### Merge Variables

Available during merge workflows:

- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Session branch name
- `${merge.target_branch}` - Main repository branch (usually master/main)
- `${merge.session_id}` - Session ID for correlation

## Debugging MapReduce Worktrees

### Inspecting Worktree State

```bash
# List all worktrees
git worktree list

# View worktree details
cd ~/.prodigy/worktrees/{project}/session-mapreduce-*
git status
git log

# View agent worktree
cd ~/.prodigy/worktrees/{project}/agent-*
git log --oneline
```

### Common Debugging Scenarios

**Agent Failed to Merge:**

1. Check merge queue: `cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/merge_queue.json`
2. Inspect failed agent worktree: `cd ~/.prodigy/worktrees/{project}/agent-*`
3. Review agent changes: `git diff master`
4. Check for conflicts: `git status`

**Parent Worktree Not Merging:**

1. Check parent worktree: `cd ~/.prodigy/worktrees/{project}/session-mapreduce-*`
2. Verify all agents merged: `git log --oneline`
3. Check for uncommitted changes: `git status`
4. Review merge history: `git log --graph --oneline --all`

### Merge Conflict Resolution

If merge conflicts occur:

```bash
# Navigate to parent worktree
cd ~/.prodigy/worktrees/{project}/session-mapreduce-*

# View conflicts
git status

# Resolve manually
vim <conflicted-file>

# Complete merge
git add <conflicted-file>
git commit
```

## Verification Commands

### Verify Main Repository is Clean

```bash
# Main repository should have no changes from MapReduce execution
git status
# Expected: nothing to commit, working tree clean
```

### Verify Worktree Isolation

```bash
# Check that parent worktree has changes
cd ~/.prodigy/worktrees/{project}/session-mapreduce-*
git status
git log --oneline

# Main repository should still be clean
cd /path/to/main/repo
git status
```

### Verify Agent Merges

```bash
# Check merge queue status
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/merge_queue.json

# Verify merged agents in parent worktree
cd ~/.prodigy/worktrees/{project}/session-mapreduce-*
git log --oneline | grep "Merge agent"
```

## Best Practices

### Worktree Management

- **Cleanup**: Remove old worktrees after successful merge: `prodigy worktree clean`
- **Monitoring**: Check worktree disk usage periodically
- **Inspection**: Review worktrees before deleting to verify results

### Merge Workflows

- **Test Before Merge**: Run tests in merge workflow to catch issues
- **Sync Upstream**: Fetch and merge origin/main before merging to main
- **Conflict Prevention**: Keep MapReduce jobs focused to minimize conflicts

### Debugging

- **Preserve Worktrees**: Don't delete worktrees until debugging is complete
- **Event Logs**: Review event logs for merge failures: `prodigy events {job_id}`
- **DLQ Review**: Check failed items that might indicate merge issues

## Troubleshooting

### Worktree Creation Fails

**Issue**: Cannot create parent or child worktree
**Solution**: Check disk space, verify git repository is valid, ensure no existing worktree with same name

### Agent Merge Fails

**Issue**: Agent results fail to merge into parent
**Solution**: Check merge queue, inspect agent worktree for conflicts, review agent changes

### Parent Merge Conflicts

**Issue**: Merging parent worktree to main causes conflicts
**Solution**: Resolve conflicts manually, consider rebasing parent worktree on latest main

### Orphaned Worktrees

**Issue**: Worktrees remain after workflow completion
**Solution**: Use `prodigy worktree clean` to remove old worktrees, or manually remove with `git worktree remove`

## See Also

- [MapReduce Workflows](mapreduce.md) - MapReduce workflow basics
- [Error Handling](error-handling.md) - Handling merge failures
- [Troubleshooting](troubleshooting.md) - General troubleshooting guide
