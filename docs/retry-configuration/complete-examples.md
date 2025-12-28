## Complete Examples

This section provides complete, runnable YAML workflow examples demonstrating various retry configurations.

**Source**: Examples based on test patterns from src/cook/retry_v2.rs:463-748

### Example 1: Basic Retry with Exponential Backoff

Simple API call with standard exponential backoff:

```yaml
name: fetch-api-data
mode: standard

commands:
  - shell: "curl -f https://api.example.com/data"
    retry_config:
      attempts: 5
      backoff: exponential
      initial_delay: "1s"
      max_delay: "30s"
```

**When it's useful**:
- External API calls
- Network-dependent operations
- Transient failure recovery

**Retry sequence** (delays occur after each failed attempt):

- Attempt 1: Immediate (no prior failure)
- Attempt 2: ~2s delay after attempt 1 fails
- Attempt 3: ~4s delay after attempt 2 fails
- Attempt 4: ~8s delay after attempt 3 fails
- Attempt 5: ~16s delay after attempt 4 fails

### Example 2: Exponential Backoff with Jitter (Distributed Systems)

Multiple parallel agents with jitter to prevent thundering herd:

```yaml
name: parallel-processing
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 10

  agent_template:
    - shell: "process-item ${item.id}"
      retry_config:
        attempts: 5
        backoff: exponential
        initial_delay: "1s"
        max_delay: "30s"
        jitter: true          # Critical for parallel agents
        jitter_factor: 0.3    # 30% randomization
```

**Why jitter matters**: Without jitter, all 10 parallel agents would retry at exactly the same time, overwhelming the recovering service.

!!! note "Understanding jitter range"
    With `jitter_factor: 0.3`, actual delays vary by ±15% of the calculated value. For example, a 2s base delay becomes 1.7s-2.3s (half the jitter factor applied in each direction).

**Source**: Jitter implementation in src/cook/retry_v2.rs:308-317

### Example 3: Conditional Retry with Error Matchers

Only retry transient errors, fail fast on permanent errors:

```yaml
name: selective-retry
mode: standard

commands:
  - shell: "curl -f https://api.example.com/resource"
    retry_config:
      attempts: 5
      backoff: exponential
      initial_delay: "1s"
      max_delay: "30s"
      retry_on:
        - network        # Connection issues
        - timeout        # Slow responses
        - server_error   # 5xx errors
```

**Behavior**:
- Retries: Network errors, timeouts, 500/502/503/504
- Fails immediately: 404, 401, 400 (permanent errors)

**Source**: ErrorMatcher enum in src/cook/retry_v2.rs:100-151

### Example 4: Retry Budget to Prevent Infinite Loops

High retry attempts with time-based cap:

```yaml
name: budget-limited-retry
mode: standard

commands:
  - shell: "long-running-operation"
    retry_config:
      attempts: 100          # High attempt count
      backoff: fibonacci
      initial_delay: "1s"
      max_delay: "60s"
      retry_budget: "10m"    # But never exceed 10 minutes total
```

**Why**: Prevents endless retries while allowing many attempts for operations that typically succeed eventually.

**Source**: retry_budget field in src/cook/retry_v2.rs:46-47, tests at lines 675-708

### Example 5: Fallback on Failure

Use cached data when API fails:

```yaml
name: fallback-example
mode: standard

commands:
  - shell: "curl -f https://api.example.com/live-data"
    retry_config:
      attempts: 3
      backoff: exponential
      initial_delay: "2s"
      max_delay: "10s"
      retry_on:
        - network
        - timeout
      on_failure:
        fallback:
          command: "cat /cache/data.json"

  # Continue processing with either live or cached data
  - shell: "process-data data.json"
```

**Execution flow**:
1. Try to fetch live data (3 attempts with exponential backoff)
2. If all attempts fail → Use cached data
3. Continue with processing

**Source**: FailureAction::Fallback in src/cook/retry_v2.rs:164

### Example 6: Continue on Failure (Non-Critical Operations)

Allow workflow to continue even if optional operations fail:

```yaml
name: mixed-criticality
mode: standard

commands:
  # Critical: must succeed
  - shell: "cargo build"
    retry_config:
      attempts: 3
      on_failure: stop

  # Optional: nice to have but not critical
  - shell: "notify-slack 'Build started'"
    retry_config:
      attempts: 2
      initial_delay: "5s"
      on_failure: continue    # Don't fail workflow if notification fails

  # Critical: must succeed
  - shell: "cargo test"
    retry_config:
      attempts: 3
      on_failure: stop
```

**Use case**: Separating critical operations from best-effort operations.

**Source**: FailureAction::Continue in src/cook/retry_v2.rs:162

### Example 7: Rate Limit Handling

Handle API rate limits with long delays:

```yaml
name: rate-limit-aware
mode: standard

commands:
  - shell: "api-call.sh"
    retry_config:
      attempts: 10
      backoff: exponential
      initial_delay: "60s"    # Start with 1-minute delay
      max_delay: "10m"        # Cap at 10 minutes
      retry_on:
        - rate_limit          # Only retry on 429 errors
```

**Why**: Rate limits often require longer delays than network errors.

**Source**: ErrorMatcher::RateLimit in src/cook/retry_v2.rs:143-147

### Example 8: Custom Pattern Matching

Retry database-specific errors:

