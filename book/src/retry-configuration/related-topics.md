## Related Topics

This section provides links to related documentation that complements retry configuration. Understanding these topics will help you build more resilient workflows.

### Within This Chapter

The following subsections provide detailed information about specific aspects of retry configuration:

- [Basic Retry Configuration](./basic-retry-configuration.md) - Start here to understand fundamental retry configuration syntax and options
- [Backoff Strategies](./backoff-strategies.md) - Control the timing between retry attempts using exponential, linear, Fibonacci, or constant delays
- [Backoff Strategy Comparison](./backoff-strategy-comparison.md) - Compare different backoff strategies with examples and use cases
- [Failure Actions](./failure-actions.md) - Define custom actions to execute when commands fail or exhaust retries
- [Conditional Retry with Error Matchers](./conditional-retry-with-error-matchers.md) - Use error patterns to selectively retry only specific failures
- [Jitter for Distributed Systems](./jitter-for-distributed-systems.md) - Add randomization to retry delays to prevent thundering herd problems
- [Retry Budget](./retry-budget.md) - Limit total retry attempts across your workflow to prevent infinite retry loops
- [Retry Metrics and Observability](./retry-metrics-and-observability.md) - Monitor retry behavior through events and logging
- [Workflow-Level vs Command-Level Retry](./workflow-level-vs-command-level-retry.md) - Understand the differences between retry scopes and when to use each
- [Best Practices](./best-practices.md) - Recommended patterns and anti-patterns for retry configuration
- [Complete Examples](./complete-examples.md) - Real-world retry configuration examples demonstrating various strategies
- [Troubleshooting](./troubleshooting.md) - Debug common retry configuration issues
- [Implementation References](./implementation-references.md) - Links to source code implementing retry logic

### Related Chapters

These chapters cover topics that interact with or complement retry configuration:

- [Error Handling](../error-handling.md) - Overall error handling strategy and how Prodigy propagates errors through workflows. Retry configuration is one component of a comprehensive error handling approach.
- [Workflow Configuration](../configuration/workflow-configuration.md) - Workflow-level settings including global retry defaults that apply to all commands unless overridden at the command level.
- [MapReduce](../mapreduce/index.md) - Retry behavior in MapReduce workflows, where individual map agents can retry independently. MapReduce adds complexity to retry semantics due to parallel execution.
- [Dead Letter Queue (DLQ)](../mapreduce/dead-letter-queue-dlq.md) - Handling failed work items in MapReduce workflows. When map agents exhaust all retries, items move to the DLQ for manual inspection and retry.
- [Environment Variables](../environment/index.md) - Use environment variables in retry configuration to parameterize retry behavior across different deployment environments (dev, staging, production).

