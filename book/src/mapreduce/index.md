# MapReduce Workflows

## Quick Start

Want to get started with MapReduce? Here's a minimal working example:

```yaml
name: my-first-mapreduce
mode: mapreduce

# Generate work items
setup:
  - shell: "echo '[{\"id\": 1, \"name\": \"task-1\"}, {\"id\": 2, \"name\": \"task-2\"}]' > items.json"

# Process items in parallel
map:
  input: "items.json"
  json_path: "$[*]"
  agent_template:
    - shell: "echo Processing ${item.name}"
  max_parallel: 5

# Aggregate results
reduce:
  - shell: "echo Completed ${map.successful}/${map.total} items"
```

Run it:
```bash
prodigy run workflow.yml
```

That's it! Now let's explore the full capabilities.

## Complete Structure

```yaml
name: parallel-processing
mode: mapreduce

# Optional setup phase
setup:
  - shell: "generate-work-items.sh"
  - shell: "debtmap analyze . --output items.json"

# Map phase: Process items in parallel
map:
  # Input source (JSON file or command)
  input: "items.json"

  # JSONPath expression to extract items
  json_path: "$.items[*]"

  # Agent template (commands run for each item)
  # Modern syntax: Commands directly under agent_template
  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

  # DEPRECATED: Nested 'commands' syntax (still supported)
  # agent_template:
  #   commands:
  #     - claude: "/process '${item}'"

  # Maximum parallel agents (can use environment variables)
  max_parallel: 10  # or max_parallel: "$MAX_WORKERS"

  # Optional: Filter items
  filter: "item.score >= 5"

  # Optional: Sort items
  sort_by: "item.priority DESC"

  # Optional: Limit number of items
  max_items: 100

  # Optional: Skip items
  offset: 10

  # Optional: Deduplicate by field
  distinct: "item.id"

  # Optional: Agent timeout in seconds
  agent_timeout_secs: 300

  # Optional: Advanced timeout configuration (alternative to agent_timeout_secs)
  # timeout_config:
  #   default: "5m"
  #   per_command: "2m"

# Reduce phase: Aggregate results
# Modern syntax: Commands directly under reduce
reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"

# DEPRECATED: Nested 'commands' syntax (still supported)
# reduce:
#   commands:
#     - claude: "/summarize ${map.results}"

# Optional: Custom merge workflow (supports two formats)
merge:
  # Simple array format
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  - shell: "cargo test"

# OR full format with timeout
# merge:
#   commands:
#     - shell: "git fetch origin"
#     - claude: "/merge-worktree ${merge.source_branch}"
#   timeout: 600  # Timeout in seconds

# Error handling policy
error_policy:
  on_item_failure: dlq  # dlq, retry, skip, stop, or custom handler name
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.2  # 20% failure rate
  error_collection: aggregate  # aggregate, immediate, or batched:N

  # Circuit breaker configuration
  circuit_breaker:
    failure_threshold: 5      # Open circuit after N failures
    success_threshold: 2      # Close circuit after N successes
    timeout: "60s"           # Duration before attempting half-open (humantime format: "1s", "1m", "5m")
    half_open_requests: 3    # Test requests in half-open state

  # Retry configuration with backoff
  retry_config:
    max_attempts: 3
    # BackoffStrategy is an enum - use one variant:
    backoff:
      exponential:
        initial: "1s"
        multiplier: 2.0

# Convenience fields (alternative to nested error_policy)
# These top-level fields map to error_policy for simpler syntax
on_item_failure: dlq
continue_on_failure: true
max_failures: 5
```


## Additional Topics

See also:
- [Environment Variables in Configuration](environment-variables-in-configuration.md)
- [Backoff Strategies](backoff-strategies.md)
- [Error Collection Strategies](error-collection-strategies.md)
- [Setup Phase (Advanced)](setup-phase-advanced.md)
- [Global Storage Architecture](global-storage-architecture.md)
- [Event Tracking](event-tracking.md)
- [Checkpoint and Resume](checkpoint-and-resume.md)
- [Dead Letter Queue (DLQ)](dead-letter-queue-dlq.md)
- [Common Pitfalls](common-pitfalls.md)
- [Troubleshooting](troubleshooting.md)
- [Performance Tuning](performance-tuning.md)
- [Real-World Use Cases](real-world-use-cases.md)
- [Workflow Format Comparison](workflow-format-comparison.md)
