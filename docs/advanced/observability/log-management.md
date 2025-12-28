# Log Management

## Log Locations

=== "Linux"
    ```bash
    # Prodigy events
    ~/.prodigy/events/{repo_name}/{job_id}/

    # Claude logs
    ~/.claude/projects/

    # Session state
    ~/.prodigy/sessions/

    # Checkpoints
    ~/.prodigy/state/{repo_name}/
    ```

=== "macOS"
    ```bash
    # Prodigy events
    ~/.prodigy/events/{repo_name}/{job_id}/

    # Claude logs
    ~/.claude/projects/

    # Session state
    ~/.prodigy/sessions/

    # Checkpoints
    ~/.prodigy/state/{repo_name}/
    ```

=== "Windows"
    ```powershell
    # Prodigy events
    %USERPROFILE%\.prodigy\events\{repo_name}\{job_id}\

    # Claude logs
    %USERPROFILE%\.claude\projects\

    # Session state
    %USERPROFILE%\.prodigy\sessions\

    # Checkpoints
    %USERPROFILE%\.prodigy\state\{repo_name}\
    ```

!!! warning "Log Storage Considerations"
    Claude JSON logs can grow large with extensive tool usage. Monitor disk space when running many MapReduce agents. Consider setting up automated cleanup for logs older than 30 days in production environments.

## Viewing Logs

Use the `prodigy logs` command to view and analyze Claude execution logs:

```bash
# Source: src/cli/args.rs:253-270
# List recent Claude logs
prodigy logs

# View the latest log
prodigy logs --latest

# Follow latest log in real-time (useful during execution)
prodigy logs --latest --tail

# Show summary of latest log
prodigy logs --latest --summary

# View logs for a specific session
prodigy logs <session-id>
```

## Cleanup

### Using Prodigy Commands

```bash
# Source: src/cli/args.rs:736-747
# Clean Claude logs older than 30 days (preview first)
prodigy clean logs --older-than 30d --dry-run

# Clean Claude logs older than 30 days
prodigy clean logs --older-than 30d

# Force cleanup without confirmation
prodigy clean logs --older-than 30d -f
```

```bash
# Source: src/cli/args.rs:307-316
# Clean all old sessions
prodigy sessions clean --all

# Force cleanup without confirmation
prodigy sessions clean --all -f
```

### Manual Cleanup

```bash
# Clean old event logs (older than 30 days)
find ~/.prodigy/events -name "*.jsonl" -mtime +30 -delete

# Clean old Claude logs (.jsonl is the primary format)
# Source: src/cli/commands/logs.rs:147-151
find ~/.claude/projects -name "*.jsonl" -mtime +30 -delete
```

## Examples

### Debug Workflow Failure

```bash
# Run with verbose output
prodigy run workflow.yml -v

# Check event log for errors
cat ~/.prodigy/events/prodigy/latest/events-*.jsonl | \
  jq -c 'select(.type == "AgentFailed")'

# Inspect Claude log
cat $(jq -r '.json_log_location' dlq-item.json) | jq '.messages[-5:]'
```

### Monitor MapReduce Progress

```bash
# Run in verbose mode
prodigy run mapreduce.yml -v &

# Watch event stream
tail -f ~/.prodigy/events/prodigy/mapreduce-123/events-*.jsonl | \
  jq -c 'select(.type == "AgentCompleted")'
```

### Analyze Token Usage

```bash
# Extract token usage from all agents
# Source: src/cli/commands/logs.rs:33-36
for log in ~/.claude/projects/**/*.jsonl; do
  echo "$log:"
  jq '.usage' "$log" 2>/dev/null || true
done
```
