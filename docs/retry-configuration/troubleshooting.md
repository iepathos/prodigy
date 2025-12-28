# Troubleshooting

Common retry configuration issues and their solutions.

!!! tip "Quick Reference"
    | Issue | Common Cause | Quick Fix |
    |-------|--------------|-----------|
    | Retries not triggering | `retry_on` matchers too specific | Add broader patterns or check error message |
    | Retries too slow | No `max_delay` cap | Set `max_delay: "30s"` |
    | Circuit breaker stuck open | Threshold too low | Increase failure threshold |
    | Thundering herd | No jitter | Enable `jitter: true` |
    | Retrying permanent errors | Empty `retry_on` | Add specific error matchers |

---

## Retries Not Triggering

!!! danger "Symptoms"
    - Command fails immediately without retry
    - Only see one attempt in logs
    - Expected retries don't happen

### retry_on Matchers Too Specific

```yaml title="Problem: Error doesn't match configured matchers"
retry_config:
  attempts: 5
  retry_on:
    - network  # (1)!
```

1. Error is "connection timeout" but matcher only catches "network"

!!! success "Solution"
    Use broader matchers or add custom patterns:

    ```yaml title="Solution: Add timeout matcher"
    retry_config:
      attempts: 5
      retry_on:
        - network
        - timeout      # Add timeout matcher
        - pattern: "connection.*timeout"  # Or custom pattern
    ```

!!! tip "Debug Tip"
    Check actual error message in logs and verify it matches one of your configured matchers. Remember: matching is case-insensitive.

**Source**: ErrorMatcher matching logic in `src/cook/retry_v2.rs:128-149`

### Empty retry_on with Unexpected Error Filtering

```yaml title="Misconception: Empty retry_on behavior"
retry_config:
  attempts: 3
  # retry_on is empty - retries everything!  # (1)!
```

1. **Reality**: Empty `retry_on` means "retry ALL errors", not "don't retry"

!!! success "Solution"
    If you see retries happening when you don't expect them, check if `retry_on` is empty. Add specific matchers to control which errors are retried.

**Source**: Default behavior in `src/cook/retry_v2.rs:42-43`

### retry_config Missing Entirely

```yaml title="Problem: No retry_config block"
commands:
  - shell: "curl https://api.example.com"
    # No retry_config - no retry happens
```

!!! success "Solution"
    Add `retry_config` block to enable retry:

    ```yaml
    commands:
      - shell: "curl https://api.example.com"
        retry_config:
          attempts: 3
    ```

---

## Retries Taking Too Long

!!! danger "Symptoms"
    - Workflow hangs during retries
    - Total retry time exceeds expectations
    - Delays grow too large

### Exponential Backoff Without max_delay

```yaml title="Problem: Exponential growth uncapped"
retry_config:
  attempts: 10
  backoff: exponential
  initial_delay: "1s"
  # No max_delay - delay can grow to minutes!  # (1)!
```

1. Without a cap, delay doubles each time: 1s → 2s → 4s → 8s → 16s → 32s → 64s...

!!! success "Solution"
    Always set `max_delay`:

    ```yaml title="Solution: Cap delays at 30 seconds"
    retry_config:
      attempts: 10
      backoff: exponential
      initial_delay: "1s"
      max_delay: "30s"  # Cap delays at 30 seconds
    ```

!!! note "Default Behavior"
    `max_delay` defaults to 30 seconds if not specified.

    **Source**: `src/cook/retry_v2.rs:451` (`default_max_delay()` function)

### Too Many Attempts Without retry_budget

```yaml title="Problem: High attempts can retry for hours"
retry_config:
  attempts: 100
  backoff: exponential
```

!!! success "Solution"
    Use `retry_budget` to cap total time:

    ```yaml title="Solution: Cap total retry time"
    retry_config:
      attempts: 100
      backoff: exponential
      retry_budget: "10m"  # Never exceed 10 minutes total
    ```

**Source**: retry_budget enforcement in tests at `src/cook/retry_v2.rs:727-747`

### Initial Delay Too High

```yaml title="Problem: First retry waits too long"
retry_config:
  attempts: 3
  initial_delay: "60s"  # 1-minute delay before first retry!
```

