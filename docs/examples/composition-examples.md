# Composition Examples

This section covers workflow composition (preview feature) and custom merge workflows.

## Example 12: Workflow Composition (Preview Feature)

> **Note**: Workflow composition features are partially implemented. Core composition logic exists but CLI integration is pending (Spec 131-133). This example shows the planned syntax.

```yaml
# Import reusable workflow fragments
imports:
  - "./workflows/common/test-suite.yml"
  - "./workflows/common/deploy.yml"

# Extend base workflow
extends: "./workflows/base-ci.yml"

name: extended-ci-workflow
mode: standard

# Template for reusable command sets
templates:
  rust_test:
    - shell: "cargo build"
    - shell: "cargo test"
    - shell: "cargo clippy"

  deploy_to_env:
    parameters:
      - env_name
      - target_url
    commands:
      - shell: "echo 'Deploying to ${env_name}'"
      - shell: "curl -X POST ${target_url}/deploy"

# Use templates in workflow
steps:
  - template: rust_test
  - template: deploy_to_env
    with:
      env_name: "production"
      target_url: "${API_URL}"
```

**Source**: Composition architecture from features.json:workflow_composition, implementation status note from drift analysis

### Planned Composition Features

- **Imports**: Reuse workflow fragments across projects
- **Extends**: Inherit from base workflows with overrides
- **Templates**: Parameterized command sets for DRY workflows
- **Parameters**: Type-safe template parameterization

### Current Status

- Core composition logic: Implemented
- Configuration parsing: Implemented
- CLI integration: Pending (Spec 131-133)
- Template rendering: Pending

### Workaround Until CLI Integration

Use YAML anchors and aliases for basic composition:

```yaml
# Define reusable blocks with anchors
.rust_test: &rust_test
  - shell: "cargo build"
  - shell: "cargo test"

.deploy: &deploy
  - shell: "echo 'Deploying...'"

# Reference with aliases
workflow:
  - *rust_test
  - *deploy
```

---

## Example 13: Custom Merge Workflows

MapReduce workflows execute in isolated git worktrees. When the workflow completes, you can define a custom merge workflow to control how changes are merged back to your original branch.

```yaml
name: code-review-with-merge
mode: mapreduce

# Environment variables available to merge commands
env:
  PROJECT_NAME: "my-project"
  NOTIFICATION_URL: "https://api.slack.com/webhooks/..."

setup:
  - shell: "find src -name '*.rs' > files.json"
  - shell: "jq -R -s -c 'split(\"\n\") | map(select(length > 0) | {path: .})' files.json > items.json"

map:
  input: "items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/review-code ${item.path}"
      commit_required: true
  max_parallel: 5

reduce:
  - claude: "/summarize-reviews ${map.results}"

# Custom merge workflow (executed when merging worktree back to original branch)
merge:
  commands:
    # Merge-specific variables are available:
    # ${merge.worktree} - Worktree name (e.g., "session-abc123")
    # ${merge.source_branch} - Source branch in worktree
    # ${merge.target_branch} - Target branch (where you started workflow)
    # ${merge.session_id} - Session ID for correlation

    # Pre-merge validation
    - shell: "echo 'Preparing to merge ${merge.worktree}'"
    - shell: "echo 'Source: ${merge.source_branch} → Target: ${merge.target_branch}'"

    # Run tests before merging
    - shell: "cargo test --all"
      on_failure:
        claude: "/fix-failing-tests before merge"
        commit_required: true
        max_attempts: 2

    # Run linting
    - shell: "cargo clippy -- -D warnings"
      on_failure:
        claude: "/fix-clippy-warnings"
        commit_required: true

    # Optional: Custom validation via Claude
    - claude: "/validate-merge-readiness ${merge.source_branch} ${merge.target_branch}"

    # Actually perform the merge using prodigy-merge-worktree
    # IMPORTANT: Always pass both source and target branches
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"

    # Post-merge notifications (using env vars)
    - shell: "echo 'Successfully merged ${PROJECT_NAME} changes from ${merge.worktree}'"
    # - shell: "curl -X POST ${NOTIFICATION_URL} -d 'Merge completed for ${PROJECT_NAME}'"

  # Optional: Timeout for entire merge phase (seconds)
  timeout: 600  # 10 minutes
```

**Source**: Merge workflow configuration from src/config/mapreduce.rs:84-94, merge variables from worktree merge orchestrator, example from workflows/mapreduce-env-example.yml:83-94, test from tests/merge_workflow_integration.rs:64-121

### Merge Workflow Features

1. **Merge-Specific Variables** (automatically provided):
   - `${merge.worktree}` - Name of the worktree being merged
   - `${merge.source_branch}` - Branch in worktree (usually `prodigy-mapreduce-...`)
   - `${merge.target_branch}` - Your original branch (main, master, feature-xyz, etc.)
   - `${merge.session_id}` - Session ID for tracking

2. **Pre-Merge Validation**:
   - Run tests, linting, or custom checks before merging
   - Use Claude commands for intelligent validation
   - Use `on_failure` handlers to fix issues automatically

3. **Environment Variables**:
   - Global `env` variables are available in merge commands
   - Useful for notifications, project-specific settings
   - Secrets are masked in merge command output

4. **Timeout Control**:
   - Optional `timeout` field (in seconds) for the merge phase
   - Prevents merge workflows from hanging indefinitely

### Important Notes

- Always pass **both** `${merge.source_branch}` and `${merge.target_branch}` to `/prodigy-merge-worktree`
- This ensures the merge targets your original branch, not a hardcoded main/master
- Without a custom merge workflow, you'll be prompted interactively to merge

### Handling Merge Failures

If merge validation fails (e.g., tests fail, linting fails), the `on_failure` handlers will attempt to fix the issues. If fixes cannot be applied automatically, the merge workflow will fail, and changes remain in the worktree for manual review:

```yaml
# Source: Pattern from workflows/mapreduce-env-example.yml:83-94
- shell: "cargo test --all"
  on_failure:
    claude: "/fix-failing-tests before merge"
    commit_required: true
    max_attempts: 2
    # If tests still fail after 2 attempts, workflow stops
    # Changes remain in worktree at ~/.prodigy/worktrees/{repo_name}/{session_id}/
```

### Recovery from Failed Merge

1. Navigate to the worktree: `cd ~/.prodigy/worktrees/{repo_name}/{session_id}/`
2. Fix issues manually and commit changes
3. Resume the merge workflow: `prodigy resume {session_id}`
4. Or manually merge: `git checkout {target_branch} && git merge {source_branch}`

### Simplified Format

If you don't need timeout configuration, you can use the simplified format:

```yaml
merge:
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

This is equivalent to `merge.commands` but more concise.
