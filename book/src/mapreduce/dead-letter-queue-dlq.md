## Dead Letter Queue (DLQ)

The Dead Letter Queue (DLQ) captures persistently failing work items for analysis and retry while allowing MapReduce jobs to continue processing other items. Instead of blocking the entire workflow when individual items fail, the DLQ provides fault tolerance and enables debugging of failure patterns.

### Overview

When a map agent fails to process a work item after exhausting retry attempts, the item is automatically sent to the DLQ. This allows the MapReduce job to complete successfully while preserving all failure information for later investigation and reprocessing.

The DLQ integrates with MapReduce through the `on_item_failure` policy, which defaults to `dlq` for MapReduce workflows. Alternative policies include `retry` (immediate retry), `skip` (ignore failures), `stop` (halt workflow), and `custom` (user-defined handling).

### Storage Structure

DLQ data is stored in the global Prodigy directory using this structure:

```
~/.prodigy/dlq/{repo_name}/{job_id}/dlq.json
```

For example:
```
~/.prodigy/dlq/prodigy/mapreduce-1234567890/dlq.json
```

This global storage architecture enables:
- **Cross-worktree access**: Multiple worktrees working on the same job share DLQ data
- **Persistent state**: DLQ survives worktree cleanup
- **Centralized monitoring**: All failures accessible from a single location

### DLQ Item Structure

Each failed item in the DLQ is stored as a `DeadLetteredItem` with comprehensive failure information:

```json
{
  "item_id": "item-123",
  "item_data": { "file": "src/main.rs", "priority": 5 },
  "failure_history": [
    {
      "attempt_number": 1,
      "timestamp": "2025-01-11T10:30:00Z",
      "error_type": "CommandFailed",
      "error_message": "cargo test failed with exit code 101",
      "stack_trace": "...",
      "agent_id": "agent-1",
      "step_failed": 2,
      "duration_ms": 45000,
      "json_log_location": "/path/to/claude-session.json"
    }
  ],
  "failure_reason": "Command execution failed after 3 attempts",
  "timestamp": "2025-01-11T10:30:00Z",
  "retry_count": 3,
  "json_log_location": "/path/to/claude-session.json",
  "reprocess_eligible": true,
  "manual_review_required": false,
  "worktree_artifacts": {
    "worktree_path": "/path/to/worktree",
    "branch_name": "agent-1",
    "uncommitted_changes": ["src/main.rs"],
    "error_logs": ["error.log"]
  }
}
```

#### Key Fields

- `item_id`: Unique identifier for the work item
- `item_data`: Original work item data from input JSON
- `failure_history`: Array of `FailureDetail` objects capturing each attempt
- `failure_reason`: High-level summary of why item failed
- `timestamp`: When item was added to DLQ
- `retry_count`: Number of failed attempts
- `json_log_location`: Path to Claude JSON log for debugging (see [Claude Command Observability](../debugging/claude-command-observability.md))
- `reprocess_eligible`: Whether item can be retried automatically
- `manual_review_required`: Whether item needs human intervention
- `worktree_artifacts`: Captured state from failed agent's worktree

### Failure Detail Fields

Each attempt in `failure_history` is a `FailureDetail` object:

```json
{
  "attempt_number": 1,
  "timestamp": "2025-01-11T10:30:00Z",
  "error_type": "CommandFailed",
  "error_message": "cargo test failed with exit code 101",
  "stack_trace": "thread 'main' panicked at src/main.rs:42...",
  "agent_id": "agent-1",
  "step_failed": 2,
  "duration_ms": 45000,
  "json_log_location": "/path/to/claude-session.json"
}
```

#### Error Types

The `error_type` field uses the `ErrorType` enum:

- `Timeout`: Agent execution exceeded timeout
- `CommandFailed`: Shell or Claude command returned non-zero exit code
- `WorktreeError`: Git worktree operation failed
- `MergeConflict`: Merge back to parent worktree failed
- `ValidationFailed`: Validation command failed
- `ResourceExhausted`: System resources (memory, disk) exhausted
- `Unknown`: Unclassified error

### Worktree Artifacts

The `WorktreeArtifacts` structure captures the agent's execution environment:

- `worktree_path`: Path to agent's isolated worktree
- `branch_name`: Git branch created for agent
- `uncommitted_changes`: Files modified but not committed
- `error_logs`: Captured error output files

These artifacts are preserved for debugging and can be accessed after failure.

## DLQ Commands

Prodigy provides comprehensive CLI commands for managing the DLQ:

### List Command

View DLQ items across jobs:

```bash
# List all DLQ items
prodigy dlq list

# List items for specific job
prodigy dlq list --job-id mapreduce-1234567890

# List only retry-eligible items
prodigy dlq list --eligible

# Limit results
prodigy dlq list --limit 10
```

### Inspect Command

Examine detailed information for a specific item:

