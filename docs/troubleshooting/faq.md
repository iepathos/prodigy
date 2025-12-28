## FAQ

### How do I debug variable interpolation issues?

When `${var}` appears literally in output instead of being replaced:

1. **Check spelling and case**: Variable names are case-sensitive
2. **Verify scope**: Ensure the variable is available (step vs workflow level)
3. **Use verbose mode**: Run with `-v` to see variable interpolation in real-time
4. **Verify capture**: If using `capture_output`, ensure the command succeeded
5. **Check syntax**: Use `${var}` for workflow variables, not just `$var`

!!! example "Debugging variable interpolation"
    ```bash
    # Add echo to verify variable value
    - shell: "echo Variable value: ${my_var}"
    ```

### What should I do when checkpoint resume fails?

If resume starts from the beginning or shows "checkpoint not found":

1. **Verify checkpoint exists**: Check `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/`
2. **Confirm job ID**: Use `prodigy sessions list` to find the correct ID
3. **Check for concurrent resume**: Look for lock files in `~/.prodigy/resume_locks/`
4. **Review checkpoint integrity**: Read the checkpoint JSON to ensure it's valid
5. **Ensure workflow unchanged**: Significant workflow changes may prevent resume

!!! tip "Quick checkpoint verification"
    ```bash
    # List available checkpoints
    ls -la ~/.prodigy/state/$(basename $(pwd))/mapreduce/jobs/
    ```

See [MapReduce Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) for complete details.

### How do I retry failed DLQ items?

To retry items that failed during MapReduce execution:

```bash
# View failed items
prodigy dlq list --job-id <job_id>

# Retry all failed items (workflow_id is the positional argument)
prodigy dlq retry <workflow_id>

# Retry with custom parallelism (default: 10)
prodigy dlq retry <workflow_id> --parallel 5

# Retry with custom max attempts (default: 3)
prodigy dlq retry <workflow_id> --max-retries 5

# Force retry even if not normally eligible
prodigy dlq retry <workflow_id> --force

# Dry run to preview retry (check eligibility)
prodigy dlq retry <workflow_id> --filter "status=failed"
```

!!! warning "Before retrying"
    Ensure the underlying issue is fixed. If the error is systematic (not transient), items will fail again.

!!! tip "Debugging failed items"
    Check `json_log_location` in DLQ entries to debug the original failure:
    ```bash
    prodigy dlq inspect <item_id> --job-id <job_id>
    ```

**Source**: `src/cli/args.rs:596-615`

### Why are my MapReduce items not being found?

If you see "No items to process" or "items.json not found":

1. **Verify input file exists**: Check the path specified in `input:`
2. **Confirm setup succeeded**: Ensure setup phase created the input file
3. **Test JSONPath**: Use `jq` to test your filter expression
4. **Validate JSON format**: Ensure the file is valid JSON with `jq .`
5. **Check file location**: Input file path is relative to workflow directory

!!! note "jq vs JSONPath syntax"
    jq uses its own filter syntax, not JSONPath. For example:

    - JSONPath: `$.items[*]`
    - jq equivalent: `.items[]`

```bash
# Test JSON extraction with jq (correct syntax)
jq '.items[]' items.json

# Validate JSON structure
jq '.' items.json

# Count items that would be processed
jq '.items | length' items.json
```

### How do I view Claude execution logs?

To see detailed logs of what Claude did during a command:

```bash
# View most recent log
prodigy logs --latest

# View with summary
prodigy logs --latest --summary

# Tail log in real-time
prodigy logs --latest --tail
```

!!! info "Log location"
    Claude logs are stored in `~/.claude/projects/` as `.jsonl` files organized by project, not in `~/.local/state/claude/logs/`.

**Direct log access:**
```bash
# Find logs in the Claude projects directory
ls ~/.claude/projects/

# View a specific project's logs
ls ~/.claude/projects/<project-hash>/

# Pretty-print a log file
cat ~/.claude/projects/<project-hash>/<session>.jsonl | jq
```

**Source**: `src/cli/commands/logs.rs:33-36`

Logs contain:

- Complete message history
- All tool invocations with parameters
- Token usage statistics
- Error details and stack traces

### What does "command not found: claude" mean?

This error indicates Claude Code CLI is not installed or not in your PATH:

1. **Verify installation**: Check if Claude Code is installed
2. **Check PATH**: Run `which claude` to see if it's accessible
3. **Use full path**: Specify `/path/to/claude` in workflow if needed
4. **Verify executable name**: Should be `claude`, not `claude-code`

!!! tip
    Installation varies by platform - refer to Claude Code documentation.

### How do I clean up orphaned worktrees?

When worktree cleanup fails during MapReduce execution:

```bash
# Clean orphaned worktrees for a job
prodigy worktree clean-orphaned <job_id>

# Dry run to preview cleanup
prodigy worktree clean-orphaned <job_id> --dry-run

# Force cleanup without confirmation
prodigy worktree clean-orphaned <job_id> --force
```

!!! warning "Common causes of cleanup failures"
    - **Locked files**: Check with `lsof`
    - **Running processes**: Check with `ps`
    - **Permission issues**: Verify with `ls -ld`
    - **Insufficient disk space**: Check with `df -h`

For details on cleanup failures, see "Cleanup Failure Handling (Spec 136)" in the CLAUDE.md file.

### Why are environment variables not being resolved?

If `${VAR}` or `$VAR` appears literally in commands:

1. **Check definition**: Ensure variable is defined in `env:` section
2. **Verify profile**: Use `--profile` flag if using profile-specific values
3. **Check scope**: Confirm variable is global or in correct scope
4. **Use correct syntax**: `${VAR}` for workflow vars, `$VAR` for shell vars
5. **Validate env_files**: Ensure external env files are loaded correctly

!!! example "Environment variable configuration"
    ```yaml
    env:
      API_KEY: "my-key"
      DATABASE_URL:
        default: "localhost"
        prod: "prod-server"
    ```

See [Environment Variables](../environment/index.md) for configuration details.

### How do I debug timeout errors?

When commands or phases time out:

1. **Increase timeout**: Adjust timeout values for long operations
2. **Check for hung processes**: Use `ps` or `top` to find stuck processes
3. **Optimize performance**: Split work into smaller chunks
4. **Use agent_timeout_secs**: Set per-agent timeout for MapReduce
5. **Look for deadlocks**: Check for concurrent operations blocking each other

!!! example "MapReduce timeout configuration"
    ```yaml
    map:
      agent_timeout_secs: 600  # 10 minutes per agent
      max_items: 10  # Process fewer items per run
    ```

### Where are event logs stored?

Event logs use a global storage architecture:

**Location**: `~/.prodigy/events/{repo_name}/{job_id}/`

**What's stored**:

- Agent lifecycle events (started, completed, failed)
- Work item processing status
- Checkpoint saves
- Error details with correlation IDs

**How to view:**

```bash
# List events for a job
prodigy events ls --job-id <job_id>

# Filter by event type
prodigy events ls --job-id <job_id> --event-type AgentCompleted

# Filter by agent ID
prodigy events ls --job-id <job_id> --agent-id agent-0

# Show only recent events (last 30 minutes)
prodigy events ls --since 30

# Limit number of events (default: 100)
prodigy events ls --job-id <job_id> --limit 50

# Show event statistics
prodigy events stats
```

!!! tip "Real-time monitoring"
    ```bash
    # Tail the event file for live updates
    tail -f ~/.prodigy/events/{repo_name}/{job_id}/events-*.jsonl
    ```

**Source**: `src/cli/args.rs:414-441`

Events are shared across worktrees, enabling centralized monitoring of parallel jobs.
