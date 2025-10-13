# Error Handling

Prodigy provides comprehensive error handling at both the workflow level (for MapReduce jobs) and the command level (for individual workflow steps). This chapter covers the practical features available for handling failures gracefully.

---

## Command-Level Error Handling

Command-level error handling allows you to specify what happens when a single workflow step fails. Use the `on_failure` configuration to define recovery, cleanup, or fallback strategies.

### Simple Forms

For basic error handling, use the simplest form that meets your needs:

```yaml
# Ignore errors - don't fail the workflow
- shell: "optional-cleanup.sh"
  on_failure: true

# Single recovery command (shell or claude)
- shell: "npm install"
  on_failure: "npm cache clean --force"

- shell: "cargo clippy"
  on_failure: "/fix-warnings"

# Multiple recovery commands
- shell: "build-project"
  on_failure:
    - "cleanup-artifacts"
    - "/diagnose-build-errors"
    - "retry-build"
```

### Advanced Configuration

For more control over error handling behavior:

```yaml
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings ${shell.output}"
    fail_workflow: false     # Continue workflow even if handler fails
    max_attempts: 3          # Retry original command up to 3 times (setting > 1 enables auto-retry)
```

**Available Fields:**
- `shell` - Shell command to run on failure
- `claude` - Claude command to run on failure
- `fail_workflow` - Whether to fail the entire workflow (default: `false`)
- `max_attempts` - Maximum retry attempts for the original command (default: `1`)
- `max_retries` - Alternative name for `max_attempts` (both are supported for backward compatibility)

**Notes:**
- When `max_attempts > 1`, Prodigy automatically retries the original command after running the failure handler (the deprecated `retry_original` flag is no longer needed)
- Retry behavior is now controlled by the `max_attempts`/`max_retries` value, not a separate flag
- You can specify both `shell` and `claude` commands - they will execute in sequence
- By default, having a handler means the workflow continues even if the step fails

### Detailed Handler Configuration

For complex error handling scenarios with multiple commands and fine-grained control:

```yaml
- shell: "deploy-production"
  on_failure:
    strategy: recovery        # Options: recovery, fallback, cleanup, custom
    timeout: 300             # Handler timeout in seconds
    handler_failure_fatal: true  # Fail workflow if handler fails
    fail_workflow: false     # Don't fail workflow if step fails
    capture:                 # Capture handler output to variables
      error_log: "handler_output"
      rollback_status: "rollback_result"
    commands:
      - shell: "rollback-deployment"
        continue_on_error: true
      - claude: "/analyze-deployment-failure"
      - shell: "notify-team"
```

**Handler Configuration Fields:**
- `strategy` - Handler strategy (recovery, fallback, cleanup, custom)
- `timeout` - Handler timeout in seconds
- `handler_failure_fatal` - Fail workflow if handler fails
- `fail_workflow` - Whether to fail the entire workflow
- `capture` - Map of variable names to capture from handler output (e.g., `error_log: "handler_output"`)
- `commands` - List of handler commands to execute

**Handler Strategies:**
- `recovery` - Try to fix the problem and retry (default)
- `fallback` - Use an alternative approach
- `cleanup` - Clean up resources
- `custom` - Custom handler logic

**Handler Command Fields:**
- `shell` or `claude` - The command to execute
- `continue_on_error` - Continue to next handler command even if this fails

### Success Handling

Execute commands when a step succeeds. The `on_success` field accepts a full WorkflowStep configuration with all available fields.

**Simple Form:**
```yaml
- shell: "deploy-staging"
  on_success:
    shell: "notify-success"
    claude: "/update-deployment-docs"
```

**Advanced Form with Full WorkflowStep Configuration:**
```yaml
- shell: "build-production"
  on_success:
    claude: "/update-build-metrics"
    timeout: 60              # Success handler timeout
    capture: "metrics"       # Capture output to variable
    working_dir: "dist"      # Run in specific directory
    when: "${build.target} == 'release'"  # Conditional execution
```

**Note:** The `on_success` handler supports all WorkflowStep fields including `timeout`, `capture`, `working_dir`, `when`, and nested `on_failure` handlers.

### Commit Requirements

Specify whether a workflow step must create a git commit:

```yaml
- claude: "/implement-feature"
  commit_required: true   # Fail if no commit is made
```

This is useful for ensuring that Claude commands that are expected to make code changes actually do so.

---

## Workflow-Level Error Policy (MapReduce)

For MapReduce workflows, you can configure workflow-level error policies that control how the entire job responds to failures. This is separate from command-level error handling and only applies to MapReduce mode.

### Basic Configuration

```yaml
name: process-items
mode: mapreduce

error_policy:
  # What to do when a work item fails
  on_item_failure: dlq      # Options: dlq, retry, skip, stop, custom:<handler_name>

  # Continue processing after failures
  continue_on_failure: true

  # Stop after this many failures
  max_failures: 10

  # Stop if failure rate exceeds threshold (0.0 to 1.0)
  failure_threshold: 0.2    # Stop if 20% of items fail

  # How to report errors
  error_collection: aggregate  # Options: aggregate, immediate, batched
```

**Item Failure Actions:**
- `dlq` - Send failed items to Dead Letter Queue for later retry (default)
- `retry` - Retry the item immediately with backoff (if retry_config is set)
- `skip` - Skip the failed item and continue
- `stop` - Stop the entire workflow on first failure
- `custom:<name>` - Use a custom failure handler (not yet implemented)

