## DLQ Commands

!!! warning "Planned Feature - Stub Implementation"
    The DLQ CLI commands are currently implemented as stubs that only print status messages. Full functionality is planned for a future release.

    **Current Status**:

    - Command definitions exist in src/cli/args.rs:577-677
    - Stub implementations in src/cli/commands/dlq.rs:1-74
    - Commands will print confirmation messages but do not execute actual operations

    See [DLQ Integration](debugging.md#integration-with-mapreduce) for current workflow-level DLQ functionality.

Prodigy provides comprehensive CLI commands for managing the DLQ. The following sections describe the planned command interface.

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

> **Warning**: This command is not yet implemented. Description below shows planned interface.

Reprocess failed items:

```bash
# Source: src/cli/args.rs:640-659
# Retry all failed items for a workflow
prodigy dlq retry <workflow_id>

# Control parallelism (default: 10)
prodigy dlq retry <workflow_id> --parallel 10

# Filter items to retry
prodigy dlq retry <workflow_id> --filter 'item.priority >= 5'

# Limit retry attempts per item (default: 3)
prodigy dlq retry <workflow_id> --max-retries 2

# Force retry even if not eligible
prodigy dlq retry <workflow_id> --force
```

**Command Parameters**:
- `<workflow_id>`: The workflow/job identifier to retry items from (e.g., "mapreduce-1234567890")
- `--parallel`: Number of concurrent retry workers (default: 10, src/cli/args.rs:653)
- `--max-retries`: Maximum retry attempts per item (default: 3, src/cli/args.rs:649)
- `--filter`: Expression to filter which items to retry (default: all eligible items, src/cli/args.rs:645)
- `--force`: Force retry of items marked as not eligible (default: false, src/cli/args.rs:657)

**Supported Flags**: `--parallel`, `--max-retries`, `--filter`, `--force`

#### Retry Behavior

The retry functionality is designed to handle large-scale DLQ reprocessing:

- **Streaming support**: Items are processed incrementally to avoid memory issues with large DLQs
- **Parallel execution**: Uses `--parallel` flag (default: 10) to control concurrent workers
- **State updates**:
  - Successful items are removed from DLQ
  - Failed items remain with updated `failure_history`
  - `failure_count` is incremented
- **Correlation tracking**: Maintains original item IDs and correlation metadata
- **Interruption safe**: Supports stopping and resuming retry operations

**Implementation Note**: The DLQ reprocessing logic uses the `DeadLetterQueue::reprocess` method (src/cook/execution/dlq.rs:202-233) which filters for `reprocess_eligible` items before attempting retry.

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
