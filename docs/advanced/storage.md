# Storage Architecture

Prodigy uses a global storage architecture for persistent state, events, and failure tracking across all workflows and sessions.

## Overview

Global storage features:
- **Centralized storage**: All data in `~/.prodigy/`
- **Repository organization**: Data grouped by repository name
- **Cross-worktree sharing**: Multiple worktrees access shared state
- **Persistent state**: Job checkpoints survive worktree cleanup
- **Efficient deduplication**: Minimize storage overhead

## Storage Structure

```
~/.prodigy/
├── events/                     # Event logs
│   └── {repo_name}/
│       └── {job_id}/
│           └── events-{timestamp}.jsonl
├── dlq/                        # Dead Letter Queue
│   └── {repo_name}/
│       └── {job_id}/
│           └── dlq-items.json
├── state/                      # State and checkpoints
│   └── {repo_name}/
│       ├── mapreduce/
│       │   └── jobs/{job_id}/
│       │       ├── setup-checkpoint.json
│       │       ├── map-checkpoint-{timestamp}.json
│       │       └── reduce-checkpoint-v1-{timestamp}.json
│       └── mappings/
│           ├── session-to-job.json
│           └── job-to-session.json
├── sessions/                   # Session tracking
│   └── {session_id}.json
├── worktrees/                  # Git worktrees
│   └── {repo_name}/
│       └── session-{session_id}/
└── orphaned_worktrees/         # Cleanup failure tracking
    └── {repo_name}/
        └── {job_id}.json
```

## Event Storage

Event logs are stored as JSONL files for efficient streaming:

```
~/.prodigy/events/{repo_name}/{job_id}/events-{timestamp}.jsonl
```

### Event Organization

- **By repository**: Events grouped by repo for easy filtering
- **By job**: Each job has dedicated event directory
- **JSONL format**: One JSON event per line for streaming
- **Timestamped files**: Rotate logs by timestamp

### Event Persistence

Events are persisted immediately:
- Agent lifecycle events (started, completed, failed)
- Work item status changes
- Checkpoint saves
- Error details with correlation IDs

### Cross-Worktree Aggregation

Multiple worktrees working on same job share event logs:
```
worktree-1 → ~/.prodigy/events/prodigy/job-123/
worktree-2 → ~/.prodigy/events/prodigy/job-123/  # Same directory
```

## State Storage

Job state and checkpoints are stored globally:

```
~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
├── job-state.json              # Overall job state
├── setup-checkpoint.json       # Setup phase results
├── map-checkpoint-*.json       # Map phase progress
└── reduce-checkpoint-v1-*.json # Reduce phase progress
```

### Checkpoint Types

**Setup Phase**:
```json
{
  "phase": "setup",
  "completed": true,
  "outputs": {...},
  "timestamp": "2025-01-11T12:00:00Z"
}
```

**Map Phase**:
```json
{
  "phase": "map",
  "completed_items": ["item-1", "item-2"],
  "in_progress_items": ["item-3"],
  "pending_items": ["item-4", "item-5"],
  "agent_results": {...},
  "timestamp": "2025-01-11T12:05:00Z"
}
```

**Reduce Phase**:
```json
{
  "phase": "reduce",
  "completed_steps": [0, 1],
  "current_step": 2,
  "step_results": {...},
  "map_results": {...},
  "timestamp": "2025-01-11T12:10:00Z"
}
```

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

## Worktree Storage

Git worktrees are created per session:

```
~/.prodigy/worktrees/{repo_name}/
├── session-abc123/             # Workflow session
└── session-mapreduce-xyz/      # MapReduce parent worktree
    ├── agent-1/                # MapReduce agent worktree
    └── agent-2/                # MapReduce agent worktree
```

### Worktree Lifecycle

1. **Creation**: Worktree created when workflow starts
2. **Execution**: All commands run in worktree context
3. **Persistence**: Worktree remains until merge or cleanup
4. **Cleanup**: Removed after successful merge

## Orphaned Worktree Tracking

When cleanup fails, worktree paths are registered:

