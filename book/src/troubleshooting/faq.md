## FAQ

### How do I debug variable interpolation issues?

When `${var}` appears literally in output instead of being replaced:

1. **Check spelling and case**: Variable names are case-sensitive
2. **Verify scope**: Ensure the variable is available (step vs workflow level)
3. **Use verbose mode**: Run with `-v` to see variable interpolation in real-time
4. **Verify capture**: If using `capture_output`, ensure the command succeeded
5. **Check syntax**: Use `${var}` for workflow variables, not just `$var`

Example debugging:
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

See [MapReduce Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) for complete details.

### How do I retry failed DLQ items?

To retry items that failed during MapReduce execution:

```bash
# View failed items
prodigy dlq show <job_id>

# Retry all failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to preview retry
prodigy dlq retry <job_id> --dry-run
```

**Important**: Before retrying, ensure the underlying issue is fixed. If the error is systematic (not transient), items will fail again.

Check `json_log_location` in DLQ entries to debug the original failure.

### Why are my MapReduce items not being found?

If you see "No items to process" or "items.json not found":

1. **Verify input file exists**: Check the path specified in `input:`
2. **Confirm setup succeeded**: Ensure setup phase created the input file
3. **Test JSONPath**: Use `jq` to test your `json_path` expression
4. **Validate JSON format**: Ensure the file is valid JSON with `jq .`
5. **Check file location**: Input file path is relative to workflow directory

Example JSONPath test:
```bash
cat items.json | jq '$.items[*]'
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

# Direct access to logs
cat ~/.local/state/claude/logs/session-*.json
```

Logs contain:
- Complete message history
- All tool invocations with parameters
- Token usage statistics
- Error details and stack traces

For detailed log analysis techniques, see "Viewing Claude Execution Logs (Spec 126)" in the project CLAUDE.md file.

### What does "command not found: claude" mean?

This error indicates Claude Code CLI is not installed or not in your PATH:

1. **Verify installation**: Check if Claude Code is installed
2. **Check PATH**: Run `which claude` to see if it's accessible
3. **Use full path**: Specify `/path/to/claude` in workflow if needed
4. **Verify executable name**: Should be `claude`, not `claude-code`

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

Common causes of cleanup failures:
- Locked files (check with `lsof`)
- Running processes (check with `ps`)
- Permission issues (verify with `ls -ld`)
- Insufficient disk space (check with `df -h`)

For details on cleanup failures, see "Cleanup Failure Handling (Spec 136)" in the CLAUDE.md file.

### Why are environment variables not being resolved?

If `${VAR}` or `$VAR` appears literally in commands:

1. **Check definition**: Ensure variable is defined in `env:` section
2. **Verify profile**: Use `--profile` flag if using profile-specific values
3. **Check scope**: Confirm variable is global or in correct scope
4. **Use correct syntax**: `${VAR}` for workflow vars, `$VAR` for shell vars
5. **Validate env_files**: Ensure external env files are loaded correctly

Example:
```yaml
env:
  API_KEY: "my-key"
  DATABASE_URL:
    default: "localhost"
    prod: "prod-server"
```

See [Environment Configuration](../environment/index.md) for configuration details.

### How do I debug timeout errors?

When commands or phases time out:

1. **Increase timeout**: Adjust timeout values for long operations
2. **Check for hung processes**: Use `ps` or `top` to find stuck processes
3. **Optimize performance**: Split work into smaller chunks
4. **Use agent_timeout_secs**: Set per-agent timeout for MapReduce
5. **Look for deadlocks**: Check for concurrent operations blocking each other

MapReduce-specific timeout configuration:
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

**How to view**:
```bash
# List events for a job
prodigy events ls --job-id <job_id>

# Follow events in real-time
prodigy events follow --job-id <job_id>

# Show event statistics
prodigy events stats

# View detailed event timeline
cat ~/.prodigy/events/{repo_name}/{job_id}/events-*.jsonl
```

**Source**: src/cli/commands/events.rs:22-98

Events are shared across worktrees, enabling centralized monitoring of parallel jobs.
