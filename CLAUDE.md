# Prodigy Project Documentation

This document contains Prodigy-specific documentation for Claude. General development guidelines are in `~/.claude/CLAUDE.md`.

## Overview

Prodigy is a workflow orchestration tool that executes Claude commands through structured YAML workflows. It manages session state, tracks execution progress, and supports parallel execution through MapReduce patterns.

## Error Handling (Spec 101, 168)

### Core Rules
- **Production code**: Never use `unwrap()` or `panic!()` - use Result types and `?` operator
- **Test code**: May use `unwrap()` and `panic!()` for test failures
- **Static patterns**: Compile-time constants (like regex) may use `expect()`

### Error Types
- Storage: `StorageError`
- Worktree: `WorktreeError`
- Command execution: `CommandError`
- General: `anyhow::Error`

### Context Preservation
Prodigy uses Stillwater's `ContextError<E>` to preserve operation context through the call stack:

```rust
use prodigy::cook::error::ResultExt;

fn process_item(id: &str) -> Result<(), ContextError<ProcessError>> {
    create_worktree(id).with_context(|| format!("Creating worktree for {}", id))?;
    execute_commands(id).context("Executing commands")?;
    Ok(())
}
```

**Benefits**: Full context trail in error messages, DLQ integration, zero runtime overhead on success path.

## Claude Observability (Spec 121)

### JSON Log Tracking
Claude Code creates detailed JSON logs at `~/.local/state/claude/logs/session-{id}.json` containing:
- Complete message history and tool invocations
- Token usage and session metadata
- Error details and stack traces

**Access logs:**
- Verbose mode: `prodigy run workflow.yml -v` shows log path after each command
- Programmatically: `result.json_log_location()`
- MapReduce events: `AgentCompleted` and `AgentFailed` include `json_log_location`
- DLQ items: `FailureDetail` preserves log location

**Debug failed agents:**
```bash
# Get log path from DLQ
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Inspect the log
cat /path/to/log.json | jq '.messages[-3:]'
```

## Custom Merge Workflows

Define custom merge workflows with validation, testing, and conflict resolution:

```yaml
merge:
  commands:
    - shell: "git fetch origin && git merge origin/main"
    - shell: "cargo test && cargo clippy"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
  timeout: 600
```

**Available variables:**
- `${merge.worktree}` - Worktree name
- `${merge.source_branch}` - Source branch (worktree)
- `${merge.target_branch}` - Target branch (original branch)
- `${merge.session_id}` - Session ID

**Streaming**: Use `-v` flag or set `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true` for real-time output.

## MapReduce Workflows

### Basic Structure
```yaml
name: workflow-name
mode: mapreduce

setup:
  - shell: "generate-work-items.sh"

map:
  input: "items.json"
  json_path: "$.items[*]"
  max_parallel: 10

  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total}'"
```

### Commit Validation (Spec 163)
Commands with `commit_required: true` enforce commit creation. Agent fails if no commit is made.

```yaml
agent_template:
  - shell: |
      echo "data" > file.txt
      git add file.txt
      git commit -m "Add data"
    commit_required: true
```

**Validation behavior:**
- HEAD SHA checked before/after command
- No new commits → agent fails with `CommitValidationFailed`
- Failed agents added to DLQ with full context
- Agents with commits: merged to parent; without commits: cleaned up

### Checkpoint & Resume (Spec 134)
Prodigy checkpoints all phases (setup, map, reduce) for recovery:

**Resume commands:**
```bash
prodigy resume <session-or-job-id>
prodigy resume-job <job_id>
```

**State preservation:**
- Setup: Checkpoint after completion
- Map: Checkpoint after configurable work items processed
- Reduce: Checkpoint after each command
- Variables, outputs, and agent results preserved

**Storage:** `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`

### Concurrent Resume Protection (Spec 140)
RAII-based locking prevents multiple resume processes:
- Exclusive lock acquired automatically before resume
- Lock released on completion or failure
- Stale locks (crashed processes) auto-detected and cleaned
- Lock files: `~/.prodigy/resume_locks/{id}.lock`

### Worktree Isolation (Spec 127)
All phases execute in isolated worktrees:

```
original_branch → parent worktree (session-xxx)
                  ├→ Setup executes here
                  ├→ Agent worktrees (branch from parent, merge back)
                  ├→ Reduce executes here
                  └→ User prompt: Merge to {original_branch}?
```

**Benefits:** Main repo untouched, parallel execution, full isolation, user-controlled merge.

### Cleanup Handling (Spec 136)
Agent success independent of cleanup status:
- Successful agents preserved even if cleanup fails
- Orphaned worktrees registered: `~/.prodigy/orphaned_worktrees/{repo}/{job_id}.json`
- Clean orphaned: `prodigy worktree clean-orphaned <job_id>`

