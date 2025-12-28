# Session & DLQ Storage

This page covers session tracking and dead letter queue storage for failed work items.

## Session Storage

Sessions are stored in a flat directory:

```
~/.prodigy/sessions/
├── session-abc123.json
├── session-mapreduce-xyz.json
└── session-def456.json
```

### Session File Format

```json
// Source: src/storage/types.rs
{
  "id": "session-abc123",
  "session_type": "Workflow",
  "status": "Running",
  "started_at": "2025-01-11T12:00:00Z",
  "metadata": {...},
  "checkpoints": [...],
  "timings": {...}
}
```

## Dead Letter Queue Storage

Failed work items are stored per job:

```
~/.prodigy/dlq/{repo_name}/{job_id}/dlq-items.json
```

### DLQ Item Format

```json
{
  "item_id": "item-1",
  "item_data": {...},
  "failure_history": [
    {
      "timestamp": "2025-01-11T12:00:00Z",
      "error": "Timeout after 300 seconds",
      "json_log_location": "/path/to/logs/session-xyz.json",
      "retry_count": 0
    }
  ],
  "last_failure": "2025-01-11T12:00:00Z"
}
```

### DLQ Features

- **Cross-worktree tracking**: Shared across parallel worktrees
- **Failure history**: Track all retry attempts
- **Log linkage**: JSON log location for debugging
- **Retry support**: Command to reprocess failed items
