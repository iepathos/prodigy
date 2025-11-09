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