!!! success "Solution"
    Use shorter initial delay, let backoff grow:

    ```yaml title="Solution: Start small, let it grow"
    retry_config:
      attempts: 3
      initial_delay: "1s"   # Start small
      backoff: exponential  # Let it grow
      max_delay: "30s"
    ```

---

## Circuit Breaker Issues

!!! danger "Symptoms"
    - Circuit breaker trips and stays open
    - Requests fail immediately after threshold
    - Recovery doesn't happen

### Failure Threshold Too Low

```rust title="Problem: Circuit opens too easily"
// Source: src/cook/retry_v2.rs:325-397
let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        1,                          // Opens after single failure!  // (1)!
        Duration::from_secs(30)
    );
```

1. A threshold of 1 means the circuit opens immediately on any failure

!!! success "Solution"
    Use appropriate threshold for failure rate:

    ```rust title="Solution: Require multiple failures"
    let executor = RetryExecutor::new(config)
        .with_circuit_breaker(
            5,                          // Open after 5 consecutive failures
            Duration::from_secs(30)
        );
    ```

### Recovery Timeout Too Long

```rust title="Problem: Circuit stays open too long"
let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        5,
        Duration::from_secs(600)    // 10-minute recovery time!
    );
```

!!! success "Solution"
    Use shorter recovery timeout for faster testing:

    ```rust title="Solution: 30-second recovery attempts"
    let executor = RetryExecutor::new(config)
        .with_circuit_breaker(
            5,
            Duration::from_secs(30)     // 30-second recovery attempts
        );
    ```

### No Success to Close Circuit

!!! warning "Problem"
    Circuit opens but downstream never recovers, so circuit never closes.

!!! success "Solution"
    - Check if downstream service is actually recovering
    - Verify circuit enters HalfOpen state and test requests succeed
    - Monitor circuit state transitions with logging

---

## Thundering Herd

!!! danger "Symptoms"
    - Service overwhelmed during recovery
    - All parallel agents retry at same time
    - Cascading failures

```yaml title="Problem: No jitter in parallel execution"
# MapReduce with 10 parallel agents, no jitter
map:
  max_parallel: 10
  agent_template:
    - shell: "api-call.sh"
      retry_config:
        attempts: 5
        backoff: exponential
        jitter: false  # All agents retry at same time!  # (1)!
```

1. Without jitter, all 10 agents calculate the same exponential delay and retry simultaneously

!!! success "Solution"
    Enable jitter to randomize retry timing:

    ```yaml title="Solution: Enable jitter"
    map:
      max_parallel: 10
      agent_template:
        - shell: "api-call.sh"
          retry_config:
            attempts: 5
            backoff: exponential
            jitter: true          # Randomize retry timing
            jitter_factor: 0.3    # 30% randomization
    ```

**Source**: Jitter application in `src/cook/retry_v2.rs:308-317`

!!! tip "See Also"
    For detailed jitter configuration, see [Jitter for Distributed Systems](jitter-for-distributed-systems.md).

---

## Retrying Non-Transient Errors

!!! danger "Symptoms"
    - Retrying 404 Not Found errors
    - Retrying authentication failures (401)
    - Wasting time on permanent failures

```yaml title="Problem: Empty retry_on retries everything"
retry_config:
  attempts: 5
  # Retries 404, 401, 400, etc. - permanent errors!
```

!!! success "Solution"
    Use selective retry with error matchers:

    ```yaml title="Solution: Only retry transient errors"
    retry_config:
      attempts: 5
      retry_on:
        - network        # Transient
        - timeout        # Transient
        - server_error   # Transient (5xx)
        # Don't retry 404, 401, 400, etc.
    ```

!!! tip "Best Practice"
    Only retry errors that might succeed on next attempt.

!!! tip "See Also"
    For detailed error matcher configuration, see [Conditional Retry with Error Matchers](conditional-retry-with-error-matchers.md).

---

## Retry Budget Issues

!!! danger "Symptoms"
    - Retries exceed configured budget
    - Budget seems ignored

!!! info "How retry_budget Works"
    - Budget is checked **before each retry**
    - If budget would be exceeded, retry stops
    - Budget includes backoff delay time
    - Budget does **NOT** include time for command execution itself

```yaml title="Example: Retry budget behavior"
retry_config:
  attempts: 10
  backoff: exponential
  initial_delay: "1s"
  max_delay: "60s"
  retry_budget: "2m"  # 2-minute budget  # (1)!
```