### Dead Letter Queue (DLQ)
Failed items stored in `~/.prodigy/dlq/{repo}/{job_id}/` with:
- Original work item data
- Failure reason, timestamp, error context
- JSON log location for debugging

**Retry failed items:**
```bash
prodigy dlq retry <job_id> [--max-parallel N] [--dry-run]
```

## Environment Variables (Spec 120)

Define variables at workflow root:

```yaml
env:
  PROJECT_NAME: "prodigy"
  API_KEY:
    secret: true
    value: "sk-abc123"
  DATABASE_URL:
    default: "postgres://localhost/dev"
    prod: "postgres://prod-server/db"
```

**Usage:** `$VAR` or `${VAR}` in all phases (setup, map, reduce, merge)
**Profiles:** `prodigy run workflow.yml --profile prod`
**Secrets:** Masked in logs, errors, events, checkpoints

## Storage Architecture

### Global Storage (Default)
```
~/.prodigy/
├── events/{repo}/{job_id}/         # Event logs
├── dlq/{repo}/{job_id}/            # Failed items
├── state/{repo}/mapreduce/jobs/    # Checkpoints
├── sessions/                       # Unified sessions
├── resume_locks/                   # Resume locks
└── worktrees/{repo}/               # Git worktrees
```

### Session Management
Sessions stored as `UnifiedSession` in `~/.prodigy/sessions/{id}.json`:
- Status: Running|Paused|Completed|Failed|Cancelled
- Metadata: Execution timing, progress, step tracking
- Checkpoints: Full state snapshots for resume
- Workflow/MapReduce data: Variables, results, worktree info

**Session-Job mapping:** Bidirectional mapping enables resume with either ID.

## Git Integration

### Branch Tracking (Spec 110)
- Worktrees capture original branch at creation
- Merge targets original branch (not hardcoded main/master)
- Fallback to default branch if original deleted
- Branch shown in merge confirmation prompt

### Worktree Management
- Located in `~/.prodigy/worktrees/{project}/`
- Automatic branch creation and cleanup
- Commit tracking with full audit trail

## Workflow Execution

### Command Types
- `claude:` - Execute Claude commands
- `shell:` - Run shell commands
- `goal_seek:` - Goal-seeking with validation
- `foreach:` - Iterate with nested commands

### Variable Interpolation
- `${item.field}` - Work item fields
- `${shell.output}` - Command output
- `${map.results}` - Map phase results
- `$ARG` - CLI arguments

### Claude Streaming Control
- **Default (verbosity 0):** Clean output, no streaming
- **Verbose (`-v`):** Real-time JSON streaming
- **Override:** `PRODIGY_CLAUDE_CONSOLE_OUTPUT=true`

### Environment Variables
- `PRODIGY_AUTOMATION=true` - Signals automated execution
- `PRODIGY_CLAUDE_STREAMING=false` - Disable JSON streaming

## Best Practices

1. **Session Hygiene**: Clean completed worktrees: `prodigy worktree clean`
2. **Error Recovery**: Check DLQ after MapReduce: `prodigy dlq show <job_id>`
3. **Workflow Design**: Keep simple and focused, include test steps
4. **Monitoring**: Use appropriate verbosity (`-v` for Claude output, `-vv`/`-vvv` for internals)
5. **Documentation**: Book workflow includes automatic drift/gap detection

## Common Commands

Use `prodigy --help` for full CLI reference. Key commands:
- `prodigy run <workflow.yml>` - Execute workflow
- `prodigy resume <id>` - Resume interrupted workflow
- `prodigy dlq retry <job_id>` - Retry failed items
- `prodigy worktree clean` - Clean completed worktrees
- `prodigy sessions list` - List all sessions
- `prodigy logs --latest` - View recent Claude logs

## Troubleshooting

### MapReduce Issues
```bash
# Check failed items
prodigy dlq show <job_id>

# Get Claude log from failed agent
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Resume from checkpoint
prodigy resume <job_id>

# Retry failed items
prodigy dlq retry <job_id>
```

### Worktree Issues
```bash
# List worktrees
prodigy worktree ls

# Clean orphaned worktrees
prodigy worktree clean-orphaned <job_id>

# Force clean stuck worktrees
prodigy worktree clean -f
```

### Debug with Verbosity
- `-v` - Shows Claude streaming output
- `-vv` - Adds debug logs
- `-vvv` - Adds trace-level logs

### View Claude Logs
```bash
# Latest log
prodigy logs --latest

# Follow live execution
prodigy logs --latest --tail

# Analyze completed log
cat ~/.claude/projects/.../uuid.jsonl | jq -c 'select(.type == "assistant")'
```

## Limitations

- No automatic context generation
- Iterations run independently (state via checkpoints)
- Limited to Claude commands in `.claude/commands/`
- Resume requires workflow files present
