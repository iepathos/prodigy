## Best Practices for Debugging

This guide provides practical techniques for diagnosing and resolving issues in Prodigy workflows.

### Quick Debugging Checklist

1. **Start simple**: Test commands individually before adding to workflow
2. **Use verbosity flags**: `-v` for Claude interactions, `-vv` for debug logs, `-vvv` for trace
3. **Check recent logs**: `prodigy logs --latest` for the last Claude execution
4. **Enable environment variables** for detailed output (see below)
5. **Validate input data** before running MapReduce workflows
6. **Test incrementally**: Add commands one at a time and verify after each
7. **Version control**: Commit working workflows before making changes

### Debugging Environment Variables

Control debugging output and logging behavior with environment variables (Source: `src/config/mod.rs:116`, `src/cook/execution/claude.rs:429-435`):

#### `PRODIGY_LOG_LEVEL`

**Purpose**: Control log verbosity
**Values**: `trace`, `debug`, `info`, `warn`, `error`
**Default**: `info`

```bash
export PRODIGY_LOG_LEVEL=debug
prodigy run workflow.yml
```

Use `debug` or `trace` to see internal state transitions, command execution details, and variable interpolation.

#### `PRODIGY_CLAUDE_CONSOLE_OUTPUT`

**Purpose**: Force streaming output regardless of verbosity
**Values**: `true`, `false`
**Default**: Not set (respects verbosity level)

```bash
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=true
prodigy run workflow.yml  # Shows Claude output even without -v flag
```

Useful for debugging specific runs without changing command flags.

#### `PRODIGY_CLAUDE_STREAMING`

**Purpose**: Control Claude JSON streaming mode
**Values**: `true`, `false`
**Default**: `true`

```bash
export PRODIGY_CLAUDE_STREAMING=false  # Disable JSON streaming
prodigy run workflow.yml
```

Streaming is enabled by default for auditability. Only disable in CI/CD environments with storage constraints.

**See also**: [Environment Variables](../configuration/environment-variables.md) for complete reference.

### Variable Interpolation Debugging

**Techniques for diagnosing variable issues**:

1. **Use echo statements** to verify variable values:
   ```yaml
   commands:
     - shell: "echo 'Debug: item=${item.name}, path=${item.path}'"
     - claude: "/process ${item.name}"
   ```

2. **Capture command outputs** with `capture_output` for later use:
   ```yaml
   commands:
     - shell: "git rev-parse HEAD"
       capture_output: current_commit
     - shell: "echo 'Current commit: ${current_commit}'"
   ```

3. **Enable verbose mode** (`-v`) to see variable interpolation in real-time
4. **Check variable scope**: Distinguish step-level vs workflow-level variables

