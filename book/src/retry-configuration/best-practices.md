## Best Practices

This guide covers best practices for configuring retry behavior in Prodigy workflows, based on the implementation patterns in `retry_v2`.

**Source**: Best practices derived from retry_v2.rs implementation and test patterns (src/cook/retry_v2.rs:463-748)

### Choosing the Right Backoff Strategy

Different backoff strategies suit different failure modes:

#### Exponential Backoff (Default - Best for Most Cases)

**When to use**:
- Network requests to external APIs
- Transient failures that self-heal over time
- Rate-limited services
- Database connection retries

**Configuration**:
```yaml
retry_config:
  attempts: 5
  backoff: exponential  # or: { base: 2.0 }
  initial_delay: "1s"
  max_delay: "30s"
  jitter: true
```

**Why**: Exponential backoff quickly backs off from rapid retries, reducing load on failing systems and allowing recovery time.

**Source**: `src/cook/retry_v2.rs:82-85` (Exponential variant), tests at lines 626-648

#### Linear Backoff

**When to use**:
- Predictable delays needed
- Testing and debugging (easier to reason about delays)
- Systems with known recovery time patterns

**Configuration**:
```yaml
retry_config:
  attempts: 5
  backoff:
    linear:
      increment: "5s"
  initial_delay: "1s"
```

**Delays**: 1s, 6s, 11s, 16s, 21s (initial + n * increment)

**Source**: `src/cook/retry_v2.rs:77-80` (Linear variant), tests at lines 604-625

#### Fibonacci Backoff

**When to use**:
- Gradual backoff with slower growth than exponential
- Good balance between responsiveness and system protection
- Distributed systems needing gentler backoff

**Configuration**:
```yaml
retry_config:
  attempts: 6
  backoff: fibonacci
  initial_delay: "1s"
  max_delay: "60s"
```

**Delays**: 1s, 2s, 3s, 5s, 8s, 13s

**Source**: `src/cook/retry_v2.rs:87` (Fibonacci variant), Fibonacci calculation at lines 424-440

#### Fixed Delay

**When to use**:
- Polling operations
- Systems with consistent retry requirements
- Simplicity preferred over optimization

**Configuration**:
```yaml
retry_config:
  attempts: 10
  backoff: fixed
  initial_delay: "5s"
```

**Source**: `src/cook/retry_v2.rs:75` (Fixed variant), tests at lines 583-603

### Setting Appropriate max_delay

Always set `max_delay` to prevent unbounded delays:

✅ **Good**:
```yaml
retry_config:
  backoff: exponential
  max_delay: "30s"  # Caps exponential growth
```

❌ **Bad**:
```yaml
retry_config:
  backoff: exponential
  # No max_delay - could delay minutes or hours!
```

**Default**: 30 seconds (src/cook/retry_v2.rs:64)

**Recommendation**:
- Interactive workflows: 10-30 seconds
- Background jobs: 60-300 seconds
- Critical paths: 5-15 seconds

### Using Jitter in Distributed Systems

**Always enable jitter for distributed systems** to prevent thundering herd:

```yaml
retry_config:
  attempts: 5
  backoff: exponential
  initial_delay: "1s"
  max_delay: "30s"
  jitter: true           # Enable jitter
  jitter_factor: 0.3     # 30% randomization
```

**Source**: `src/cook/retry_v2.rs:308-317` (apply_jitter method), tests at lines 650-673

**Why**: Without jitter, multiple parallel agents/processes retry at the same time, overwhelming recovering systems.

**Jitter Formula** (src/cook/retry_v2.rs:311-315):
```rust
let jitter_range = delay_ms as f64 * jitter_factor;
let random_offset = thread_rng().gen_range(-jitter_range..=jitter_range);
adjusted_delay = delay + Duration::from_millis(random_offset as u64);
```

**When to use jitter**:
- MapReduce workflows with parallel agents
- Multiple services hitting the same API
- Distributed systems with shared resources
- Any parallel retry scenario

**When to skip jitter**:
- Single-instance workflows
- Deterministic testing
- Debugging retry behavior

### Retry Budget for Preventing Infinite Loops

Use `retry_budget` to cap total retry time:

```yaml
retry_config:
  attempts: 100          # High attempt count
  backoff: exponential
  retry_budget: "5m"     # But limit total time to 5 minutes
```

**Source**: `src/cook/retry_v2.rs:46-47` (retry_budget field)

**Why**: Prevents workflows from retrying indefinitely when `attempts` is set high.

**Use Cases**:
- Long-running operations where total time matters
- SLA-constrained workflows
- Preventing resource exhaustion
- Fail-fast on persistent failures

**Implementation Note**: Retry budget is checked before each retry attempt. If budget is exhausted, retries stop immediately (verified in test at src/cook/retry_v2.rs:675-708).

### Selective Retry with Error Matchers

**Don't retry everything** - use `retry_on` for transient errors only:

✅ **Good** (selective retry):
```yaml
retry_config:
  attempts: 5
  retry_on:
    - network
    - timeout
    - server_error  # 5xx errors
```

❌ **Bad** (retry all errors):
```yaml
retry_config:
  attempts: 5
  # Empty retry_on retries everything, including 404, 401, etc.
```

**Why**: Some errors are **permanent** and retrying wastes time:
- 404 Not Found - resource doesn't exist
- 401 Unauthorized - credentials are invalid
- 400 Bad Request - request is malformed

**Transient errors worth retrying**:
- Network connectivity issues
- Timeouts
- 5xx server errors
- Rate limits (with appropriate backoff)
- Database locks (temporary)

