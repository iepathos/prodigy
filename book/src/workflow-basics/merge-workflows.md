## Merge Workflows

Merge workflows execute when merging worktree changes back to the main branch. This feature enables custom validation, testing, and conflict resolution before integrating changes.

**When to use merge workflows:**
- Run tests before merging
- Validate code quality
- Handle merge conflicts automatically
- Sync with upstream changes

```yaml
merge:
  - shell: "git fetch origin"
  - shell: "git merge origin/main"
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
  timeout: 600  # Optional: timeout for entire merge phase (seconds)
```

**Available merge variables:**
- `${merge.worktree}` - Worktree name (e.g., "prodigy-session-abc123")
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (usually main/master)
- `${merge.session_id}` - Session ID for correlation

These variables are only available within the merge workflow context.

