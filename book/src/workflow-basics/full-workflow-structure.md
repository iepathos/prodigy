## Full Workflow Structure

For more complex workflows, use the full format with explicit configuration:

```yaml
# Full format with environment and merge configuration
commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"

# Global environment variables (available to all commands)
env:
  NODE_ENV: production
  API_URL: https://api.example.com

# Secret environment variables (masked in logs)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files to load (.env format)
env_files:
  - .env.production

# Environment profiles (switch contexts easily)
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"

# Custom merge workflow (for worktree integration)
# Simplified format (direct array of commands)
merge:
  - shell: "git fetch origin"
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"

# OR with timeout (use config object format)
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main"
    - shell: "cargo test"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600  # Timeout in seconds for entire merge phase
```

**Source**: Merge workflow structure from `src/config/mapreduce.rs:86-124`

## Merge Workflow Formats

Prodigy supports two formats for merge workflows:

1. **Direct Array Format** - For simple merge operations without timeout:
   ```yaml
   merge:
     - shell: "git fetch origin"
     - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
   ```

2. **Config Object Format** - When you need to specify a timeout:
   ```yaml
   merge:
     commands:
       - shell: "git fetch origin"
       - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
     timeout: 600  # Optional timeout in seconds
   ```

**When to Use Custom Merge Workflows:**

Custom merge workflows execute when merging worktree changes back to the main branch. Use them for:
- Pre-merge validation and testing
- Automatic conflict resolution
- Running CI checks before merge
- Cleaning up temporary files

If no custom merge workflow is specified, Prodigy uses default merge behavior.

**Source**: Deserializer implementation in `src/config/mapreduce.rs:96-124`

## Merge Context Variables

The following variables are available in merge workflows:

- `${merge.worktree}` - Name of the worktree being merged
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (your original branch when workflow started)
- `${merge.session_id}` - Session ID for correlation and debugging

**Example with all variables**:
```yaml
merge:
  - shell: |
      echo "Merging worktree: ${merge.worktree}"
      echo "From: ${merge.source_branch}"
      echo "To: ${merge.target_branch}"
      echo "Session: ${merge.session_id}"
  - claude: "/validate-merge --branch ${merge.source_branch}"
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Source**: Variable substitution from `tests/merge_workflow_integration.rs:274-290`

## Real-World Examples

**Example 1: Pre-merge Validation** (from `workflows/implement.yml:32-42`):
```yaml
merge:
  - claude: "/prodigy-merge-master"  # Merge main into worktree first
  - claude: "/prodigy-ci"             # Run CI checks and auto-fix issues
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Example 2: Cleanup and Testing** (from `workflows/workflow-syntax-drift.yml:38-49`):
```yaml
merge:
  - shell: "rm -rf .prodigy/syntax-analysis"
  - shell: "git add -A && git commit -m 'chore: cleanup temp files' || true"
  - shell: "git fetch origin"
  - claude: "/prodigy-merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Example 3: With Environment Variables** (from `workflows/mapreduce-env-example.yml:82-94`):
```yaml
merge:
  commands:
    - shell: "echo Merging changes for $PROJECT_NAME"
    - claude: "/validate-merge --branch ${merge.source_branch} --project $PROJECT_NAME"
    - shell: "echo Merge completed"
  timeout: 600
```

## See Also

- [Merge Workflows](merge-workflows.md) - Detailed merge workflow documentation
- [Environment Configuration](environment-configuration.md) - Environment variables and secrets
- [Command Types](command-types.md) - Available command types in workflows
