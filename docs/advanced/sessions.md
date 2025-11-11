# Session Management

Prodigy uses a unified session system to track all workflow and MapReduce executions. Sessions maintain state, support checkpointing, and enable resuming interrupted workflows.

## Overview

Session management provides:
- **Session types** - Standard workflows and MapReduce jobs
- **Lifecycle tracking** - State transitions from start to completion
- **Resume capabilities** - Continue from interruptions using session or job IDs
- **State persistence** - Durable storage across sessions

## Session Types

### Workflow Sessions

Standard workflow execution tracking:

```json
{
  "id": "session-abc123",
  "session_type": "Workflow",
  "status": "Running",
  "workflow_data": {
    "workflow_name": "my-workflow",
    "current_step": 2,
    "total_steps": 5,
    "completed_steps": [0, 1]
  }
}
```

### MapReduce Sessions

MapReduce job state management:

```json
{
  "id": "session-xyz789",
  "session_type": "MapReduce",
  "status": "Running",
  "mapreduce_data": {
    "job_id": "mapreduce-1234567890",
    "current_phase": "map",
    "work_items_completed": 45,
    "work_items_total": 100
  }
}
```

## Session Lifecycle

Sessions transition through states:

1. **Running** - Active execution
2. **Paused** - Interrupted, ready to resume
3. **Completed** - Successfully finished
4. **Failed** - Terminated with errors
5. **Cancelled** - User-initiated stop

### State Transitions

```
Creation → Running → Paused → Resumed → Completed
                   ↓
                 Failed
```

## Session Storage

Sessions are stored in:

```
~/.prodigy/sessions/{session-id}.json
```

Structure:

```json
{
  "id": "session-abc123",
  "session_type": "Workflow",
  "status": "Running",
  "started_at": "2025-11-11T12:00:00Z",
  "updated_at": "2025-11-11T12:05:00Z",
  "metadata": {
    "execution_start_time": "2025-11-11T12:00:00Z",
    "workflow_type": "standard",
    "total_steps": 5,
    "current_step": 2
  },
  "timings": {
    "step1": {"secs": 10, "nanos": 0},
    "step2": {"secs": 15, "nanos": 0}
  }
}
```

## Resume Capabilities

Resume interrupted workflows using session IDs or job IDs:

```bash
# Resume using session ID
prodigy resume session-abc123

# Resume using job ID (MapReduce)
prodigy resume-job mapreduce-1234567890

# Unified resume (auto-detects ID type)
prodigy resume <id>
```

### Session-Job Mapping

MapReduce jobs maintain bidirectional mapping:

```
~/.prodigy/state/{repo_name}/mappings/
├── session-to-job.json
└── job-to-session.json
```

This enables resume with either identifier.

## Checkpoints

Checkpoints capture execution state for recovery:

### Workflow Checkpoints

```
~/.prodigy/state/{session-id}/checkpoints/
└── checkpoint-{timestamp}.json
```

### MapReduce Checkpoints

```
~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
├── setup-checkpoint.json
├── map-checkpoint-{timestamp}.json
└── reduce-checkpoint-v1-{timestamp}.json
```

## Listing Sessions

View all sessions:

```bash
prodigy sessions list
```

Filter by status:

```bash
prodigy sessions list --status Running
prodigy sessions list --status Paused
```

## Session Metadata

Sessions track:
- **Execution timing** - Start, update, completion timestamps
- **Progress** - Current step, completed steps
- **Variables** - Workflow variable state
- **Error details** - Failure information if applicable

## Concurrent Resume Protection

Lock mechanism prevents multiple resume processes:

```bash
# If another process is resuming:
Error: Resume already in progress for job <job_id>
Lock held by: PID 12345 on hostname
```

Locks are automatically cleaned up when:
- Resume completes
- Process crashes (stale lock detection)

Manual cleanup if needed:

```bash
rm ~/.prodigy/resume_locks/<job_id>.lock
```

## See Also

- [Checkpoint and Resume](../mapreduce/checkpoint-resume.md) - MapReduce resume details
- [Storage Architecture](storage.md) - Storage locations and structure
- [Observability and Logging](observability.md) - Session event tracking
