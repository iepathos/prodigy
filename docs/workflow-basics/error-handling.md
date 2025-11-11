# Error Handling

Prodigy provides multi-level error handling with circuit breakers, retry strategies, and Dead Letter Queue for robust workflow execution.

## Overview

Error handling in Prodigy operates at three levels:
- **Workflow-level policies**: Global error behavior for entire workflow
- **Command-level handlers**: Per-command error handling with `on_failure`
- **Item-level actions**: MapReduce work item failure strategies

## Workflow-Level Policies

Define global error behavior in the `error_policy` block:

```yaml
error_policy:
  continue_on_failure: true    # Continue after errors
  max_failures: 5              # Stop after N failures
  failure_threshold: 0.2       # Stop at 20% failure rate
  error_collection: aggregate  # How to collect errors

- shell: "cargo test"
- shell: "cargo build"
- shell: "cargo clippy"
```

### Policy Options

- **continue_on_failure** (boolean): Whether to continue executing steps after a failure
- **max_failures** (number): Maximum number of failures before stopping workflow
- **failure_threshold** (0.0-1.0): Maximum failure rate (e.g., 0.2 = 20%)
- **error_collection** (string): How to aggregate errors
  - `aggregate` - Collect all errors and report at end
  - `immediate` - Stop and report on first error
  - `batched:N` - Report every N errors

## Command-Level Handlers

Use `on_failure` and `on_success` for per-command error handling:

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/debug-test-failure --output ${shell.stderr}"

- claude: "/implement-feature"
  commit_required: true
  on_failure:
    - claude: "/analyze-error ${shell.stderr}"
    - shell: "git reset --hard HEAD"  # Rollback on failure
```

### Handler Types

**on_failure** - Execute when command fails:
```yaml
- shell: "integration-test.sh"
  on_failure:
    claude: "/analyze-failure ${shell.stderr}"
    shell: "docker-compose logs"
```

**on_success** - Execute when command succeeds:
```yaml
- shell: "cargo build --release"
  on_success:
    shell: "strip target/release/prodigy"
    shell: "cp target/release/prodigy /usr/local/bin/"
```

## Circuit Breakers

Prevent cascading failures with circuit breaker configuration:

```yaml
error_policy:
  circuit_breaker:
    failure_threshold: 5      # Open after 5 failures
    success_threshold: 2      # Close after 2 successes
    timeout: "60s"           # Wait before attempting half-open
    half_open_requests: 3    # Test requests in half-open state
```

### Circuit States

1. **Closed** (normal): All requests processed
2. **Open**: No requests processed, fast-fail immediately
3. **Half-Open**: Limited requests to test recovery

### Configuration

- **failure_threshold**: Consecutive failures to open circuit
- **success_threshold**: Consecutive successes to close circuit from half-open
- **timeout**: Duration to wait before entering half-open (humantime format: "1s", "1m", "5m")
- **half_open_requests**: Number of test requests in half-open state

### Example

```yaml
error_policy:
  circuit_breaker:
    failure_threshold: 3
    success_threshold: 2
    timeout: "30s"
    half_open_requests: 1

- shell: "curl https://api.example.com/health"
  # After 3 failures, circuit opens
  # After 30s, allows 1 test request
  # After 2 successes, circuit closes
```

## Retry Strategies

Configure sophisticated retry behavior with backoff:

```yaml
error_policy:
  retry_config:
    max_attempts: 3
    backoff:
      exponential:
        initial: "1s"
        multiplier: 2.0
```

### Backoff Strategies

**Fixed** - Constant delay between retries:
```yaml
backoff:
  fixed: "5s"
```

**Linear** - Incrementing delay:
```yaml
backoff:
  linear:
    initial: "1s"
    increment: "2s"
```

**Exponential** - Exponential backoff:
```yaml
backoff:
  exponential:
    initial: "1s"
    multiplier: 2.0
    max_delay: "60s"
```

**Fibonacci** - Fibonacci sequence delays:
```yaml
backoff:
  fibonacci:
    initial: "1s"
    max_delay: "120s"
```

### Jitter

Add randomization to prevent thundering herd:
```yaml
retry_config:
  max_attempts: 5
  jitter: true
  backoff:
    exponential:
      initial: "1s"
      multiplier: 2.0
```

## Dead Letter Queue (DLQ)

Failed MapReduce work items are automatically routed to the DLQ for retry:

```yaml
mode: mapreduce

error_policy:
  on_item_failure: dlq  # Route failed items to DLQ

map:
  input: "items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/process ${item}"
```

### DLQ Features

- **Automatic routing**: Failed items stored with error details
- **Failure history**: Track retry attempts and error messages
- **JSON log location**: Link to Claude execution logs for debugging
- **Retry command**: `prodigy dlq retry <job_id>`

### Retry Failed Items

```bash
# Retry all failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to inspect
prodigy dlq retry <job_id> --dry-run
```

### View DLQ Contents

```bash
# Show failed items
prodigy dlq show <job_id>

# Get JSON format
prodigy dlq show <job_id> --format json
```

## Item Failure Actions

Control what happens when individual work items fail:

```yaml
error_policy:
  on_item_failure: dlq     # dlq, retry, skip, stop, or custom

  # Alternative: top-level convenience field
on_item_failure: dlq
```

### Actions

- **dlq**: Send to Dead Letter Queue for later retry
- **retry**: Retry immediately with backoff
- **skip**: Skip failed item and continue
- **stop**: Stop entire workflow on first failure
- **custom**: User-defined error handler

## Error Context

Errors include rich context for debugging:

```yaml
- shell: "cargo test"
  capture_output: test_output
  on_failure:
    claude: "/debug-test --stderr ${shell.stderr} --exit-code ${shell.exit_code}"
```

Available error context:
- `${shell.stderr}` - Error output
- `${shell.exit_code}` - Exit code
- `${shell.duration}` - Execution time
- `${shell.success}` - Boolean success flag

## Examples

### Resilient API Integration

```yaml
error_policy:
  retry_config:
    max_attempts: 5
    backoff:
      exponential:
        initial: "2s"
        multiplier: 2.0
        max_delay: "60s"
    jitter: true

  circuit_breaker:
    failure_threshold: 3
    success_threshold: 2
    timeout: "30s"

- shell: "curl -f https://api.example.com/data"
  on_failure:
    shell: "echo 'API unavailable, will retry with backoff'"
```

### MapReduce with DLQ

```yaml
mode: mapreduce

error_policy:
  on_item_failure: dlq
  max_failures: 10
  failure_threshold: 0.1  # Stop at 10% failure rate

map:
  input: "items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/process ${item}"
      timeout: 300
      on_failure:
        shell: "echo 'Item ${item.id} failed, sent to DLQ'"
```

### Progressive Error Handling

```yaml
- shell: "cargo test"
  on_failure:
    - claude: "/analyze-test-failure ${shell.stderr}"
    - shell: "cargo clean"
    - shell: "cargo test"  # Retry after clean
      on_failure:
        - claude: "/deep-analysis ${shell.stderr}"
        - shell: "notify-team.sh 'Tests still failing after retry'"
```

## See Also

- [Conditional Execution](conditional-execution.md) - Using conditions with error handlers
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) - DLQ details and retry
- [MapReduce Workflows](../mapreduce/index.md) - Error handling at scale
- [Command Types](command-types.md) - Commands supporting error handlers