**Error Collection Strategies:**
- `aggregate` - Collect all errors and report at the end (default)
- `immediate` - Report errors as they occur
- `batched: {size: N}` - Report errors in batches of N items

### Circuit Breaker

Prevent cascading failures by opening a circuit after consecutive failures:

```yaml
error_policy:
  circuit_breaker:
    failure_threshold: 5      # Open circuit after 5 consecutive failures
    success_threshold: 2      # Close circuit after 2 successes
    timeout: 30s             # Duration in humantime format (e.g., 30s, 1m, 500ms)
    half_open_requests: 3    # Test requests in half-open state
```

**Note:** The `timeout` field uses humantime format supporting `1s`, `100ms`, `2m`, `30s` for duration parsing

### Retry Configuration with Backoff

Configure automatic retry behavior for failed items:

```yaml
error_policy:
  on_item_failure: retry
  retry_config:
    max_attempts: 3
    backoff:
      type: exponential
      initial: 1s            # Initial delay (duration format)
      multiplier: 2          # Double delay each retry
```

**Backoff Strategy Options:**

```yaml
# Fixed delay between retries
# Always waits the same duration
backoff:
  type: fixed
  delay: 1s

# Linear increase in delay
# Calculates: delay = initial + (retry_count * increment)
# Example: 1s, 1.5s, 2s, 2.5s...
backoff:
  type: linear
  initial: 1s
  increment: 500ms

# Exponential backoff (recommended)
# Calculates: delay = initial * (multiplier ^ retry_count)
# Example: 1s, 2s, 4s, 8s...
backoff:
  type: exponential
  initial: 1s
  multiplier: 2

# Fibonacci sequence delays
# Calculates: delay = initial * fibonacci(retry_count)
# Example: 1s, 1s, 2s, 3s, 5s...
backoff:
  type: fibonacci
  initial: 1s
```

**Important:** All duration values use humantime format (e.g., `1s`, `100ms`, `2m`, `30s`), not milliseconds.

### Error Metrics

Prodigy automatically tracks error metrics for MapReduce jobs using the `ErrorMetrics` structure:

**Available Fields:**
- `total_items` - Total number of work items processed
- `successful` - Number of items that completed successfully
- `failed` - Number of items that failed
- `skipped` - Number of items that were skipped
- `failure_rate` - Percentage of failures (0.0 to 1.0)
- `error_types` - Map of error types to their frequency counts
- `failure_patterns` - Detected recurring error patterns with suggested remediation

**Accessing Metrics:**

Access metrics during execution or after completion to understand job health:

```yaml
# In your reduce phase
reduce:
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"
  - shell: "echo 'Failure rate: ${map.failure_rate}'"
```

You can also access metrics programmatically via the Prodigy API or through CLI commands like `prodigy events` to view detailed error statistics.

---

## Dead Letter Queue (DLQ)

The Dead Letter Queue stores failed work items from MapReduce jobs for later retry or analysis. This is only available for MapReduce workflows, not regular workflows.

### Sending Items to DLQ

Configure your MapReduce workflow to use DLQ:

```yaml
mode: mapreduce
error_policy:
  on_item_failure: dlq
```

Failed items are automatically sent to the DLQ with:
- Original work item data
- Failure reason and error message
- Timestamp of failure
- Attempt history

### Retrying Failed Items

Use the CLI to retry failed items:

```bash
# Retry all failed items for a job
prodigy dlq retry <job_id>

# Retry with custom parallelism (default: 5)
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run
```

**DLQ Retry Features:**
- Streams items to avoid memory issues with large queues
- Respects original workflow's max_parallel setting (unless overridden)
- Preserves correlation IDs for tracking
- Updates DLQ state (removes successful, keeps failed)
- Supports interruption and resumption
- Shared across worktrees for centralized failure tracking

### DLQ Storage

DLQ data is stored in:
```
~/.prodigy/dlq/{repo_name}/{job_id}/
```

This centralized storage allows multiple worktrees to share the same DLQ.

---

## Best Practices

### When to Use Command-Level Error Handling

- **Recovery:** Use `on_failure` to fix issues and retry (e.g., clearing cache before reinstalling)
- **Cleanup:** Use `strategy: cleanup` to clean up resources after failures
- **Fallback:** Use `strategy: fallback` for alternative approaches
- **Notifications:** Use handler commands to notify teams of failures

### When to Use Workflow-Level Error Policy

- **MapReduce jobs:** Use error_policy for consistent failure handling across all work items
- **Failure thresholds:** Use max_failures or failure_threshold to prevent runaway jobs
- **Circuit breakers:** Use when external dependencies might fail cascading
- **DLQ:** Use for large batch jobs where you want to retry failures separately

### Error Information Available

When a command fails, you can access error information in handler commands:

```yaml
- shell: "risky-command"
  on_failure:
    claude: "/analyze-error ${shell.output}"
```

The `${shell.output}` variable contains the command's stdout/stderr output.

### Common Patterns

**Cleanup and Retry:**
```yaml
- shell: "npm install"
  on_failure:
    - "npm cache clean --force"
    - "rm -rf node_modules"
    - "npm install"
```

**Conditional Recovery:**
```yaml
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
  max_attempts: 3
  fail_workflow: false
```

**Critical Step with Notification:**
```yaml
- shell: "deploy-production"
  on_failure:
    commands:
      - shell: "rollback-deployment"
      - shell: "notify-team 'Deployment failed'"
    fail_workflow: true   # Still fail workflow after cleanup
```
