# Git Integration

Prodigy provides deep git integration with worktree isolation, automatic commit tracking, and customizable merge workflows.

## Overview

Git integration features:
- **Worktree isolation**: Each session runs in isolated git worktree
- **Automatic commits**: Track all changes with automatic git commits
- **Commit tracking**: Full audit trail of modifications
- **Smart merging**: Customizable merge workflows with validation
- **Branch tracking**: Intelligent merge target detection

## Worktree Management

Every Prodigy session executes in an isolated git worktree:

```
~/.prodigy/worktrees/{repo_name}/
└── session-{session_id}/   # Isolated worktree for this session
```

### Benefits

- **Isolation**: Main repository remains untouched during execution
- **Parallelism**: Multiple sessions can run concurrently
- **Safety**: Failed workflows don't pollute main repo
- **Debugging**: Worktrees preserve full execution history

### Worktree Lifecycle

1. **Creation**: Prodigy creates worktree when workflow starts
2. **Execution**: All commands run in worktree context
3. **Changes**: Modifications committed automatically
4. **Completion**: User prompted to merge back to original branch
5. **Cleanup**: Worktree removed after successful merge

### Manual Worktree Management

```bash
# List worktrees
prodigy worktree ls

# Clean completed worktrees
prodigy worktree clean

# Force cleanup
prodigy worktree clean -f

# Clean orphaned worktrees (failed cleanup)
prodigy worktree clean-orphaned <job_id>
```

## Commit Tracking

Prodigy automatically creates commits for trackable changes:

### Automatic Commits

```yaml
- claude: "/implement-feature"
  commit_required: true  # Expect git commit from Claude

- shell: "cargo fmt"
  commit_required: true  # Create commit for formatting changes
```

### Commit Messages

Generated commit messages include:
- Command that created the change
- Workflow context
- Session ID for traceability

### Tracking Metrics

Access commit information via variables:
- `${git.commits}` - Number of commits created
- `${git.last_commit}` - SHA of last commit
- `${git.changed_files}` - Count of modified files

## Branch Tracking

Prodigy tracks the original branch for intelligent merging:

### Original Branch Detection

When creating a worktree:
- Captures current branch name (e.g., `feature/ui-improvements`)
- Falls back to default branch for detached HEAD
- Stores in worktree state for session lifetime

### Merge Target Logic

Merge back to the tracked original branch:
```yaml
# User on feature/auth-refactor when starting workflow
# Worktree merges back to feature/auth-refactor (not main!)
```

### Special Cases

- **Feature branches**: Merge back to exact feature branch
- **Detached HEAD**: Falls back to default branch (main/master)
- **Deleted branch**: Falls back to default branch if original deleted
- **Branch rename**: Uses branch name from worktree creation time

## Merge Workflows

Customize the merge process with validation and testing:

```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main"  # Sync with main first
    - shell: "cargo test"             # Run tests before merge
    - shell: "cargo clippy"           # Check for linting issues
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600  # 10 minutes for merge operations
```

### Merge Variables

Available in merge workflows:
- `${merge.worktree}` - Worktree name being merged
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (original branch)
- `${merge.session_id}` - Session ID for correlation

### Example: Pre-Merge Validation

```yaml
merge:
  commands:
    - shell: "cargo build --release"
    - shell: "cargo test --all"
    - shell: "cargo fmt --check"
    - shell: "cargo clippy -- -D warnings"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

### Example: Conflict Resolution

```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main --no-commit"
    - claude: "/resolve-conflicts"
      on_failure:
        shell: "git merge --abort"
    - shell: "git add -A"
    - shell: "git commit -m 'Merge main and resolve conflicts'"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

## MapReduce Worktree Isolation

In MapReduce workflows, all phases execute in isolated worktrees:

```
original_branch (e.g., feature/my-feature)
    ↓
parent worktree (session-xxx) ← Setup and reduce execute here
    ├→ agent-1 worktree → processes item, merges to parent
    ├→ agent-2 worktree → processes item, merges to parent
    └→ agent-N worktree → processes item, merges to parent
```

### Isolation Guarantees

1. **Setup phase**: Executes in parent worktree
2. **Map phase**: Each agent runs in child worktree
3. **Reduce phase**: Executes in parent worktree
4. **Final merge**: Parent worktree merges back to original branch

### Verification

Verify main repository is clean after MapReduce:

```bash
# Check main repo (should be clean)
git status

# Check worktree has changes
cd ~/.prodigy/worktrees/{repo}/session-xxx/
git status
git log
```

## Orphaned Worktree Recovery

Handle cleanup failures gracefully:

### Orphaned Worktree Registry

When cleanup fails, worktree path is registered:
```
~/.prodigy/orphaned_worktrees/{repo_name}/{job_id}.json
```

### Cleanup Command

```bash
# List orphaned worktrees
prodigy worktree clean-orphaned <job_id>

# Dry run
prodigy worktree clean-orphaned <job_id> --dry-run

# Force cleanup
prodigy worktree clean-orphaned <job_id> --force
```

### Common Cleanup Issues

- **Permission denied**: Check directory permissions
- **Disk full**: Free up space before retry
- **Directory busy**: Close editors/processes using worktree
- **Git locks**: Wait for concurrent git operations to complete

## Examples

### Feature Branch Workflow

```yaml
# User on feature/authentication
name: implement-auth

- claude: "/implement-auth-module"
  commit_required: true

- shell: "cargo test"
  on_failure:
    claude: "/fix-auth-tests"

# Merge back to feature/authentication (not main!)
merge:
  commands:
    - shell: "cargo test --all"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

### Multi-Stage CI with Merge Validation

```yaml
name: comprehensive-ci

- shell: "cargo build"
  commit_required: true

- shell: "cargo test"
  on_failure:
    claude: "/debug-test-failure"
    commit_required: true

- shell: "cargo clippy"
  commit_required: true

merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/${merge.target_branch}"
      on_failure:
        claude: "/resolve-merge-conflicts"
    - shell: "cargo test --release"
    - shell: "cargo doc --no-deps"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 900
```

## See Also

- [Worktree Isolation (Spec 127)](../../CLAUDE.md#worktree-isolation-spec-127-spec-134) - Technical details
- [Branch Tracking (Spec 110)](../../CLAUDE.md#branch-tracking-spec-110) - Original branch detection
- [Session Management](sessions.md) - Session lifecycle and worktree coordination
- [MapReduce Workflows](../mapreduce/index.md) - Parallel execution with worktrees
