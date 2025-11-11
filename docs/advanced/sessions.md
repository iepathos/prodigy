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
// Source: src/unified_session/state.rs:66-113
{
  "id": "session-abc123",
  "session_type": "Workflow",
  "status": "Running",
  "workflow_data": {
    "workflow_id": "workflow-1234567890",
    "workflow_name": "my-workflow",
    "current_step": 2,
    "total_steps": 5,
    "completed_steps": [0, 1],
    "variables": {},
    "iterations_completed": 0,
    "files_changed": 0,
    "worktree_name": "session-abc123"
  }
}
```

### MapReduce Sessions

MapReduce job state management:

```json
// Source: src/unified_session/state.rs:117-125
{
  "id": "session-xyz789",
  "session_type": "MapReduce",
  "status": "Running",
  "mapreduce_data": {
    "job_id": "mapreduce-1234567890",
    "total_items": 100,
    "processed_items": 45,
    "failed_items": 2,
    "agent_count": 5,
    "phase": "Map",
    "reduce_results": null
  }
}
```

Available phases (src/unified_session/state.rs:129-134):
- `Setup` - Preparing work items and environment
- `Map` - Processing items in parallel
- `Reduce` - Aggregating results
- `Complete` - All phases finished

## Session Lifecycle

Sessions transition through states:

```rust
// Source: src/unified_session/state.rs:92-99
pub enum SessionStatus {
    Initializing,  // Session created, not yet started
    Running,       // Active execution
    Paused,        // Interrupted, ready to resume
    Completed,     // Successfully finished
    Failed,        // Terminated with errors
    Cancelled,     // User-initiated stop
}
```

### State Transitions

```
Creation → Initializing → Running → Paused → Resumed → Completed
                                  ↓
                                Failed
                                  ↓
                              Cancelled
```

## Session Storage

Sessions are stored in:

```
~/.prodigy/sessions/{session-id}.json
```

Structure:

```json
// Source: src/unified_session/state.rs:66-81
{
  "id": "session-abc123",
  "session_type": "Workflow",
  "status": "Running",
  "started_at": "2025-11-11T12:00:00Z",
  "updated_at": "2025-11-11T12:05:00Z",
  "completed_at": null,
  "metadata": {
    "execution_start_time": "2025-11-11T12:00:00Z",
    "workflow_type": "standard",
    "total_steps": 5,
    "current_step": 2
  },
  "checkpoints": [],
  "timings": {
    "step1": {"secs": 10, "nanos": 0},
    "step2": {"secs": 15, "nanos": 0}
  },
  "error": null,
  "workflow_data": {
    "workflow_id": "workflow-1234567890",
    "workflow_name": "my-workflow",
    "current_step": 2,
    "total_steps": 5,
    "completed_steps": [0, 1],
    "variables": {},
    "iterations_completed": 0,
    "files_changed": 0,
    "worktree_name": "session-abc123"
  },
  "mapreduce_data": null
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

Lock mechanism prevents multiple resume processes from operating on the same session/job simultaneously:

```bash
# If another process is resuming:
Error: Resume already in progress for job <job_id>
Lock held by: PID 12345 on hostname (acquired 2025-11-11 10:30:00 UTC)
```

**Lock Storage** (src/cook/execution/resume_lock.rs:52):
```
~/.prodigy/resume_locks/
├── session-abc123.lock
├── mapreduce-xyz789.lock
└── ...
```

Each lock file contains JSON metadata:
- Process ID (PID) of the holding process
- Hostname where the process is running
- Timestamp when lock was acquired
- Job/session ID being locked

Locks are automatically cleaned up when:
- Resume completes (RAII pattern)
- Process crashes (stale lock detection with platform-specific process checks)

Manual cleanup if needed (rarely required):

```bash
rm ~/.prodigy/resume_locks/<job_id>.lock
```

## See Also

- [Checkpoint and Resume](../mapreduce/checkpoint-resume.md) - MapReduce resume details
- [Storage Architecture](storage.md) - Storage locations and structure
- [Observability and Logging](observability.md) - Session event tracking
