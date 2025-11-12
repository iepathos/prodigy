## Backoff Strategies

> **Note**: This subsection documents the **enhanced retry system** (`retry_v2::RetryConfig`) used for command-level retry configuration. For workflow-level retry (MapReduce error policies), see [Workflow-Level vs Command-Level Retry](./workflow-level-vs-command-level-retry.md). The enhanced system provides more sophisticated backoff options and features.

Prodigy supports five backoff strategies for controlling delay between retries. Backoff strategies determine how the delay between retry attempts increases over time, helping to avoid overwhelming systems while maximizing chances of success.

All backoff strategies use `initial_delay` as the base delay and respect the `max_delay` cap. Delays are calculated per attempt and can be combined with [jitter](jitter-for-distributed-systems.md) to avoid thundering herd problems.

**Source**: BackoffStrategy enum defined in `src/cook/retry_v2.rs:70-98`

**Default Strategy**: If no backoff strategy is specified, Prodigy uses **Exponential** backoff with a base of 2.0 (src/cook/retry_v2.rs:92-98).

### Fixed Backoff

Fixed backoff uses a constant delay between all retry attempts. This is the simplest strategy and works well when you want predictable, consistent retry timing.

**Delay Pattern**: Same delay for every attempt
- Attempt 1: `initial_delay`
- Attempt 2: `initial_delay`
- Attempt 3: `initial_delay`

**YAML Configuration**:
```yaml
map:
  agent_template:
    - shell: "flaky-command"
      retry_config:
        attempts: 5
        initial_delay: "2s"
        backoff: fixed
```

> **Note**: `backoff: fixed` is the shorthand for the Fixed unit variant. Equivalent to `backoff: { fixed: null }` (src/cook/retry_v2.rs:75).

**When to Use**:
- Simple retry scenarios where timing is not critical
- Testing and development environments
- When you want predictable retry intervals

### Linear Backoff

Linear backoff increases the delay by a fixed increment for each retry attempt. This provides gradual backoff that's easy to reason about.

**Delay Pattern**: Increases by constant increment
- Attempt 1: `initial_delay`
- Attempt 2: `initial_delay + increment`
- Attempt 3: `initial_delay + 2 * increment`

**YAML Configuration**:
```yaml
map:
  agent_template:
    - shell: "database-query"
      retry_config:
        attempts: 5
        initial_delay: "1s"
        backoff:
          linear:
            increment: "2s"
```

**Example Timeline** (initial_delay=1s, increment=2s):
- Attempt 1: 1s
- Attempt 2: 3s (1s + 2s)
- Attempt 3: 5s (1s + 4s)
- Attempt 4: 7s (1s + 6s)

**When to Use**:
- Moderate load situations
- When you need predictable but increasing delays
- API rate limiting scenarios with linear cooldown

### Exponential Backoff

Exponential backoff multiplies the delay by a base factor for each retry, causing delays to grow rapidly. This is the **default strategy** and is recommended for most retry scenarios.

**Delay Pattern**: Multiplies by base^(attempt-1)
- Attempt 1: `initial_delay * base^0`
- Attempt 2: `initial_delay * base^1`
- Attempt 3: `initial_delay * base^2`

**YAML Configuration** (default base=2.0):
```yaml
map:
  agent_template:
    - shell: "network-call"
      retry_config:
        attempts: 5
        initial_delay: "1s"
        max_delay: "60s"
        # Uses exponential backoff with base 2.0 by default
```

**YAML Configuration** (custom base):
```yaml
map:
  agent_template:
    - shell: "api-request"
      retry_config:
        attempts: 5
        initial_delay: "1s"
        max_delay: "60s"
        backoff:
          exponential:
            base: 3.0  # More aggressive backoff
```

**Example Timeline** (initial_delay=1s, base=2.0):
- Attempt 1: 1s (1 * 2^0)
- Attempt 2: 2s (1 * 2^1)
- Attempt 3: 4s (1 * 2^2)
- Attempt 4: 8s (1 * 2^3)
- Attempt 5: 16s (1 * 2^4)

**When to Use**:
- Network requests and API calls (default choice)
- Situations where quick retries might make things worse
- Distributed systems with cascading failures
- When you want rapid backoff to give systems time to recover

### Fibonacci Backoff

Fibonacci backoff uses the Fibonacci sequence (1, 1, 2, 3, 5, 8, 13...) to calculate delays. This provides a middle ground between linear and exponential backoff, growing quickly but not as aggressively as exponential.

