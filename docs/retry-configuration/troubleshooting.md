## Troubleshooting

Common retry configuration issues and their solutions.

### Issue: Retries Not Triggering

**Symptoms**:
- Command fails immediately without retry
- Only see one attempt in logs
- Expected retries don't happen

**Possible Causes and Solutions**:

#### 1. retry_on Matchers Too Specific

```yaml
# Problem: Error doesn't match any configured matcher
retry_config:
  attempts: 5
  retry_on:
    - network
    # Error is "connection timeout" but matcher only catches "network"
```

**Solution**: Use broader matchers or add custom patterns

```yaml
retry_config:
  attempts: 5
  retry_on:
    - network
    - timeout      # Add timeout matcher
    - pattern: "connection.*timeout"  # Or custom pattern
```

**Debug**: Check actual error message in logs and verify it matches one of your configured matchers (remember: matching is case-insensitive).

**Source**: ErrorMatcher matching logic in src/cook/retry_v2.rs:128-149

#### 2. Empty retry_on with Unexpected Error Filtering

```yaml
# Misconception: Empty retry_on means "don't retry"
# Reality: Empty retry_on means "retry ALL errors"
retry_config:
  attempts: 3
  # retry_on is empty - retries everything!
```

**Solution**: If you see retries happening when you don't expect them, check if `retry_on` is empty.

**Source**: Default behavior in src/cook/retry_v2.rs:42-43

#### 3. retry_config Missing Entirely

```yaml
commands:
  - shell: "curl https://api.example.com"
    # No retry_config - no retry happens
```

**Solution**: Add `retry_config` block to enable retry.

---

### Issue: Retries Happening But Taking Too Long

**Symptoms**:
- Workflow hangs during retries
- Total retry time exceeds expectations
- Delays grow too large

**Possible Causes and Solutions**:

#### 1. Exponential Backoff Without max_delay

```yaml
# Problem: Exponential growth uncapped
retry_config:
  attempts: 10
  backoff: exponential
  initial_delay: "1s"
  # No max_delay - delay can grow to minutes!
```

**Solution**: Always set `max_delay`

```yaml
retry_config:
  attempts: 10
  backoff: exponential
  initial_delay: "1s"
  max_delay: "30s"  # Cap delays at 30 seconds
```

**Default**: max_delay defaults to 30 seconds if not specified (src/cook/retry_v2.rs:64)

#### 2. Too Many Attempts Without retry_budget

```yaml
# Problem: High attempts can retry for hours
retry_config:
  attempts: 100
  backoff: exponential
```

**Solution**: Use `retry_budget` to cap total time

```yaml
retry_config:
  attempts: 100
  backoff: exponential
  retry_budget: "10m"  # Never exceed 10 minutes total
```

**Source**: retry_budget enforcement in tests at src/cook/retry_v2.rs:675-708

#### 3. Initial Delay Too High

```yaml
# Problem: First retry waits too long
retry_config:
  attempts: 3
  initial_delay: "60s"  # 1-minute delay before first retry!
```

**Solution**: Use shorter initial delay, let backoff grow

```yaml
retry_config:
  attempts: 3
  initial_delay: "1s"   # Start small
  backoff: exponential  # Let it grow
  max_delay: "30s"
```

---

### Issue: Circuit Breaker Always Open

**Symptoms**:
- Circuit breaker trips and stays open
- Requests fail immediately after threshold
- Recovery doesn't happen

**Possible Causes and Solutions**:

#### 1. Failure Threshold Too Low

```rust
// Problem: Circuit opens too easily
let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        1,                          // Opens after single failure!
        Duration::from_secs(30)
    );
```

**Solution**: Use appropriate threshold for failure rate

```rust
let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        5,                          // Open after 5 consecutive failures
        Duration::from_secs(30)
    );
```

**Source**: CircuitBreaker implementation in src/cook/retry_v2.rs:325-397

#### 2. Recovery Timeout Too Long

```rust
// Problem: Circuit stays open too long
let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        5,
        Duration::from_secs(600)    // 10-minute recovery time!
    );
```

**Solution**: Use shorter recovery timeout for faster testing

```rust
let executor = RetryExecutor::new(config)
    .with_circuit_breaker(
        5,
        Duration::from_secs(30)     // 30-second recovery attempts
    );
```

#### 3. No Success to Close Circuit

**Problem**: Circuit opens but downstream never recovers, so circuit never closes

**Solution**:
- Check if downstream service is actually recovering
- Verify circuit enters HalfOpen state and test requests succeed
- Monitor circuit state transitions with logging

---

### Issue: Thundering Herd (Multiple Parallel Agents Retrying Simultaneously)

**Symptoms**:
- Service overwhelmed during recovery
- All parallel agents retry at same time
- Cascading failures

**Problem**:

