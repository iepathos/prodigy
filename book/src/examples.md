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
# Global environment variables
env:
  NODE_ENV: production
  API_URL: https://api.production.com

# Secrets (automatically masked in logs)
secrets:
  API_KEY:
    secret: true
    value: "${SECRET_API_KEY}"

  # Optional: Add provider for external secret stores
  # DB_PASSWORD:
  #   secret: true
  #   value: "${DB_PASSWORD}"
  #   provider: "vault"  # Optional: external secret store

# Environment profiles for different contexts
# Note: Variables go directly under the profile name, not nested under 'env'
profiles:
  production:
    API_URL: https://api.production.com
    LOG_LEVEL: error

  staging:
    API_URL: https://api.staging.com
    LOG_LEVEL: warn

# Load additional variables from .env files
# Note: Paths are relative to workflow file location
env_files:
  - .env
  - .env.production

# Workflow steps (no 'commands' wrapper in simple format)
- shell: "cargo build --release"

# Use environment variables in commands
- shell: "echo 'Deploying to ${NODE_ENV} at ${API_URL}'"

# Override environment for specific step using env field
- shell: "./deploy.sh"
  env:
    LOG_LEVEL: debug
```

**Note:** Profiles are activated using the `--profile <name>` CLI flag when running workflows. For example:
```bash
# Use production profile
prodigy run workflow.yml --profile production

# Use staging profile
prodigy run workflow.yml --profile staging
```

Variables go directly under the profile name (not nested under 'env') because profiles use flattened serialization.

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
  distinct: "item.id"  # Prevent duplicate work items based on ID field

  # Advanced timeout configuration (optional)
  timeout_config:
    agent_timeout_secs: 600  # 10 minutes per agent
    # Additional timeout levels for fine-grained control:
    # item_timeout_secs: 300  # 5 minutes per item
    # phase_timeout_secs: 3600  # 1 hour for entire map phase

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
The bidirectional session-job mapping is stored in `~/.prodigy/state/{repo_name}/mappings/` and created when the workflow starts.

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

**Future capabilities** (not yet implemented, but planned):
- **Git context variables**: Access `files_modified`, `files_added` from git operations
- **Pattern filtering**: Filter file lists with `:*.rs` syntax
- **Format modifiers**: Advanced output transformation with `:json`, `:lines`, `:csv`
