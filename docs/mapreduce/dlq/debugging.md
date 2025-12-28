## Debugging with DLQ

### Accessing Claude JSON Logs

Each `FailureDetail` includes a `json_log_location` field pointing to the Claude Code JSON log for that execution. This log contains:
- Complete conversation history
- All tool invocations and results
- Error details and stack traces
- Token usage statistics

```bash
# View JSON log from DLQ item
cat $(prodigy dlq inspect item-123 | jq -r '.failure_history[0].json_log_location')

# Pretty-print with jq
cat /path/to/session.json | jq '.'
```

For more details on Claude JSON logs, see [Retry Metrics and Observability](../../retry-configuration/retry-metrics-and-observability.md).

### Common Debugging Workflow

!!! example "Step-by-Step Debugging"
    Follow this workflow to diagnose and fix failures systematically:

1. **List failed items**:
   ```bash
   prodigy dlq list --job-id mapreduce-1234567890
   ```

2. **Inspect specific failure**:
   ```bash
   prodigy dlq inspect item-123
   ```

3. **Examine Claude logs**:
   ```bash
   cat /path/to/claude-session.json | jq '.messages[-3:]'
   ```

4. **Analyze failure patterns**:
   ```bash
   prodigy dlq analyze --job-id mapreduce-1234567890
   ```

5. **Fix underlying issue** (code bug, config error, etc.)

6. **Retry failed items**:
   ```bash
   prodigy dlq retry mapreduce-1234567890
   ```

## Integration with MapReduce

The DLQ is tightly integrated with MapReduce workflows through the `on_item_failure` policy:

```yaml
# Source: Workflow configuration (see src/config/mapreduce.rs for MapConfig schema)
name: my-workflow
mode: mapreduce

map:
  input: "items.json"
  json_path: "$.items[*]"

  # Default policy: send failures to DLQ
  on_item_failure: dlq

  agent_template:
    - claude: "/process '${item}'"
```

### Available Policies

- **`dlq`** (default): Failed items sent to DLQ, job continues
- **`retry`**: Immediate retry with exponential backoff
- **`skip`**: Ignore failures, mark as skipped, continue
- **`stop`**: Halt entire workflow on first failure
- **`custom`**: User-defined failure handler

### Failure Flow

```
Work Item Processing
       |
   Command Failed
       |
  Retry Attempts (if configured)
       |
   Still Failing?
       |
on_item_failure: dlq
       |
Create DeadLetteredItem
       |
Save to ~/.prodigy/dlq/{repo}/{job_id}/mapreduce/dlq/{job_id}/items/{item_id}.json
       |
Continue Processing Other Items
```
