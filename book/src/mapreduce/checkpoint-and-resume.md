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

