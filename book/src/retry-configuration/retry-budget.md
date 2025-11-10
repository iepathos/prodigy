## Retry Budget

A retry budget provides a time-based upper bound on retry operations, preventing workflows from hanging indefinitely even when attempt counts are high. The retry budget limits the cumulative **delay time** (not total execution time) spent on retries.

**Source**: `src/cook/retry_v2.rs:47` (retry_budget field in RetryConfig struct)

### Configuration

The `retry_budget` field accepts human-readable duration formats using the `humantime_serde` parser:

```yaml
retry_config:
  attempts: 10
  retry_budget: "5m"        # 5 minutes
  backoff:
    exponential:
      base: 2.0
  initial_delay: "1s"
```

**Supported Duration Formats**:
- Seconds: `"30s"`, `"300s"`
- Minutes: `"5m"`, `"10m"`
- Hours: `"1h"`, `"2h"`
- Combined: `"1h30m"`, `"2m30s"`

**Source**: Duration field with `humantime_serde` annotation at `src/cook/retry_v2.rs:47`

### How It Works

Prodigy uses **two complementary mechanisms** to enforce retry budgets:

#### 1. Duration-Based Enforcement (Active Execution)

During active command execution, the `RetryExecutor` tracks cumulative delay time and checks the budget before each retry:

```rust
// From src/cook/retry_v2.rs:236-244
if let Some(budget) = self.config.retry_budget {
    if total_delay + jittered_delay > budget {
        warn!("Retry budget exhausted for {}", context);
        return Err(anyhow!("Retry budget exhausted"));
    }
}
```

**Behavior**:
- Tracks `total_delay`: cumulative duration spent waiting between retries
- Before each retry, checks: `total_delay + next_delay > budget`
- If the next retry would exceed the budget, stops immediately
- Returns error: `"Retry budget exhausted"`

**Source**: `src/cook/retry_v2.rs:236-244`

#### 2. Timestamp-Based Enforcement (Stateful Tracking)

For checkpoint/resume scenarios, the `RetryStateManager` uses expiration timestamps:

```rust
// From src/cook/retry_state.rs:299-301
retry_budget_expires_at: config
    .retry_budget
    .map(|budget| Utc::now() + ChronoDuration::from_std(budget).unwrap())
```

**Behavior**:
- Calculates expiration: `retry_budget_expires_at = now + budget_duration`
- Set at **first failure** (not at command start)
- Before each retry attempt, checks: `Utc::now() >= retry_budget_expires_at`
- Prevents retries even after workflow interruption and resume

**Source**: `src/cook/retry_state.rs:299-301, 342-346`

### Interaction with Attempts and Backoff

**Critical behavior**: Retries stop when **EITHER** the attempts limit **OR** the retry budget is exceeded, whichever comes first.

**Example 1: Budget Limits Before Attempts**
```yaml
retry_config:
  attempts: 100           # High attempt limit
  retry_budget: "2m"      # Budget will be hit first
  backoff:
    exponential:
      base: 2.0
  initial_delay: "1s"
```

With exponential backoff (1s, 2s, 4s, 8s, 16s, 32s, 64s...), delays sum to ~2 minutes after 7-8 attempts. The budget stops retries at **~8 attempts** even though 100 are allowed.

**Example 2: Attempts Limit Before Budget**
```yaml
retry_config:
  attempts: 3             # Attempts will be hit first
  retry_budget: "10m"     # Budget won't be reached
  backoff: fixed
  initial_delay: "5s"
```

Total delay: 5s + 5s + 5s = 15 seconds, well under the 10-minute budget. Stops after **3 attempts**.

**Source**: Dual checking logic in `src/cook/retry_v2.rs:236-244` and `src/cook/retry_state.rs:337-347`

### What Time is Counted?

**Included in Budget**:
- ✅ Backoff delay time (waiting between retries)
- ✅ Jitter-adjusted delays (if jitter is enabled)

**NOT Included in Budget**:
- ❌ Command execution time
- ❌ Time for successful operations
- ❌ Time before first failure

**Example Timeline**:
```
Command Start → Execute (30s) → Fail
                ↓ retry_budget timer starts
                Wait 1s (counted) → Execute (30s, NOT counted) → Fail
                Wait 2s (counted) → Execute (30s, NOT counted) → Fail
                Wait 4s (counted) → Execute (30s, NOT counted) → Success
Total Time: 30+1+30+2+30+4+30 = 127 seconds
Budget Used: 1+2+4 = 7 seconds
```