**See also**: [Common Issues](index.md#variable-interpolation-issues) for variable troubleshooting patterns.

### Log and State Inspection

**Event Logs** (MapReduce execution tracking):
```bash
# View event log for a specific job
ls ~/.prodigy/events/{repo_name}/{job_id}/

# Follow events in real-time
tail -f ~/.prodigy/events/{repo_name}/{job_id}/events-*.jsonl
```

**Unified Sessions** (workflow state):
```bash
# Check current session state
cat ~/.prodigy/sessions/{session-id}.json | jq .

# View session status
jq '.status, .metadata' ~/.prodigy/sessions/{session-id}.json
```

**Checkpoints** (MapReduce resume state):
```bash
# List available checkpoints
ls ~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/
```

**See also**: [Event Tracking](../mapreduce/event-tracking.md) for event log structure.

### Advanced Claude Log Analysis

Prodigy creates detailed JSON logs for every Claude command execution. Use `jq` to analyze these logs:

**View most recent log**:
```bash
prodigy logs --latest
```

**Watch live execution**:
```bash
prodigy logs --latest --tail
```

**Extract specific information** (Source: CLAUDE.md "Viewing Claude Execution Logs"):

```bash
# Extract error messages
cat ~/.local/state/claude/logs/session-abc123.json | jq 'select(.type == "error")'

# View all tool invocations
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages[].content[] | select(.type == "tool_use")'

# Analyze token usage
cat ~/.local/state/claude/logs/session-abc123.json | jq '.usage'

# Filter by specific tool
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages[].content[] | select(.type == "tool_use" and .name == "Bash")'

# View assistant responses
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages[] | select(.role == "assistant")'
```

**JSONL format** (newer logs use line-delimited JSON):
```bash
# Count messages by type
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -r '.type' | sort | uniq -c

# Extract tool uses
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.type == "assistant") | .content[]? | select(.type == "tool_use")'

# View token usage
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.usage)'
```

### MapReduce Debugging

**Dead Letter Queue (DLQ)** for failed work items:

```bash
# View failed items with error details
prodigy dlq show <job_id>

# Check Claude execution logs for failed items
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Retry failed items (with dry run first)
prodigy dlq retry <job_id> --dry-run
prodigy dlq retry <job_id>
```

**See also**: [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) for DLQ management.

### Checkpoint Inspection

When resume fails, inspect checkpoint state to identify issues:

**Checkpoint file types** (Source: CLAUDE.md "MapReduce Checkpoint and Resume"):
- `setup-checkpoint.json`: Setup phase results and artifacts
- `map-checkpoint-{timestamp}.json`: Map phase progress and work item state
- `reduce-checkpoint-v1-{timestamp}.json`: Reduce phase step results and variables
- `job-state.json`: Overall job state and metadata

**What to check**:

```bash
# Inspect map checkpoint
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/map-checkpoint-*.json | jq '
  {
    total: .work_items | length,
    completed: [.work_items[] | select(.state == "Completed")] | length,
    pending: [.work_items[] | select(.state == "Pending")] | length,
    in_progress: [.work_items[] | select(.state == "InProgress")] | length
  }
'

# Check for failed items with error details
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/map-checkpoint-*.json | \
  jq '.work_items[] | select(.state == "Failed") | {item_id, error}'

# Verify reduce checkpoint variables
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/reduce-checkpoint-*.json | jq '.variables'
```

**Signs of corruption or inconsistencies**:
- Work items with `InProgress` state but no active agent
- Mismatch between `completed_count` and actual completed items
- Missing or null `variables` in reduce checkpoint
- Duplicate `item_id` values in work items array

**Recovery strategies**:
- **Minor corruption**: Manually edit checkpoint to fix state (move `InProgress` items back to `Pending`)
- **Major corruption**: Delete checkpoint and restart phase from beginning
- **Variable issues**: Restore missing variables from previous checkpoint or setup output

**See also**: [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) for checkpoint structure.

### Workflow-Specific Debugging

Different workflow types have different debugging considerations:

**Standard Workflows**:
- Verify variable interpolation with echo commands
- Test steps individually before chaining
- Check captured outputs are in correct scope

**MapReduce Workflows**:
- Verify agent isolation (check individual agent worktrees)
- Debug parallel execution issues with event logs
- Investigate merge conflicts in parent worktree after agent completion
- Check work item discovery with `--dry-run` on map phase

**Goal Seek Workflows**:
- Validate score calculation logic
- Check convergence criteria (threshold, max iterations)
- Debug score trends with iteration history
- Verify goal direction (maximize vs minimize)

**Foreach Workflows**:
- Test with small item lists first
- Check parallel execution limits (`max_parallel`)
- Verify item processing order if order matters
- Debug failed items with individual command execution

### Performance Debugging

Identify and resolve performance bottlenecks:

**Timing Analysis**:
```bash
# Check session timings
cat ~/.prodigy/sessions/{session-id}.json | jq '.timings'

# View step durations
cat ~/.prodigy/sessions/{session-id}.json | jq '.timings | to_entries | sort_by(.value.secs) | reverse'
```

**MapReduce Agent Duration**:
```bash
# Track agent execution times from event logs
cat ~/.prodigy/events/{repo}/{job_id}/events-*.jsonl | \
  jq 'select(.event_type == "AgentCompleted") | {agent_id, duration_secs: .duration.secs}'
```

**Resource Monitoring**:
- Check disk space: `df -h ~/.prodigy/`
- Monitor parallel execution: `ps aux | grep prodigy`
- Track memory usage: `top` or `htop` during execution

**Timeout Issues** (Source: `src/config/mapreduce.rs`, see [Timeout Configuration](../advanced/timeout-configuration.md)):
- Adjust `timeout_config` for map/reduce phases
- Set `agent_timeout_secs` for long-running agents
- Use `PRODIGY_TIMEOUT` environment variable for global timeout override

### Worktree Debugging

**Examine worktree execution history**:

```bash
# Navigate to worktree
cd ~/.prodigy/worktrees/{repo}/{session}/

# View all commits (execution trace)
git log --oneline

# Check current branch state
git status

# View specific commit details
git show <commit-hash>

# Compare with parent branch
git diff origin/master
```

**Verify worktree isolation**:
```bash
# Check main repo is clean
git status

# Verify worktree has changes
cd ~/.prodigy/worktrees/{repo}/{session}/
git status
```

**See also**: [MapReduce Worktree Architecture](../mapreduce-worktree-architecture.md) for worktree isolation guarantees.

### Interactive Debugging Techniques

**Dry-Run Mode**:
```bash
# Preview DLQ retry actions without executing
prodigy dlq retry <job_id> --dry-run

# Test workflow validation without execution
prodigy validate workflow.yml
```

**Manual Checkpoint Inspection and Modification**:
1. Pause workflow execution (Ctrl+C)
2. Inspect checkpoint files (see Checkpoint Inspection above)
3. Edit checkpoint JSON to fix state if needed
4. Resume with `prodigy resume <session-id>`

**Git History Tracing**:
```bash
# View execution sequence from commits
cd ~/.prodigy/worktrees/{repo}/{session}/
git log --oneline --decorate --graph

# Find when specific file was modified
git log --follow -- path/to/file

# See what changed in specific step
git show <commit-hash>
```

### Error Message Analysis

**Read error messages carefully** - MapReduce error types indicate specific failure modes:

- `WorkItemProcessingError`: Individual agent execution failure (check DLQ)
- `CheckpointLoadError`: Checkpoint corruption or missing file (inspect checkpoint directory)
- `TimeoutError`: Execution exceeded configured timeout (adjust timeout settings)
- `ValidationError`: Workflow configuration issue (check YAML syntax and validation output)
- `WorktreeError`: Git worktree operation failure (check disk space, permissions)

**Common error patterns** and fixes in [Common Error Messages](common-error-messages.md).

### Getting Help

When seeking support, include:

1. **Full error messages** (not just excerpts)
2. **Workflow configuration** (YAML file)
3. **Verbosity output** (`-vv` or `-vvv`)
4. **Recent Claude log**: `prodigy logs --latest`
5. **Session state**: `cat ~/.prodigy/sessions/{session-id}.json`
6. **Environment details**: OS, Prodigy version, Claude Code version
7. **Reproduction steps**: Minimal example that demonstrates the issue

**Where to get help**:
- GitHub Issues: https://github.com/prodigy-ai/prodigy
- Documentation: https://prodigy.dev/docs
- Community Discord: (link in README)
