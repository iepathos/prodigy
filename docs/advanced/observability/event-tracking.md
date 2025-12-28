# Event Tracking

All workflow operations are logged to JSONL event files:

```
~/.prodigy/events/{repo_name}/{job_id}/
└── events-{timestamp}.jsonl
```

!!! tip "Event Storage Best Practice"
    Events are stored globally in `~/.prodigy/events/` to enable cross-worktree aggregation. Multiple worktrees working on the same job share the same event log, making it easy to monitor parallel execution.

## Event Types

**AgentStarted** - Agent execution begins:
```json
{
  "type": "AgentStarted",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "item_id": "item-1",
  "timestamp": "2025-01-11T12:00:00Z"
}
```

**AgentCompleted** - Agent finishes successfully:
```json
{
  "type": "AgentCompleted",  // (1)!
  "job_id": "mapreduce-123",  // (2)!
  "agent_id": "agent-1",  // (3)!
  "duration": {"secs": 30, "nanos": 0},  // (4)!
  "commits": ["abc123", "def456"],  // (5)!
  "json_log_location": "/path/to/logs/session-xyz.json"  // (6)!
}
```

1. Event type indicating successful completion
2. MapReduce job identifier
3. Unique agent identifier for this work item
4. Total execution time for the agent
5. Git commits created during execution
6. Path to Claude's detailed JSON log for debugging

**AgentFailed** - Agent encounters errors:
```json
{
  "type": "AgentFailed",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "error": "Timeout after 300 seconds",
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

**WorkItemProcessed** - Item completion:
```json
{
  "type": "WorkItemProcessed",
  "job_id": "mapreduce-123",
  "item_id": "item-1",
  "status": "completed",
  "result": {...}
}
```

**CheckpointSaved** - State persistence:
```json
{
  "type": "CheckpointSaved",
  "job_id": "mapreduce-123",
  "phase": "map",
  "checkpoint_path": "/path/to/checkpoint.json",
  "timestamp": "2025-01-11T12:05:00Z"
}
```

**ClaudeMessage** - Claude interaction messages:
```json
// Source: src/cook/execution/events/event_types.rs:164-169
{
  "type": "ClaudeMessage",
  "agent_id": "agent-1",
  "content": "Analyzing file structure...",
  "message_type": "assistant",
  "json_log_location": "/path/to/logs/session-xyz.json"
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
