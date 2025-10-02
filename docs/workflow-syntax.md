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
  # Modern syntax: Commands directly under agent_template
  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

  # DEPRECATED: Nested 'commands' syntax (still supported)
  # agent_template:
  #   commands:
  #     - claude: "/process '${item}'"

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
# Modern syntax: Commands directly under reduce
reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"

# DEPRECATED: Nested 'commands' syntax (still supported)
# reduce:
#   commands:
#     - claude: "/summarize ${map.results}"

# Optional: Custom merge workflow (supports two formats)
merge:
  # Simple array format
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  - shell: "cargo test"

# OR full format with timeout
# merge:
#   commands:
#     - shell: "git fetch origin"
#     - claude: "/merge-worktree ${merge.source_branch}"
#   timeout: 600  # Timeout in seconds

# Error handling policy
error_policy:
  on_item_failure: dlq  # dlq, retry, skip, stop, or custom handler name
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.2  # 20% failure rate
  error_collection: aggregate  # aggregate, immediate, or batched:N

  # Circuit breaker configuration
  circuit_breaker:
    failure_threshold: 5      # Open circuit after N failures
    success_threshold: 2      # Close circuit after N successes
    timeout: 60              # Seconds before attempting half-open
    half_open_requests: 3    # Test requests in half-open state

  # Retry configuration with backoff
  retry_config:
    max_attempts: 3
    backoff:
      type: exponential      # fixed, linear, exponential, fibonacci
      initial: 1000          # Initial delay in ms
      multiplier: 2          # For exponential
      max_delay: 30000       # Maximum delay in ms

# Convenience fields (alternative to nested error_policy)
# These top-level fields map to error_policy for simpler syntax
on_item_failure: dlq
continue_on_failure: true
max_failures: 5
```

### Setup Phase (Advanced)

The setup phase supports two formats: simple array OR full configuration object.

```yaml
# Simple array format
setup:
  - shell: "prepare-data.sh"
  - shell: "analyze-codebase.sh"

# Full configuration format with timeout and capture
setup:
  commands:
    - shell: "prepare-data.sh"
    - shell: "analyze-codebase.sh"

  # Timeout for entire setup phase (seconds)
  timeout: 300

  # Capture outputs from setup commands
  capture_outputs:
    # Simple format (legacy - just index)
    file_count: 0  # Capture from command at index 0

    # Full CaptureConfig format
    analysis_result:
      command_index: 1
      format: json  # string, number, json, lines, boolean
```

**Setup Phase Fields:**
- `commands` - Array of commands to execute (or use simple array format at top level)
- `timeout` - Timeout for entire setup phase in seconds
- `capture_outputs` - Map of variable names to command outputs (supports Simple(index) or full CaptureConfig)

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
    # DEPRECATED: 'command' field (use 'shell' instead)
    result_file: "result.json"
    threshold: 95  # Percentage completion required (default: 100.0)
    timeout: 60
    expected_schema: "validation-schema.json"  # Optional JSON schema

    # What to do if incomplete
    on_incomplete:
      claude: "/complete-implementation ${validation.gaps}"
      max_attempts: 3
      fail_workflow: true
      commit_required: true
      prompt: "Implementation incomplete. Continue?"  # Optional interactive prompt
```

**ValidationConfig Fields:**
- `shell` or `claude` - Single validation command (use `shell`, not deprecated `command`)
- `commands` - Array of commands for multi-step validation
- `result_file` - Path to JSON file with validation results
- `threshold` - Minimum completion percentage (default: 100.0)
- `timeout` - Timeout in seconds
- `expected_schema` - JSON schema for validation output structure

**OnIncompleteConfig Fields:**
- `shell` or `claude` - Single gap-filling command
- `commands` - Array of commands for multi-step gap filling
- `max_attempts` - Maximum retry attempts
- `fail_workflow` - Whether to fail workflow if validation incomplete
- `commit_required` - Whether to require commit after gap filling
- `prompt` - Optional interactive prompt for user guidance

**Alternative: Array format for multi-step validation**