```yaml
name: database-retry
mode: standard

commands:
  - shell: "sqlite3 db.sqlite 'INSERT INTO ...'"
    retry_config:
      attempts: 5
      backoff: linear
      initial_delay: "100ms"
      retry_on:
        - pattern: "database.*locked"
        - pattern: "SQLITE_BUSY"
        - pattern: "cannot commit.*in progress"
```

**Source**: ErrorMatcher::Pattern in src/cook/retry_v2.rs:113

### Example 9: Fibonacci Backoff for Gradual Recovery

Gentler backoff curve for services needing recovery time:

```yaml
name: fibonacci-backoff-example
mode: standard

commands:
  - shell: "connect-to-recovering-service.sh"
    retry_config:
      attempts: 8
      backoff: fibonacci
      initial_delay: "1s"
      max_delay: "60s"
```

**Delay sequence**: 1s, 2s, 3s, 5s, 8s, 13s, 21s, 34s

**Why Fibonacci**: Grows slower than exponential, giving services more time to recover without aggressive backoff.

**Source**: Fibonacci calculation in src/cook/retry_v2.rs:425-440

### Example 10: Linear Backoff for Predictable Delays

Testing or debugging with consistent delays:

```yaml
name: linear-backoff-example
mode: standard

commands:
  - shell: "test-operation.sh"
    retry_config:
      attempts: 5
      backoff:
        linear:
          increment: "3s"
      initial_delay: "1s"
```

**Delay sequence**: 1s, 4s, 7s, 10s, 13s (initial + (attempt-1) × increment)

**Source**: BackoffStrategy::Linear in src/cook/retry_v2.rs:77-80

### Example 11: Fixed Delay for Polling

Consistent polling interval:

```yaml
name: polling-example
mode: standard

commands:
  - shell: "check-job-status.sh"
    retry_config:
      attempts: 20
      backoff: fixed
      initial_delay: "5s"
```

**Delay sequence**: 5s between every attempt

**Use case**: Status polling, health checks

**Source**: BackoffStrategy::Fixed in src/cook/retry_v2.rs:75

### Example 12: Complex Multi-Command Workflow

Real-world example combining multiple retry strategies:

```yaml
name: deployment-workflow
mode: standard

commands:
  # Step 1: Build (critical, retry network issues)
  - shell: "cargo build --release"
    retry_config:
      attempts: 3
      backoff: exponential
      retry_on:
        - network
      on_failure: stop

  # Step 2: Run tests (critical, no retry on real failures)
  - shell: "cargo test"
    retry_config:
      attempts: 2
      initial_delay: "5s"
      retry_on:
        - pattern: "temporary.*failure"
      on_failure: stop

  # Step 3: Upload artifacts (retry with backoff)
  - shell: "upload-to-s3.sh artifacts/"
    retry_config:
      attempts: 5
      backoff: exponential
      initial_delay: "2s"
      max_delay: "60s"
      jitter: true
      retry_on:
        - network
        - timeout
        - server_error
      on_failure:
        fallback:
          command: "save-to-local-backup.sh artifacts/"

  # Step 4: Notify (optional, don't block on failure)
  - shell: "notify-deployment.sh"
    retry_config:
      attempts: 2
      initial_delay: "5s"
      on_failure: continue

  # Step 5: Health check (retry with fixed delay)
  - shell: "health-check.sh"
    retry_config:
      attempts: 10
      backoff: fixed
      initial_delay: "10s"
      on_failure: stop
```

### Example 13: MapReduce with DLQ and Retry

MapReduce workflow with error handling:

```yaml
name: mapreduce-with-retry
mode: mapreduce

error_policy:
  on_item_failure: dlq        # Send failures to Dead Letter Queue
  continue_on_failure: true   # Keep processing other items
  max_failures: 5             # Stop if more than 5 items fail

map:
  input: "work-items.json"
  json_path: "$.items[*]"
  max_parallel: 10

  agent_template:
    - shell: "process-item ${item.id}"
      retry_config:
        attempts: 3
        backoff: exponential
        initial_delay: "1s"
        max_delay: "30s"
        jitter: true          # Important for parallel agents
        retry_on:
          - network
          - timeout
        on_failure: stop      # Let DLQ handle final failures

reduce:
  - shell: "aggregate-results ${map.results}"
```

**Error handling flow**:
1. Each work item is retried up to 3 times per agent
2. If all retries fail → Item goes to DLQ
3. Processing continues for other items
4. After map phase, retry DLQ items with: `prodigy dlq retry <job_id>`

**Source**: WorkflowErrorPolicy in src/cook/workflow/error_policy.rs:132-178

### Testing Your Retry Configuration

Validate retry behavior with controlled failures:

```yaml
name: test-retry-behavior
mode: standard

commands:
  # Use a script that fails N times then succeeds
  - shell: "./fail-then-succeed.sh 2"  # Fails 2 times, succeeds on 3rd
    retry_config:
      attempts: 5
      backoff: exponential
      initial_delay: "1s"
```

**fail-then-succeed.sh** example:
```bash
#!/bin/bash
FAIL_COUNT=${1:-2}
STATE_FILE="/tmp/retry-test-$$"

if [ ! -f "$STATE_FILE" ]; then
  echo "0" > "$STATE_FILE"
fi

CURRENT=$(cat "$STATE_FILE")
NEXT=$((CURRENT + 1))
echo "$NEXT" > "$STATE_FILE"

if [ "$NEXT" -le "$FAIL_COUNT" ]; then
  echo "Attempt $NEXT: Simulated failure"
  exit 1
else
  echo "Attempt $NEXT: Success!"
  rm "$STATE_FILE"
  exit 0
fi
```
