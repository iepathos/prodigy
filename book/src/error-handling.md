# Error Handling

## Workflow-Level Error Policy

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

---

## Command-Level Error Handling

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

---

## Dead Letter Queue (DLQ)

Failed items in MapReduce workflows are sent to DLQ for retry:

```bash
# Retry failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 5

# Dry run
prodigy dlq retry <job_id> --dry-run
```

**DLQ Features:**
- Stores failed work items with failure reason and timestamp
- Supports automatic reprocessing via `prodigy dlq retry`
- Configurable parallel execution and resource limits
- Shared across worktrees for centralized failure tracking
- Streams items to avoid memory issues with large queues
- Respects original workflow's max_parallel setting
- Preserves correlation IDs for tracking
- Supports interruption and resumption
