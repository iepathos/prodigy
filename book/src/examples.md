# Examples

## Example 1: Simple Build and Test

```yaml
- shell: "cargo build"
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
- shell: "cargo clippy"
```

---

## Example 2: Coverage Improvement with Goal Seeking

```yaml
- goal_seek:
    goal: "Achieve 80% test coverage"
    claude: "/improve-coverage"  # Can also use 'shell' for shell commands
    validate: |
      coverage=$(cargo tarpaulin | grep 'Coverage' | sed 's/.*: \([0-9.]*\)%.*/\1/')
      echo "score: ${coverage%.*}"
    threshold: 80
    max_attempts: 5
```

**Note:** The `goal_seek` command will automatically commit changes made by the Claude command. Commit behavior is controlled by the command execution, not by the `goal_seek` configuration.

---

## Example 3: Foreach Iteration

```yaml
# Test multiple configurations in sequence
- foreach:
    - rust-version: "1.70"
      profile: debug
    - rust-version: "1.71"
      profile: release
    - rust-version: "stable"
      profile: release
  do:
    - shell: "rustup install ${foreach.item.rust-version}"
    - shell: "cargo +${foreach.item.rust-version} build --profile ${foreach.item.profile}"
    - shell: "cargo +${foreach.item.rust-version} test"

# Parallel foreach with error handling
- foreach:
    - "web-service"
    - "api-gateway"
    - "worker-service"
  parallel: 3  # Options: false (sequential), true (default parallelism), or number (specific count)
  continue_on_error: true
  do:
    - shell: "cd services/${foreach.item} && cargo build"
    - shell: "cd services/${foreach.item} && cargo test"
      on_failure:
        claude: "/fix-service-tests ${foreach.item}"
```

---

## Example 4: Parallel Code Review

```yaml
name: parallel-code-review
mode: mapreduce

setup:
  - shell: "find src -name '*.rs' > files.txt"
  - shell: "jq -R -s -c 'split(\"\n\") | map(select(length > 0) | {path: .})' files.txt > items.json"

map:
  input: items.json
  json_path: "$[*]"  # Process all items in root array
  agent_template:
    - claude: "/review-file ${item.path}"
      id: "review"
      capture_output: "review_result"
      capture_format: "json"  # Formats: string, json, lines, number, boolean - see Example 5
    - shell: "cargo check ${item.path}"
  max_parallel: 5

reduce:
  - claude: "/summarize-reviews ${map.results}"
```

**Note:** JSONPath `"$[*]"` matches all items in the root array. Since the setup phase creates an array of `{path: ...}` objects, each map agent receives an `item` object with `item.path` available for use in commands.

**Advanced JSONPath Patterns:**
- `$.items[*]` - Extract items from nested object
- `$.items[*].files[*]` - Extract from nested arrays (flattens results)
- `$.items[?(@.priority > 5)]` - Filter items by condition
- `$[?(@.severity == 'critical')]` - Filter array by field value

---

## Example 5: Conditional Deployment

```yaml
- shell: "cargo test --quiet && echo true || echo false"
  id: "test"
  capture_output: "test_result"  # Canonical field name (alias: 'capture')
  capture_format: "boolean"  # Supported formats explained below
  timeout: 300  # Timeout in seconds (5 minutes)

- shell: "cargo build --release"
  when: "${test_result} == true"

- shell: "docker build -t myapp ."
  when: "${test_result} == true"
  on_success:
    shell: "docker push myapp:latest"
```

**Note:** `capture_format` options:
- `string` - Raw text output (default)
- `json` - Parse output as JSON object
- `lines` - Split output into array of lines
- `number` - Parse output as numeric value
- `boolean` - Parse as true/false based on exit code or output text

**Advanced capture options:**
```yaml
# Capture specific streams (stdout, stderr, exit_code, success, duration)
- shell: "cargo build 2>&1"
  capture_output: "build_output"
  capture_streams: "stdout,stderr,exit_code"  # Capture multiple streams

# Access captured values
- shell: "echo 'Exit code was ${build_output.exit_code}'"
```

