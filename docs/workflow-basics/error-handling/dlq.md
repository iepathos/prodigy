# Dead Letter Queue (DLQ)

The Dead Letter Queue stores failed work items from MapReduce jobs for later retry or analysis. This is only available for MapReduce workflows, not regular workflows.

## Sending Items to DLQ

Configure your MapReduce workflow to use DLQ:

```yaml
mode: mapreduce
error_policy:
  on_item_failure: dlq
```

Failed items are automatically sent to the DLQ with:
- Original work item data
- Failure reason and error message
- Timestamp of failure
- Attempt history
- JSON log location for debugging

## Retrying Failed Items

Use the CLI to retry failed items:

```bash
# Retry all failed items for a job
prodigy dlq retry <job_id>

# Retry with custom parallelism (default: 5)
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run
```

**DLQ Retry Features:**
- Streams items to avoid memory issues with large queues
- Respects original workflow's max_parallel setting (unless overridden)
- Preserves correlation IDs for tracking
- Updates DLQ state (removes successful, keeps failed)
- Supports interruption and resumption
- Shared across worktrees for centralized failure tracking

## View DLQ Contents

```bash
# Show failed items
prodigy dlq show <job_id>

# Get JSON format
prodigy dlq show <job_id> --format json
```

## DLQ Storage

DLQ data is stored in:
```
~/.prodigy/dlq/{repo_name}/{job_id}/
```

This centralized storage allows multiple worktrees to share the same DLQ.
