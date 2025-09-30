# Prodigy Session Documentation for Claude

This document explains how Prodigy manages sessions and provides information to Claude during development iterations.

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It manages session state, tracks execution progress, and supports parallel execution through MapReduce patterns.

## Error Handling Guidelines (Spec 101)

### Production Code Requirements

**CRITICAL**: Production code must NEVER use `unwrap()` or `panic!()` directly. All error conditions must be handled gracefully using Result types and the `?` operator.

#### Prohibited Patterns in Production Code
```rust
// NEVER DO THIS in production code:
let value = some_option.unwrap();        // Will panic on None
let result = some_result.unwrap();       // Will panic on Err
panic!("Something went wrong");          // Explicit panic
```

#### Required Patterns for Error Handling
```rust
// DO THIS instead:
let value = some_option.context("Failed to get value")?;
let result = some_result.context("Operation failed")?;
return Err(anyhow!("Something went wrong"));
```

#### Safe Fallback Patterns
```rust
// For Options:
let value = some_option.unwrap_or(default_value);
let value = some_option.unwrap_or_else(|| compute_default());
let value = some_option.map_or(default, |v| transform(v));

// For Results:
let value = some_result.unwrap_or(default_value);
let value = some_result.unwrap_or_else(|e| {
    log::warn!("Failed with error: {}, using default", e);
    default_value
});
```

### Test Code Exceptions

Test code MAY use `unwrap()` and `panic!()` as they serve as appropriate test failure mechanisms:
```rust
#[test]
fn test_something() {
    let result = function_under_test();
    assert!(result.is_ok());
    let value = result.unwrap();  // OK in tests - will fail test on error
}
```

### Static Compilation Patterns

For compile-time constants like regex patterns that are known to be valid:
```rust
// OK - Regex is statically known to be valid
static PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d+$").expect("Invalid regex pattern")
});
```

### Error Context Best Practices

1. **Always add context** when propagating errors:
   ```rust
   file_operation()
       .context("Failed to perform file operation")?;
   ```

2. **Include relevant details** in error messages:
   ```rust
   read_file(&path)
       .with_context(|| format!("Failed to read file: {}", path.display()))?;
   ```

3. **Use appropriate error types** for each module:
   - Storage operations: `StorageError`
   - Worktree operations: `WorktreeError`
   - Command execution: `CommandError`
   - General operations: `anyhow::Error`

### Troubleshooting Common Issues

#### Issue: "thread 'main' panicked at..."
**Cause**: An `unwrap()` or `panic!()` in production code
**Solution**: Find the location in the stack trace and replace with proper error handling

#### Issue: "called `Option::unwrap()` on a `None` value"
**Cause**: Attempting to unwrap a None Option
**Solution**: Use `unwrap_or()`, `unwrap_or_else()`, or `?` operator with context

#### Issue: "called `Result::unwrap()` on an `Err` value"
**Cause**: Attempting to unwrap an Err Result
**Solution**: Use `?` operator to propagate the error or handle it explicitly

### Validation and Testing

All error handling changes must:
1. Pass existing tests without modification
2. Include new tests for error paths
3. Maintain backward compatibility
4. Provide clear error messages for debugging

## Custom Merge Workflows

Prodigy now supports configurable merge workflows that execute when merging worktree changes back to the main branch. This allows you to customize the merge process with your own validation, conflict resolution, and post-merge steps.

### Merge Workflow Configuration

You can define a custom merge workflow in your YAML file using the `merge` block:

```yaml
# Custom merge workflow
merge:
  commands:
    - shell: "git fetch origin"
    - shell: "git merge origin/main"  # Merge main into worktree first
    - shell: "cargo test"              # Run tests
    - shell: "cargo clippy"            # Run linting
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
    - shell: "echo 'Successfully merged ${merge.worktree}'"
  timeout: 600  # 10 minutes timeout for merge operations
```

### Merge-Specific Variables