```yaml
- claude: "/implement-feature"
  validate:
    # When using array format, ValidationConfig uses default threshold (100.0)
    # and creates a commands array
    - shell: "run-tests.sh"
    - shell: "check-coverage.sh"
    - claude: "/validate-implementation --output validation.json"
      result_file: "validation.json"
```

**Alternative: Multi-step gap filling**

```yaml
- claude: "/implement-feature"
  validate:
    shell: "validate.sh"
    result_file: "result.json"
    on_incomplete:
      commands:
        - claude: "/analyze-gaps ${validation.gaps}"
        - shell: "run-fix-script.sh"
        - claude: "/verify-fixes"
      max_attempts: 2
```

---

## Variable Interpolation

### Available Variables

#### Standard Variables
- `${workflow.name}` - Workflow name
- `${workflow.id}` - Workflow unique identifier
- `${workflow.iteration}` - Current iteration number
- `${step.name}` - Current step name
- `${step.index}` - Current step index
- `${step.files_changed}` - Files changed in current step
- `${workflow.files_changed}` - All files changed in workflow

#### Output Variables
- `${shell.output}` - Output from last shell command
- `${claude.output}` - Output from last Claude command
- `${last.output}` - Output from last executed command (any type)
- `${last.exit_code}` - Exit code from last command
- `${handler.output}` - Output from handler command
- `${test.output}` - Output from test command
- `${goal_seek.output}` - Output from goal-seeking command

#### MapReduce Variables
- `${item}` - Current work item in map phase
- `${item.value}` - Value of current item (for simple items)
- `${item.path}` - Path field of current item
- `${item.name}` - Name field of current item
- `${item.*}` - Access any item field using wildcard pattern (e.g., `${item.id}`, `${item.priority}`)
- `${item_index}` - Index of current item in the list
- `${item_total}` - Total number of items being processed
- `${map.key}` - Current map key
- `${map.total}` - Total items processed
- `${map.successful}` - Successfully processed items
- `${map.failed}` - Failed items
- `${map.results}` - Aggregated results
- `${worker.id}` - ID of the current worker agent

#### Merge Variables
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch
- `${merge.target_branch}` - Target branch
- `${merge.session_id}` - Session ID

#### Validation Variables
- `${validation.completion}` - Completion percentage
- `${validation.completion_percentage}` - Completion percentage (numeric)
- `${validation.implemented}` - List of implemented features
- `${validation.missing}` - Missing requirements
- `${validation.gaps}` - Gap details
- `${validation.status}` - Status (complete/incomplete/failed)

#### Git Context Variables
- `${step.commits}` - Commits in current step
- `${workflow.commits}` - All workflow commits

#### Legacy Variable Aliases

These legacy aliases are supported for backward compatibility but should be replaced with modern equivalents:

- `$ARG` / `$ARGUMENT` - Legacy aliases for `${item.value}` (available in WithArguments mode)
- `$FILE` / `$FILE_PATH` - Legacy aliases for `${item.path}` (available in WithFilePattern mode)

**Note:** Use the modern `${item.*}` syntax in new workflows instead of legacy aliases.

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
    duration: true  # Capture execution duration

# Access captured data
- shell: "echo 'Exit code: ${test_results.exit_code}'"
- shell: "echo 'Success: ${test_results.success}'"
- shell: "echo 'Duration: ${test_results.duration}s'"
```

---

## Environment Configuration

### Global Environment Configuration

```yaml
# Inherit parent process environment (default: true)
inherit: true

# Global environment variables
env:
  # Static variables (EnvValue::Static)
  NODE_ENV: production
  PORT: "3000"

  # Dynamic variables (EnvValue::Dynamic - computed from command)
  WORKER_COUNT:
    command: "nproc || echo 4"
    cache: true  # Cache result for reuse

  # Conditional variables (EnvValue::Conditional)
  DEPLOY_TARGET:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"
