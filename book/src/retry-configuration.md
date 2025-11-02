# Retry Configuration

Prodigy provides sophisticated retry mechanisms with multiple backoff strategies to handle transient failures gracefully. The retry system supports both command-level and workflow-level configurations with fine-grained control over retry behavior.

## Overview

Prodigy has two retry systems that work together:

1. **Enhanced Retry System** - Rich, configurable retry with multiple backoff strategies, jitter, circuit breakers, and conditional retry (from `src/cook/retry_v2.rs`)
2. **Workflow-Level Retry** - Simpler retry configuration for workflow-level error policies (from `src/cook/workflow/error_policy.rs`)

This chapter focuses on the enhanced retry system which provides comprehensive retry capabilities.

## RetryConfig Structure

The `RetryConfig` struct controls retry behavior with the following fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `attempts` | `u32` | `3` | Maximum number of retry attempts |
| `backoff` | `BackoffStrategy` | `Exponential` | Strategy for calculating delays between retries |
| `initial_delay` | `Duration` | `1s` | Initial delay before first retry |
| `max_delay` | `Duration` | `30s` | Maximum delay between any two retries |
| `jitter` | `bool` | `false` | Whether to add randomness to delays |
| `jitter_factor` | `f64` | `0.3` | Amount of jitter (0.0 to 1.0) |
| `retry_on` | `Vec<ErrorMatcher>` | `[]` | Retry only on specific error types (empty = retry all) |
| `retry_budget` | `Option<Duration>` | `None` | Maximum total time for all retry attempts |
| `on_failure` | `FailureAction` | `Stop` | Action to take after all retries exhausted |

### Duration Format

All duration fields use the `humantime` format. Examples:
- `1s` - 1 second
- `30s` - 30 seconds
- `5m` - 5 minutes
- `100ms` - 100 milliseconds

## Basic Retry Configuration

The simplest retry configuration uses default values:

```yaml
retry:
  attempts: 3
```

This will:
- Retry up to 3 times
- Use exponential backoff (base 2.0)
- Start with 1 second delay
- Cap delays at 30 seconds

## Backoff Strategies

Prodigy supports five backoff strategies for controlling delay between retries:

### Fixed Backoff

Same delay for every retry attempt:

```yaml
retry:
  attempts: 5
  backoff: fixed
  initial_delay: 2s
```

**Delay sequence**: 2s, 2s, 2s, 2s, 2s

**Use case**: Simple retry logic when you want consistent delays.

### Linear Backoff

Delay increases by a fixed amount each retry:

```yaml
retry:
  attempts: 4
  backoff:
    linear:
      increment: 2s
  initial_delay: 1s
```

**Delay sequence**: 1s, 3s, 5s, 7s

The `increment` field specifies the additional delay added on each retry attempt. In this example, the delay starts at 1s and increases by 2s on each retry.

**Use case**: Gradual backoff when you expect quick recovery.

### Exponential Backoff

Delay doubles (or increases by `base` factor) each retry:

```yaml
retry:
  attempts: 5
  backoff:
    exponential:
      base: 2.0
  initial_delay: 1s
  max_delay: 30s
```

**Delay sequence**: 1s, 2s, 4s, 8s, 16s

**Use case**: Most common strategy - backs off quickly to avoid overwhelming failing services. Default strategy.

### Fibonacci Backoff

Delay follows Fibonacci sequence:

```yaml
retry:
  attempts: 6
  backoff: fibonacci
  initial_delay: 1s
```

**Delay sequence**: 1s, 1s, 2s, 3s, 5s, 8s

**Use case**: Smoother increase than exponential, good for distributed systems.

### Custom Backoff

Specify exact delays for each retry:

```yaml
retry:
  attempts: 4
  backoff:
    custom:
      delays:
        - secs: 1
          nanos: 0
        - secs: 2
          nanos: 0
        - secs: 5
          nanos: 0
        - secs: 10
          nanos: 0
```

**Delay sequence**: Exactly as specified

**Use case**: When you need precise control over timing (e.g., aligning with rate limit windows).

**Note**: Custom backoff delays require explicit Duration format with `secs` and `nanos` fields. The `delays` field does not use humantime format unlike other duration fields in RetryConfig. Once the custom delays array is exhausted, any remaining retry attempts will use the `max_delay` value.

## Backoff Strategy Comparison

