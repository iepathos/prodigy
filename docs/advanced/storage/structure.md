# Storage Structure

This page covers the directory layout and organization of Prodigy's global storage system.

## Storage Structure

```mermaid
graph TD
    Root["~/.prodigy/"] --> Events["events/"]
    Root --> DLQ["dlq/"]
    Root --> State["state/"]
    Root --> Sessions["sessions/"]
    Root --> Worktrees["worktrees/"]
    Root --> Orphaned["orphaned_worktrees/"]

    Events --> ERepo["{repo_name}/"]
    ERepo --> EJob["{job_id}/"]
    EJob --> EFiles["events-{timestamp}.jsonl"]

    DLQ --> DRepo["{repo_name}/"]
    DRepo --> DJob["{job_id}/"]
    DJob --> DFiles["dlq-items.json"]

    State --> SRepo["{repo_name}/"]
    SRepo --> MapReduce["mapreduce/jobs/{job_id}/"]
    SRepo --> Mappings["mappings/"]
    MapReduce --> Setup["setup-checkpoint.json"]
    MapReduce --> Map["map-checkpoint-*.json"]
    MapReduce --> Reduce["reduce-checkpoint-v1-*.json"]

    Sessions --> SessionFiles["{session_id}.json"]

    Worktrees --> WRepo["{repo_name}/"]
    WRepo --> WSession["session-{session_id}/"]

    Orphaned --> ORepo["{repo_name}/"]
    ORepo --> OJob["{job_id}.json"]

    style Root fill:#e1f5ff
    style Events fill:#fff3e0
    style DLQ fill:#ffebee
    style State fill:#e8f5e9
    style Sessions fill:#f3e5f5
    style Worktrees fill:#e0f2f1
```

**Figure**: Global storage hierarchy showing repository-organized structure.

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

!!! note "Auto-Generated Paths"
    Event file paths include auto-generated timestamp suffixes (e.g., `events-20250111120000.jsonl`). This enables log rotation and prevents conflicts when multiple processes write events simultaneously.

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

```mermaid
graph LR
    WT1[Worktree 1<br/>Agent processing item-1] --> Events["~/.prodigy/events/<br/>prodigy/job-123/"]
    WT2[Worktree 2<br/>Agent processing item-2] --> Events
    WT3[Worktree N<br/>Agent processing item-N] --> Events

    Events --> Monitor[Centralized<br/>Monitoring]

    style WT1 fill:#fff3e0
    style WT2 fill:#fff3e0
    style WT3 fill:#fff3e0
    style Events fill:#e8f5e9
    style Monitor fill:#e1f5ff
```

**Figure**: Cross-worktree event aggregation enabling centralized monitoring.

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
// Source: src/cook/execution/mapreduce/checkpoint/types.rs
{
  "phase": "setup",
  "completed": true,
  "outputs": {...},
  "timestamp": "2025-01-11T12:00:00Z"
}
```

**Map Phase**:
```json
// Source: src/cook/execution/mapreduce/checkpoint/types.rs
{
  "phase": "map",
  "completed_items": ["item-1", "item-2"],      // (1)!
  "in_progress_items": ["item-3"],              // (2)!
  "pending_items": ["item-4", "item-5"],        // (3)!
  "agent_results": {...},                       // (4)!
  "timestamp": "2025-01-11T12:05:00Z"
}
```

1. Work items successfully processed by agents
2. Items currently being processed (moved back to pending on resume)
3. Items waiting to be processed
4. Full results from completed agents (commits, outputs, timings)

**Reduce Phase**:
```json
// Source: src/cook/execution/mapreduce/checkpoint/types.rs
{
  "phase": "reduce",
  "completed_steps": [0, 1],     // (1)!
  "current_step": 2,              // (2)!
  "step_results": {...},          // (3)!
  "map_results": {...},           // (4)!
  "timestamp": "2025-01-11T12:10:00Z"
}
```

1. Indices of reduce commands that have completed
2. Index of the currently executing reduce command
3. Output captured from completed reduce steps
4. Aggregated results from all map agents (available to reduce commands)

!!! note "Checkpoint File Naming"
    Checkpoint files include auto-generated timestamp suffixes:
    - Setup: `setup-checkpoint.json` (no timestamp, only one per job)
    - Map: `map-checkpoint-{timestamp}.json` (multiple checkpoints during map phase)
    - Reduce: `reduce-checkpoint-v1-{timestamp}.json` (versioned with timestamp)
