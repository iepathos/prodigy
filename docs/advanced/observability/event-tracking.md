# Event Tracking

All workflow operations are logged to JSONL event files:

```
~/.prodigy/events/{repo_name}/{job_id}/
└── events-{timestamp}.jsonl
```

!!! tip "Event Storage Best Practice"
    Events are stored globally in `~/.prodigy/events/` to enable cross-worktree aggregation. Multiple worktrees working on the same job share the same event log, making it easy to monitor parallel execution.

## Agent Events

Agent events track the lifecycle of individual work item processing.

**agent_started** - Agent execution begins:
```json
// Source: src/cook/execution/events/event_types.rs:41-47
{
  "event_type": "agent_started",  // (1)!
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "item_id": "item-1",
  "worktree": "agent-1-worktree",  // (2)!
  "attempt": 1  // (3)!
}
```

1. Event types use `event_type` field with snake_case values
2. Worktree path where agent executes
3. Attempt number (1 for first try, increments on retry)

**agent_completed** - Agent finishes successfully:
```json
// Source: src/cook/execution/mapreduce/event.rs:139-147
{
  "event_type": "agent_completed",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "item_id": "item-1",  // (1)!
  "duration": {"secs": 30, "nanos": 0},
  "timestamp": "2025-01-11T12:00:30Z",
  "cleanup_status": "success",  // (2)!
  "commits": ["abc123", "def456"],
  "json_log_location": "/path/to/logs/session-xyz.json"  // (3)!
}
```

1. ID of the work item that was processed
2. Worktree cleanup status: `"success"`, `"failed"`, or `null`
3. Path to Claude's detailed JSON log for debugging