**Delay Pattern**: Uses Fibonacci sequence multiplier
- Attempt 1: `initial_delay * 1`
- Attempt 2: `initial_delay * 1`
- Attempt 3: `initial_delay * 2`
- Attempt 4: `initial_delay * 3`
- Attempt 5: `initial_delay * 5`

**YAML Configuration**:
```yaml
map:
  agent_template:
    - shell: "distributed-operation"
      retry_config:
        attempts: 6
        initial_delay: "1s"
        max_delay: "30s"
        backoff: fibonacci
```

> **Note**: `backoff: fibonacci` is the shorthand for the Fibonacci unit variant, similar to Fixed (src/cook/retry_v2.rs:87).

**Example Timeline** (initial_delay=1s):
- Attempt 1: 1s (fib(1) = 1)
- Attempt 2: 1s (fib(2) = 1)
- Attempt 3: 2s (fib(3) = 2)
- Attempt 4: 3s (fib(4) = 3)
- Attempt 5: 5s (fib(5) = 5)
- Attempt 6: 8s (fib(6) = 8)

**When to Use**:
- Distributed systems where you want balanced backoff
- Situations where exponential is too aggressive
- When you want retry intervals that grow naturally but moderately

### Custom Backoff

Custom backoff allows you to specify an explicit sequence of delays for complete control over retry timing. If the retry attempt exceeds the number of delays specified, the `max_delay` is used.

**Delay Pattern**: Uses explicit delay list
- Delays are used in order from the array
- If attempts exceed delays array length, uses `max_delay`

**YAML Configuration**:
```yaml
map:
  agent_template:
    - shell: "custom-retry-operation"
      retry_config:
        attempts: 5
        max_delay: "60s"
        backoff:
          custom:
            delays:
              - secs: 1
                nanos: 0
              - secs: 3
                nanos: 0
              - secs: 7
                nanos: 0
              - secs: 15
                nanos: 0
              # Attempt 5 would use max_delay (60s)
```

> **Note**: Custom backoff delays use Duration struct format (`{secs: N, nanos: 0}`) instead of humantime strings like "1s". This is because `Vec<Duration>` doesn't have the `humantime_serde` annotation (src/cook/retry_v2.rs:89).

**Example Timeline**:
- Attempt 1: 1s (delays[0])
- Attempt 2: 3s (delays[1])
- Attempt 3: 7s (delays[2])
- Attempt 4: 15s (delays[3])
- Attempt 5: 60s (max_delay, delays[4] doesn't exist)

**When to Use**:
- When you need precise control over retry timing
- Integration with third-party APIs with specific retry requirements
- Complex retry scenarios with non-standard delay patterns
- Testing specific timing scenarios

**Edge Cases**:
- Empty delays array: Falls back to `max_delay` for all attempts
- Fewer delays than attempts: Uses `max_delay` for remaining attempts

## Integration with RetryConfig

Backoff strategies work together with other retry configuration options:

**Complete Example**:
```yaml
map:
  input: "work-items.json"
  json_path: "$.items[*]"

  agent_template:
    - shell: "process-item ${item.id}"
      retry_config:
        attempts: 5
        initial_delay: "2s"      # Base delay for backoff calculation
        max_delay: "60s"         # Cap on calculated delays
        backoff:
          exponential:
            base: 2.0
        jitter: true             # Add randomization (±25% by default)
        jitter_factor: 0.25
        retry_on:
          - timeout              # Built-in timeout matcher
          - pattern: "connection refused"  # Custom pattern for specific errors
```

**How It Works Together**:
1. Backoff strategy calculates base delay using `initial_delay`
2. Calculated delay is capped at `max_delay`
3. If `jitter: true`, randomization is applied (±`jitter_factor` percentage)
4. Final delay is applied before next retry attempt
5. If `retry_on` is specified, error must match one of the matchers to trigger retry

> **Error Matcher Syntax**: The `retry_on` field uses `ErrorMatcher` enum variants. Built-in matchers (`timeout`, `network`, `server_error`, `rate_limit`) use lowercase names. Custom patterns use `pattern: "regex"` syntax. See [Conditional Retry with Error Matchers](conditional-retry-with-error-matchers.md) for complete documentation.

See also:
- [Basic Retry Configuration](basic-retry-configuration.md) - Overall retry configuration options
- [Backoff Strategy Comparison](backoff-strategy-comparison.md) - Visual comparison of strategies
- [Jitter for Distributed Systems](jitter-for-distributed-systems.md) - How jitter prevents thundering herd
- [Retry Metrics and Observability](retry-metrics-and-observability.md) - Track retry performance

