# Prodigy Session Documentation for Claude

This document explains how Prodigy manages sessions and provides information to Claude during development iterations.

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It manages session state, tracks execution progress, and supports parallel execution through MapReduce patterns.

## Directory Structure

### Local Storage (Legacy)
```
.prodigy/
├── session_state.json         # Current session state and timing
├── validation-result.json     # Workflow validation results
├── events/                    # MapReduce event logs (legacy)
│   └── {job_id}/             # Job-specific events
│       ├── {timestamp}.json  # Individual event records
│       └── checkpoint.json   # Job checkpoint for resumption
└── dlq/                      # Dead Letter Queue for failed items (legacy)
    └── {job_id}.json         # Failed work items for retry
```

### Global Storage (Default)
```
~/.prodigy/
├── events/
│   └── {repo_name}/          # Events grouped by repository
│       └── {job_id}/         # Job-specific events
│           └── events-{timestamp}.jsonl  # Event log files
├── dlq/
│   └── {repo_name}/          # DLQ grouped by repository
│       └── {job_id}/         # Job-specific failed items
├── state/
│   └── {repo_name}/          # State grouped by repository
│       └── mapreduce/        # MapReduce job states
│           └── jobs/
│               └── {job_id}/ # Job-specific checkpoints
└── worktrees/
    └── {repo_name}/          # Git worktrees for sessions
```

## Session Management

### Session State (`session_state.json`)
Tracks the current cooking session:
```json
{
  "session_id": "cook-1234567890",
  "status": "InProgress|Completed|Failed",
  "started_at": "2024-01-01T12:00:00Z",
  "iterations_completed": 2,
  "files_changed": 5,
  "worktree_name": "prodigy-session-123",
  "iteration_timings": [[1, {"secs": 120, "nanos": 0}]],
  "command_timings": [["claude: /prodigy-lint", {"secs": 60, "nanos": 0}]]
}
```

### Environment Variables

When executing Claude commands, Prodigy sets these environment variables:
- `PRODIGY_AUTOMATION="true"` - Signals automated execution mode
- `PRODIGY_USE_LOCAL_STORAGE="true"` - Force local storage instead of global (optional)
- `PRODIGY_REMOVE_LOCAL_AFTER_MIGRATION="true"` - Remove local storage after migration (optional)

## Global Storage Architecture

### Overview
Prodigy uses a global storage architecture by default, storing all events, state, and DLQ data in `~/.prodigy/`. This enables:
- **Cross-worktree event aggregation**: Multiple worktrees working on the same job share event logs
- **Persistent state management**: Job checkpoints survive worktree cleanup
- **Centralized monitoring**: All job data accessible from a single location
- **Efficient storage**: Deduplication across worktrees

### Migration from Local Storage
When upgrading from an older version:
- Prodigy automatically detects existing local storage
- Data is migrated to global storage on first run
- Set `PRODIGY_REMOVE_LOCAL_AFTER_MIGRATION=true` to remove local storage after migration
- Use `PRODIGY_USE_LOCAL_STORAGE=true` to continue using local storage

## MapReduce Features

### Parallel Execution
Prodigy supports parallel execution of work items across multiple Claude agents:
- Each agent runs in an isolated git worktree
- Work items are distributed automatically
- Results are aggregated in the reduce phase
- Failed items can be retried via the DLQ

### Event Tracking
Events are logged to `~/.prodigy/events/{repo_name}/{job_id}/` for debugging:
- Agent lifecycle events (started, completed, failed)
- Work item processing status
- Checkpoint saves for resumption
- Error details with correlation IDs
- Cross-worktree event aggregation for parallel jobs

### Dead Letter Queue (DLQ)
Failed work items are stored in `~/.prodigy/dlq/{repo_name}/{job_id}/` for manual review and retry:
- Contains the original work item data
- Includes failure reason and timestamp
- **Important**: DLQ reprocessing is not yet implemented
  - The `prodigy dlq reprocess` command exists but returns error: 'DLQ reprocessing is not yet implemented'
  - Failed items must be manually reviewed and re-run if needed
- Shared across worktrees for centralized failure tracking

## Workflow Execution

### Command Types
Prodigy supports several command types in workflows:
- `claude:` - Execute Claude commands via Claude Code CLI
- `shell:` - Run shell commands
- `test:` - **Deprecated** - This command type is deprecated and will show a warning. Use `shell:` instead for running tests

### Variable Interpolation
Workflows support variable interpolation:
- `${item.field}` - Access work item fields in MapReduce
- `${shell.output}` - Capture command output
- `${map.results}` - Access map phase results in reduce
- `$ARG` - Pass arguments from command line

### Error Handling
Commands can specify error handling behavior:
- `on_failure:` - Commands to run on failure
- `max_attempts:` - Retry count
- `fail_workflow:` - Whether to fail entire workflow
- `commit_required:` - Whether a git commit is expected

## Git Integration

### Worktree Management
Prodigy uses git worktrees for isolation:
- Each session gets its own worktree
- Located in `~/.prodigy/worktrees/{project-name}/`
- Automatic branch creation and management
- Clean merge back to parent branch

### Commit Tracking
All changes are tracked via git commits:
- Each successful command creates a commit
- Commit messages include command details
- Full audit trail of all modifications

## Available Commands

Prodigy CLI commands:
- `prodigy cook` - Execute a workflow
- `prodigy worktree` - Manage git worktrees
- `prodigy init` - Initialize Claude commands
- `prodigy resume-job` - Display MapReduce job status (**Note**: Actual job resumption is not yet implemented, this command only prints job status)
- `prodigy events` - View execution events
- `prodigy dlq` - Manage failed work items

## Best Practices

1. **Session Hygiene**: Clean up completed worktrees with `prodigy worktree clean`
2. **Error Recovery**: Check DLQ for failed items after MapReduce jobs
3. **Workflow Design**: Keep workflows simple and focused
4. **Testing**: Always include test steps in workflows
5. **Monitoring**: Use `--verbose` flag for detailed execution logs

## Limitations

- No automatic context analysis or generation
- Each iteration runs independently (no memory between sessions)
- Context directory feature is planned but not implemented
- Limited to Claude commands available in `.claude/commands/`

## Troubleshooting

### Session Issues
- Check `.prodigy/session_state.json` for session status
- View events in `.prodigy/events/` for detailed logs
- Use `--verbose` flag for more output

### MapReduce Failures
- Check `.prodigy/dlq/` for failed items
- View job status with `prodigy resume-job` (actual resumption not yet implemented)
- Review checkpoint in `.prodigy/events/{job_id}/checkpoint.json`
- **Note**: To retry failed items, you must currently re-run the workflow manually

### Worktree Problems
- List worktrees with `prodigy worktree ls`
- Clean stuck worktrees with `prodigy worktree clean -f`
- Check `~/.prodigy/worktrees/` for orphaned directories