**agent_failed** - Agent encounters errors:
```json
// Source: src/cook/execution/mapreduce/event.rs:149-156
{
  "event_type": "agent_failed",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "item_id": "item-1",
  "error": "Timeout after 300 seconds",
  "timestamp": "2025-01-11T12:05:00Z",
  "failure_reason": "Timeout",  // (1)!
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

1. Failure reason enum - see [Failure Reasons](#failure-reasons) below

**agent_retrying** - Agent retry attempt:
```json
// Source: src/cook/execution/events/event_types.rs:67-72
{
  "event_type": "agent_retrying",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "attempt": 2,
  "backoff_ms": 5000
}
```

**agent_progress** - Progress update during execution:
```json
// Source: src/cook/execution/events/event_types.rs:48-53
{
  "event_type": "agent_progress",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "step": "Running tests",
  "progress_pct": 75.0
}
```

### Failure Reasons

The `failure_reason` field in `agent_failed` events uses these values:

| Reason | Description |
|--------|-------------|
| `Timeout` | Agent execution exceeded time limit |
| `CommandFailed` | Command exited with non-zero code (includes `exit_code`) |
| `CommitValidationFailed` | Required commit was not created (includes `command`) |
| `MergeConflict` | Merge conflict or merge failure |
| `WorktreeError` | Worktree creation or management error |
| `Unknown` | Unclassified failure |

## Phase Events

Phase events track the overall progress of MapReduce execution stages.

**map_phase_started** - Map phase begins:
```json
// Source: src/cook/execution/mapreduce/event.rs:122-125
{
  "event_type": "map_phase_started",
  "total_items": 100,
  "timestamp": "2025-01-11T12:00:00Z"
}
```

**map_phase_completed** - Map phase finishes:
```json
// Source: src/cook/execution/mapreduce/event.rs:127-131
{
  "event_type": "map_phase_completed",
  "successful": 95,
  "failed": 5,
  "timestamp": "2025-01-11T12:30:00Z"
}
```

**reduce_phase_started** - Reduce phase begins:
```json
// Source: src/cook/execution/mapreduce/event.rs:158
{
  "event_type": "reduce_phase_started",
  "timestamp": "2025-01-11T12:30:01Z"
}
```

**reduce_phase_completed** - Reduce phase finishes:
```json
// Source: src/cook/execution/mapreduce/event.rs:160
{
  "event_type": "reduce_phase_completed",
  "timestamp": "2025-01-11T12:35:00Z"
}
```

## Job Lifecycle Events

Job events track the overall MapReduce job lifecycle.

**job_started** - Job begins execution:
```json
// Source: src/cook/execution/events/event_types.rs:13-18
{
  "event_type": "job_started",
  "job_id": "mapreduce-123",
  "config": { ... },  // (1)!
  "total_items": 100,
  "timestamp": "2025-01-11T12:00:00Z"
}
```

1. Full MapReduceConfig including max_parallel, timeouts, etc.

**job_completed** - Job finishes successfully:
```json
// Source: src/cook/execution/events/event_types.rs:19-24
{
  "event_type": "job_completed",
  "job_id": "mapreduce-123",
  "duration": {"secs": 1800, "nanos": 0},
  "success_count": 95,
  "failure_count": 5
}
```

**job_failed** - Job fails entirely:
```json
// Source: src/cook/execution/events/event_types.rs:25-29
{
  "event_type": "job_failed",
  "job_id": "mapreduce-123",
  "error": "Too many failures exceeded threshold",
  "partial_results": 50
}
```

**job_paused** - Job paused (checkpointed):
```json
// Source: src/cook/execution/events/event_types.rs:30-33
{
  "event_type": "job_paused",
  "job_id": "mapreduce-123",
  "checkpoint_version": 5
}
```

**job_resumed** - Job resumed from checkpoint:
```json
// Source: src/cook/execution/events/event_types.rs:34-38
{
  "event_type": "job_resumed",
  "job_id": "mapreduce-123",
  "checkpoint_version": 5,
  "pending_items": 45
}
```

## Checkpoint Events

Checkpoint events track state persistence for resume capability.

**checkpoint_created** - State saved to disk:
```json
// Source: src/cook/execution/events/event_types.rs:75-79
{
  "event_type": "checkpoint_created",
  "job_id": "mapreduce-123",
  "version": 5,  // (1)!
  "agents_completed": 55  // (2)!
}
```

1. Checkpoint version number (increments with each save)
2. Number of agents completed at checkpoint time

**checkpoint_loaded** - State restored from disk:
```json
// Source: src/cook/execution/events/event_types.rs:80-83
{
  "event_type": "checkpoint_loaded",
  "job_id": "mapreduce-123",
  "version": 5
}
```

**checkpoint_failed** - Checkpoint operation failed:
```json
// Source: src/cook/execution/events/event_types.rs:84-87
{
  "event_type": "checkpoint_failed",
  "job_id": "mapreduce-123",
  "error": "Disk full"
}
```

## Worktree Events

Worktree events track Git worktree lifecycle during agent execution.

**worktree_created** - Agent worktree created:
```json
// Source: src/cook/execution/events/event_types.rs:90-95
{
  "event_type": "worktree_created",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "worktree_name": "agent-1-worktree",
  "branch": "agent-1-branch"
}
```

**worktree_merged** - Agent changes merged to parent:
```json
// Source: src/cook/execution/events/event_types.rs:96-100
{
  "event_type": "worktree_merged",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "target_branch": "session-branch"
}
```

**worktree_cleaned** - Agent worktree removed:
```json
// Source: src/cook/execution/events/event_types.rs:101-105
{
  "event_type": "worktree_cleaned",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "worktree_name": "agent-1-worktree"
}
```

## DLQ Events

Dead Letter Queue events track failed items for later retry.

**dlq_item_added** - Failed item added to DLQ:
```json
// Source: src/cook/execution/events/event_types.rs:121-126
{
  "event_type": "dlq_item_added",
  "job_id": "mapreduce-123",
  "item_id": "item-42",
  "error_signature": "timeout:300s",  // (1)!
  "failure_count": 3
}
```

1. Error signature for grouping similar failures

**dlq_item_removed** - Item removed from DLQ:
```json
// Source: src/cook/execution/events/event_types.rs:127-130
{
  "event_type": "dlq_item_removed",
  "job_id": "mapreduce-123",
  "item_id": "item-42"
}
```

**dlq_items_reprocessed** - DLQ items retried:
```json
// Source: src/cook/execution/events/event_types.rs:131-134
{
  "event_type": "dlq_items_reprocessed",
  "job_id": "mapreduce-123",
  "count": 5
}
```

**dlq_items_evicted** - Old items removed from DLQ:
```json
// Source: src/cook/execution/events/event_types.rs:135-138
{
  "event_type": "dlq_items_evicted",
  "job_id": "mapreduce-123",
  "count": 10
}
```

**dlq_analysis_generated** - Failure pattern analysis complete:
```json
// Source: src/cook/execution/events/event_types.rs:139-142
{
  "event_type": "dlq_analysis_generated",
  "job_id": "mapreduce-123",
  "patterns": 3  // (1)!
}
```

1. Number of distinct failure patterns detected

## Claude Observability Events

Claude-specific events for detailed debugging of agent interactions.

**claude_session_started** - Claude session begins:
```json
// Source: src/cook/execution/events/event_types.rs:158-163
{
  "event_type": "claude_session_started",
  "agent_id": "agent-1",
  "session_id": "session-xyz",
  "model": "claude-sonnet-4-20250514",
  "tools": ["Read", "Write", "Bash", "Grep"]
}
```

**claude_tool_invoked** - Claude calls a tool:
```json
// Source: src/cook/execution/events/event_types.rs:145-151
{
  "event_type": "claude_tool_invoked",
  "agent_id": "agent-1",
  "tool_name": "Read",
  "tool_id": "tool-123",
  "parameters": {"file_path": "/path/to/file.rs"},
  "timestamp": "2025-01-11T12:01:00Z"
}
```

**claude_token_usage** - Token usage for an agent:
```json
// Source: src/cook/execution/events/event_types.rs:152-157
{
  "event_type": "claude_token_usage",
  "agent_id": "agent-1",
  "input_tokens": 5000,
  "output_tokens": 2000,
  "cache_tokens": 1500
}
```

**claude_message** - Claude interaction message:
```json
// Source: src/cook/execution/events/event_types.rs:164-169
{
  "event_type": "claude_message",
  "agent_id": "agent-1",
  "content": "Analyzing file structure...",
  "message_type": "assistant",
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

## Performance Events

Performance events for operational monitoring.

**queue_depth_changed** - Work queue status:
```json
// Source: src/cook/execution/events/event_types.rs:108-113
{
  "event_type": "queue_depth_changed",
  "job_id": "mapreduce-123",
  "pending": 45,
  "active": 10,
  "completed": 45
}
```

**memory_pressure** - Memory usage alert:
```json
// Source: src/cook/execution/events/event_types.rs:114-118
{
  "event_type": "memory_pressure",
  "job_id": "mapreduce-123",
  "used_mb": 7500,
  "limit_mb": 8000
}
```

## Event Organization

Events are organized by repository and job:
```
~/.prodigy/events/
└── prodigy/                    # (1)!
    ├── mapreduce-123/          # (2)!
    │   └── events-20250111.jsonl  # (3)!
    └── mapreduce-456/
        └── events-20250111.jsonl
```

1. Repository name for multi-repo support
2. Job ID groups all events for this MapReduce run
3. JSONL file with one event per line (append-only)

## Querying Events

Use `jq` to filter and analyze events:

=== "Filter by Event Type"
    ```bash
    # Get all agent failures
    cat events-*.jsonl | jq -c 'select(.event_type == "agent_failed")'

    # Get all checkpoint events
    cat events-*.jsonl | jq -c 'select(.event_type | startswith("checkpoint"))'
    ```

=== "Analyze Failures"
    ```bash
    # Group failures by reason
    cat events-*.jsonl | jq -c 'select(.event_type == "agent_failed")' | \
      jq -s 'group_by(.failure_reason) | map({reason: .[0].failure_reason, count: length})'

    # Find timeout failures
    cat events-*.jsonl | jq -c 'select(.event_type == "agent_failed" and .failure_reason == "Timeout")'
    ```

=== "Track Progress"
    ```bash
    # Count completed vs failed
    cat events-*.jsonl | jq -sc '[
      (map(select(.event_type == "agent_completed")) | length),
      (map(select(.event_type == "agent_failed")) | length)
    ] | {completed: .[0], failed: .[1]}'

    # Get latest queue depth
    cat events-*.jsonl | jq -c 'select(.event_type == "queue_depth_changed")' | tail -1
    ```

=== "Token Usage"
    ```bash
    # Total token usage across agents
    cat events-*.jsonl | jq -c 'select(.event_type == "claude_token_usage")' | \
      jq -s '{
        total_input: (map(.input_tokens) | add),
        total_output: (map(.output_tokens) | add),
        total_cache: (map(.cache_tokens) | add)
      }'
    ```

**Real-time monitoring:**
```bash
# Tail events as they happen
tail -f ~/.prodigy/events/prodigy/mapreduce-123/events-*.jsonl | jq -c '.'

# Watch for failures only
tail -f ~/.prodigy/events/prodigy/mapreduce-123/events-*.jsonl | \
  jq -c 'select(.event_type == "agent_failed")'
```