```yaml
# MapReduce with 10 parallel agents, no jitter
map:
  max_parallel: 10
  agent_template:
    - shell: "api-call.sh"
      retry_config:
        attempts: 5
        backoff: exponential
        jitter: false  # All agents retry at same time!
```

**Solution**: Enable jitter

```yaml
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

**Why**: Without jitter, all 10 agents calculate the same exponential delay and retry simultaneously.

**Source**: Jitter application in src/cook/retry_v2.rs:308-317

---

### Issue: Retrying Non-Transient Errors

**Symptoms**:
- Retrying 404 Not Found errors
- Retrying authentication failures (401)
- Wasting time on permanent failures

**Problem**:

```yaml
# Empty retry_on retries everything
retry_config:
  attempts: 5
  # Retries 404, 401, 400, etc. - permanent errors!
```

**Solution**: Use selective retry with error matchers

```yaml
retry_config:
  attempts: 5
  retry_on:
    - network        # Transient
    - timeout        # Transient
    - server_error   # Transient (5xx)
    # Don't retry 404, 401, 400, etc.
```

**Best Practice**: Only retry errors that might succeed on next attempt.

---

### Issue: Retry Budget Not Working as Expected

**Symptoms**:
- Retries exceed configured budget
- Budget seems ignored

**Possible Cause**: Misunderstanding retry_budget behavior

**How retry_budget Works**:
- Budget is checked **before each retry**
- If budget would be exceeded, retry stops
- Budget includes backoff delay time
- Budget does NOT include time for command execution itself

**Example**:

```yaml
retry_config:
  attempts: 10
  backoff: exponential
  initial_delay: "1s"
  max_delay: "60s"
  retry_budget: "2m"  # 2-minute budget
```

**Behavior**:
- If accumulated delays + next delay > 2 minutes → Stop
- Command execution time is NOT counted in budget
- If command takes 1 minute to execute each time, total time could be: 2m (budget) + (attempts * 1m execution) = ~12 minutes

**Source**: retry_budget field in src/cook/retry_v2.rs:46-47, tests at lines 675-708

---

### Issue: Fallback Command Also Failing

**Symptoms**:
- Primary command fails after retries
- Fallback command executes but also fails
- Workflow stops

**Problem**: Fallback command isn't reliable

```yaml
retry_config:
  attempts: 3
  on_failure:
    fallback:
      command: "curl https://backup-api.com/data"  # This can also fail!
```

**Solution**: Make fallback truly reliable

```yaml
retry_config:
  attempts: 3
  on_failure:
    fallback:
      command: "cat /cache/data.json"  # Local cache, very reliable
```

**Best Practice**: Fallback commands should be:
- Local operations (file reads, not network calls)
- Idempotent
- Very unlikely to fail
- Fast

---

### Issue: Retry Metrics Not Matching Expectations

**Symptoms**:
- Metrics show different attempt count than configured
- Unexpected success/failure counts

**Debugging**:

```rust
// Access retry metrics for debugging
let metrics = executor.metrics();
println!("Total attempts: {}", metrics.total_attempts);
println!("Successful: {}", metrics.successful_attempts);
println!("Failed: {}", metrics.failed_attempts);
println!("Retries: {:?}", metrics.retries);  // Vec<(attempt, delay)>
```

**Source**: RetryMetrics struct in src/cook/retry_v2.rs:399-422

**Check**:
- `total_attempts` = successful + failed
- `retries` vector shows actual delays used
- Compare with configured backoff strategy

---

### Debugging Workflow

When retry behavior is unexpected:

1. **Check retry_on matchers**:
   - Verify error message matches configured matchers
   - Remember matching is case-insensitive
   - Empty `retry_on` = retry all errors

2. **Check backoff configuration**:
   - Verify `max_delay` is set
   - Check `initial_delay` isn't too high
   - Ensure `backoff` strategy matches intent

3. **Check retry_budget**:
   - Remember budget is delay time, not total time
   - Budget checked before each retry
   - Command execution time NOT included

4. **Enable jitter for parallel workflows**:
   - Always use jitter in MapReduce
   - Set `jitter_factor` between 0.1 and 0.5

5. **Use selective retry**:
   - Don't retry permanent errors (404, 401, 400)
   - Use `retry_on` to specify transient errors only

6. **Monitor metrics**:
   - Access `RetryMetrics` for actual attempt counts
   - Verify delays match expectations
   - Check circuit breaker state transitions

---

### Getting Help

If retry behavior is still unclear:

1. **Check retry_v2.rs implementation**: src/cook/retry_v2.rs
2. **Review tests**: src/cook/retry_v2.rs:463-748 (comprehensive test coverage)
3. **Enable verbose logging**: Add logging around retry logic to see what's happening
4. **Test with simple cases**: Start with fixed backoff and 2-3 attempts to isolate issue
