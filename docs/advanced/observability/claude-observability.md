# Claude Observability

Detailed Claude execution logs capture complete interactions.

## JSON Log Location

Every Claude command creates a JSON log file:
```
~/.local/state/claude/logs/session-{session_id}.json
```

## Log Contents

Complete conversation history:
- User messages and prompts
- Claude responses
- Tool invocations with parameters
- Tool results
- Token usage statistics
- Error details and stack traces

## Accessing JSON Logs

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

## Analyzing JSON Logs

!!! example "Common Log Analysis Tasks"
    The examples below show how to extract specific information from Claude JSON logs using `jq`. These patterns are useful for debugging agent failures, tracking token usage, and understanding Claude's decision-making process.

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

!!! tip "Choosing the Right Verbosity Level"
    Start with default output for production workflows. Use `-v` when debugging Claude interactions or when you need to see streaming output. Reserve `-vv` and `-vvv` for deep troubleshooting of Prodigy internals.

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
