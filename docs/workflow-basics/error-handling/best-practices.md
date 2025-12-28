# Best Practices

## Choosing the Right Error Handling Level

Understanding when to use command-level versus workflow-level error handling is crucial for building robust workflows.

!!! note "Two Levels of Error Handling"
    Prodigy provides error handling at two distinct levels, and you can use both in the same workflow for defense in depth. Command-level handlers respond to specific step failures, while workflow-level policies apply consistent rules across all MapReduce items.

| **Aspect** | **Command-Level (`on_failure`)** | **Workflow-Level (`error_policy`)** |
|------------|-----------------------------------|--------------------------------------|
| **Scope** | Single workflow step | Entire MapReduce job |
| **Availability** | All workflow modes | MapReduce mode only |
| **Use Case** | Step-specific recovery logic | Consistent handling across all items |
| **Retry Control** | Per-command retry with `max_attempts` | Per-item retry with backoff strategies |
| **Failure Action** | Custom handler commands | DLQ, retry, skip, or stop |
| **Circuit Breaker** | Not available | Available with configurable thresholds |
| **Best For** | Targeted recovery, cleanup, notifications | Batch processing, rate limiting, cascading failure prevention |

## When to Use Command-Level Error Handling

- **Recovery:** Use `on_failure` to fix issues and retry (e.g., clearing cache before reinstalling)
- **Cleanup:** Use `strategy: cleanup` to clean up resources after failures
- **Fallback:** Use `strategy: fallback` for alternative approaches
- **Notifications:** Use handler commands to notify teams of failures
- **Step-Specific Logic:** When different steps need different error handling strategies

## When to Use Workflow-Level Error Policy

- **MapReduce jobs:** Use error_policy for consistent failure handling across all work items
- **Failure thresholds:** Use max_failures or failure_threshold to prevent runaway jobs
- **Circuit breakers:** Use when external dependencies might fail cascading
- **DLQ:** Use for large batch jobs where you want to retry failures separately
- **Rate Limiting:** Use backoff strategies to avoid overwhelming external services
- **Batch Processing:** When processing hundreds or thousands of items with similar error patterns

## Error Information Available

When a command fails, you can access error information in handler commands:

```yaml
- shell: "risky-command"
  on_failure:
    claude: "/analyze-error ${shell.output}"
```

The `${shell.output}` variable contains the command's stdout/stderr output.

## Common Patterns

!!! example "Cleanup and Retry"
    This pattern is useful when failures are caused by corrupted caches or stale state:

```yaml
- shell: "npm install"
  on_failure:
    - "npm cache clean --force"    # (1)!
    - "rm -rf node_modules"         # (2)!
    - "npm install"                 # (3)!

1. Clean npm cache to remove corrupted entries
2. Remove node_modules to ensure clean state
3. Retry installation from scratch
```

**Conditional Recovery:**
```yaml
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
  max_attempts: 3
  fail_workflow: false
```

**Critical Step with Notification:**
```yaml
- shell: "deploy-production"
  on_failure:
    commands:
      - shell: "rollback-deployment"
      - shell: "notify-team 'Deployment failed'"
    fail_workflow: true   # Still fail workflow after cleanup
```

**Resilient API Integration:**
```yaml
error_policy:
  retry_config:
    max_attempts: 5
    backoff:
      exponential:
        initial: "2s"
        multiplier: 2.0
        max_delay: "60s"
    jitter: true

  circuit_breaker:
    failure_threshold: 3
    success_threshold: 2
    timeout: "30s"

- shell: "curl -f https://api.example.com/data"
  on_failure:
    shell: "echo 'API unavailable, will retry with backoff'"
```

**MapReduce with DLQ:**
```yaml
mode: mapreduce

error_policy:
  on_item_failure: dlq
  max_failures: 10
  failure_threshold: 0.1  # Stop at 10% failure rate

map:
  input: "items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/process ${item}"
      timeout: 300
      on_failure:
        shell: "echo 'Item ${item.id} failed, sent to DLQ'"
```

**Progressive Error Handling:**
```yaml
- shell: "cargo test"
  on_failure:
    - claude: "/analyze-test-failure ${shell.stderr}"
    - shell: "cargo clean"
    - shell: "cargo test"  # Retry after clean
      on_failure:
        - claude: "/deep-analysis ${shell.stderr}"
        - shell: "notify-team.sh 'Tests still failing after retry'"
```

**Combined Error Handling Strategies (MapReduce):**

For complex MapReduce workflows, combine multiple error handling features:

!!! example "Defense in Depth: Multi-Layer Error Handling"
    This example demonstrates how to combine command-level and workflow-level error handling for maximum resilience:

```yaml
# Process API endpoints with comprehensive error handling
mode: mapreduce
error_policy:
  on_item_failure: retry          # (1)!
  continue_on_failure: true       # (2)!
  max_failures: 50                # (3)!
  failure_threshold: 0.15         # (4)!

  # Retry with exponential backoff
  retry_config:
    max_attempts: 3               # (5)!
    backoff:
      type: exponential
      initial: 2s
      multiplier: 2               # (6)!

  # Protect against cascading failures
  circuit_breaker:
    failure_threshold: 10         # (7)!
    success_threshold: 3
    timeout: 60s
    half_open_requests: 5

  # Report errors in batches of 10
  error_collection:
    batched:
      size: 10                    # (8)!

map:
  agent_template:
    - claude: "/process-endpoint ${item.path}"
      on_failure:
        # Item-level recovery before workflow-level retry
        claude: "/diagnose-api-error ${shell.output}"
        max_attempts: 2           # (9)!

1. Try immediate retry first (workflow-level)
2. Don't stop entire job on failures
3. Stop if 50 total failures occur
4. Stop if 15% of items fail (runaway protection)
5. Retry each failed item up to 3 times
6. Delays: 2s, 4s, 8s between retries
7. Open circuit after 10 consecutive failures
8. Report errors in batches to reduce noise
9. Item-level handler can retry twice before workflow-level retry
```

This configuration provides multiple layers of protection:
1. Item-level error handlers for immediate recovery attempts
2. Automatic retry with exponential backoff for transient failures
3. Circuit breaker to prevent overwhelming failing dependencies
4. Failure thresholds to stop runaway jobs early
5. Batched error reporting to reduce noise