```

**Environment Control:**
- `inherit: false` - Start with clean environment instead of inheriting from parent process (default: true)

### Secrets Management

```yaml
secrets:
  # Simple format (syntactic sugar - parsed into structured format)
  API_KEY: "${env:SECRET_API_KEY}"
  DB_PASSWORD: "${file:~/.secrets/db.pass}"

  # Structured format (Provider variant)
  AWS_SECRET:
    provider: aws
    key: "my-app/api-key"

  VAULT_SECRET:
    provider: vault
    key: "secret/data/myapp"
    version: "v2"  # Optional version

  # Custom provider
  CUSTOM_SECRET:
    provider: custom-provider
    key: "secret-id"
```

**Supported Secret Providers:**
- `env` - Environment variable reference
- `file` - Read from file
- `vault` - HashiCorp Vault integration
- `aws` - AWS Secrets Manager
- `custom` - Custom provider (extensible)

### Environment Profiles

```yaml
profiles:
  development:
    description: "Development environment with debug enabled"
    env:
      NODE_ENV: development
      DEBUG: "true"
      API_URL: http://localhost:3000

  production:
    description: "Production environment configuration"
    env:
      NODE_ENV: production
      DEBUG: "false"
      API_URL: https://api.example.com

# Activate profile globally
active_profile: "development"
# OR use dynamic profile selection
active_profile: "${DEPLOY_ENV}"

commands:
  - shell: "npm run build"
```

**Note:** Profile activation uses the `active_profile` field at the root WorkflowConfig level, not at the command level.

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
  on_item_failure: dlq  # Options: dlq, retry, skip, stop, custom:<handler_name>

  # Continue after failures
  continue_on_failure: true

  # Stop after N failures
  max_failures: 10

  # Stop if failure rate exceeds threshold
  failure_threshold: 0.2  # 20%

  # How to collect errors
  error_collection: aggregate  # aggregate, immediate, batched:N

  # Circuit breaker configuration
  circuit_breaker:
    failure_threshold: 5      # Open circuit after N consecutive failures
    success_threshold: 2      # Close circuit after N successes in half-open state
    timeout: 60              # Seconds before attempting half-open state
    half_open_requests: 3    # Number of test requests in half-open state

  # Retry configuration with backoff strategies
  retry_config:
    max_attempts: 3
    backoff:
      type: exponential      # Options: fixed, linear, exponential, fibonacci
      initial: 1000          # Initial delay in milliseconds
      multiplier: 2          # Multiplier for exponential backoff
      max_delay: 30000       # Maximum delay in milliseconds
```

**Backoff Strategy Options:**
- `fixed` - Fixed delay between retries: `{type: fixed, delay: 1000}`
- `linear` - Linear increase: `{type: linear, initial: 1000, increment: 500}`
- `exponential` - Exponential increase: `{type: exponential, initial: 1000, multiplier: 2}`
- `fibonacci` - Fibonacci sequence: `{type: fibonacci, initial: 1000}`

**Error Metrics:**
Prodigy automatically tracks error metrics including total items, successful/failed/skipped counts, failure rate, and can detect failure patterns with suggested remediation actions.

### Command-Level Error Handling

```yaml
# Using on_failure with OnFailureConfig
- shell: "cargo clippy"
  on_failure:
    command:
      claude: "/fix-warnings ${shell.output}"
    max_attempts: 3
    fail_workflow: false  # Don't fail entire workflow
    strategy: exponential  # Backoff strategy

# Note: continue_on_error is only available in legacy CommandMetadata format
# For WorkflowStepCommand, use on_failure with fail_workflow: false instead
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
    env:
      API_URL: https://api.production.com
  staging:
    env:
      API_URL: https://api.staging.com

# Activate profile based on DEPLOY_ENV
active_profile: "${DEPLOY_ENV}"

commands:
  - shell: "cargo build --release"
  - shell: "echo 'Deploying to ${DEPLOY_ENV} at ${API_URL}'"
  - shell: "deploy.sh ${DEPLOY_ENV}"
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

## Advanced Command Features

### Enhanced Retry Configuration

```yaml
# Retry with exponential backoff
- shell: "flaky-api-call.sh"
  retry:
    max_attempts: 5
    backoff:
      type: exponential
      initial: 1000  # 1 second
      multiplier: 2
      max_delay: 30000  # 30 seconds
      jitter: true
