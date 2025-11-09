## Error Collection Strategies

The `error_collection` field controls how errors are reported during workflow execution.

### Syntax Flexibility

Error collection can be configured in two ways for backward compatibility:

**Top-level convenience syntax** (recommended for simple workflows):
```yaml
name: my-workflow
mode: mapreduce

error_collection: aggregate  # Top-level field

map:
  # ... map configuration
```

**Nested under error_policy block** (recommended when using other error policy features):
```yaml
name: my-workflow
mode: mapreduce

error_policy:
  error_collection: aggregate
  continue_on_failure: true
  max_failures: 10
  # ... other error policy fields

map:
  # ... map configuration
```

Both syntaxes are fully supported. Use the top-level syntax for simplicity, or the nested syntax when configuring multiple error policy fields together.

### Available Strategies

**Aggregate (default)**:
```yaml
error_collection: aggregate
```
- Collects all errors in memory and reports at workflow end
- Errors are stored as they occur but not logged
- Full error list displayed when workflow completes
- Use for: Final summary reports, batch processing where individual failures don't need immediate attention
- Trade-off: Low noise, but you won't see errors until completion

**Immediate**:
```yaml
error_collection: immediate
```
- Logs each error as soon as it happens via `warn!` level logging
- No error collection in memory
- Errors visible in real-time during execution
- Use for: Debugging, development, real-time monitoring
- Trade-off: More verbose output, but immediate visibility

**Batched**:
```yaml
error_collection: batched:10
```
- Collects errors in memory until batch size is reached
- When N errors collected, logs the entire batch via `warn!` level logging
- Batch buffer is cleared after logging
- Use for: Progress updates without spam, monitoring long-running jobs
- Trade-off: Balance between noise and visibility (e.g., `batched:10` reports every 10 failures)

### Complete Example

Combining error collection with other error policy features:

```yaml
name: data-processing
mode: mapreduce

error_policy:
  # Report errors in batches of 5
  error_collection: batched:5

  # Send failed items to DLQ instead of failing workflow
  on_item_failure: dlq

  # Continue processing even if items fail
  continue_on_failure: true

  # Stop if failure rate exceeds 30%
  failure_threshold: 0.3

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process '${item}'"
```

**Note**: If `error_collection` is not specified, the default behavior is `aggregate`.

See also: [Error Handling](../error-handling.md), [Dead Letter Queue](dead-letter-queue-dlq.md), [Circuit Breaker Configuration](circuit-breaker-configuration.md)

