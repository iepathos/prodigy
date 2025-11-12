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

**See also**: [Common Issues](index.md#variables-not-interpolating) for variable troubleshooting patterns.

### Profile Debugging

Profiles allow different configuration values for different environments (dev, staging, prod). When profile-specific values aren't being used, debug the profile selection and fallback behavior.

**Verify Active Profile**:
```bash
# Check which profile is active (from command line)
prodigy run workflow.yml --profile prod -v

# Profile is shown in verbose output during variable interpolation
```

**Profile Configuration Structure** (Source: CLAUDE.md "Environment Variables (Spec 120)"):
```yaml
env:
  API_URL:
    default: "http://localhost:3000"
    staging: "https://staging.api.com"
    prod: "https://api.com"

  DEBUG_MODE:
    default: "true"
    prod: "false"
```

**Common Profile Issues**:

1. **Profile not applied**: Using default value instead of profile-specific
   - **Debug**: Check `--profile` flag is passed correctly
   - **Verify**: Profile name matches exactly (case-sensitive)
   - **Test**: Echo variable to see which value is used: `shell: "echo API_URL=$API_URL"`

2. **Profile doesn't exist**: "Invalid profile" error
   - **Debug**: List defined profiles in `env:` section of workflow
   - **Fix**: Add profile definition or use correct profile name

3. **Variable undefined in profile**: Falls back to default
   - **Debug**: Check if variable has profile-specific value defined
   - **Expected**: Fallback to `default` is normal behavior if profile value missing

**Debugging Profile Selection**:
```bash
# Add debug output to workflow
- shell: "echo Active environment: $ENVIRONMENT"
- shell: "echo API URL: $API_URL"
- shell: "echo Debug mode: $DEBUG_MODE"
```

**Profile Fallback Behavior**:
- If profile value exists: Use profile-specific value
- If profile value missing: Fall back to `default`
- If no `default`: Variable is undefined

### Sub-Workflow Debugging

Sub-workflows allow composing complex workflows from smaller, reusable pieces. Debugging sub-workflows requires understanding result passing, context isolation, and failure propagation.

**Sub-Workflow Execution Flow** (Source: features.json:workflow_composition.sub_workflows):
```yaml
# Parent workflow
steps:
  - sub_workflow: "./workflows/build.yml"
    capture_result: build_output

  - sub_workflow: "./workflows/test.yml"
    params:
      artifact_path: "${build_output.artifact_path}"
```

**Common Sub-Workflow Issues**:

1. **Result not passed correctly between sub-workflows**
   - **Debug**: Check `capture_result` is used to capture sub-workflow output
   - **Verify**: Variable interpolation uses correct captured variable name
   - **Test**: Echo captured result to verify structure

2. **Sub-workflow failure doesn't fail parent**
   - **Debug**: Check failure propagation settings
   - **Expected**: By default, sub-workflow failures should fail parent workflow

3. **Context isolation issues**: Variables not available in sub-workflow
   - **Debug**: Use `params:` to explicitly pass variables to sub-workflow
   - **Understanding**: Sub-workflows have isolated context by default
   - **Solution**: Pass required variables via `params`

**Debugging Sub-Workflow Execution**:
```bash
# Check sub-workflow was invoked
cat ~/.prodigy/sessions/{session-id}.json | jq '.workflow_data.completed_steps'

# View sub-workflow results
cat ~/.prodigy/sessions/{session-id}.json | jq '.workflow_data.variables |
  to_entries[] | select(.key | contains("result"))'

# Check sub-workflow commits in worktree
cd ~/.prodigy/worktrees/{repo}/{session}/
git log --grep="sub_workflow" --oneline
```

**Sub-Workflow Parameter Passing**:
```yaml
# Explicit parameter passing
- sub_workflow: "./workflows/deploy.yml"
  params:
    environment: "staging"
    version: "${build_version}"
    artifact: "${build_artifact}"
```

**Debugging Parameter Interpolation**:
- Use verbose mode: `prodigy run workflow.yml -v`
- Add echo statements in sub-workflow to verify parameters received
- Check sub-workflow definition expects parameters with correct names

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

### Circuit Breaker Debugging

When commands fail repeatedly, Prodigy's circuit breaker prevents further attempts until recovery timeout expires. Understanding circuit breaker states is essential for troubleshooting persistent failures.

**Circuit Breaker States** (Source: src/cook/retry_state.rs:126-149):
- **Closed**: Normal operation, retries allowed
- **Open**: Circuit tripped due to failures, all attempts immediately fail
- **Half-Open**: Testing recovery, limited attempts allowed

**Diagnosing Circuit Breaker Issues**:

```bash
# Check checkpoint for circuit breaker state
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/*checkpoint*.json | \
  jq '.circuit_breaker_states'

# Look for open circuits
cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/*checkpoint*.json | \
  jq '.circuit_breaker_states | to_entries[] | select(.value.state == "Open")'
```

**Circuit Breaker Configuration** (from retry config):
```yaml
# Source: src/cook/retry_state.rs:126-144
retry:
  circuit_breaker:
    failure_threshold: 5        # Failures before opening
    recovery_timeout: 30s       # Wait before half-open
    half_open_max_calls: 3      # Test attempts in half-open
```

**Common Circuit Breaker Problems**:

1. **Circuit stuck open**: Too many failures, waiting for recovery_timeout
   - **Solution**: Wait for timeout to expire, or fix underlying issue and restart
   - **Debug**: Check `last_failure_at` timestamp and `recovery_timeout`

2. **Circuit repeatedly opening**: Underlying issue not resolved
   - **Solution**: Fix root cause (permissions, credentials, resource availability)
   - **Debug**: Review `failure_count` and error messages in retry history

3. **Half-open failures**: Recovery attempts still failing
   - **Solution**: Increase `recovery_timeout` to allow more time for system recovery
   - **Debug**: Check `half_open_success_count` vs `half_open_max_calls`

**Adjusting Circuit Breaker Thresholds**:
- Increase `failure_threshold` for transient errors: `failure_threshold: 10`
- Increase `recovery_timeout` for slow-recovering services: `recovery_timeout: 60s`
- Increase `half_open_max_calls` for more recovery attempts: `half_open_max_calls: 5`

**Monitoring Circuit Breaker State**:
```bash
# View circuit state transitions over time
cat ~/.prodigy/events/{repo}/{job_id}/events-*.jsonl | \
  jq 'select(.circuit_breaker_state) | {timestamp, command_id, state: .circuit_breaker_state}'
```

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
