# Observability and Logging

Prodigy provides comprehensive execution monitoring and debugging through event tracking, Claude execution logs, and configurable verbosity levels.

## Overview

Observability features:
- **Event tracking**: JSONL event streams for all operations
- **Claude observability**: Detailed Claude execution logs with tool invocations
- **Verbosity control**: Granular output control from clean to trace-level
- **Log analysis**: Tools for inspecting execution history
- **Performance metrics**: Token usage and timing information

## Event Tracking

All workflow operations are logged to JSONL event files:

```
~/.prodigy/events/{repo_name}/{job_id}/
└── events-{timestamp}.jsonl
```

### Event Types

**AgentStarted** - Agent execution begins:
```json
{
  "type": "AgentStarted",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "item_id": "item-1",
  "timestamp": "2025-01-11T12:00:00Z"
}
```

**AgentCompleted** - Agent finishes successfully:
```json
{
  "type": "AgentCompleted",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "duration": {"secs": 30, "nanos": 0},
  "commits": ["abc123", "def456"],
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

**AgentFailed** - Agent encounters errors:
```json
{
  "type": "AgentFailed",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "error": "Timeout after 300 seconds",
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

**WorkItemProcessed** - Item completion:
```json
{
  "type": "WorkItemProcessed",
  "job_id": "mapreduce-123",
  "item_id": "item-1",
  "status": "completed",
  "result": {...}
}
```

**CheckpointSaved** - State persistence:
```json
{
  "type": "CheckpointSaved",
  "job_id": "mapreduce-123",
  "phase": "map",
  "checkpoint_path": "/path/to/checkpoint.json",
  "timestamp": "2025-01-11T12:05:00Z"
}
```

**ClaudeMessage** - Claude interaction messages:
```json
// Source: src/cook/execution/events/event_types.rs:164-169
{
  "type": "ClaudeMessage",
  "agent_id": "agent-1",
  "content": "Analyzing file structure...",
  "message_type": "assistant",
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

### Event Organization

Events are organized by repository and job:
```
~/.prodigy/events/
└── prodigy/                    # Repository name
    ├── mapreduce-123/          # Job ID
    │   └── events-20250111.jsonl
    └── mapreduce-456/
        └── events-20250111.jsonl
```

## Claude Observability

Detailed Claude execution logs capture complete interactions:

### JSON Log Location

Every Claude command creates a JSON log file:
```
~/.local/state/claude/logs/session-{session_id}.json
```

### Log Contents

Complete conversation history:
- User messages and prompts
- Claude responses
- Tool invocations with parameters
- Tool results
- Token usage statistics
- Error details and stack traces

### Accessing JSON Logs

**Via Verbose Output (-v flag)**:
```bash
prodigy run workflow.yml -v
```

Output includes log location:
```
Executing: claude /my-command
Claude JSON log: /Users/user/.local/state/claude/logs/session-abc123.json
✓ Command completed
```

**In MapReduce Events**:
```json
{
  "type": "AgentCompleted",
  "agent_id": "agent-1",
  "json_log_location": "/path/to/logs/session-xyz.json"
}
```

**In DLQ Items**:
```json
{
  "item_id": "item-1",
  "failure_history": [{
    "error": "Command failed",
    "json_log_location": "/path/to/logs/session-xyz.json"
  }]
}
```

### Analyzing JSON Logs

**View complete conversation**:
```bash
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages'
```

**Check tool invocations**:
```bash
cat ~/.local/state/claude/logs/session-abc123.json | \
  jq '.messages[].content[] | select(.type == "tool_use")'
```

**Analyze token usage**:
```bash
cat ~/.local/state/claude/logs/session-abc123.json | jq '.usage'
```

**Extract errors**:
```bash
cat ~/.local/state/claude/logs/session-abc123.json | \
  jq '.messages[] | select(.role == "assistant") | .content[] | select(.type == "error")'
```

## Verbosity Control

Granular output control with verbosity flags:

### Levels

**Default (verbosity = 0)**:
- Clean, minimal output
- Progress indicators
- Results only

**Verbose (-v, verbosity = 1)**:
- Claude streaming JSON output
- Command execution details
- Log file locations

**Debug (-vv, verbosity = 2)**:
- Internal debug logs
- Execution traces
- State transitions

**Trace (-vvv, verbosity = 3)**:
- Trace-level internal logging
- Full execution details
- Performance metrics

### Usage

```bash
# Default: clean output
prodigy run workflow.yml

# Verbose: show Claude streaming
prodigy run workflow.yml -v

# Debug: internal logs
prodigy run workflow.yml -vv

# Trace: maximum detail
prodigy run workflow.yml -vvv
```

### Environment Override

Force streaming output regardless of verbosity:
```bash
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=true
prodigy run workflow.yml
```

## Debugging MapReduce Failures

### Using JSON Logs

When a MapReduce agent fails:

1. **Check DLQ for json_log_location**:
```bash
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'
```

2. **Inspect the Claude JSON log**:
```bash
cat /path/from/step1/session-xyz.json | jq
```

3. **Identify failing tool**:
```bash
cat /path/from/step1/session-xyz.json | jq '.messages[-3:]'
```

4. **Understand context**:
- Review full conversation history
- Check tool invocations and results
- Examine token usage for context issues
- Look for error messages

## Performance Metrics

### Token Usage

Track token consumption:
```json
{
  "usage": {
    "input_tokens": 1234,
    "output_tokens": 567,
    "cache_read_tokens": 89,
    "cache_creation_tokens": 0
  }
}
```

### Execution Timing

Monitor performance:
```json
{
  "timings": {
    "step1": {"secs": 10, "nanos": 500000000},
    "step2": {"secs": 25, "nanos": 0},
    "total": {"secs": 35, "nanos": 500000000}
  }
}
```

## Event Query Examples

### Correlation IDs

Events include optional correlation IDs for tracing related operations across multiple agents:

```json
// Source: src/storage/types.rs:75
{
  "type": "AgentStarted",
  "job_id": "mapreduce-123",
  "agent_id": "agent-1",
  "correlation_id": "trace-abc-123",
  "timestamp": "2025-01-11T12:00:00Z"
}
```

**Filter events by correlation ID**:
```bash
# Source: src/cook/execution/events/filter.rs:63
# Find all events related to a specific workflow trace
cat ~/.prodigy/events/prodigy/mapreduce-123/events-*.jsonl | \
  jq -c 'select(.correlation_id == "trace-abc-123")'
```

**Track an agent workflow end-to-end**:
```bash
# Get correlation ID from initial event
CORRELATION_ID=$(cat events.jsonl | jq -r 'select(.type == "AgentStarted") | .correlation_id' | head -1)

# Find all related events
cat events.jsonl | jq -c "select(.correlation_id == \"$CORRELATION_ID\")"
```

### Find Failed Agents

```bash
cat ~/.prodigy/events/prodigy/mapreduce-123/events-*.jsonl | \
  jq -c 'select(.type == "AgentFailed")'
```

### Calculate Success Rate

```bash
# Count completed
completed=$(cat events.jsonl | jq 'select(.type == "AgentCompleted")' | wc -l)

# Count failed
failed=$(cat events.jsonl | jq 'select(.type == "AgentFailed")' | wc -l)

# Calculate rate
echo "Success rate: $(($completed * 100 / ($completed + $failed)))%"
```

### Find Slowest Agents

```bash
cat events.jsonl | \
  jq -c 'select(.type == "AgentCompleted") | {agent_id, duration: .duration.secs}' | \
  sort -k2 -n -r | \
  head -10
```

## Log Management

### Log Locations

- **Prodigy events**: `~/.prodigy/events/{repo_name}/{job_id}/`
- **Claude logs**: `~/.local/state/claude/logs/`
- **Session state**: `~/.prodigy/sessions/`
- **Checkpoints**: `~/.prodigy/state/{repo_name}/`

### Cleanup

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

## See Also

- [Event Tracking (MapReduce)](../mapreduce/event-tracking.md) - MapReduce event details
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) - Failure tracking and retry
- [Session Management](sessions.md) - Session state and checkpoints
- [Claude Observability (Spec 121)](../../CLAUDE.md#claude-command-observability-spec-121) - Technical details
