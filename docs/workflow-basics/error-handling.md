# Error Handling

Prodigy provides multi-level error handling to ensure workflows are resilient and recoverable. Configure error policies at workflow and command levels, use circuit breakers to prevent cascading failures, and leverage sophisticated retry strategies.

## Overview

Error handling in Prodigy includes:
- **Workflow-level policies** - Global error behavior for entire workflow
- **Command-level handlers** - Per-command failure and success handlers
- **Circuit breakers** - Prevent cascading failures
- **Retry strategies** - Automatic retry with backoff algorithms
- **Dead Letter Queue** - Failed item management and recovery

## Workflow-Level Policies

Configure global error handling for the entire workflow:

```yaml
name: my-workflow
error_policy:
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.2  # 20% failure rate
  error_collection: first  # first|last|all
```

### Policy Options

- `continue_on_failure` - Continue after errors (default: false)
- `max_failures` - Stop workflow after N failures
- `failure_threshold` - Stop if failure rate exceeds threshold (0.0-1.0)
- `error_collection` - How to aggregate errors (first, last, all)

## Command-Level Handlers

Handle errors at the command level with `on_failure` and `on_success`:

```yaml
- shell: "cargo test"
  on_failure:
    - claude: "/fix-test-failures"
    - shell: "cargo test"  # Retry after fix
  on_success:
    - shell: "echo 'All tests passed!'"
```

### Handler Options

- `on_failure` - Commands to execute on failure
- `on_success` - Commands to execute on success
- `commit_required` - Expect git commit from command
- `continue_on_error` - Don't fail workflow on error

## Circuit Breakers

Prevent cascading failures with circuit breaker patterns:

```yaml
error_policy:
  circuit_breaker:
    failure_threshold: 5      # Open after 5 failures
    success_threshold: 3      # Close after 3 successes
    timeout: 60               # Seconds before retry
    half_open_requests: 1     # Test requests in recovery
```

Circuit breaker states:
- **Closed** - Normal operation, tracking failures
- **Open** - Fast-fail mode, rejecting requests
- **Half-Open** - Testing recovery with limited requests

## Retry Strategies

Configure automatic retries with sophisticated backoff algorithms:

```yaml
- shell: "flaky-api-call.sh"
  retry:
    max_attempts: 3
    backoff: exponential
    jitter: true
```

### Backoff Strategies

- `fixed` - Constant delay between attempts
- `linear` - Incremental increase
- `exponential` - Exponential backoff (recommended)
- `fibonacci` - Fibonacci sequence delays

### Retry Configuration

```yaml
retry:
  max_attempts: 5
  backoff: exponential
  initial_delay: 1000  # milliseconds
  max_delay: 60000     # milliseconds
  jitter: true         # Randomize to prevent thundering herd
```

## Dead Letter Queue (DLQ)

Failed MapReduce work items are automatically routed to the DLQ for inspection and retry:

```yaml
map:
  item_failure_action: dlq  # dlq|retry|skip|stop
```

### DLQ Management

View failed items:

```bash
prodigy dlq show <job_id>
```

Retry failed items:

```bash
prodigy dlq retry <job_id> --max-parallel 10
```

Dry run to inspect:

```bash
prodigy dlq retry <job_id> --dry-run
```

## Error Context and Debugging

All errors include context for debugging:

- Error message and stack trace
- Command that failed
- Work item being processed
- Environment variables
- Correlation IDs for event tracking

Access detailed logs:

```bash
prodigy events <job_id>
```

## Item Failure Actions

Configure what happens when a work item fails in MapReduce:

- `dlq` - Send to Dead Letter Queue for later retry
- `retry` - Retry immediately with backoff
- `skip` - Continue to next item
- `stop` - Halt entire workflow
- `custom` - User-defined handler

## See Also

- [Dead Letter Queue](../mapreduce/dlq.md) - DLQ details and retry commands
- [Conditional Execution](conditional-execution.md) - Conditional failure handlers
- [Observability and Logging](../advanced/observability.md) - Debugging failed workflows