---

## Example 6: Multi-Step Validation

```yaml
- claude: "/implement-feature auth"
  commit_required: true
  validate:
    commands:
      - shell: "cargo test auth"
      - shell: "cargo clippy -- -D warnings"
      - claude: "/validate-implementation --output validation.json"
    result_file: "validation.json"
    threshold: 90
    on_incomplete:
      claude: "/complete-gaps ${validation.gaps}"
      commit_required: true
      max_attempts: 2
```

---

## Example 7: Environment-Aware Workflow

```yaml
# Global environment variables (including secrets with masking)
env:
  # Regular variables
  NODE_ENV: production
  API_URL: https://api.production.com

  # Secrets (automatically masked in logs)
  # Use secret: true and value fields for sensitive data
  API_KEY:
    secret: true
    value: "${SECRET_API_KEY}"

  # Secret with external provider
  DB_PASSWORD:
    secret: true
    value: "${DB_PASSWORD}"
    # provider: "vault"  # Optional: external secret store (not yet implemented)

# Environment profiles for different contexts
profiles:
  production:
    env:
      API_URL: https://api.production.com
      LOG_LEVEL: error
    description: "Production environment with error-level logging"

  staging:
    env:
      API_URL: https://api.staging.com
      LOG_LEVEL: warn
    description: "Staging environment with warning-level logging"

# Load additional variables from .env files
# Note: Paths are relative to workflow file location
env_files:
  - .env
  - .env.production

# Workflow steps
- shell: "cargo build --release"

# Use environment variables in commands
- shell: "echo 'Deploying to ${NODE_ENV} at ${API_URL}'"

# Override environment for specific step using env field
- shell: "./deploy.sh"
  env:
    LOG_LEVEL: debug
```

**Source**: Environment configuration from src/cook/environment/config.rs:12-36, secret masking from src/cook/environment/config.rs:84-96

**Note:** Profiles are activated using the `--profile <name>` CLI flag when running workflows. For example:
```bash
# Use production profile
prodigy run workflow.yml --profile production

# Use staging profile
prodigy run workflow.yml --profile staging
```

**Secrets Masking**: Variables with `secret: true` are automatically masked in:
- Command output logs
- Error messages
- Event logs
- Checkpoint files

Example masked output:
```
$ echo 'API key is ***'
```

**Source**: Example workflow from workflows/mapreduce-env-example.yml:7-40, profile structure from tests/environment_workflow_test.rs:68-88

---

## Example 8: Complex MapReduce with Error Handling

```yaml
name: tech-debt-elimination
mode: mapreduce

setup:
  - shell: "debtmap analyze . --output debt.json"

map:
  input: debt.json
  json_path: "$.items[*]"
  filter: "item.severity == 'critical'"
  sort_by: "item.priority DESC"
  max_items: 20
  max_parallel: 5
  distinct: "item.id"  # Deduplication: Prevents processing duplicate items based on this field

  # Timeout configuration (optional - default is 600 seconds / 10 minutes)
  timeout_config:
    agent_timeout_secs: 600  # Maximum time per agent execution
    cleanup_grace_period_secs: 30  # Time allowed for cleanup after timeout

  agent_template:
    - claude: "/fix-debt-item '${item.description}'"
      commit_required: true
    - shell: "cargo test"
      on_failure:
        claude: "/debug-and-fix"

reduce:
  - shell: "debtmap analyze . --output debt-after.json"
  - claude: "/compare-debt-reports --before debt.json --after debt-after.json"

error_policy:
  on_item_failure: dlq  # Default: dlq (failed items to Dead Letter Queue)
  continue_on_failure: true  # Default: true (continue despite failures)
  max_failures: 5  # Optional: stop after N failures
  failure_threshold: 0.3  # Optional: stop if >30% fail
  error_collection: aggregate  # Default: aggregate (Options: aggregate, immediate, batched:{size})
```

