## Checkpoint and Resume

Prodigy provides comprehensive checkpoint and resume capabilities for MapReduce workflows, ensuring work can be recovered from any point of failure. Checkpoints are automatically created during workflow execution, preserving all state needed to continue from where you left off. This enables resilient workflows that can survive interruptions, crashes, or planned pauses without losing progress.

### Checkpoint Behavior

Checkpoints are automatically created at strategic points during workflow execution:

**Setup Phase Checkpointing**:
- Checkpoint created after successful setup completion
- Preserves setup output, generated artifacts, and environment state
- Stored as `setup-checkpoint.json`
- Resume restarts setup from beginning (idempotent operations recommended)

**Map Phase Checkpointing**:
- Checkpoints created after processing configurable number of work items
- Tracks completed, in-progress, and pending work items
- Stores agent results and failure details for recovery
- Resume continues from last successful checkpoint
- In-progress items are moved back to pending on resume
- Stored as `map-checkpoint-{timestamp}.json`

**Reduce Phase Checkpointing**:
- Checkpoint created after each reduce command execution
- Tracks completed steps, step results, variables, and map results
- Enables resume from any point in reduce phase execution
- Resume continues from last completed step
- Stored as `reduce-checkpoint-v1-{timestamp}.json`

### Checkpoint Interval Configuration

Prodigy controls when checkpoints are created through configurable intervals. The checkpoint strategy differs between workflow types:

**Standard Workflow Checkpoints** (src/cook/workflow/checkpoint.rs:20):
- **Default interval**: 60 seconds between checkpoints
- **Configurable**: Use `.with_interval(Duration)` in checkpoint manager builder
- **Decision logic**: Compares elapsed time since last checkpoint

**MapReduce Checkpoints** (src/cook/execution/mapreduce/checkpoint/types.rs:242-264):
- **Default `interval_items`**: 100 work items per checkpoint
- **Default `interval_duration`**: 300 seconds (5 minutes) per checkpoint
- **Dual triggers**: Checkpoint created when either item count OR duration threshold is reached
- **Retention policy**:
  - `max_checkpoints`: Keep 10 most recent checkpoints
  - `max_age`: Retain checkpoints for 7 days (604,800 seconds)
  - `keep_final`: Always preserve final checkpoint

**Example MapReduce checkpoint configuration**:

```yaml
name: my-workflow
mode: mapreduce

checkpoint:
  interval_items: 50      # Checkpoint every 50 items (default: 100)
  interval_duration: 600  # Checkpoint every 10 minutes (default: 300s)
  max_checkpoints: 15     # Keep 15 recent checkpoints (default: 10)
  max_age: 1209600        # Keep for 14 days (default: 604,800s / 7 days)
```

These intervals balance between checkpoint overhead and recovery granularity. More frequent checkpoints enable finer-grained resume but increase I/O overhead.

### Resume Commands

MapReduce jobs can be resumed using either session IDs or job IDs:

```bash
# Resume using session ID
prodigy resume session-mapreduce-1234567890

# Resume using job ID
prodigy resume-job mapreduce-1234567890

# Unified resume command (auto-detects ID type)
prodigy resume mapreduce-1234567890
```

**Session-Job Mapping**:

The `SessionJobMapping` structure provides bidirectional mapping between session and job identifiers (src/storage/session_job_mapping.rs:14-26):

- **Storage location**: `~/.prodigy/state/{repo_name}/mappings/`
- **Mapping fields**:
  - `session_id`: Unique identifier for the workflow session
  - `job_id`: MapReduce job identifier
  - `workflow_name`: Name of the workflow for easier identification
  - `created_at`: Timestamp when the mapping was created
- **Created**: Automatically when MapReduce workflow starts
- **Purpose**: Enables resume with either session ID or job ID

This mapping allows you to resume a workflow using whichever identifier is more convenient or available in your context.

### State Preservation

All critical state is preserved across resume operations:

**Variables and Context**:
- Workflow variables preserved across resume
- Captured outputs from setup and reduce phases
- Environment variables maintained
- Map results available to reduce phase after resume

**Work Item State**:
- **Completed items**: Preserved with full results
- **In-progress items**: Moved back to pending on resume
- **Failed items**: Tracked with retry counts and error details
- **Pending items**: Continue processing from where left off

**Agent State**:
- Active agent information preserved
- Resource allocation tracked
- Worktree paths recorded for cleanup

### Resume Strategies

Based on checkpoint state and phase, different resume strategies apply:

- **Setup Phase**: Restart setup from beginning (idempotent operations recommended)
- **Map Phase**: Continue from last checkpoint, re-process in-progress items
- **Reduce Phase**: Continue from last completed step
- **Validate and Continue**: Verify checkpoint integrity before resuming

### Storage Structure

Checkpoints are stored in a structured directory hierarchy:

