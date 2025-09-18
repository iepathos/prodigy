# Prodigy Session Documentation for Claude

This document explains how Prodigy manages sessions and provides information to Claude during development iterations.

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It manages session state, tracks execution progress, and supports parallel execution through MapReduce patterns.

## MapReduce Workflow Syntax

Prodigy supports MapReduce workflows for massive parallel processing. The syntax follows the specification in the whitepaper:

### Basic MapReduce Structure
```yaml
name: workflow-name
mode: mapreduce

# Optional setup phase
setup:
  - shell: "generate-work-items.sh"
  - shell: "analyze-codebase --output items.json"

# Map phase: Process items in parallel
map:
  input: "items.json"          # JSON file with array of items
  json_path: "$.items[*]"      # JSONPath to extract items

  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

  max_parallel: 10             # Number of concurrent agents
  filter: "item.score >= 5"    # Optional: filter items
  sort_by: "item.priority DESC" # Optional: process order
  max_items: 100               # Optional: limit items per run

# Reduce phase: Aggregate results
reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"
```

### Key Syntax Changes from Previous Versions
- **Agent Template**: No longer uses nested `commands` array - commands are directly under `agent_template`
- **Reduce Phase**: Commands are directly under `reduce`, not nested under `commands`
- **Error Handling**: Simplified `on_failure` syntax without `max_attempts` and `fail_workflow`
- **Removed Parameters**: No longer supports `timeout_per_agent`, `retry_on_failure`, or other deprecated parameters

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
- `PRODIGY_CLAUDE_STREAMING="true"` - Enables streaming mode for Claude commands (when verbosity >= 1)
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
Failed work items are stored in `~/.prodigy/dlq/{repo_name}/{job_id}/` for review and retry:
- Contains the original work item data
- Includes failure reason and timestamp
- Supports automatic reprocessing via `prodigy dlq reprocess`
- Configurable parallel execution and resource limits
- Shared across worktrees for centralized failure tracking

#### DLQ Reprocessing
The `prodigy dlq reprocess` command allows you to retry failed items:

```bash
# Reprocess all failed items for a job
prodigy dlq reprocess <job_id>

# Reprocess with custom parallelism (default: 5)
prodigy dlq reprocess <job_id> --max-parallel 10

# Dry run to see what would be reprocessed
prodigy dlq reprocess <job_id> --dry-run
```

Features:
- Streams items to avoid memory issues with large queues
- Respects original workflow's max_parallel setting
- Preserves correlation IDs for tracking
- Updates DLQ state (removes successful, keeps failed)
- Supports interruption and resumption

## Workflow Execution

### Command Types
Prodigy supports several command types in workflows:
- `claude:` - Execute Claude commands via Claude Code CLI
- `shell:` - Run shell commands
- `goal_seek:` - Run goal-seeking operations with validation
- `foreach:` - Iterate over lists with nested commands

### Variable Interpolation
Workflows support variable interpolation:
- `${item.field}` - Access work item fields in MapReduce
- `${shell.output}` - Capture command output
- `${map.results}` - Access map phase results in reduce
- `$ARG` - Pass arguments from command line

### Error Handling
Commands can specify error handling behavior:
- `on_failure:` - Commands to run on failure
- `commit_required:` - Whether a git commit is expected

### Claude Streaming Output Control

Prodigy provides fine-grained control over Claude interaction visibility through verbosity levels:

**Default mode (verbosity = 0):**
- Clean, minimal output showing only progress and results
- No Claude JSON streaming output displayed
- Optimal for production workflows and CI/CD

**Verbose mode (verbosity >= 1, `-v` flag):**
- Shows Claude streaming JSON output in real-time
- Enables debugging of Claude interactions
- Useful for development and troubleshooting

**Environment Override:**
- Set `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` to force streaming output regardless of verbosity
- Useful for debugging specific runs without changing command flags

This design ensures clean output by default while preserving debugging capabilities when needed.

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
- `prodigy cook` - Execute a workflow (with `--resume` flag for checkpoint-based resume)
- `prodigy resume` - Resume an interrupted workflow from checkpoint
- `prodigy worktree` - Manage git worktrees
- `prodigy init` - Initialize Claude commands
- `prodigy resume-job` - Resume MapReduce jobs with enhanced options
- `prodigy events` - View execution events
- `prodigy dlq` - Manage and reprocess failed work items
- `prodigy checkpoints` - Manage workflow checkpoints
- `prodigy sessions` - View and manage session state

## Best Practices

1. **Session Hygiene**: Clean up completed worktrees with `prodigy worktree clean`
2. **Error Recovery**: Check DLQ for failed items after MapReduce jobs
3. **Workflow Design**: Keep workflows simple and focused
4. **Testing**: Always include test steps in workflows
5. **Monitoring**: Use verbosity flags for appropriate detail level:
   - Default: Clean output for production use
   - `-v`: Claude streaming output for debugging interactions
   - `-vv`/`-vvv`: Additional internal logs and tracing

## Limitations

- No automatic context analysis or generation
- Each iteration runs independently (memory preserved via checkpoints and state)
- Context directory feature is planned but not implemented
- Limited to Claude commands available in `.claude/commands/`
- Resume functionality requires workflow files to be present

## Troubleshooting

### Session Issues
- Check `.prodigy/session_state.json` for session status
- View events in `.prodigy/events/` for detailed logs
- Use verbosity flags for debugging:
  - `-v`: Shows Claude streaming output
  - `-vv`: Adds debug logs
  - `-vvv`: Adds trace-level logs

### MapReduce Failures
- Check `.prodigy/dlq/` for failed items
- Reprocess failed items with `prodigy dlq reprocess <job_id>`
- Resume MapReduce jobs with `prodigy resume-job <job_id>`
- Review checkpoint in `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`

### Worktree Problems
- List worktrees with `prodigy worktree ls`
- Clean stuck worktrees with `prodigy worktree clean -f`
- Check `~/.prodigy/worktrees/` for orphaned directories