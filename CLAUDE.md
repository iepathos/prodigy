# Prodigy Session Documentation for Claude

This document explains how Prodigy manages sessions and provides information to Claude during development iterations.

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It manages session state, tracks execution progress, and supports parallel execution through MapReduce patterns.

## Directory Structure

```
.prodigy/
├── session_state.json         # Current session state and timing
├── validation-result.json     # Workflow validation results
├── events/                    # MapReduce event logs
│   └── {job_id}/             # Job-specific events
│       ├── {timestamp}.json  # Individual event records
│       └── checkpoint.json   # Job checkpoint for resumption
└── dlq/                      # Dead Letter Queue for failed items
    └── {job_id}.json         # Failed work items for retry
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

When executing Claude commands, Prodigy sets this environment variable:
- `PRODIGY_AUTOMATION="true"` - Signals automated execution mode

## MapReduce Features

### Parallel Execution
Prodigy supports parallel execution of work items across multiple Claude agents:
- Each agent runs in an isolated git worktree
- Work items are distributed automatically
- Results are aggregated in the reduce phase
- Failed items can be retried via the DLQ

### Event Tracking
Events are logged to `.prodigy/events/{job_id}/` for debugging:
- Agent lifecycle events (started, completed, failed)
- Work item processing status
- Checkpoint saves for resumption
- Error details with correlation IDs

### Dead Letter Queue (DLQ)
Failed work items are stored in `.prodigy/dlq/` for manual review and retry:
- Contains the original work item data
- Includes failure reason and timestamp
- Can be reprocessed with `prodigy dlq retry`

## Workflow Execution

### Command Types
Prodigy supports several command types in workflows:
- `claude:` - Execute Claude commands via Claude Code CLI
- `shell:` - Run shell commands
- `test:` - Run tests with special handling

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
- `prodigy resume-job` - Resume MapReduce jobs
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
- Resume with `prodigy resume-job`
- Review checkpoint in `.prodigy/events/{job_id}/checkpoint.json`

### Worktree Problems
- List worktrees with `prodigy worktree ls`
- Clean stuck worktrees with `prodigy worktree clean -f`
- Check `~/.prodigy/worktrees/` for orphaned directories