1. Budget only counts delay time, not command execution time

!!! warning "Important"
    **Behavior**:

    - If accumulated delays + next delay > 2 minutes → Stop
    - Command execution time is NOT counted in budget
    - If command takes 1 minute to execute each time, total time could be: 2m (budget) + (attempts × 1m execution) = ~12 minutes

**Source**: `retry_budget` field in `src/cook/retry_v2.rs:46-47`, tests at lines 727-747

!!! tip "See Also"
    For detailed retry budget configuration, see [Retry Budget](retry-budget.md).

---

## Fallback Command Failures

!!! danger "Symptoms"
    - Primary command fails after retries
    - Fallback command executes but also fails
    - Workflow stops

```yaml title="Problem: Fallback isn't reliable"
retry_config:
  attempts: 3
  on_failure:
    fallback:
      command: "curl https://backup-api.com/data"  # This can also fail!
```

!!! success "Solution"
    Make fallback truly reliable:

    ```yaml title="Solution: Use local cache"
    retry_config:
      attempts: 3
      on_failure:
        fallback:
          command: "cat /cache/data.json"  # Local cache, very reliable
    ```

!!! tip "Best Practice"
    Fallback commands should be:

    - Local operations (file reads, not network calls)
    - Idempotent
    - Very unlikely to fail
    - Fast

!!! tip "See Also"
    For detailed failure action configuration, see [Failure Actions](failure-actions.md).

---

## Retry Metrics Mismatch

!!! danger "Symptoms"
    - Metrics show different attempt count than configured
    - Unexpected success/failure counts

??? example "Debugging with RetryMetrics"
    ```rust title="Access retry metrics"
    // Source: src/cook/retry_v2.rs:399-422
    let metrics = executor.metrics();
    println!("Total attempts: {}", metrics.total_attempts);
    println!("Successful: {}", metrics.successful_attempts);
    println!("Failed: {}", metrics.failed_attempts);
    println!("Retries: {:?}", metrics.retries);  // Vec<(attempt, delay)>
    ```

    **Check**:

    - `total_attempts` = successful + failed
    - `retries` vector shows actual delays used
    - Compare with configured backoff strategy

!!! tip "See Also"
    For detailed metrics and observability, see [Retry Metrics and Observability](retry-metrics-and-observability.md).

---

## Debugging Workflow

When retry behavior is unexpected, follow this checklist:

??? note "1. Check retry_on matchers"
    - Verify error message matches configured matchers
    - Remember matching is case-insensitive
    - Empty `retry_on` = retry all errors

??? note "2. Check backoff configuration"
    - Verify `max_delay` is set
    - Check `initial_delay` isn't too high
    - Ensure `backoff` strategy matches intent

??? note "3. Check retry_budget"
    - Remember budget is delay time, not total time
    - Budget checked before each retry
    - Command execution time NOT included

??? note "4. Enable jitter for parallel workflows"
    - Always use jitter in MapReduce
    - Set `jitter_factor` between 0.1 and 0.5

??? note "5. Use selective retry"
    - Don't retry permanent errors (404, 401, 400)
    - Use `retry_on` to specify transient errors only

??? note "6. Monitor metrics"
    - Access `RetryMetrics` for actual attempt counts
    - Verify delays match expectations
    - Check circuit breaker state transitions

---

## Getting Help

If retry behavior is still unclear:

1. **Check retry_v2.rs implementation**: `src/cook/retry_v2.rs`
2. **Review tests**: `src/cook/retry_v2.rs:463-749` (comprehensive test coverage)
3. **Enable verbose logging**: Add logging around retry logic to see what's happening
4. **Test with simple cases**: Start with fixed backoff and 2-3 attempts to isolate issue

!!! tip "Related Documentation"
    - [Backoff Strategies](backoff-strategies.md) - Detailed backoff configuration
    - [Conditional Retry with Error Matchers](conditional-retry-with-error-matchers.md) - Error matching patterns
    - [Jitter for Distributed Systems](jitter-for-distributed-systems.md) - Preventing thundering herd
    - [Retry Budget](retry-budget.md) - Budget configuration
    - [Failure Actions](failure-actions.md) - Fallback and failure handling
    - [Retry Metrics and Observability](retry-metrics-and-observability.md) - Monitoring retries
