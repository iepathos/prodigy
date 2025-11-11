# Git Integration

Prodigy provides deep git integration with worktree isolation, automatic commit tracking, smart merging, and branch management. All workflow executions run in isolated git worktrees to protect your main repository.

## Overview

Git integration features:
- **Worktree management** - Isolated execution environments per session
- **Commit tracking** - Automatic commits for change audit trail
- **Merge workflows** - Customizable merge process with validation
- **Branch tracking** - Intelligent branch management and targeting

## Worktree Management

Every workflow execution runs in an isolated git worktree:

```
original_branch (e.g., master, feature-xyz)
    ↓
session worktree (session-abc123)
    ├→ Workflow executes here
    ├→ All changes tracked independently
    └→ Merge back to original branch on completion
```

Benefits:
- Main repository never modified during execution
- Multiple concurrent workflow runs
- Clean rollback on failures
- Full execution history preserved

### Creating Worktrees

```rust
// Source: src/worktree/builder.rs:134-152
let manager = WorktreeBuilder::new()
    .with_base_path(base_path)
    .build()?;

// Create worktree with auto-generated ID
let session = manager.create_session().await?;

// Create worktree with specific ID
let session = manager.create_session_with_id("session-abc123").await?;
```

### Worktree Location

Worktrees are stored in:

```
~/.prodigy/worktrees/{repo_name}/{session_id}/
```

### Cleanup

Clean up completed worktrees:

```bash
# List worktrees
prodigy worktree ls

# Clean specific worktree
prodigy worktree clean session-abc123

# Force cleanup
prodigy worktree clean -f session-abc123
```

**Cleanup Behavior** (Source: tests/worktree_cleanup_integration.rs:32-51):
- After successful merge, uncommitted changes in worktree are safe to discard
- Auto-cleanup uses force=true when session is marked as merged
- Prevents "Auto-cleanup failed" warnings after merge completion

## Commit Tracking

Prodigy automatically creates git commits for changes:

```yaml
# Source: workflows/implement.yml
- claude: "/implement-feature '${item.name}'"
  # Automatically commits changes made by Claude

- shell: "cargo fmt"
  commit_required: true  # Expect changes to be committed
```

### Commit Messages

Commits include:
- Command that made the change
- Timestamp
- Session correlation ID

Example commit message:

```
claude: /implement-feature 'user-authentication'

Session: session-abc123
Timestamp: 2025-11-11T10:30:00Z
```

### Change Audit Trail

View all changes made during workflow:

```bash
cd ~/.prodigy/worktrees/prodigy/session-abc123/
git log

# View changes with stats
git log --stat

# View specific commit
git show <commit-hash>
```

## Merge Workflows

Customize the merge process with validation and conflict resolution:

```yaml
# Source: workflows/mkdocs-drift.yml:86-93
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main"  # Pre-merge main
    - shell: "cargo test"             # Validate
    - shell: "cargo clippy"           # Lint
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600
```

!!! note "Merge Target Branch"
    Always pass both `${merge.source_branch}` and `${merge.target_branch}` to the `/prodigy-merge-worktree` command. This ensures the merge targets the branch you were on when you started the workflow, not a hardcoded main/master branch.

### Merge Variables

Available in merge commands:
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (original branch)
- `${merge.session_id}` - Session ID

### Pre-Merge Validation

Run tests before merging:

```yaml
# Source: workflows/implement.yml:33-38
merge:
  commands:
    - shell: "cargo test --all"
    - shell: "cargo clippy -- -D warnings"
    - shell: "cargo fmt --check"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

### Merge with Environment Variables

Use environment variables in merge workflows:

```yaml
# Source: workflows/mapreduce-env-example.yml:83-89
merge:
  commands:
    - shell: "echo Merging changes for $PROJECT_NAME"
    - shell: "echo Debug mode was: $DEBUG_MODE"
    - claude: "/validate-merge --branch ${merge.source_branch} --project $PROJECT_NAME"
```

## Branch Tracking

Prodigy tracks the original branch when creating worktrees:

```bash
# Start workflow from feature branch
git checkout feature/new-feature
prodigy run workflow.yml

# Worktree merges back to feature/new-feature
```

### Original Branch Detection

```rust
// Source: src/worktree/manager_queries.rs:144
pub async fn get_merge_target(&self, session_name: &str) -> Result<String> {
    // Reads original_branch from WorktreeState
    // Falls back to default branch if original was deleted
}
```

Detection behavior:
- **Feature branches**: Tracks exact branch name (e.g., `feature/new-feature`)
- **Detached HEAD**: Falls back to default branch (main/master)
- **Deleted branches**: Falls back to main/master at merge time

### Merge Target Logic

Default behavior: Merge to tracked original branch

Example prompt:

```
Merge session-abc123 to feature/my-feature? [y/N]
```

!!! tip "Branch Workflow"
    When you start a workflow from a feature branch, all changes merge back to that feature branch, not main. This enables safe experimentation without affecting the main branch.

## Worktree Isolation for MapReduce

MapReduce workflows use nested worktrees for complete isolation:

```
original_branch
    ↓
parent worktree (session-xxx)
    ├→ Setup phase executes here
    ├→ Agent worktrees branch from parent
    │  ├→ agent-1 → processes item → merges back to parent
    │  ├→ agent-2 → processes item → merges back to parent
    │  └→ agent-N → processes item → merges back to parent
    ├→ Reduce phase executes here (aggregates results)
    └→ User prompt: Merge to {original_branch}?
```

!!! note "Complete Isolation"
    All MapReduce phases (setup, map, reduce) execute in the isolated parent worktree. The main repository remains untouched until final user-approved merge.

### Benefits

- **Safety**: Main repository never modified during workflow execution
- **Parallelism**: Multiple map agents work concurrently without conflicts
- **Reproducibility**: Each workflow run starts from a clean state
- **Debugging**: Worktree preserves full execution history for analysis
- **Recovery**: Failed workflows don't pollute the main repository

## Orphaned Worktree Recovery

If cleanup fails (e.g., permission denied, disk full), worktrees are tracked as orphaned in `~/.prodigy/orphaned_worktrees/{repo_name}/{job_id}.json`:

```bash
# List orphaned worktrees for a job
prodigy worktree clean-orphaned <job_id>

# Dry run to see what would be cleaned
prodigy worktree clean-orphaned <job_id> --dry-run

# Force cleanup
prodigy worktree clean-orphaned <job_id> --force
```

### Common Cleanup Failure Causes

- **Permission Denied**: Directory locked by process or insufficient permissions
- **Disk Full**: Not enough space to perform cleanup operations
- **Directory Busy**: Files open in editor or process using directory
- **Git Locks**: Repository locked by concurrent git operation

!!! tip "Agent Success Preserved"
    Agent execution status is independent of cleanup status. If an agent completes successfully but cleanup fails, the agent is still marked as successful and its work is preserved.

## See Also

- [Session Management](sessions.md) - Session lifecycle and state
- [MapReduce Overview](../mapreduce/overview.md) - MapReduce worktree architecture
- [Storage Architecture](storage.md) - Worktree storage locations