```
~/.prodigy/orphaned_worktrees/{repo_name}/{job_id}.json
```

### Registry Format

```json
{
  "job_id": "mapreduce-123",
  "orphaned_worktrees": [
    {
      "agent_id": "agent-1",
      "item_id": "item-1",
      "worktree_path": "/Users/user/.prodigy/worktrees/prodigy/agent-1",
      "timestamp": "2025-01-11T12:00:00Z",
      "error": "Permission denied"
    }
  ]
}
```

## Session-Job Mapping

Bidirectional mapping enables resume with session or job IDs:

```
~/.prodigy/state/{repo_name}/mappings/
├── session-to-job.json
└── job-to-session.json
```

### Mapping Format

**session-to-job.json**:
```json
{
  "session-mapreduce-xyz": "mapreduce-123"
}
```

**job-to-session.json**:
```json
{
  "mapreduce-123": "session-mapreduce-xyz"
}
```

## Storage Benefits

### Cross-Worktree Data Sharing

Multiple worktrees working on same job share:
- Event logs
- DLQ items
- Checkpoints
- Job state

This enables:
- Parallel execution visibility
- Centralized failure tracking
- Consistent state management

### Persistent State Management

State survives worktree cleanup:
- Resume after worktree deleted
- Access job data without worktree
- Historical analysis of completed jobs

### Centralized Monitoring

All job data accessible from single location:
- View events across all worktrees
- Monitor job progress globally
- Analyze performance metrics

### Efficient Storage

Deduplication across worktrees:
- Single event log per job (not per worktree)
- Shared checkpoint files
- Reduced storage overhead

## Storage Maintenance

### Cleanup Commands

```bash
# Clean old events (30+ days)
find ~/.prodigy/events -name "*.jsonl" -mtime +30 -delete

# Clean completed sessions
prodigy sessions clean --completed

# Clean orphaned worktrees
prodigy worktree clean-orphaned <job_id>

# Clean DLQ after successful retry
prodigy dlq clear <job_id>
```

### Storage Usage

Check storage consumption:
```bash
# Total storage
du -sh ~/.prodigy/

# By category
du -sh ~/.prodigy/events
du -sh ~/.prodigy/state
du -sh ~/.prodigy/sessions
du -sh ~/.prodigy/worktrees
```

## Migration from Local Storage

Legacy local storage (deprecated):
```
.prodigy/
├── session_state.json          # Deprecated
├── events/                     # Moved to ~/.prodigy/events
└── dlq/                        # Moved to ~/.prodigy/dlq
```

Global storage benefits:
- Cross-repository visibility
- Persistent state across worktrees
- Centralized monitoring and debugging

## Examples

### Access Job Data

```bash
# View events
cat ~/.prodigy/events/prodigy/mapreduce-123/events-*.jsonl | jq

# Check checkpoint
cat ~/.prodigy/state/prodigy/mapreduce/jobs/mapreduce-123/map-checkpoint-*.json | jq

# Inspect DLQ
cat ~/.prodigy/dlq/prodigy/mapreduce-123/dlq-items.json | jq
```

### Find Session by Job ID

```bash
# Look up session ID
job_id="mapreduce-123"
session_id=$(jq -r ".\"$job_id\"" ~/.prodigy/state/prodigy/mappings/job-to-session.json)

# View session
cat ~/.prodigy/sessions/$session_id.json | jq
```

### Analyze Storage Growth

```bash
# Event log size over time
find ~/.prodigy/events -name "*.jsonl" -printf '%TY-%Tm-%Td %s %p\n' | \
  sort | \
  awk '{size+=$2} END {print "Total events:", size/1024/1024, "MB"}'

# Checkpoint size
du -sh ~/.prodigy/state/*/mapreduce/jobs/*/
```

## See Also

- [Event Tracking](../mapreduce/event-tracking.md) - Event log format and usage
- [Session Management](sessions.md) - Session storage and lifecycle
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) - DLQ storage and retry
- [Git Integration](git-integration.md) - Worktree storage and management