| Strategy | Attempt 1 | Attempt 2 | Attempt 3 | Attempt 4 | Attempt 5 | Best For |
|----------|-----------|-----------|-----------|-----------|-----------|----------|
| Fixed (2s) | 2s | 2s | 2s | 2s | 2s | Simple retry |
| Linear (+2s) | 1s | 3s | 5s | 7s | 9s | Gradual backoff |
| Exponential (base 2.0) | 1s | 2s | 4s | 8s | 16s | Most failures |
| Fibonacci | 1s | 1s | 2s | 3s | 5s | Distributed systems |

## Jitter for Distributed Systems

Jitter adds randomness to retry delays to prevent the "thundering herd" problem where many clients retry at the same time.

```yaml
retry:
  attempts: 5
  backoff:
    exponential:
      base: 2.0
  initial_delay: 10s
  jitter: true
  jitter_factor: 0.5
```

With `jitter_factor: 0.5`:
- A 10s delay becomes a random delay between **7.5s and 12.5s**
- A 20s delay becomes a random delay between **15s and 25s**

The jitter is applied as: `delay + random(-delay * factor / 2, +delay * factor / 2)`

The implementation uses Rust's `random_range` with inclusive bounds on both ends. For example, with a 10s delay and factor 0.5: `10s + random(-2.5s, +2.5s)` = 7.5s to 12.5s

**When to use jitter**:
- Multiple clients accessing the same service
- Distributed systems with many workers
- Rate-limited APIs
- Preventing synchronized retry storms

## Conditional Retry with Error Matchers

By default, Prodigy retries all errors. Use `retry_on` to retry only specific error types:

**Note**: All error matching is case-insensitive. Error messages are normalized to lowercase before pattern comparison.

### Network Errors

```yaml
retry:
  attempts: 3
  retry_on:
    - network
```

Matches errors containing: `network`, `connection`, `refused`, `unreachable`

### Timeout Errors

```yaml
retry:
  attempts: 5
  retry_on:
    - timeout
  backoff:
    exponential:
      base: 2.0
  initial_delay: 2s
```

Matches errors containing: `timeout`, `timed out`

### Server Errors (HTTP 5xx)

```yaml
retry:
  attempts: 3
  retry_on:
    - server_error
```

Matches errors containing: `500`, `502`, `503`, `504`, `server error`

### Rate Limit Errors

```yaml
retry:
  attempts: 5
  retry_on:
    - rate_limit
  initial_delay: 60s
```

Matches errors containing: `rate limit`, `429`, `too many requests`

### Custom Error Patterns

Use regex patterns for custom error matching:

```yaml
retry:
  attempts: 3
  retry_on:
    - pattern: "database connection|pool exhausted"
    - pattern: "ECONNRESET"
```

**Note**: Pattern matchers use Rust regex syntax. If the pattern is not valid regex, it will silently fail to match any errors (returns false on regex compilation error). Always test your patterns to ensure they're valid.

### Multiple Error Types

Retry on any matching error type:

```yaml
retry:
  attempts: 5
  retry_on:
    - network
    - timeout
    - rate_limit
  backoff: fibonacci
  initial_delay: 1s
```

## Retry Budget

A retry budget limits the total time spent on retries to prevent indefinite retry loops:

```yaml
retry:
  attempts: 10
  retry_budget: 5m
  backoff:
    exponential:
      base: 2.0
  initial_delay: 1s
```

In this example:
- Allows up to 10 retry attempts
- **BUT** stops retrying if total time exceeds 5 minutes
- Useful for preventing workflows from hanging indefinitely

**Without retry budget**: Exponential backoff with 10 attempts could take hours
**With retry budget**: Guarantees workflow fails within 5 minutes

## Failure Actions

Configure what happens after all retries are exhausted:

### Stop Workflow (Default)

```yaml
retry:
  attempts: 3
  on_failure: stop
```

Stops the entire workflow execution on final failure.

### Continue with Next Step

```yaml
retry:
  attempts: 3
  on_failure: continue
```

Logs the failure but continues workflow execution.

### Fallback Command

```yaml
retry:
  attempts: 3
  on_failure:
    fallback:
      command: "echo 'Primary command failed, using fallback'"
```

Executes a fallback command if primary command fails after all retries.

## Complete Examples

### Retry Network Requests

```yaml
retry:
  attempts: 5
  backoff:
    exponential:
      base: 2.0
  initial_delay: 1s
  max_delay: 30s
  jitter: true
  jitter_factor: 0.3
  retry_on:
    - network
    - timeout
    - server_error
  retry_budget: 2m
```