**Source**: `src/cook/retry_v2.rs:100-151` (ErrorMatcher enum), tests at lines 463-582

### Combining Retry with Circuit Breakers

For high-reliability systems, combine retry with circuit breakers:

```rust
// Circuit breaker configuration (applied via RetryExecutor)
let executor = RetryExecutor::new(retry_config)
    .with_circuit_breaker(
        5,                          // failure_threshold
        Duration::from_secs(30)     // recovery_timeout
    );
```

**Source**: `src/cook/retry_v2.rs:184-188` (with_circuit_breaker method), `src/cook/retry_v2.rs:325-397` (CircuitBreaker implementation)

**Circuit Breaker States**:
1. **Closed**: Normal operation, requests flow through
2. **Open**: Circuit tripped after threshold failures, requests fail immediately
3. **HalfOpen**: Testing recovery, limited requests allowed

**Why**: Circuit breakers prevent cascading failures by failing fast when downstream systems are down, rather than retrying indefinitely.

**Best Practice**: Use circuit breakers for:
- External API calls
- Database connections
- Microservice communication
- Any dependency that might fail completely

### Failure Action Strategies

Choose `on_failure` based on operation criticality:

#### Critical Operations (Use: Stop)
```yaml
retry_config:
  attempts: 3
  on_failure: stop
```

**Examples**:
- Database migrations
- Deployment prerequisites
- Data integrity checks
- Security validations

#### Optional Operations (Use: Continue)
```yaml
retry_config:
  attempts: 2
  on_failure: continue
```

**Examples**:
- Cache warmup
- Metrics collection
- Notifications
- Non-critical cleanup

#### Fallback Operations (Use: Fallback)
```yaml
retry_config:
  attempts: 3
  on_failure:
    fallback:
      command: "cat cached-data.json"
```

**Examples**:
- API with cache fallback
- Primary/secondary data sources
- Graceful degradation scenarios

**Source**: `src/cook/retry_v2.rs:153-165` (FailureAction enum)

### Retry Configuration Anti-Patterns

❌ **Don't**: Set very high attempts without retry_budget
```yaml
retry_config:
  attempts: 1000  # Could retry for hours!
```

✅ **Do**: Combine high attempts with retry_budget
```yaml
retry_config:
  attempts: 100
  retry_budget: "5m"  # Caps total time
```

---

❌ **Don't**: Use exponential backoff without max_delay
```yaml
retry_config:
  backoff: exponential
  # Delay could grow to minutes!
```

✅ **Do**: Always set max_delay
```yaml
retry_config:
  backoff: exponential
  max_delay: "30s"
```

---

❌ **Don't**: Retry non-idempotent operations
```yaml
retry_config:
  attempts: 5
  # If command creates resources, retries might duplicate!
```

✅ **Do**: Make operations idempotent or skip retry
```yaml
# Use idempotency tokens, check-before-create, etc.
```

---

❌ **Don't**: Skip jitter in parallel workflows
```yaml
# MapReduce with 10 parallel agents
retry_config:
  jitter: false  # All agents retry simultaneously!
```

✅ **Do**: Enable jitter for parallel execution
```yaml
retry_config:
  jitter: true
  jitter_factor: 0.3
```

### Testing Retry Configuration

Use tests to validate retry behavior (patterns from src/cook/retry_v2.rs:463-748):

```rust
#[tokio::test]
async fn test_retry_with_exponential_backoff() {
    let config = RetryConfig {
        attempts: 3,
        backoff: BackoffStrategy::Exponential { base: 2.0 },
        initial_delay: Duration::from_millis(100),
        ..Default::default()
    };

    let executor = RetryExecutor::new(config);

    // Test that retries happen with correct delays
    // Verify exponential growth: 100ms, 200ms, 400ms
}
```

**Test Coverage**:
- Verify retry attempts match configuration
- Check backoff delays are correct
- Ensure error matchers work as expected
- Validate circuit breaker state transitions
- Test retry budget enforcement

### Production Monitoring

Monitor retry metrics for operational insight:

```rust
// Access retry metrics
let metrics = executor.metrics();
println!("Total attempts: {}", metrics.total_attempts);
println!("Successful: {}", metrics.successful_attempts);
println!("Failed: {}", metrics.failed_attempts);
```

**Source**: `src/cook/retry_v2.rs:320-322` (metrics() method), `src/cook/retry_v2.rs:399-422` (RetryMetrics struct)

**Key Metrics**:
- Total attempts vs successful attempts (success rate)
- Retry counts per operation (identify problematic operations)
- Delay distributions (verify backoff is working)
- Circuit breaker state changes (detect system issues)

### Summary

1. **Use exponential backoff** for most retry scenarios (default)
2. **Always set max_delay** to prevent unbounded delays
3. **Enable jitter** for distributed/parallel systems
4. **Use retry_budget** to cap total retry time
5. **Be selective** with `retry_on` - don't retry permanent errors
6. **Combine with circuit breakers** for high-reliability systems
7. **Choose appropriate failure actions** based on operation criticality
8. **Test retry behavior** to ensure it works as expected
9. **Monitor retry metrics** in production

### See Also

- [Backoff Strategies](./backoff-strategies.md) - Detailed backoff documentation
- [Jitter for Distributed Systems](./jitter-for-distributed-systems.md) - Preventing thundering herd
- [Retry Budget](./retry-budget.md) - Time-based retry limits
- [Conditional Retry with Error Matchers](./conditional-retry-with-error-matchers.md) - Selective retry
- [Failure Actions](./failure-actions.md) - Handling final failures
- [Complete Examples](./complete-examples.md) - Real-world retry configurations