```bash
# Inspect item by ID
prodigy dlq inspect item-123

# Inspect item in specific job
prodigy dlq inspect item-123 --job-id mapreduce-1234567890
```

Output includes:
- Full item data
- Complete failure history with all attempts
- Error messages and stack traces
- JSON log locations for debugging
- Worktree artifacts

### Analyze Command

Analyze failure patterns across items:

```bash
# Analyze all failures
prodigy dlq analyze

# Analyze specific job
prodigy dlq analyze --job-id mapreduce-1234567890

# Export analysis results
prodigy dlq analyze --export analysis.json
```

Analysis output includes:
- `pattern_groups`: Failures grouped by error type and message similarity
- `error_distribution`: Histogram of error types
- `temporal_distribution`: Failures over time

### Retry Command

Reprocess failed items:

```bash
# Retry all failed items for a job
prodigy dlq retry mapreduce-1234567890

# Control parallelism (default: 5, uses workflow's max_parallel)
prodigy dlq retry mapreduce-1234567890 --max-parallel 10

# Filter items to retry
prodigy dlq retry mapreduce-1234567890 --filter 'item.priority >= 5'

# Limit retry attempts per item
prodigy dlq retry mapreduce-1234567890 --max-retries 2

# Force retry without confirmation
prodigy dlq retry mapreduce-1234567890 --force
```

#### Retry Behavior

- **Streaming support**: Items are processed incrementally to avoid memory issues with large DLQs
- **Parallel execution**: Respects workflow's `max_parallel` setting or custom `--max-parallel` value
- **State updates**:
  - Successful items are removed from DLQ
  - Failed items remain with updated `failure_history`
  - `retry_count` is incremented
- **Correlation tracking**: Maintains original item IDs and correlation metadata
- **Interruption safe**: Supports stopping and resuming retry operations

### Export Command

Export DLQ items for external analysis:

```bash
# Export to JSON (default)
prodigy dlq export dlq-items.json

# Export to CSV
prodigy dlq export dlq-items.csv --format csv

# Export specific job
prodigy dlq export output.json --job-id mapreduce-1234567890
```

### Stats Command

View DLQ statistics:

```bash
# Overall DLQ statistics
prodigy dlq stats

# Stats for specific workflow
prodigy dlq stats --workflow-id my-workflow
```

Output includes:
- Total items in DLQ
- Items by error type
- Average retry count
- Retry-eligible vs manual review required
- Temporal distribution

### Clear Command

Remove items from DLQ:

```bash
# Clear all items for a workflow (prompts for confirmation)
prodigy dlq clear my-workflow

# Skip confirmation prompt
prodigy dlq clear my-workflow --yes
```

### Purge Command

Clean up old DLQ items:

```bash
# Purge items older than 30 days
prodigy dlq purge --older-than-days 30

# Purge for specific job
prodigy dlq purge --older-than-days 30 --job-id mapreduce-1234567890

# Skip confirmation
prodigy dlq purge --older-than-days 30 --yes
```

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

For more details on Claude JSON logs, see [Claude Command Observability](../debugging/claude-command-observability.md).

### Common Debugging Workflow

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
       ↓
   Command Failed
       ↓
  Retry Attempts (if configured)
       ↓
   Still Failing?
       ↓
on_item_failure: dlq
       ↓
Create DeadLetteredItem
       ↓
Save to ~/.prodigy/dlq/{repo}/{job_id}/dlq.json
       ↓
Continue Processing Other Items
```

## Best Practices

### When to Retry vs Manual Fix

**Automatic Retry** (via `prodigy dlq retry`):
- Transient failures (network timeouts, resource contention)
- Flaky tests or intermittent issues
- Items that may succeed with more resources (`--max-parallel 1`)

**Manual Fix** (code changes, then retry):
- Logic errors in processing code
- Invalid assumptions about item data
- Missing dependencies or configuration
- Systematic failures affecting multiple items

### DLQ Management

1. **Monitor regularly**: Use `prodigy dlq stats` to track failure rates
2. **Analyze patterns**: Run `prodigy dlq analyze` to identify systematic issues
3. **Clean up old items**: Periodically run `prodigy dlq purge` to remove resolved failures
4. **Set review flags**: Mark `manual_review_required: true` for items needing human investigation

### Performance Considerations

- **Large DLQs**: Retry command uses streaming to handle thousands of items efficiently
- **Parallelism**: Tune `--max-parallel` based on failure type (CPU-bound vs I/O-bound)
- **Filtering**: Use `--filter` to target specific subsets of failures

## Cross-References

- [Checkpoint and Resume](./checkpoint-and-resume.md): DLQ state preserved in checkpoints
- [Event Tracking](./event-tracking.md): DLQ operations emit trackable events
- [Error Handling](./error-handling.md): Broader error handling strategies
- [Worktree Architecture](./worktree-architecture.md): Agent isolation and artifact preservation