**Behavior**:
- Retries up to 5 times on network/timeout/server errors
- Exponential backoff: 1s, 2s, 4s, 8s, 16s (with jitter)
- Total retry time limited to 2 minutes
- Other errors (like syntax errors) fail immediately

### Retry Rate-Limited APIs

```yaml
retry:
  attempts: 3
  backoff: fibonacci
  initial_delay: 60s
  max_delay: 300s
  retry_on:
    - rate_limit
  retry_budget: 15m
```

**Behavior**:
- Waits 60s, 60s, 120s between retries (Fibonacci sequence)
- Only retries on rate limit errors (429, "rate limit exceeded")
- Gives up after 15 minutes total

### Retry Flaky Tests

```yaml
retry:
  attempts: 3
  backoff: fixed
  initial_delay: 500ms
  on_failure: continue
```

**Behavior**:
- Quick retries with 500ms between attempts
- Fixed delay (doesn't increase)
- Continues workflow even if test fails 3 times

### Retry with Circuit Breaker

The `RetryExecutor` supports circuit breakers to prevent cascading failures. This example shows the **programmatic Rust API** for advanced use cases. For workflow-level circuit breaker configuration using YAML, see the [Error Handling](./error-handling.md) chapter which covers `error_policy` circuit breaker settings.

```rust
use prodigy::cook::retry_v2::{RetryConfig, RetryExecutor};
use std::time::Duration;

let config = RetryConfig {
    attempts: 5,
    ..Default::default()
};

let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        3,                             // Open after 3 consecutive failures
        Duration::from_secs(30)        // Stay open for 30 seconds
    );

let result = executor
    .execute_with_retry(|| async {
        // Your operation here
        Ok("success")
    }, "my-operation")
    .await?;
```

**Circuit breaker states**:
- **Closed**: Normal operation, requests pass through
- **Open**: Too many failures, immediately reject requests
- **Half-Open**: After timeout, test with limited requests

## Workflow-Level vs Command-Level Retry

Prodigy has two retry systems that serve different purposes:

### Field Name Differences

The two retry systems use different field names and structures:

| Feature | retry_v2::RetryConfig | error_policy::RetryConfig |
|---------|----------------------|---------------------------|
| Max attempts | `attempts` | `max_attempts` |
| Backoff | `backoff` (enum variants) | `backoff` (enum with nested fields) |
| Exponential base | `exponential: { base }` | `exponential: { initial, multiplier }` |
| Linear increment | `linear: { increment }` | `linear: { initial, increment }` |
| Fixed delay | `backoff: fixed` | `fixed: { delay }` |

### Command-Level Retry (Enhanced)

Configured per command using `RetryConfig`:

```yaml
commands:
  - shell: "curl https://api.example.com/data"
    retry:
      attempts: 5
      backoff: exponential
      retry_on: [network, timeout]
```

**Features**:
- Rich backoff strategies (exponential, fibonacci, custom)
- Jitter support
- Conditional retry (error matchers)
- Retry budget
- Circuit breaker integration
- Detailed retry metrics

### Workflow-Level Retry (Error Policy)

Configured in workflow error policy from `error_policy.rs`:

```yaml
error_policy:
  on_item_failure: retry
  retry_config:
    max_attempts: 3
    backoff:
      exponential:
        initial: 1s
        multiplier: 2.0
```

**Features**:
- Simpler configuration
- Workflow-level failure handling
- Integrates with DLQ (Dead Letter Queue)
- Circuit breaker support
- Error metrics and pattern detection

**When to use each**:
- **Command-level**: For specific commands that need sophisticated retry logic
- **Workflow-level**: For consistent retry behavior across all workflow items in MapReduce jobs

### Command Metadata Override

Individual commands can override workflow-level retry settings:

```yaml
# Workflow default
retry:
  attempts: 3

# Command override
commands:
  - claude: "/analyze ${item}"
    metadata:
      retries: 5  # This command gets 5 attempts instead of 3
```

From `src/config/command.rs:135`.

## Retry Metrics and Observability

The retry system tracks metrics for monitoring:

```rust
let executor = RetryExecutor::new(config);
// ... execute operations ...
let metrics = executor.metrics().await;

println!("Total attempts: {}", metrics.total_attempts);
println!("Successful: {}", metrics.successful_attempts);
println!("Failed: {}", metrics.failed_attempts);
println!("Retry history: {:?}", metrics.retries);
```

Metrics include:
- `total_attempts` - Total number of attempts made
- `successful_attempts` - Number of successful operations
- `failed_attempts` - Number of failed operations
- `retries` - Vec of (attempt_number, delay) pairs

## Best Practices

### Choosing a Backoff Strategy

1. **Use Exponential for most cases** - Default strategy, works well for most failures
2. **Use Fibonacci for distributed systems** - Smoother curve, avoids overwhelming services
3. **Use Linear for quick recovery scenarios** - When failures are brief and predictable
4. **Use Fixed only for very specific cases** - When you know exact timing requirements
5. **Use Custom for rate-limited APIs** - Align retries with API rate limit windows

### Setting Appropriate Timeouts

- **initial_delay**: Start small (1-2s) for transient errors, larger (30-60s) for rate limits
- **max_delay**: Cap at reasonable time (30s for interactive, 5m for background jobs)
- **retry_budget**: Always set this to prevent infinite retry loops
  - Interactive operations: 1-5 minutes
  - Background jobs: 15-30 minutes
  - Critical operations: 1 hour

### Using Jitter

Enable jitter when:
- ✅ Multiple clients/workers accessing the same service
- ✅ Distributed systems with parallel execution
- ✅ Rate-limited APIs
- ❌ Single client applications
- ❌ When exact timing is critical

### Conditional Retry

Always use `retry_on` to avoid retrying permanent failures:

```yaml
# ❌ BAD: Retries syntax errors forever
retry:
  attempts: 10
  backoff: exponential

# ✅ GOOD: Only retries transient failures
retry:
  attempts: 10
  retry_on: [network, timeout, server_error]
  backoff: exponential
```

### Retry Budget Guidelines

Set retry budgets based on operation criticality:

| Operation Type | Suggested Budget | Reasoning |
|---------------|------------------|-----------|
| User-facing API calls | 5-10 seconds | User is waiting |
| Background sync | 5-15 minutes | Can afford patience |
| Critical data operations | 30-60 minutes | Must complete |
| Non-critical tasks | 1-5 minutes | Fail fast |

## Troubleshooting

### Retries Taking Too Long

**Problem**: Workflow hangs during retries

**Solutions**:
1. Add a `retry_budget` to cap total retry time
2. Reduce `max_delay` to speed up retry cycles
3. Reduce `attempts` to fail faster
4. Use `linear` or `fibonacci` instead of `exponential`

### Not Retrying When Expected

**Problem**: Operation fails without retrying

**Causes**:
1. Error doesn't match `retry_on` patterns
2. Already at max `attempts`
3. `retry_budget` exhausted
4. Circuit breaker is open

**Debug**:
- Remove `retry_on` to retry all errors
- Check error message matches patterns (case-insensitive)
- Increase `retry_budget` or `attempts`
- Check circuit breaker status

### Thundering Herd Problem

**Problem**: Many workers retry at the same time, overwhelming service

**Solution**:
```yaml
retry:
  attempts: 5
  jitter: true
  jitter_factor: 0.5  # 50% jitter range
  backoff: fibonacci   # Smoother than exponential
```

### Rate Limit Issues

**Problem**: Keep hitting rate limits despite retries

**Solution**:
```yaml
retry:
  attempts: 3
  retry_on:
    - rate_limit
  backoff:
    custom:
      delays:
        - secs: 60
          nanos: 0
        - secs: 120
          nanos: 0
        - secs: 300
          nanos: 0
  retry_budget: 15m
```

## Related Topics

- [Error Handling](./error-handling.md) - Overall error handling strategy
- [Workflow Configuration](./workflow-configuration.md) - Workflow-level settings
- [MapReduce](./mapreduce.md) - Retry in MapReduce workflows
- [Dead Letter Queue](./dead-letter-queue.md) - Handling failed items

## Implementation References

- Enhanced retry system: `src/cook/retry_v2.rs:14-461`
- Workflow error policy: `src/cook/workflow/error_policy.rs:91-129`
- Command metadata: `src/config/command.rs:135`
- Circuit breaker: `src/cook/retry_v2.rs:325-397`
- Error matchers: `src/cook/retry_v2.rs:100-151`
- Retry metrics: `src/cook/retry_v2.rs:399-422`
