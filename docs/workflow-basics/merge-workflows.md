## Merge Workflows

Merge workflows execute when merging worktree changes back to the main branch. This feature enables custom validation, testing, and conflict resolution before integrating changes.

**When to use merge workflows:**
- Run tests before merging
- Validate code quality
- Handle merge conflicts automatically
- Sync with upstream changes

### Configuration Formats

Merge workflows support two configuration formats (src/config/mapreduce.rs:96-123):

**1. Simplified Format** (direct array of commands, no timeout support):

```yaml
merge:
  - shell: "git fetch origin"
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**2. Full Format** (config object with commands array and optional timeout):

```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main"
    - shell: "cargo test"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600  # Optional: timeout for entire merge phase (seconds)
```

Use the simplified format for quick merge workflows without timeouts. Use the full format when you need timeout control for long-running merge operations.

**Important**: Always pass both `${merge.source_branch}` and `${merge.target_branch}` to the `/prodigy-merge-worktree` command. This ensures the merge targets the branch you were on when you started the workflow, not a hardcoded main/master branch.

### Available Merge Variables

The following variables are available exclusively within merge workflow commands. Variable interpolation happens before command execution, and these variables are NOT available in setup/map/reduce phases:

- `${merge.worktree}` - Worktree name (e.g., "prodigy-session-abc123")
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (the branch you were on when workflow started)
- `${merge.session_id}` - Session ID for correlation

Merge workflows also have access to all workflow [environment variables](environment-configuration.md) defined in the env block, including profile-specific values and secrets.

### Claude Merge Streaming

Claude commands in merge workflows respect verbosity settings (src/worktree/merge_orchestrator.rs:521-534):

- Use `-v` flag for real-time streaming output
- Set `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` to force streaming regardless of verbosity
- Default behavior shows clean minimal output

This provides full visibility into Claude's merge operations and tool invocations.

### Real-World Examples

**Pre-merge CI validation** (workflows/implement.yml:33-41):

```yaml
merge:
  - claude: "/prodigy-merge-master"  # Merge main into worktree first
  - claude: "/prodigy-ci"  # Run all CI checks
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Environment-aware merge** (workflows/mapreduce-env-example.yml:83-93):

```yaml
merge:
  commands:
    - shell: "echo Merging changes for $PROJECT_NAME"
    - shell: "echo Debug mode was: $DEBUG_MODE"
    - claude: "/validate-merge --branch ${merge.source_branch} --project $PROJECT_NAME"
```

**Documentation workflow with cleanup** (workflows/book-docs-drift.yml:93-100):

```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - claude: "/prodigy-merge-master --project ${PROJECT_NAME}"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```
