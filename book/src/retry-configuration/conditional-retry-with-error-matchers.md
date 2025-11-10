## Conditional Retry with Error Matchers

By default, Prodigy retries all errors when `retry_on` is empty. Use the `retry_on` field to retry only specific error types, allowing fine-grained control over which failures should trigger retries.

**Source**: `src/cook/retry_v2.rs:100-151` (ErrorMatcher enum definition)

**Case Sensitivity Behavior**:
- **Built-in matchers** (Network, Timeout, ServerError, RateLimit): **Case-insensitive** - error messages are normalized to lowercase before matching
- **Pattern matcher**: **Case-sensitive by default** - matches against original error message case (src/cook/retry_v2.rs:142-148)
  - Use regex flag `(?i)` for case-insensitive pattern matching
  - Example: `pattern: '(?i)database locked'` matches "Database Locked", "DATABASE LOCKED", etc.

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
    - pattern: "(?i)database locked"    # Case-insensitive: matches any case
    - pattern: "SQLITE_BUSY"            # Case-sensitive: exact match only
    - pattern: "(?i)temporary failure"  # Case-insensitive
```

**Pattern Syntax**:
- Regex patterns matched against original error message (case-sensitive by default)
- Use `(?i)` flag at start of pattern for case-insensitive matching
- Invalid regex patterns return false (no match)

**Source**: `src/cook/retry_v2.rs:142-148` (Pattern variant implementation)

**Use Case**: Matching application-specific error messages or database-specific errors.

**Case Sensitivity Examples**:
- `pattern: "SQLITE_BUSY"` - Only matches "SQLITE_BUSY" (not "sqlite_busy")
- `pattern: "(?i)SQLITE_BUSY"` - Matches "SQLITE_BUSY", "sqlite_busy", "Sqlite_Busy", etc.
- `pattern: "(?i)database.*locked"` - Case-insensitive regex with wildcards

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

### Regex Pattern Syntax and Case Sensitivity

Understanding case sensitivity is critical when using Pattern matchers:

| Matcher Type | Case Sensitivity | Normalization |
|-------------|------------------|---------------|
| Network | Case-insensitive | Error converted to lowercase |
| Timeout | Case-insensitive | Error converted to lowercase |
| ServerError | Case-insensitive | Error converted to lowercase |
| RateLimit | Case-insensitive | Error converted to lowercase |
| Pattern | **Case-sensitive by default** | No normalization (original case) |

**Making Pattern Matching Case-Insensitive**:

Use the `(?i)` flag at the start of your regex pattern:

```yaml
retry_on:
  # Case-insensitive patterns (recommended)
  - pattern: "(?i)connection refused"  # Matches any case variation
  - pattern: "(?i)database.*locked"    # Case-insensitive with wildcards

  # Case-sensitive patterns (use with caution)
  - pattern: "SQLITE_BUSY"             # Only matches exact case
  - pattern: "ERROR: Authentication"   # Must match exact case
```

**Invalid Regex Handling**:

If a pattern contains invalid regex syntax, it returns `false` (no match):

```yaml
retry_on:
  - pattern: "[invalid(regex"  # Invalid syntax → returns false → no retry
```

**Source**: src/cook/retry_v2.rs:142-148

### Advanced Pattern Matching

Use regex patterns for precise error matching:

```yaml
retry_config:
  attempts: 3
  retry_on:
    # Case-insensitive patterns (recommended for flexible matching)
    - pattern: "(?i)SQLite.*database is locked"
    - pattern: "(?i)deadlock detected"

    # Case-sensitive pattern (exact match required)
    - pattern: "SQLITE_BUSY"
```

**Pattern Matching Logic** (src/cook/retry_v2.rs:116-150):
1. Each matcher's `matches()` method is called with the error message
2. Built-in matchers normalize error to lowercase before checking
3. Pattern matcher applies regex to **original case** of error message
4. Invalid regex patterns return false (no match, no retry)
5. If any matcher returns true, error is retryable

### Implementation Details

The matching logic is implemented in `ErrorMatcher::matches()`:

```rust
// Simplified implementation (src/cook/retry_v2.rs:116-150)
impl ErrorMatcher {
    pub fn matches(&self, error_msg: &str) -> bool {
        let error_lower = error_msg.to_lowercase();
        match self {
            Self::Network => {
                // Case-insensitive matching via lowercase normalization
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
                // Case-sensitive by default - matches against original error_msg
                // Use (?i) flag in pattern for case-insensitive matching
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(error_msg)  // Uses original case, not error_lower
                } else {
                    false  // Invalid regex = no match
                }
            }
        }
    }
}
```

### Testing Error Matchers

The retry_v2 module includes tests for built-in matchers (src/cook/retry_v2.rs:463-502):

```rust
#[test]
fn test_error_matcher_network() {
    let matcher = ErrorMatcher::Network;
    assert!(matcher.matches("Connection refused"));
    assert!(matcher.matches("Network unreachable"));
    assert!(matcher.matches("connection timeout"));  // Case-insensitive
    assert!(!matcher.matches("Syntax error"));
}

#[test]
fn test_error_matcher_timeout() {
    let matcher = ErrorMatcher::Timeout;
    assert!(matcher.matches("Operation timeout"));
    assert!(matcher.matches("Request timed out"));
    assert!(!matcher.matches("Network error"));
}

#[test]
fn test_error_matcher_rate_limit() {
    let matcher = ErrorMatcher::RateLimit;
    assert!(matcher.matches("Rate limit exceeded"));
    assert!(matcher.matches("Error 429"));
    assert!(matcher.matches("Too many requests"));
    assert!(!matcher.matches("Server error"));
}
```

**Note**: Pattern matcher tests are not yet implemented in the test suite. The above tests cover Network, Timeout, and RateLimit matchers only.

### See Also

- [Basic Retry Configuration](./basic-retry-configuration.md) - Core retry configuration
- [Failure Actions](./failure-actions.md) - What happens when all retries are exhausted
- [Best Practices](./best-practices.md) - When to use selective retry
- [Complete Examples](./complete-examples.md) - Full workflow examples with error matchers
