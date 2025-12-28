# Log Management

## Log Locations

=== "Linux"
    ```bash
    # Prodigy events
    ~/.prodigy/events/{repo_name}/{job_id}/

    # Claude logs
    ~/.local/state/claude/logs/

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
    ~/.local/state/claude/logs/

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
    %USERPROFILE%\.local\state\claude\logs\

    # Session state
    %USERPROFILE%\.prodigy\sessions\

    # Checkpoints
    %USERPROFILE%\.prodigy\state\{repo_name}\
    ```

!!! warning "Log Storage Considerations"
    Claude JSON logs can grow large with extensive tool usage. Monitor disk space when running many MapReduce agents. Consider setting up automated cleanup for logs older than 30 days in production environments.

## Cleanup

```bash
# Clean old event logs (older than 30 days)
find ~/.prodigy/events -name "*.jsonl" -mtime +30 -delete

# Clean old Claude logs
find ~/.local/state/claude/logs -name "*.json" -mtime +30 -delete

# Clean completed sessions
prodigy sessions clean --completed
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
for log in ~/.local/state/claude/logs/session-*.json; do
  echo "$log:"
  jq '.usage' "$log"
done
```
