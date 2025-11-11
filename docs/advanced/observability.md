# Observability and Logging

Prodigy provides comprehensive execution monitoring and debugging through event tracking, Claude execution logs, and granular verbosity control.

## Overview

Observability features:
- **Event tracking** - JSONL event streams for all operations
- **Claude observability** - Detailed Claude execution logs with tool invocations
- **Verbosity control** - Granular output control for different contexts
- **Log analysis** - Tools for debugging and monitoring

## Event Tracking

Prodigy logs all operations as JSONL events:

```
~/.prodigy/events/{repo_name}/{job_id}/
└── events-{timestamp}.jsonl
```

### Event Types

- `AgentStarted` - Agent execution begins
- `AgentCompleted` - Agent finishes with commits
- `AgentFailed` - Agent encounters errors
- `ClaudeMessage` - Claude AI interactions
- `WorkItemProcessed` - Item completion
- `CheckpointSaved` - State persistence

### Event Structure

```json
{
  "timestamp": "2025-11-11T12:00:00Z",
  "event_type": "AgentCompleted",
  "correlation_id": "job-abc123",
  "data": {
    "agent_id": "agent-1",
    "duration": 30,
    "commits": ["abc123", "def456"]
  }
}
```

### Viewing Events

```bash
# View all events for a job
prodigy events <job_id>

# Filter by event type
prodigy events <job_id> --type AgentFailed

# Follow events in real-time
prodigy events <job_id> --follow
```

## Claude Observability

Every Claude command creates a JSON log file with complete execution details.

### Log File Location

```
~/.claude/projects/{worktree-path}/{uuid}.jsonl
```

The log path is displayed after command execution:

```
✅ Completed | Log: ~/.claude/projects/.../6ded63ac.jsonl
```

### Log Contents

Claude logs include:
- Complete message history (user and assistant messages)
- All tool invocations with parameters and results
- Token usage statistics (input, output, cache tokens)
- Session metadata (model, tools available, timestamps)
- Error details and stack traces

### Viewing Claude Logs

Watch live as Claude executes:

```bash
tail -f ~/.claude/projects/.../6ded63ac.jsonl
```

Analyze completed logs:

```bash
# View all events
cat ~/.claude/projects/.../6ded63ac.jsonl

# View only tool uses
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.type == "assistant") | .content[]? | select(.type == "tool_use")'

# View token usage
cat ~/.claude/projects/.../6ded63ac.jsonl | jq -c 'select(.usage)'
```

### Using the prodigy logs Command

```bash
# View most recent Claude log
prodigy logs --latest

# View with summary
prodigy logs --latest --summary

# Tail the latest log (follow mode)
prodigy logs --latest --tail

# List recent logs
prodigy logs
```

## Verbosity Control

Control output detail with verbosity flags:

### Default Mode (verbosity = 0)

Clean, minimal output:
- Progress indicators
- Results summary
- No Claude streaming

```bash
prodigy run workflow.yml
```

### Verbose Mode (-v)

Claude streaming and command details:
- Real-time Claude JSON output
- Command execution details
- Useful for debugging

```bash
prodigy run workflow.yml -v
```

### Debug Mode (-vv)

Debug logs with execution traces:
- Internal operation logs
- Timing information
- State transitions

```bash
prodigy run workflow.yml -vv
```

### Trace Mode (-vvv)

Trace-level internal logging:
- All internal operations
- Detailed event tracking
- Maximum verbosity

```bash
prodigy run workflow.yml -vvv
```

### Environment Override

Force streaming output regardless of verbosity:

```bash
PRODIGY_CLAUDE_CONSOLE_OUTPUT=true prodigy run workflow.yml
```

## Debugging Failed MapReduce Agents

Failed agents include log paths in DLQ entries:

```bash
# Show DLQ items with log locations
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Inspect the Claude log
cat /path/from/above/session-xyz.json | jq
```

### Debugging Workflow

1. Check DLQ for failed items
2. Extract json_log_location from failure history
3. Inspect Claude JSON log for error details
4. Review tool invocations and responses
5. Identify root cause

## Log Analysis Techniques

### Find Errors in Events

```bash
cat ~/.prodigy/events/{repo}/{job}/events-*.jsonl | \
  jq -c 'select(.event_type == "AgentFailed")'
```

### Calculate Success Rate

```bash
# Count completed vs failed agents
completed=$(cat events-*.jsonl | jq -c 'select(.event_type == "AgentCompleted")' | wc -l)
failed=$(cat events-*.jsonl | jq -c 'select(.event_type == "AgentFailed")' | wc -l)
echo "Success rate: $((completed * 100 / (completed + failed)))%"
```

### Track Execution Timeline

```bash
cat events-*.jsonl | jq -r '[.timestamp, .event_type] | @tsv' | sort
```

## See Also

- [Session Management](sessions.md) - Session lifecycle and state
- [Dead Letter Queue](../mapreduce/dlq.md) - Failed item debugging
- [Error Handling](../workflow-basics/error-handling.md) - Error policies