**Note:** The entire `error_policy` block is optional with sensible defaults. If not specified, failed items go to the Dead Letter Queue (`on_item_failure: dlq`), workflow continues despite failures (`continue_on_failure: true`), and errors are aggregated at the end (`error_collection: aggregate`). Use `max_failures` or `failure_threshold` to fail fast if too many items fail.

**Deduplication with `distinct`**: The `distinct` field enables idempotent processing by preventing duplicate work items. When specified, Prodigy extracts the value of the given field (e.g., `item.id`) from each work item and ensures only unique values are processed. This is useful when:
- Input data may contain duplicates
- Resuming a workflow after adding new items
- Preventing wasted work on identical items
- Ensuring exactly-once processing semantics

Example: With `distinct: "item.id"`, if your input contains `[{id: "1", ...}, {id: "2", ...}, {id: "1", ...}]`, only items with IDs "1" and "2" will be processed (the duplicate "1" is skipped).

**Resuming MapReduce Workflows:**
MapReduce jobs can be resumed using either the session ID or job ID:
```bash
# Resume using session ID
prodigy resume session-mapreduce-1234567890

# Resume using job ID
prodigy resume-job mapreduce-1234567890

# Unified resume command (auto-detects ID type)
prodigy resume mapreduce-1234567890
```

**Session-Job ID Mapping**: The bidirectional mapping between session IDs and job IDs is stored in `~/.prodigy/state/{repo_name}/mappings/` and created automatically when the MapReduce workflow starts. This allows you to resume using either the session ID (e.g., `session-mapreduce-1234567890`) or the job ID (e.g., `mapreduce-1234567890`), and Prodigy will automatically find the correct checkpoint data.

**Source**: Session-job mapping implementation from MapReduce checkpoint and resume (Spec 134)

**Debugging Failed Agents:**
When agents fail, DLQ entries include a `json_log_location` field pointing to the Claude JSON log file for debugging:
```bash
# View failed items and their log locations
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Inspect the Claude interaction for a failed agent
cat <json_log_location> | jq
```
This allows you to see exactly what tools Claude invoked and why the agent failed.

---

## Example 9: Generating Configuration Files

```yaml
# Generate a JSON configuration file
- write_file:
    path: "config/deployment.json"
    format: json  # Options: text, json, yaml
    create_dirs: true  # Create parent directories if they don't exist
    content:
      environment: production
      api_url: "${API_URL}"
      features:
        - auth
        - analytics
        - notifications
      timeout: 30

# Generate a YAML configuration file
- write_file:
    path: "config/services.yml"
    format: yaml
    content:
      services:
        web:
          image: "myapp:latest"
          ports:
            - "8080:8080"
        database:
          image: "postgres:15"
          environment:
            POSTGRES_DB: "${DB_NAME}"

# Generate a plain text report
- write_file:
    path: "reports/summary.txt"
    format: text
    mode: "0644"  # File permissions (optional)
    content: |
      Deployment Summary
      ==================
      Environment: ${NODE_ENV}
      API URL: ${API_URL}
      Timestamp: $(date)
```

---

## Example 10: Advanced Features

```yaml
# Nested error handling with retry configuration
- shell: "cargo build --release"
  on_failure:
    shell: "cargo clean"
    on_success:
      shell: "cargo build --release"
      max_attempts: 2
  on_success:
    shell: "cargo test --release"

# Complex conditional execution with max_attempts
- shell: "cargo test"
  id: "test"
  capture_output: "test_output"

- claude: "/fix-tests"
  when: "${test_output} contains 'FAILED'"
  max_attempts: 3

# Conditional deployment based on test results
- shell: "cargo build --release"
  when: "${test.exit_code} == 0"

# Multi-condition logic
- shell: "./deploy.sh"
  when: "${test_output} contains 'passed' and ${build_output} contains 'Finished'"
```

**Note:** Advanced features currently supported:
- **Nested handlers**: Chain `on_failure` and `on_success` handlers for complex error recovery
- **Max attempts**: Combine with conditional execution for automatic retry logic
- **Conditional execution**: Use `when` clauses with captured output or variables
- **Complex conditionals**: Combine multiple conditions with `and`/`or` operators
- **Working directory**: Per-command directory control using `working_dir` field in step environment
- **Git context variables**: Automatic tracking of file changes during workflow execution

