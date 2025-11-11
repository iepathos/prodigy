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

```yaml title="workflow.yml"
# Source: src/cook/workflow/error_policy.rs:133-160
name: my-workflow
error_policy:
  on_item_failure: dlq        # dlq|retry|skip|stop
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.2      # 20% failure rate
  error_collection: aggregate # aggregate|immediate|batched
```

### Policy Options

- `on_item_failure` - Action for failed items: `dlq`, `retry`, `skip`, `stop`, or `custom`
- `continue_on_failure` - Continue after errors (default: false)
- `max_failures` - Stop workflow after N failures
- `failure_threshold` - Stop if failure rate exceeds threshold (0.0-1.0)
- `error_collection` - Error reporting strategy: `aggregate`, `immediate`, or `batched`

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

```yaml title="workflow.yml"
# Source: src/cook/workflow/error_policy.rs:48-87
error_policy:
  circuit_breaker:
    failure_threshold: 5      # Open after 5 failures
    success_threshold: 3      # Close after 3 successes
    timeout: 30s              # Duration before retry (default: 30s)
    half_open_requests: 3     # Test requests in recovery (default: 3)
```

### Circuit Breaker States

- **Closed** - Normal operation, tracking failures
- **Open** - Fast-fail mode, rejecting requests immediately
- **Half-Open** - Testing recovery with limited requests

!!! tip "Circuit Breaker Troubleshooting"
    If your circuit breaker is frequently opening:

    - **Check failure_threshold**: May be too low for your workload
    - **Verify timeout duration**: Ensure downstream services have time to recover
    - **Review half_open_requests**: Increase if recovery tests are too sensitive
    - **Monitor circuit state**: Use `prodigy events <job_id>` to track state transitions

    Common issues:

    - Circuit stuck in open state: Increase `timeout` or decrease `success_threshold`
    - Premature opening: Increase `failure_threshold`
    - Slow recovery: Increase `half_open_requests` to test recovery faster

## Retry Strategies

Configure automatic retries with sophisticated backoff algorithms:

```yaml title="workflow.yml"
# Source: src/cook/workflow/error_policy.rs:90-128
- shell: "flaky-api-call.sh"
  retry:
    max_attempts: 3
    backoff: exponential
```

### Backoff Strategies

Four backoff strategies are available, each with different performance characteristics:

=== "Exponential (Recommended)"
    ```yaml
    # Source: src/cook/workflow/error_policy.rs:116-117
    retry:
      max_attempts: 5
      backoff:
        exponential:
          initial: 1s
          multiplier: 2.0  # Doubles each attempt
    ```
    **Performance**: Fast initial retry, rapidly increasing delays. Best for transient failures.

    **Delays**: 1s → 2s → 4s → 8s → 16s

=== "Linear"
    ```yaml
    # Source: src/cook/workflow/error_policy.rs:111-114
    retry:
      max_attempts: 5
      backoff:
        linear:
          initial: 1s
          increment: 2s  # Adds 2s each attempt
    ```
    **Performance**: Predictable, moderate increase. Good for rate-limited APIs.

    **Delays**: 1s → 3s → 5s → 7s → 9s

=== "Fibonacci"
    ```yaml
    # Source: src/cook/workflow/error_policy.rs:118-119
    retry:
      max_attempts: 5
      backoff:
        fibonacci:
          initial: 1s
    ```
    **Performance**: Balanced growth rate between linear and exponential.

    **Delays**: 1s → 1s → 2s → 3s → 5s

=== "Fixed"
    ```yaml
    # Source: src/cook/workflow/error_policy.rs:109-110
    retry:
      max_attempts: 5
      backoff:
        fixed:
          delay: 5s
    ```
    **Performance**: Constant delay. Use when retry timing doesn't matter.

    **Delays**: 5s → 5s → 5s → 5s → 5s

!!! note "Performance Impact"
    - **Exponential**: Fastest recovery for transient failures, but can delay retry significantly after few attempts
    - **Linear**: Predictable total retry time, moderate resource usage
    - **Fibonacci**: Good balance of retry speed and backoff
    - **Fixed**: Simplest but may not respect downstream service recovery time

### Full Retry Configuration

```yaml title="workflow.yml"
retry:
  max_attempts: 5
  backoff: exponential
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

## Combined Error Handling Example

Combine multiple error handling levels for robust, production-ready workflows:

```yaml title="workflow.yml"
# Source: workflows/mkdocs-drift.yml:79-84
name: production-workflow
mode: mapreduce

# Workflow-level policy: Global error behavior
error_policy:
  on_item_failure: dlq          # Failed items go to DLQ
  continue_on_failure: true     # Keep processing other items
  max_failures: 10              # Stop if 10 items fail
  failure_threshold: 0.15       # Stop if 15% failure rate
  error_collection: aggregate   # Collect all errors

  # Circuit breaker: Prevent cascading failures
  circuit_breaker:
    failure_threshold: 5
    success_threshold: 3
    timeout: 30s
    half_open_requests: 3

map:
  input: "work-items.json"
  agent_template:
    # Command-level handler: Per-command error handling
    - shell: "process-item.sh ${item.id}"
      retry:
        max_attempts: 3
        backoff:
          exponential:
            initial: 1s
            multiplier: 2.0
      on_failure:
        - claude: "/diagnose-failure ${item.id}"
        - shell: "log-failure.sh ${item.id}"
      on_success:
        - shell: "notify-success.sh ${item.id}"

reduce:
  - claude: "/summarize-results"
    continue_on_error: false  # Reduce must succeed
```

This example demonstrates:

1. **Workflow-level**: Global policy with DLQ routing and circuit breaker
2. **Command-level**: Retry with exponential backoff and failure handlers
3. **Layered protection**: Circuit breaker prevents cascading failures while retries handle transient errors
4. **Graceful degradation**: Failed items go to DLQ for later processing instead of blocking workflow

## See Also

- [Dead Letter Queue](../mapreduce/dlq.md) - DLQ details and retry commands
- [Conditional Execution](conditional-execution.md) - Conditional failure handlers
- [Observability and Logging](../advanced/observability.md) - Debugging failed workflows