The following variables are available in merge workflows:
- `${merge.worktree}` - Name of the worktree being merged
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (usually main or master)
- `${merge.session_id}` - Session ID for correlation

### Claude Merge Streaming

The Claude merge command now respects the same verbosity settings as other workflow commands:
- With `-v` (verbose) or higher, you'll see real-time JSON streaming output from Claude
- Set `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` to force streaming output regardless of verbosity
- This provides full visibility into Claude's merge operations and any tool invocations

### Example Workflows

#### Pre-merge Validation
```yaml
merge:
  commands:
    - shell: "cargo build --release"
    - shell: "cargo test --all"
    - shell: "cargo fmt --check"
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

#### Conflict Resolution Strategy
```yaml
merge:
  commands:
    - shell: "git merge origin/main --no-commit"
    - claude: "/resolve-conflicts"
    - shell: "git add -A"
    - shell: "git commit -m 'Merge main and resolve conflicts'"
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

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

## Global Storage Architecture

### Overview
Prodigy uses a global storage architecture by default, storing all events, state, and DLQ data in `~/.prodigy/`. This enables:
- **Cross-worktree event aggregation**: Multiple worktrees working on the same job share event logs
- **Persistent state management**: Job checkpoints survive worktree cleanup
- **Centralized monitoring**: All job data accessible from a single location
- **Efficient storage**: Deduplication across worktrees

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
- Supports automatic reprocessing via `prodigy dlq retry`
- Configurable parallel execution and resource limits
- Shared across worktrees for centralized failure tracking

#### DLQ Retry
The `prodigy dlq retry` command allows you to retry failed items:

```bash
# Retry all failed items for a job
prodigy dlq retry <job_id>

# Retry with custom parallelism (default: 5)
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run
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

### Branch Tracking (Spec 110)
Prodigy tracks the original branch when creating worktrees to enable intelligent merge behavior:

**Original Branch Detection**:
- When creating a worktree, Prodigy captures the current branch as `original_branch`
- For feature branches: Tracks the exact branch name (e.g., `feature/my-feature`)
- For detached HEAD: Falls back to repository's default branch (main or master)
- Stored in worktree state for lifetime of the session

**Merge Target Logic**:
- Default behavior: Merge back to the tracked `original_branch`
- If original branch was deleted: Fall back to default branch (main/master)
- Merge target is displayed in the merge confirmation prompt
- Example: "Merge session-abc123 to feature/my-feature? [y/N]"

**Special Cases**:
- **Feature Branch Workflow**: Worktree created from `feature/ui-updates` merges back to `feature/ui-updates`
- **Detached HEAD**: Worktree tracks default branch (main/master) as fallback
- **Deleted Branch**: If original branch is deleted, falls back to main/master
- **Branch Rename**: Uses branch name at worktree creation time

**Implementation Details**:
- `WorktreeManager::create_session()` captures original branch using `git rev-parse --abbrev-ref HEAD`
- `WorktreeManager::get_merge_target()` determines merge target with fallback logic
- Merge target is shown in orchestrator's completion prompt for user confirmation

## Available Commands

Prodigy CLI commands:
- `prodigy run` - Execute a workflow (with `--resume` flag for checkpoint-based resume)
- `prodigy resume` - Resume an interrupted workflow from checkpoint
- `prodigy worktree` - Manage git worktrees
- `prodigy init` - Initialize Claude commands
- `prodigy resume-job` - Resume MapReduce jobs with enhanced options
- `prodigy events` - View execution events
- `prodigy dlq` - Manage and retry failed work items
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
- Retry failed items with `prodigy dlq retry <job_id>`
- Resume MapReduce jobs with `prodigy resume-job <job_id>`
- Review checkpoint in `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`

### Worktree Problems
- List worktrees with `prodigy worktree ls`
- Clean stuck worktrees with `prodigy worktree clean -f`
- Check `~/.prodigy/worktrees/` for orphaned directories