**Example of working_dir usage:**
```yaml
# Run command in specific directory
- shell: "cargo test"
  working_dir: "subproject/"  # Execute in subproject/ directory

# Or set for multiple commands
- shell: "npm install"
  env:
    NODE_ENV: production
  working_dir: "frontend/"
```

**Git Context Variables (Spec 122):**
Prodigy automatically tracks file changes during workflow execution and exposes them as variables:

```yaml
# Access files changed in current step
- shell: "echo 'Modified files: ${step.files_modified}'"
- shell: "echo 'Added files: ${step.files_added}'"
- shell: "echo 'Deleted files: ${step.files_deleted}'"

# Format as JSON array
- shell: "echo '${step.files_modified:json}'"

# Filter by glob pattern (only Rust files)
- shell: "echo 'Rust files changed: ${step.files_modified:*.rs}'"

# Access workflow-level aggregations
- shell: "echo 'Total commits: ${workflow.commit_count}'"
- shell: "echo 'All modified files: ${workflow.files_modified}'"

# Access uncommitted changes
- shell: "echo 'Staged files: ${git.staged_files}'"
- shell: "echo 'Unstaged files: ${git.unstaged_files}'"

# Pattern filtering for git context
- claude: "/review-changes ${git.modified_files|pattern:**/*.rs}"
- shell: "echo 'Changed Rust files: ${git.staged_files|pattern:**/*.rs}'"

# Basename-only output (no paths)
- shell: "echo 'File names: ${git.modified_files|basename}'"
```

**Available Git Context Variables:**
- `${step.files_added}` - Files added in current step
- `${step.files_modified}` - Files modified in current step
- `${step.files_deleted}` - Files deleted in current step
- `${step.commits}` - Commit SHAs created in this step
- `${step.insertions}` - Lines inserted in this step
- `${step.deletions}` - Lines deleted in this step
- `${workflow.commit_count}` - Total commits in workflow
- `${workflow.files_modified}` - All files modified across workflow
- `${git.staged_files}` - Currently staged files (uncommitted)
- `${git.unstaged_files}` - Modified but unstaged files
- `${git.modified_files}` - All uncommitted modifications

**Supported Formats and Filters:**
- `:json` - Format as JSON array
- `:*.rs` - Filter by glob pattern (e.g., `*.rs`, `src/**/*.py`)
- `|pattern:**/*.rs` - Advanced glob pattern filtering
- `|basename` - Extract just file names without paths

**Source**: Git context tracking from src/cook/workflow/git_context.rs:1-120, variable resolution from src/cook/workflow/git_context.rs:36-42

**Troubleshooting MapReduce Cleanup:**
If agent worktree cleanup fails (due to disk full, permission errors, etc.), use the orphaned worktree cleanup command:
```bash
# List and clean orphaned worktrees for a specific job
prodigy worktree clean-orphaned <job_id>

# Dry run to preview what would be cleaned
prodigy worktree clean-orphaned <job_id> --dry-run

# Force cleanup without confirmation
prodigy worktree clean-orphaned <job_id> --force
```
Note: Agent execution status is independent of cleanup status. If an agent completes successfully but cleanup fails, the agent is still marked as successful and results are preserved.

**Source**: Orphaned worktree cleanup from MapReduce cleanup failure handling (Spec 136)

---

## Example 11: Custom Merge Workflows

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

**Merge Workflow Features:**

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

**Important Notes:**
- Always pass **both** `${merge.source_branch}` and `${merge.target_branch}` to `/prodigy-merge-worktree`
- This ensures the merge targets your original branch, not a hardcoded main/master
- Without a custom merge workflow, you'll be prompted interactively to merge

**Simplified Format:**
If you don't need timeout configuration, you can use the simplified format:

```yaml
merge:
  - shell: "cargo test"
  - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

This is equivalent to `merge.commands` but more concise.
