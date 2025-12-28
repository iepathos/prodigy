## DLQ Commands

!!! warning "Planned Feature - Stub Implementation"
    The DLQ CLI commands are currently implemented as stubs that only print status messages. Full functionality is planned for a future release.

    **Current Status**:

    - Command definitions exist in `src/cli/args.rs` (DlqCommands enum at lines 534-631)
    - Stub implementations in `src/cli/commands/dlq.rs`
    - Commands will print confirmation messages but do not execute actual operations

    See [DLQ Integration](debugging.md#integration-with-mapreduce) for current workflow-level DLQ functionality.

Prodigy provides comprehensive CLI commands for managing the DLQ. The following sections describe the planned command interface.

!!! info "Related Documentation"
    - [DLQ Overview](overview.md) - Core concepts and architecture
    - [DLQ Internals](internals.md) - Implementation details
    - [Debugging Guide](debugging.md) - Troubleshooting failed items

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

!!! tip "Quick Filtering"
    Use `--eligible` to focus on items that can be automatically retried, filtering out those requiring manual intervention.

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

!!! note "Planned Feature"
    This command is not yet implemented. Description below shows planned interface.

Reprocess failed items:

```bash
# Source: src/cli/args.rs:595-615
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

| Parameter | Description | Default |
|-----------|-------------|---------|
| `<workflow_id>` | Workflow/job identifier to retry items from | Required |
| `--parallel` | Number of concurrent retry workers | 10 |
| `--max-retries` | Maximum retry attempts per item | 3 |
| `--filter` | Expression to filter which items to retry | All eligible |
| `--force` | Force retry of items marked as not eligible | false |

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

!!! abstract "Implementation Details"
    The DLQ reprocessing logic uses the `DeadLetterQueue::reprocess` method which filters for `reprocess_eligible` items before attempting retry. See [DLQ Internals](internals.md) for more details.

### Export Command

Export DLQ items for external analysis:

=== "JSON Format"

    ```bash
    # Export to JSON (default)
    prodigy dlq export dlq-items.json

    # Export specific job
    prodigy dlq export output.json --job-id mapreduce-1234567890
    ```

=== "CSV Format"

    ```bash
    # Export to CSV
    prodigy dlq export dlq-items.csv --format csv
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