```
~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
├── setup-checkpoint.json           # Setup phase results
├── map-checkpoint-{timestamp}.json  # Map phase progress
├── reduce-checkpoint-v1-{timestamp}.json  # Reduce phase progress
└── job-state.json                  # Overall job state
```

### Checkpoint File Structure

Checkpoint files contain JSON-serialized state for recovery. Here's what each checkpoint type stores:

**MapReduce Checkpoint** (src/cook/execution/mapreduce/checkpoint/types.rs:48-64):

```json
{
  "metadata": {
    "checkpoint_id": "ckpt-1704556800",
    "version": 1,
    "phase": "Map",
    "created_at": "2025-01-11T12:00:00Z",
    "items_processed": 150,
    "items_total": 500
  },
  "work_items": {
    "pending": ["item-151", "item-152", "..."],
    "in_progress": [],
    "completed": ["item-1", "item-2", "..."],
    "failed": []
  },
  "agent_state": {
    "active_agents": [],
    "agent_assignments": {},
    "agent_results": {
      "item-1": {"status": "success", "commits": ["abc123"]},
      "item-2": {"status": "success", "commits": ["def456"]}
    },
    "resource_allocation": {
      "max_parallel": 10,
      "current_agents": 0
    }
  },
  "variables": {
    "workflow_vars": {"PROJECT_NAME": "prodigy"},
    "captured_vars": {},
    "environment_vars": {},
    "item_vars": {}
  },
  "error_state": {
    "error_count": 0,
    "dlq_items": [],
    "error_threshold_reached": false
  },
  "reason": "Interval",
  "checksum": "sha256:abc123..."
}
```

**Workflow Checkpoint** (src/cook/workflow/checkpoint.rs:26-57):

```json
{
  "workflow_id": "workflow-1704556800",
  "version": 1,
  "execution_state": {
    "current_step_index": 3,
    "status": "Running",
    "started_at": "2025-01-11T12:00:00Z",
    "updated_at": "2025-01-11T12:05:00Z"
  },
  "completed_steps": [
    {
      "step_index": 0,
      "command": "shell: cargo build",
      "status": "Success",
      "duration": {"secs": 45, "nanos": 0},
      "completed_at": "2025-01-11T12:01:00Z"
    }
  ],
  "variable_state": {
    "BUILD_OUTPUT": "/target/release/prodigy",
    "VERSION": "1.0.0"
  },
  "workflow_hash": "sha256:def789..."
}
```

**Key Fields**:
- **metadata/execution_state**: Current progress and timestamps
- **work_items**: Work item status tracking (MapReduce only)
- **agent_state**: Agent results and resource allocation (MapReduce only)
- **variables/variable_state**: Preserved workflow variables for resume
- **completed_steps**: Audit trail of successfully completed steps
- **checksum/workflow_hash**: Integrity verification

These structures enable Prodigy to reconstruct exact execution state during resume operations.

### Concurrent Resume Protection

Prodigy prevents multiple resume processes from running on the same session/job simultaneously using an RAII-based locking mechanism:

**Lock Behavior**:
- Resume automatically acquires exclusive lock before starting
- Lock creation is atomic - fails if another process holds the lock
- Lock automatically released when resume completes or fails (RAII pattern)
- Stale locks (from crashed processes) are automatically detected and cleaned up

**Lock Metadata**:
Lock files contain:
- Process ID (PID) of the holding process
- Hostname where the process is running
- Timestamp when lock was acquired
- Job/session ID being locked

**Lock Storage**:
```
~/.prodigy/resume_locks/
├── session-abc123.lock
├── mapreduce-xyz789.lock
└── ...
```

**Error Messages**:
If a resume is blocked by an active lock:

```bash
$ prodigy resume <job_id>
Error: Resume already in progress for job <job_id>
Lock held by: PID 12345 on hostname (acquired 2025-01-11 10:30:00 UTC)
Please wait for the other process to complete, or use --force to override.
```

**Stale Lock Detection**:
- Platform-specific process existence check (Unix: `kill -0`, Windows: `tasklist`)
- If holding process is no longer running, lock is automatically removed
- New resume attempt succeeds after stale lock cleanup

**Safety Guarantees**:
- **Data Corruption Prevention**: Only one process can modify job state at a time
- **No Duplicate Work**: Work items cannot be processed by multiple agents concurrently
- **Consistent State**: Checkpoint updates are serialized
- **Automatic Cleanup**: RAII pattern ensures locks are released even on errors
- **Cross-Host Safety**: Hostname in lock prevents conflicts across machines

### Example Resume Workflow

Here's a typical workflow for resuming an interrupted MapReduce job:

1. **Workflow interrupted** during reduce phase (e.g., laptop closed, terminal killed)
2. **Find job ID** with `prodigy sessions list` or `prodigy resume-job list`
3. **Resume execution** using `prodigy resume <session-or-job-id>`
4. **Prodigy loads checkpoint** from `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`
5. **Reconstructs execution state** with all variables, work items, and progress
6. **Continues from last completed step** in reduce phase (or re-processes in-progress map items)

