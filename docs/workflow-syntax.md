# Prodigy Workflow Syntax Guide

Complete reference for creating Prodigy workflow files in YAML format.

## Table of Contents

- [Workflow Types](#workflow-types)
- [Standard Workflows](#standard-workflows)
- [MapReduce Workflows](#mapreduce-workflows)
- [Command Types](#command-types)
- [Variable Interpolation](#variable-interpolation)
- [Environment Configuration](#environment-configuration)
- [Advanced Features](#advanced-features)
- [Error Handling](#error-handling)
- [Examples](#examples)

---

## Workflow Types

Prodigy supports two primary workflow types:

1. **Standard Workflows**: Sequential command execution
2. **MapReduce Workflows**: Parallel processing with map and reduce phases

---

## Standard Workflows

### Basic Structure

```yaml
# Simple array format (most common)
- shell: "echo 'Starting workflow...'"
- claude: "/prodigy-analyze"
- shell: "cargo test"
```

### Full Configuration Format

```yaml
# Full format with environment and merge configuration
commands:
  - shell: "cargo build"
  - claude: "/prodigy-test"

# Global environment variables
env:
  NODE_ENV: production
  API_URL: https://api.example.com

# Secret environment variables (masked in logs)
secrets:
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files to load (.env format)
env_files:
  - .env.production

# Environment profiles
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"

# Custom merge workflow
merge:
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  timeout: 600  # Optional timeout in seconds
```

---

## MapReduce Workflows

### Complete Structure

```yaml
name: parallel-processing
mode: mapreduce

# Optional setup phase
setup:
  - shell: "generate-work-items.sh"
  - shell: "debtmap analyze . --output items.json"

# Map phase: Process items in parallel
map:
  # Input source (JSON file or command)
  input: "items.json"

  # JSONPath expression to extract items
  json_path: "$.items[*]"

  # Agent template (commands run for each item)
  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

  # Maximum parallel agents
  max_parallel: 10

  # Optional: Filter items
  filter: "item.score >= 5"

  # Optional: Sort items
  sort_by: "item.priority DESC"

  # Optional: Limit number of items
  max_items: 100

  # Optional: Skip items
  offset: 10

  # Optional: Deduplicate by field
  distinct: "item.id"

  # Optional: Agent timeout in seconds
  agent_timeout_secs: 300

# Reduce phase: Aggregate results
reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"

# Optional: Custom merge workflow
merge:
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  - shell: "cargo test"

# Error handling policy
error_policy:
  on_item_failure: dlq  # dlq, retry, skip, stop, or custom
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.2  # 20% failure rate
  error_collection: aggregate  # aggregate, immediate, or batched:N
```

### Setup Phase (Advanced)

```yaml
setup:
  commands:
    - shell: "prepare-data.sh"
    - shell: "analyze-codebase.sh"

  # Timeout for entire setup phase
  timeout: 300

  # Capture outputs from setup commands
  capture_outputs:
    file_count:
      command_index: 0
      format: number
    analysis_result:
      command_index: 1
      format: json
```

---

## Command Types

### 1. Shell Commands

```yaml
# Simple shell command
- shell: "cargo test"

# With output capture
- shell: "ls -la | wc -l"
  capture: "file_count"

# With failure handling
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings ${shell.output}"

# With timeout
- shell: "cargo bench"
  timeout: 600  # seconds

# With conditional execution
- shell: "cargo build --release"
  when: "${tests_passed}"
```

### 2. Claude Commands

```yaml
# Simple Claude command
- claude: "/prodigy-analyze"

# With arguments
- claude: "/prodigy-implement-spec ${spec_file}"

# With commit requirement
- claude: "/prodigy-fix-bugs"
  commit_required: true

# With output capture
- claude: "/prodigy-generate-plan"
  capture: "implementation_plan"
```

### 3. Goal-Seeking Commands

Iteratively refine code until a validation threshold is met.

```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/prodigy-coverage --improve"
    validate: "cargo tarpaulin --print-summary | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
    fail_on_incomplete: true
  commit_required: true
```

**Fields:**
- `goal`: Human-readable description
- `claude` or `shell`: Command to execute for refinement
- `validate`: Command that outputs `score: N` (0-100)
- `threshold`: Minimum score to consider complete
- `max_attempts`: Maximum refinement iterations
- `timeout_seconds`: Optional timeout per attempt
- `fail_on_incomplete`: Whether to fail workflow if threshold not met

### 4. Foreach Commands

Iterate over a list with optional parallelism.

```yaml
- foreach:
    input: "find . -name '*.rs' -type f"  # Command
    # OR
    # input: ["file1.rs", "file2.rs"]    # List

    parallel: 5  # Number of parallel executions (or true/false)

    do:
      - claude: "/analyze-file ${item}"
      - shell: "cargo check ${item}"

    continue_on_error: true
    max_items: 50
```

### 5. Validation Commands

Validate implementation completeness with automatic retry.

```yaml
- claude: "/implement-auth-spec"
  validate:
    shell: "debtmap validate --spec auth.md --output result.json"
    result_file: "result.json"
    threshold: 95  # Percentage completion required
    timeout: 60

    # What to do if incomplete
    on_incomplete:
      claude: "/complete-implementation ${validation.gaps}"
      max_attempts: 3
      fail_workflow: true
      commit_required: true
```

**Alternative: Array format for multi-step validation**

```yaml
- claude: "/implement-feature"
  validate:
    - shell: "run-tests.sh"
    - shell: "check-coverage.sh"
    - claude: "/validate-implementation --output validation.json"
      result_file: "validation.json"
```

---

## Variable Interpolation

### Available Variables

#### Standard Variables
- `${shell.output}` - Output from last shell command
- `${claude.output}` - Output from last Claude command
- `${step.files_changed}` - Files changed in current step
- `${workflow.files_changed}` - All files changed in workflow

#### MapReduce Variables
- `${item}` - Current work item in map phase
- `${item.field}` - Access item field (e.g., `${item.id}`)
- `${map.total}` - Total items processed
- `${map.successful}` - Successfully processed items
- `${map.failed}` - Failed items
- `${map.results}` - Aggregated results

#### Merge Variables
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch
- `${merge.target_branch}` - Target branch
- `${merge.session_id}` - Session ID

#### Validation Variables
- `${validation.completion}` - Completion percentage
- `${validation.missing}` - Missing requirements
- `${validation.gaps}` - Gap details
- `${validation.status}` - Status (complete/incomplete/failed)

#### Git Context Variables
- `${step.commits}` - Commits in current step
- `${step.commit_count}` - Number of commits in step
- `${step.insertions}` - Lines inserted in step
- `${step.deletions}` - Lines deleted in step
- `${workflow.commits}` - All workflow commits
- `${workflow.commit_count}` - Total commits

### Custom Variable Capture

```yaml
# Capture to custom variable
- shell: "ls -la | wc -l"
  capture: "file_count"
  capture_format: number  # number, string, json, lines, boolean

# Use in next command
- shell: "echo 'Found ${file_count} files'"

# Capture specific streams
- shell: "cargo test 2>&1"
  capture: "test_results"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: true
    success: true

# Access captured data
- shell: "echo 'Exit code: ${test_results.exit_code}'"
- shell: "echo 'Success: ${test_results.success}'"
```

---

## Environment Configuration

### Environment Variables

```yaml
env:
  # Static variables
  NODE_ENV: production
  PORT: "3000"

  # Dynamic variables (computed from command)
  WORKER_COUNT:
    command: "nproc || echo 4"
    cache: true

  # Conditional variables
  DEPLOY_TARGET:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"
```

### Secrets Management

```yaml
secrets:
  # Reference environment variable
  API_KEY: "${env:SECRET_API_KEY}"

  # Reference from file
  DB_PASSWORD: "${file:~/.secrets/db.pass}"
```

### Environment Profiles

```yaml
profiles:
  development:
    NODE_ENV: development
    DEBUG: "true"
    API_URL: http://localhost:3000

  production:
    NODE_ENV: production
    DEBUG: "false"
    API_URL: https://api.example.com

# Use profile
commands:
  - shell: "npm run build"
    env:
      profile: development  # Apply development profile
```

---

## Advanced Features

### Conditional Execution

```yaml
# Based on expression
- shell: "cargo build --release"
  when: "${tests_passed}"

# On success
- shell: "cargo test"
  on_success:
    shell: "cargo bench"

# On failure
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings"
    max_attempts: 3
    fail_workflow: false

# On exit code
- shell: "cargo check"
  on_exit_code:
    0:
      shell: "echo 'Success!'"
    101:
      claude: "/fix-compilation-errors"
```

### Output Capture Formats

```yaml
# String (default)
- shell: "git rev-parse HEAD"
  capture: "commit_hash"
  capture_format: string

# Number
- shell: "wc -l < file.txt"
  capture: "line_count"
  capture_format: number

# JSON
- shell: "cargo metadata --format-version 1"
  capture: "metadata"
  capture_format: json

# Lines (array)
- shell: "find . -name '*.rs'"
  capture: "rust_files"
  capture_format: lines

# Boolean
- shell: "test -f README.md && echo true || echo false"
  capture: "has_readme"
  capture_format: boolean
```

### Nested Conditionals

```yaml
- shell: "cargo check"
  on_success:
    shell: "cargo build --release"
    on_success:
      shell: "cargo test --release"
      on_failure:
        claude: "/debug-failures '${shell.output}'"
```

### Timeout Configuration

```yaml
# Command-level timeout
- shell: "cargo bench"
  timeout: 600  # 10 minutes

# MapReduce agent timeout
map:
  agent_timeout_secs: 300
  timeout_config:
    default: 300
    per_command:
      "cargo test": 600
      "cargo bench": 1800
```

---

## Error Handling

### Workflow-Level Error Policy

```yaml
# For MapReduce workflows
error_policy:
  # What to do when item fails
  on_item_failure: dlq  # Options: dlq, retry, skip, stop

  # Continue after failures
  continue_on_failure: true

  # Stop after N failures
  max_failures: 10

  # Stop if failure rate exceeds threshold
  failure_threshold: 0.2  # 20%

  # How to collect errors
  error_collection: aggregate  # aggregate, immediate, batched:N
```

### Command-Level Error Handling

```yaml
# Continue on error
- shell: "cargo clippy"
  continue_on_error: true

# Retry configuration
- shell: "flaky-test.sh"
  on_failure:
    claude: "/debug-test"
    max_attempts: 3
    fail_workflow: false  # Don't fail entire workflow
```

### Dead Letter Queue (DLQ)

Failed items in MapReduce workflows are sent to DLQ for retry:

```bash
# Retry failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 5

# Dry run
prodigy dlq retry <job_id> --dry-run
```

---

## Examples

### Example 1: Simple Build and Test

```yaml
- shell: "cargo build"
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
- shell: "cargo clippy"
```

### Example 2: Coverage Improvement with Goal Seeking

```yaml
- goal_seek:
    goal: "Achieve 80% test coverage"
    claude: "/improve-coverage"
    validate: |
      coverage=$(cargo tarpaulin | grep 'Coverage' | sed 's/.*: \([0-9.]*\)%.*/\1/')
      echo "score: ${coverage%.*}"
    threshold: 80
    max_attempts: 5
  commit_required: true
```

### Example 3: Parallel Code Review

```yaml
name: parallel-code-review
mode: mapreduce

setup:
  - shell: "find src -name '*.rs' > files.txt"
  - shell: "jq -R -s -c 'split(\"\n\") | map(select(length > 0) | {path: .})' files.txt > items.json"

map:
  input: items.json
  json_path: "$.[:1]"
  agent_template:
    - claude: "/review-file ${item.path}"
    - shell: "cargo check ${item.path}"
  max_parallel: 5

reduce:
  - claude: "/summarize-reviews ${map.results}"
```

### Example 4: Conditional Deployment

```yaml
- shell: "cargo test --quiet && echo true || echo false"
  capture: "tests_passed"
  capture_format: boolean

- shell: "cargo build --release"
  when: "${tests_passed}"

- shell: "docker build -t myapp ."
  when: "${tests_passed}"
  on_success:
    shell: "docker push myapp:latest"
```

### Example 5: Multi-Step Validation

```yaml
- claude: "/implement-feature auth"
  commit_required: true
  validate:
    - shell: "cargo test auth"
    - shell: "cargo clippy -- -D warnings"
    - claude: "/validate-implementation --output validation.json"
      result_file: "validation.json"
      threshold: 90
      on_incomplete:
        - claude: "/complete-gaps ${validation.gaps}"
          commit_required: true
        max_attempts: 2
```

### Example 6: Environment-Aware Workflow

```yaml
env:
  DEPLOY_ENV:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"

profiles:
  production:
    API_URL: https://api.production.com
  staging:
    API_URL: https://api.staging.com

commands:
  - shell: "cargo build --release"
  - shell: "echo 'Deploying to ${DEPLOY_ENV}'"
  - shell: "deploy.sh ${DEPLOY_ENV}"
    env:
      profile: "${DEPLOY_ENV}"
```

### Example 7: Complex MapReduce with Error Handling

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

  agent_template:
    - claude: "/fix-debt-item '${item.description}'"
      commit_required: true
    - shell: "cargo test"
      on_failure:
        claude: "/debug-and-fix"
        max_attempts: 2

reduce:
  - shell: "debtmap analyze . --output debt-after.json"
  - claude: "/compare-debt-reports --before debt.json --after debt-after.json"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.3
```

---

## Command Reference

### Command Fields

All command types support these common fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier for referencing outputs |
| `timeout` | number | Command timeout in seconds |
| `commit_required` | boolean | Whether command should create a git commit |
| `when` | string | Conditional execution expression |
| `capture` | string | Variable name to capture output |
| `capture_format` | string | Format: `string`, `number`, `json`, `lines`, `boolean` |
| `capture_streams` | object | Which streams to capture (stdout, stderr, etc.) |
| `on_success` | object | Command to run on success |
| `on_failure` | object | Command to run on failure |
| `validate` | object | Validation configuration |

### Deprecated Fields

These fields are deprecated but still supported:

- `test:` - Use `shell:` with `on_failure:` instead
- `command:` in validation - Use `shell:` instead
- `capture_output: true/false` - Use `capture:` with variable name instead

---

## Best Practices

1. **Use descriptive variable names** for captured output
2. **Set appropriate timeouts** for long-running commands
3. **Use validation** for iterative refinement tasks
4. **Leverage goal-seeking** for quality improvements
5. **Use MapReduce** for parallel processing of independent items
6. **Handle errors gracefully** with on_failure and error_policy
7. **Keep workflows modular** by breaking into smaller steps
8. **Use environment profiles** for different deployment targets
9. **Capture important outputs** for use in later steps
10. **Document complex workflows** with comments

---

## Troubleshooting

### Common Issues

1. **Variables not interpolating**: Ensure proper `${}` syntax
2. **Capture not working**: Check `capture_format` matches output type
3. **Validation failing**: Ensure validate command outputs `score: N` format
4. **MapReduce items not found**: Verify JSONPath expression with test data
5. **Timeout errors**: Increase timeout values or optimize commands

### Debug Tips

```yaml
# Enable verbose output
- shell: "set -x; your-command"

# Inspect variables
- shell: "echo 'Variable value: ${my_var}'"

# Capture all streams for debugging
- shell: "cargo test 2>&1"
  capture: "test_output"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: true
```

---

## Version Compatibility

This documentation reflects Prodigy workflow syntax as of version 0.2.0+

For older versions, some features may not be available:
- Goal-seeking: 0.1.5+
- Validation with on_incomplete: 0.1.7+
- MapReduce custom merge: 0.1.8+
- Advanced capture formats: 0.1.9+
- Conditional execution (when): 0.2.0+
