# Retry Configuration

Prodigy provides sophisticated retry mechanisms with multiple backoff strategies to handle transient failures gracefully. The retry system supports both command-level and workflow-level configurations with fine-grained control over retry behavior.

## Overview

Prodigy has two retry systems that work together:

1. **Enhanced Retry System** - Rich, configurable retry with multiple backoff strategies, jitter, circuit breakers, and conditional retry (from `src/cook/retry_v2.rs`)
2. **Workflow-Level Retry** - Simpler retry configuration for workflow-level error policies (from `src/cook/workflow/error_policy.rs`)

This chapter focuses on the enhanced retry system which provides comprehensive retry capabilities. Circuit breakers prevent cascading failures by temporarily stopping retries when a threshold of consecutive failures is reached.

## RetryConfig Structure

This table documents the enhanced retry system (`retry_v2::RetryConfig`). For workflow-level retry configuration, see the [Workflow-Level vs Command-Level Retry](workflow-level-vs-command-level-retry.md) subsection.

The `RetryConfig` struct controls retry behavior with the following fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `attempts` | `u32` | `3` | Maximum number of retry attempts |
| `backoff` | `BackoffStrategy` | `Exponential (base: 2.0)` | Strategy for calculating delays between retries |
| `initial_delay` | `Duration` | `1s` | Initial delay before first retry |
| `max_delay` | `Duration` | `30s` | Maximum delay between any two retries |
| `jitter` | `bool` | `false` | Whether to add randomness to delays |
| `jitter_factor` | `f64` | `0.3` | Amount of jitter (0.0 to 1.0) |
| `retry_on` | `Vec<ErrorMatcher>` | `[]` | Retry only on specific error types (empty = retry all) |
| `retry_budget` | `Option<Duration>` | `None` | Maximum total time for all retry attempts |
| `on_failure` | `FailureAction` | `Stop` | Action to take after all retries exhausted |

**Source**: RetryConfig struct defined in `src/cook/retry_v2.rs:14-52`

### Circuit Breakers

Circuit breakers are configured separately via `RetryExecutor`, **not as part of RetryConfig**. Circuit breakers provide fail-fast behavior when downstream systems are consistently failing, preventing resource exhaustion from repeated failed retries.

**Configuration** (programmatic):
```rust
let executor = RetryExecutor::new(retry_config)
    .with_circuit_breaker(
        5,                          // failure_threshold: open after 5 consecutive failures
        Duration::from_secs(30)     // recovery_timeout: attempt recovery after 30 seconds
    );
```

**Source**: `src/cook/retry_v2.rs:184-188` (with_circuit_breaker method), `src/cook/retry_v2.rs:325-397` (CircuitBreaker implementation)

**Circuit States**:
- **Closed**: Normal operation, retries are attempted
- **Open**: Circuit tripped, requests fail immediately without retry
- **HalfOpen**: Testing recovery, limited requests allowed

See [Best Practices](best-practices.md) for guidance on combining retry with circuit breakers for high-reliability systems.

## Additional Topics

See also:
- [Basic Retry Configuration](basic-retry-configuration.md)
- [Backoff Strategies](backoff-strategies.md)
- [Backoff Strategy Comparison](backoff-strategy-comparison.md)
- [Jitter for Distributed Systems](jitter-for-distributed-systems.md)
- [Conditional Retry with Error Matchers](conditional-retry-with-error-matchers.md)
- [Retry Budget](retry-budget.md)
- [Failure Actions](failure-actions.md)
- [Complete Examples](complete-examples.md)
- [Workflow-Level vs Command-Level Retry](workflow-level-vs-command-level-retry.md)
- [Retry Metrics and Observability](retry-metrics-and-observability.md)
- [Best Practices](best-practices.md)
- [Troubleshooting](troubleshooting.md)
- [Related Topics](related-topics.md)
- [Implementation References](implementation-references.md)