```

### Working Directory

```yaml
# Run command in specific directory
- shell: "npm install"
  working_dir: "/path/to/project"

- shell: "pwd"  # Will show /path/to/project
```

### Auto-Commit

```yaml
# Automatically commit changes if detected
- claude: "/refactor-code"
  auto_commit: true
```

### Step-Level Environment Configuration

Commands support step-specific environment configuration with advanced control:

```yaml
# Basic step-level environment variables
- shell: "echo $API_URL"
  env:
    API_URL: "https://api.staging.com"
    DEBUG: "true"

# Advanced step environment features
- shell: "isolated-command.sh"
  working_dir: "/tmp/sandbox"  # Change working directory
  clear_env: true              # Clear all parent environment variables
  temporary: true              # Restore previous environment after step
  env:
    ISOLATED_VAR: "value"
```

**Step Environment Fields:**
- `env` - Step-specific environment variables (HashMap<String, String>)
- `working_dir` - Working directory for command execution
- `clear_env` - Start with clean environment (default: false)
- `temporary` - Restore previous environment after step completes (default: false)

### Output File Redirection

```yaml
# Redirect output to file
- shell: "cargo test"
  output_file: "test-results.txt"
```

### Modular Handlers

```yaml
# Use custom handler
- handler:
    name: "custom-validator"
    attributes:
      path: "src/"
      threshold: 80
```

### Step Validation

```yaml
# Validate step success after execution
- shell: "deploy.sh"
  step_validate:
    shell: "curl -f https://app.com/health"
    timeout: 30
    on_failure:
      shell: "rollback.sh"
```

### Advanced Exit Code Handling

```yaml
# Map exit codes to full workflow steps
- shell: "cargo check"
  on_exit_code:
    0:
      shell: "echo 'Build successful'"
    101:
      claude: "/fix-compilation-errors"
      commit_required: true
    other:
      shell: "echo 'Unexpected error'"
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
| `capture` | string | Variable name to capture output (replaces deprecated `capture_output`) |
| `capture_format` | enum | Format: `string`, `number`, `json`, `lines`, `boolean` |
| `capture_streams` | object | CaptureStreams object with fields: `stdout` (bool), `stderr` (bool), `exit_code` (bool), `success` (bool), `duration` (bool) |
| `on_success` | object | Command to run on success |
| `on_failure` | object | OnFailureConfig with nested command, max_attempts, fail_workflow, strategy |
| `on_exit_code` | map | Maps exit codes to full WorkflowStep objects (e.g., `101: {claude: "/fix"}`) |
| `validate` | object | Validation configuration |
| `handler` | object | HandlerStep for modular command handlers |
| `retry` | object | RetryConfig for enhanced retry with exponential backoff and jitter |
| `working_dir` | string | Working directory for command execution |
| `env` | map | Command-level environment variables (HashMap<String, String>) |
| `output_file` | string | Redirect command output to a file |
| `auto_commit` | boolean | Automatically create commit if changes detected (default: false) |
| `commit_config` | object | Advanced CommitConfig for commit control |
| `step_validate` | object | StepValidationSpec for post-execution validation |
| `skip_validation` | boolean | Skip step validation (default: false) |
| `validation_timeout` | number | Timeout in seconds for validation operations |
| `ignore_validation_failure` | boolean | Continue workflow even if validation fails (default: false) |

### Deprecated Fields

These fields are deprecated but still supported for backward compatibility:

- `test:` - Use `shell:` with `on_failure:` instead
- `command:` in ValidationConfig - Use `shell:` instead
- `capture_output: true/false` - Use `capture: "variable_name"` instead
- Nested `commands:` in `agent_template` and `reduce` - Use direct array format instead
- Legacy variable aliases (`$ARG`, `$ARGUMENT`, `$FILE`, `$FILE_PATH`) - Use modern `${item.*}` syntax

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