### Best Practices

**Designing Resumable Workflows**:
- Make setup commands idempotent (safe to run multiple times)
- Avoid side effects that can't be safely repeated
- Use descriptive work item IDs for easier debugging
- Test resume behavior by intentionally interrupting workflows

**Troubleshooting Resume Issues**:
- Check checkpoint files exist: `ls ~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`
- List available sessions: `prodigy sessions list` (shows session IDs, job IDs, and status)
- List available checkpoints: `prodigy checkpoints list`
- Show detailed checkpoint info: `prodigy checkpoints show <job_id>`
- Validate checkpoint integrity: `prodigy checkpoints validate <checkpoint_id>`
- Review event logs: `prodigy events <job_id>`
- Check for stale locks: `ls ~/.prodigy/resume_locks/`
- Clean old checkpoints: `prodigy checkpoints clean --all`

### Troubleshooting Stale Locks

Resume locks can occasionally become "stale" if a resume process crashes or is forcefully terminated before releasing its lock. Prodigy includes automatic detection and cleanup, but here's how to troubleshoot persistent lock issues:

**Identifying Stale Locks**:

When a resume is blocked by a lock, you'll see this error message:

```bash
$ prodigy resume mapreduce-1234567890
Error: Resume already in progress for job mapreduce-1234567890
Lock held by: PID 12345 on hostname (acquired 2025-01-11 10:30:00 UTC)
Please wait for the other process to complete, or use --force to override.
```

**Automatic Cleanup** (src/cook/execution/resume_lock.rs:96-120):

Prodigy automatically detects stale locks using platform-specific process existence checks:

- **Unix/Linux/macOS**: Uses `kill -0 <PID>` to check if process is running
- **Windows**: Uses `tasklist` to verify process existence
- **Cross-host detection**: Compares hostname in lock file to current system

If the holding process is no longer running, the lock is automatically removed and resume proceeds.

**Manual Verification Steps**:

1. **Check if the process is actually running**:
   ```bash
   # Extract PID from error message (e.g., 12345)
   ps aux | grep 12345
   ```

2. **Examine the lock file**:
   ```bash
   cat ~/.prodigy/resume_locks/mapreduce-1234567890.lock
   ```

   Lock file contains:
   ```json
   {
     "job_id": "mapreduce-1234567890",
     "process_id": 12345,
     "hostname": "my-laptop",
     "acquired_at": "2025-01-11T10:30:00Z"
   }
   ```

3. **Verify process on the same host**:
   ```bash
   # If hostname matches your current system
   ps -p 12345

   # If process doesn't exist, the lock is stale
   # Retry resume - it will auto-clean the stale lock
   ```

4. **Cross-host lock scenario**:
   - If `hostname` in lock file differs from your current system, the process may be running elsewhere
   - Verify the other system is actually running the resume
   - If other system crashed or is offline, the lock is stale

**Manual Lock Removal** (last resort):

If automatic cleanup fails (rare), you can manually remove the lock:

```bash
# ⚠️ Only do this if you're certain the process is dead!
rm ~/.prodigy/resume_locks/mapreduce-1234567890.lock

# Then retry resume
prodigy resume mapreduce-1234567890
```

**Prevention Tips**:

- Always let resume processes complete naturally (don't use `kill -9`)
- Use `Ctrl+C` for graceful interruption (triggers RAII cleanup)
- The RAII pattern ensures locks are released even on panic or early exit
- Stale locks are rare in normal operation

**When Automatic Cleanup Fails**:

Test coverage (tests/concurrent_resume_test.rs:77-105) validates stale lock detection. If you encounter persistent stale locks:

1. Verify the lock file has valid JSON structure
2. Check file permissions on `~/.prodigy/resume_locks/`
3. Confirm platform-specific process check is working (`kill -0` on Unix, `tasklist` on Windows)
4. Report issue with lock file contents and platform details

**Available checkpoint commands** (src/cli/args.rs:363-413):
- `prodigy checkpoints list` - List all available checkpoints
- `prodigy checkpoints show <job_id>` - Show detailed checkpoint information
- `prodigy checkpoints validate <checkpoint_id>` - Verify checkpoint integrity
- `prodigy checkpoints clean` - Delete checkpoints for completed workflows

### See Also

- [Dead Letter Queue (DLQ)](./dead-letter-queue-dlq.md) - Managing failed work items during resume operations
- [Event Tracking](./event-tracking.md) - Understanding events logged during checkpoint creation and resume
- [Global Storage Architecture](./global-storage-architecture.md) - Storage locations for checkpoints and state files
- [Troubleshooting](./troubleshooting.md) - General MapReduce workflow debugging techniques

