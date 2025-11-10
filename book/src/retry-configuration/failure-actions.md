## Failure Actions

Configure what happens after all retry attempts are exhausted using the `on_failure` field. Failure actions determine workflow behavior when a command fails despite retries.

**Source**: `src/cook/retry_v2.rs:153-165` (FailureAction enum definition)

### Available Failure Actions

The `FailureAction` enum provides three strategies for handling final failures:

#### 1. Stop (Default)

Stop the workflow execution immediately when all retries are exhausted:

```yaml
retry_config:
  attempts: 3
  on_failure: stop
```

**Behavior**:
- Workflow execution halts
- Error is propagated to the caller
- No subsequent commands are executed
- Exit code reflects the failure

**Source**: `src/cook/retry_v2.rs:160` (Stop variant)

**Use Case**: Critical operations where failure should prevent further execution (database migrations, deployment prerequisites).

**Default**: This is the default action when `on_failure` is omitted (src/cook/retry_v2.rs:66).

#### 2. Continue

Continue workflow execution despite the failure:

```yaml
retry_config:
  attempts: 3
  on_failure: continue
```

**Behavior**:
- Failure is logged but not fatal
- Workflow continues to next command
- Useful for non-critical operations
- Final workflow status may still be successful

**Source**: `src/cook/retry_v2.rs:162` (Continue variant)

**Use Case**: Optional operations like cache warmup, non-critical notifications, or best-effort cleanup tasks.

**Example**:
```yaml
commands:
  # Critical: must succeed
  - shell: "cargo build"
    retry_config:
      attempts: 3
      on_failure: stop

  # Optional: can fail without blocking workflow
  - shell: "notify-slack 'Build completed'"
    retry_config:
      attempts: 2
      on_failure: continue

  # Critical: must succeed
  - shell: "cargo test"
    retry_config:
      attempts: 3
      on_failure: stop
```

#### 3. Fallback Command

Execute an alternative command when all retries fail:

```yaml
retry_config:
  attempts: 3
  on_failure:
    fallback:
      command: "echo 'Primary failed, using fallback' && ./fallback.sh"
```

**Behavior**:
- Primary command is attempted with retries
- If all retries fail, fallback command is executed
- Fallback command runs **without retry** (single attempt)
- If fallback succeeds, workflow continues
- If fallback fails, workflow stops

**Source**: `src/cook/retry_v2.rs:164` (Fallback variant with command field)

**Use Case**:
- Graceful degradation (use cache when API fails)
- Alternative data sources
- Cleanup or notification on failure
- Circuit breaker fallback logic

**Example with Fallback**:
```yaml
commands:
  - shell: "curl https://api.example.com/live-data"
    retry_config:
      attempts: 5
      backoff: exponential
      retry_on:
        - network
        - timeout
      on_failure:
        fallback:
          command: "cat cached-data.json"
```

**Execution Flow**:
1. Try `curl https://api.example.com/live-data`
2. If network/timeout error → Retry with exponential backoff
3. Repeat up to 5 attempts
4. If all attempts fail → Execute fallback: `cat cached-data.json`
5. If fallback succeeds → Continue workflow
6. If fallback fails → Stop workflow

### Combining with Error Matchers

Failure actions work together with error matchers for sophisticated error handling:

```yaml
retry_config:
  attempts: 3
  retry_on:
    - network
    - timeout
  on_failure: continue
```

**Behavior**:
- Only network and timeout errors trigger retry
- Other errors (e.g., 404, auth failures) fail immediately
- If all 3 retry attempts fail → Continue workflow anyway

### Fallback Command Requirements

**Important**: Fallback commands must be idempotent and reliable:

✅ **Good Fallback Examples**:
- Reading from cache: `cat cache/data.json`
- Using default values: `echo '{"default": true}'`
- Logging failure: `logger "API call failed"`
- Sending alerts: `notify-ops "Degraded mode active"`

❌ **Bad Fallback Examples**:
- Commands that might also fail with retry: `curl https://backup-api.com`
- Destructive operations: `rm -rf data/`
- Commands requiring retry themselves

### Multiple Failure Actions in Workflow

Different commands can have different failure actions:

```yaml
commands:
  # Must succeed - stop if all retries fail
  - shell: "initialize-database"
    retry_config:
      attempts: 3
      on_failure: stop

  # Best effort - continue if it fails
  - shell: "warm-cache"
    retry_config:
      attempts: 2
      on_failure: continue

  # Use fallback if primary fails
  - shell: "fetch-config https://primary.com/config"
    retry_config:
      attempts: 3
      on_failure:
        fallback:
          command: "fetch-config https://backup.com/config"

  # Must succeed - stop if all retries fail
  - shell: "run-tests"
    retry_config:
      attempts: 1
      on_failure: stop
```

### Implementation Details

The `FailureAction` enum is defined as:

```rust
// Source: src/cook/retry_v2.rs:153-165
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureAction {
    /// Stop workflow execution (default)
    Stop,
    /// Continue with next command
    Continue,
    /// Execute fallback command
    Fallback { command: String },
}

impl Default for FailureAction {
    fn default() -> Self {
        Self::Stop
    }
}
```

### Retry Executor Integration

The `RetryExecutor` handles failure actions after exhausting retries (simplified logic):

```rust
// Conceptual flow (see src/cook/retry_v2.rs:191-262)
async fn execute_with_retry() -> Result<T> {
    for attempt in 1..=self.config.attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < self.config.attempts => {
                // Continue retrying
                sleep(calculate_delay(attempt)).await;
            }
            Err(e) => {
                // All retries exhausted - apply failure action
                match self.config.on_failure {
                    FailureAction::Stop => return Err(e),
                    FailureAction::Continue => {
                        log::warn!("Command failed but continuing: {}", e);
                        return Ok(default_value);
                    }
                    FailureAction::Fallback { command } => {
                        return execute_fallback(&command).await;
                    }
                }
            }
        }
    }
}
```

### Best Practices

1. **Use Stop for Critical Operations**: Database migrations, deployments, infrastructure setup
2. **Use Continue for Optional Tasks**: Notifications, caching, metrics
3. **Use Fallback for Degraded Mode**: Cache when API fails, backup data sources
4. **Keep Fallbacks Simple**: Avoid complex logic that might also fail
5. **Document Fallback Behavior**: Make it clear when degraded mode is active

### See Also

- [Basic Retry Configuration](./basic-retry-configuration.md) - Configuring retry attempts
- [Conditional Retry with Error Matchers](./conditional-retry-with-error-matchers.md) - Selective retry
- [Best Practices](./best-practices.md) - When to use each failure action
- [Complete Examples](./complete-examples.md) - Full workflow examples with failure actions
