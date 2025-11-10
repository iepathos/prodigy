## Conditional Retry with Error Matchers

By default, Prodigy retries all errors when `retry_on` is empty. Use the `retry_on` field to retry only specific error types, allowing fine-grained control over which failures should trigger retries.

**Source**: `src/cook/retry_v2.rs:100-151` (ErrorMatcher enum definition)

**Note**: All error matching is case-insensitive. Error messages are normalized to lowercase before pattern comparison (src/cook/retry_v2.rs:128-149).

### Available Error Matchers

The `ErrorMatcher` enum provides five built-in matchers for common error categories:

#### 1. Network Errors

Matches network connectivity issues:

```yaml
retry_config:
  attempts: 5
  retry_on:
    - network
```

**Matches** (case-insensitive):
- "network"
- "connection"
- "refused"
- "unreachable"

**Source**: `src/cook/retry_v2.rs:128-132`

**Use Case**: Retrying HTTP requests, database connections, or API calls that fail due to network issues.

#### 2. Timeout Errors

Matches timeout-related failures:

```yaml
retry_config:
  attempts: 3
  retry_on:
    - timeout
```

**Matches** (case-insensitive):
- "timeout"
- "timed out"

**Source**: `src/cook/retry_v2.rs:133-137`

**Use Case**: Retrying slow external services or operations with strict time limits.

#### 3. Server Errors

Matches HTTP 5xx server errors:

```yaml
retry_config:
  attempts: 4
  retry_on:
    - server_error
```

**Matches** (case-insensitive):
- "500"
- "502"
- "503"
- "504"
- "server error"

**Source**: `src/cook/retry_v2.rs:138-142`

**Use Case**: Retrying API requests during transient server failures or deployments.

#### 4. Rate Limit Errors

Matches rate limiting responses:

```yaml
retry_config:
  attempts: 10
  initial_delay: "60s"
  retry_on:
    - rate_limit
```

**Matches** (case-insensitive):
- "rate limit"
- "429"
- "too many requests"

**Source**: `src/cook/retry_v2.rs:143-147`

**Use Case**: Retrying API calls with exponential backoff when hitting rate limits.

#### 5. Custom Pattern Matching

Match specific error messages using regex patterns:

```yaml
retry_config:
  attempts: 3
  retry_on:
    - pattern: "database locked"
    - pattern: "temporary failure"
    - pattern: "ECONNRESET"
```

**Pattern Syntax**: Regex patterns matched against error message (case-insensitive)

**Source**: `src/cook/retry_v2.rs:113` (Pattern variant)

**Use Case**: Matching application-specific error messages or database-specific errors.

### Combining Multiple Matchers

You can specify multiple error matchers to retry on any of them:

```yaml
retry_config:
  attempts: 5
  backoff: exponential
  initial_delay: "2s"
  retry_on:
    - network
    - timeout
    - server_error
```

This configuration retries if the error matches **any** of:
- Network errors
- Timeout errors
- Server errors (5xx)

**Behavior**: Matchers are evaluated with OR logic - if any matcher matches, the error is retryable.

### Empty retry_on (Retry All Errors)

When `retry_on` is empty or omitted, **all errors trigger retry**:

```yaml
retry_config:
  attempts: 3
  # retry_on is empty - retries all errors
```

**Source**: `src/cook/retry_v2.rs:42-43` (retry_on field with default `Vec::new()`)

This is equivalent to having no error filtering - every failure triggers the retry logic.

### Selective Retry Example

Only retry transient network and timeout issues, but fail immediately on other errors:

```yaml
commands:
  - shell: "curl -f https://api.example.com/data"
    retry_config:
      attempts: 5
      backoff: exponential
      initial_delay: "1s"
      max_delay: "30s"
      retry_on:
        - network
        - timeout
```

**Behavior**:
- If curl fails with "connection refused" → Retry
- If curl fails with "timeout" → Retry
- If curl fails with "404 Not Found" → **Fail immediately** (no retry)
- If curl fails with "401 Unauthorized" → **Fail immediately** (no retry)

### Advanced Pattern Matching

Use regex patterns for precise error matching:

```yaml
retry_config:
  attempts: 3
  retry_on:
    - pattern: "SQLite.*database is locked"
    - pattern: "SQLITE_BUSY"
    - pattern: "deadlock detected"
```

**Pattern Matching** (src/cook/retry_v2.rs:128-149):
1. Error message is converted to lowercase
2. Each matcher's `matches()` method is called
3. For `Pattern` variant, regex is applied (case-insensitive)
4. If any matcher returns true, error is retryable

### Implementation Details

The matching logic is implemented in `ErrorMatcher::matches()`:

```rust
// Simplified implementation (src/cook/retry_v2.rs:128-149)
impl ErrorMatcher {
    pub fn matches(&self, error: &str) -> bool {
        let error_lower = error.to_lowercase();
        match self {
            Self::Network => {
                error_lower.contains("network")
                    || error_lower.contains("connection")
                    || error_lower.contains("refused")
                    || error_lower.contains("unreachable")
            }
            Self::Timeout => {
                error_lower.contains("timeout") || error_lower.contains("timed out")
            }
            Self::ServerError => {
                error_lower.contains("500")
                    || error_lower.contains("502")
                    || error_lower.contains("503")
                    || error_lower.contains("504")
                    || error_lower.contains("server error")
            }
            Self::RateLimit => {
                error_lower.contains("rate limit")
                    || error_lower.contains("429")
                    || error_lower.contains("too many requests")
            }
            Self::Pattern(pattern) => {
                // Regex matching (case-insensitive)
                Regex::new(pattern).map(|re| re.is_match(&error_lower)).unwrap_or(false)
            }
        }
    }
}
```

### Testing Error Matchers

The retry_v2 module includes comprehensive tests for error matching (src/cook/retry_v2.rs:463-748):

```rust
#[test]
fn test_error_matcher_network() {
    let matcher = ErrorMatcher::Network;
    assert!(matcher.matches("Connection refused"));
    assert!(matcher.matches("Network unreachable"));
}
```

### See Also

- [Basic Retry Configuration](./basic-retry-configuration.md) - Core retry configuration
- [Failure Actions](./failure-actions.md) - What happens when all retries are exhausted
- [Best Practices](./best-practices.md) - When to use selective retry
- [Complete Examples](./complete-examples.md) - Full workflow examples with error matchers