If `retry_budget: "5s"`, the workflow would fail at the third retry (1+2+4 = 7s > 5s budget).

### Best Practices

#### Choosing Appropriate Retry Budget Values

Consider these factors when setting retry budgets:

**1. Workflow Timeout Requirements**
- If your CI/CD pipeline times out at 10 minutes, set `retry_budget` to ensure retries complete within that window
- Leave buffer for command execution time (budget only counts delays)

**2. Balance Between Attempts and Time**
- High attempts + exponential backoff = potential for very long delays
- Use retry budget as a safety net: `attempts: 50` with `retry_budget: "5m"` gives flexibility but prevents runaway retries

**3. Typical Operation Duration**
- If commands take 2 minutes each, budget of "30s" with 5 attempts means max total time: 30s (delays) + 10 minutes (5 × 2min execution) = 10.5 minutes
- Account for execution time separately from budget

**4. User Experience Expectations**
- Interactive workflows: Short budgets (1-2 minutes) for faster feedback
- Background jobs: Longer budgets (10-30 minutes) for better reliability

#### Example Configurations

**Fast-Failing Web Requests**:
```yaml
retry_config:
  attempts: 10
  retry_budget: "1m"
  backoff:
    exponential:
      base: 2.0
  initial_delay: "500ms"
  max_delay: "10s"
```

**Resilient Background Processing**:
```yaml
retry_config:
  attempts: 20
  retry_budget: "10m"
  backoff: fibonacci
  initial_delay: "2s"
  max_delay: "60s"
```

**CI/CD with Strict Timeout**:
```yaml
retry_config:
  attempts: 100          # Generous attempts
  retry_budget: "8m"     # But hard 8-minute budget (CI times out at 10m)
  backoff:
    exponential:
      base: 2.0
  initial_delay: "1s"
  max_delay: "30s"
```

### Error Messages and Debugging

When the retry budget is exceeded, the command fails with the last error encountered:

**Error Message**:
```
Error: Retry budget exhausted
```

**Debug Logs** (from `src/cook/retry_state.rs:344`):
```
DEBUG Command <command_id> retry budget expired
```

**What Happens**:
1. Retry budget check fails before next retry
2. Command returns error immediately (no final retry attempt)
3. If `on_failure` is configured, fallback action is triggered
4. Otherwise, workflow fails with "Retry budget exhausted" error

### Troubleshooting

**Issue: Budget Seems to Be Ignored**

Remember that the budget counts **delay time**, not **total time**. If your commands take a long time to execute, total workflow time can significantly exceed the budget.

**Solution**: If you need a hard total time limit, use a timeout mechanism at the workflow level, not just retry budget.

**Issue: Retries Stop Earlier Than Expected**

Check if jitter is enabled. Jitter adds randomization to delays, which can cause the budget to be reached sooner than calculated base delays would suggest.

**Solution**: Account for jitter factor when calculating expected budget consumption: `max_delay = base_delay * (1 + jitter_factor)`.

### Real-World Example

From test suite (`src/cook/retry_v2.rs:727-747`):

```rust
#[tokio::test]
async fn test_retry_budget() {
    let config = RetryConfig {
        attempts: 10,                                  // Allow many attempts
        initial_delay: Duration::from_millis(50),
        retry_budget: Some(Duration::from_millis(100)), // But cap at 100ms delay
        ..Default::default()
    };
    let executor = RetryExecutor::new(config);

    let start = Instant::now();
    let result = executor
        .execute_with_retry(
            || async { Err::<i32, _>(anyhow!("Persistent failure")) },
            "test",
        )
        .await;

    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_millis(200)); // Budget enforced
}
```

This test demonstrates:
- 10 attempts allowed, but budget limits actual retries
- Total delay time capped at 100ms despite potentially more attempts
- Workflow completes quickly (< 200ms) instead of running all 10 attempts

### See Also

- [Basic Retry Configuration](./basic-retry-configuration.md) - Understanding retry fundamentals
- [Backoff Strategies](./backoff-strategies.md) - How delays are calculated
- [Troubleshooting](./troubleshooting.md) - Debugging retry budget issues
- [Complete Examples](./complete-examples.md) - Full workflow examples with retry budgets

