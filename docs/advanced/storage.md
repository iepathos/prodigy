# Storage Architecture

Prodigy uses a global storage architecture to enable cross-worktree data sharing, persistent state management, centralized monitoring, and efficient deduplication.

## Overview

Storage architecture provides:
- **Global storage structure** - All data in `~/.prodigy/` directory
- **Cross-repository organization** - Events and state grouped by repository
- **Event storage** - JSONL event logs for monitoring and debugging
- **State persistence** - Durable checkpoints and session state

## Global Storage Structure

All Prodigy data is stored in `~/.prodigy/`:

```
~/.prodigy/
├── events/              # Event logs
│   └── {repo_name}/
│       └── {job_id}/
│           └── events-{timestamp}.jsonl
├── dlq/                 # Dead Letter Queue
│   └── {repo_name}/
│       └── {job_id}/
├── state/               # Persistent state
│   └── {repo_name}/
│       ├── mapreduce/
│       │   └── jobs/
│       │       └── {job_id}/
│       └── mappings/
├── sessions/            # Session tracking
│   └── {session_id}.json
└── worktrees/           # Git worktrees
    └── {repo_name}/
```

## Repository Organization

Data is organized by repository name to:
- Isolate data across different projects
- Enable multi-repository workflows
- Simplify cleanup per project
- Support cross-worktree aggregation

Example for "prodigy" repository:

```
~/.prodigy/
├── events/prodigy/
├── dlq/prodigy/
├── state/prodigy/
└── worktrees/prodigy/
```

## Event Storage

Events are stored as JSONL (JSON Lines) files:

```
~/.prodigy/events/{repo_name}/{job_id}/
└── events-{timestamp}.jsonl
```

### Event File Format

Each line is a JSON object:

```json
{"timestamp":"2025-11-11T12:00:00Z","event_type":"AgentStarted","agent_id":"agent-1"}
{"timestamp":"2025-11-11T12:00:30Z","event_type":"AgentCompleted","agent_id":"agent-1","commits":["abc123"]}
```

### Event Aggregation

Multiple worktrees working on the same job share event logs:
- Events from all agents written to same log
- Chronological ordering by timestamp
- Correlation IDs link related events

## State Persistence

Persistent state includes:

### MapReduce Job State

```
~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
├── setup-checkpoint.json
├── map-checkpoint-{timestamp}.json
├── reduce-checkpoint-v1-{timestamp}.json
└── job-state.json
```

### Session State

```
~/.prodigy/sessions/{session_id}.json
```

### Session-Job Mappings

```
~/.prodigy/state/{repo_name}/mappings/
├── session-to-job.json
└── job-to-session.json
```

## Dead Letter Queue Storage

Failed work items stored per job:

```
~/.prodigy/dlq/{repo_name}/{job_id}/
└── dlq-items.json
```

Structure:

```json
{
  "items": [
    {
      "item_id": "item-123",
      "work_item": {...},
      "failure_history": [
        {
          "timestamp": "2025-11-11T12:00:00Z",
          "error": "Agent failed",
          "json_log_location": "/path/to/claude-log.jsonl"
        }
      ]
    }
  ]
}
```

## Worktree Storage

Git worktrees for session isolation:

```
~/.prodigy/worktrees/{repo_name}/
├── session-abc123/
├── session-def456/
└── agent-xyz789/
```

Each worktree is a full git checkout with:
- Independent working directory
- Separate git metadata
- Isolated change tracking

## Storage Benefits

### Cross-Worktree Data Sharing

Multiple worktrees can:
- Write to the same event log
- Access the same DLQ
- Share checkpoints
- Coordinate execution

### Persistent State

State survives:
- Worktree cleanup
- Session termination
- System restarts

### Centralized Monitoring

All data accessible from:
- Single location (`~/.prodigy/`)
- Repository-based organization
- Consistent structure

### Efficient Deduplication

- Event aggregation prevents duplicate tracking
- Shared state reduces storage overhead
- Repository-level organization enables cleanup

## Cleanup and Maintenance

### Worktree Cleanup

```bash
# Clean specific worktree
prodigy worktree clean session-abc123

# Clean all completed worktrees
prodigy worktree clean --all

# Force cleanup of stuck worktrees
prodigy worktree clean -f session-abc123
```

### Event Log Cleanup

Events persist for auditing. Manually clean old events:

```bash
# Remove events older than 30 days
find ~/.prodigy/events -name "*.jsonl" -mtime +30 -delete
```

### Session Cleanup

Remove completed sessions:

```bash
# Remove completed session
rm ~/.prodigy/sessions/session-abc123.json

# Clean sessions older than 7 days
find ~/.prodigy/sessions -name "*.json" -mtime +7 -delete
```

## See Also

- [Session Management](sessions.md) - Session state and lifecycle
- [Observability and Logging](observability.md) - Event tracking details
- [Git Integration](git-integration.md) - Worktree management
