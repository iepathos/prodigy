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

## Commit Tracking

Prodigy automatically creates git commits for changes:

```yaml
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
```

## Merge Workflows

Customize the merge process with validation and conflict resolution:

```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main"  # Pre-merge main
    - shell: "cargo test"             # Validate
    - shell: "cargo clippy"           # Lint
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600
```

### Merge Variables

Available in merge commands:
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (original branch)
- `${merge.session_id}` - Session ID

### Pre-Merge Validation

Run tests before merging:

```yaml
merge:
  commands:
    - shell: "cargo test --all"
    - shell: "cargo clippy -- -D warnings"
    - shell: "cargo fmt --check"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
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

- Feature branches: Tracks exact branch name
- Detached HEAD: Falls back to default branch (main/master)
- Deleted branches: Falls back to main/master

### Merge Target Logic

Default behavior: Merge to tracked original branch

Example prompt:

```
Merge session-abc123 to feature/my-feature? [y/N]
```

## Worktree Isolation for MapReduce

MapReduce workflows use nested worktrees:

```
original_branch
    ↓
parent worktree (session-xxx)
    ├→ Setup phase executes here
    ├→ Agent worktrees branch from parent
    │  ├→ agent-1 → merges back to parent
    │  ├→ agent-2 → merges back to parent
    │  └→ agent-N → merges back to parent
    ├→ Reduce phase executes here
    └→ User prompt: Merge to {original_branch}?
```

## Orphaned Worktree Recovery

If cleanup fails, worktrees are tracked as orphaned:

```bash
# List orphaned worktrees for a job
prodigy worktree clean-orphaned <job_id>

# Dry run to see what would be cleaned
prodigy worktree clean-orphaned <job_id> --dry-run

# Force cleanup
prodigy worktree clean-orphaned <job_id> --force
```

## See Also

- [Session Management](sessions.md) - Session lifecycle and state
- [MapReduce Overview](../mapreduce/overview.md) - MapReduce worktree architecture
- [Storage Architecture](storage.md) - Worktree storage locations
