# Observability and Logging

Prodigy provides comprehensive execution monitoring and debugging through event tracking, Claude execution logs, and configurable verbosity levels.

## Overview

Observability features:
- **Event tracking**: JSONL event streams for all operations
- **Claude observability**: Detailed Claude execution logs with tool invocations
- **Verbosity control**: Granular output control from clean to trace-level
- **Log analysis**: Tools for inspecting execution history
- **Performance metrics**: Token usage and timing information

```mermaid
graph TD
    Workflow[Workflow Execution] --> Events[Event System]
    Workflow --> Claude[Claude Commands]
    Workflow --> Verbosity[Verbosity Control]

    Events --> JSONL[JSONL Event Files<br/>~/.prodigy/events/]
    Events --> Types[Event Types<br/>AgentStarted, Completed, Failed]

    Claude --> JSONLog[JSON Logs<br/>~/.local/state/claude/logs/]
    Claude --> Tools[Tool Invocations]
    Claude --> Tokens[Token Usage]

    Verbosity --> Clean[Default: Clean Output]
    Verbosity --> Verbose["-v: Show Streaming"]
    Verbosity --> Debug["-vv: Debug Logs"]
    Verbosity --> Trace["-vvv: Trace Details"]

    JSONL --> Analysis[Log Analysis]
    JSONLog --> Analysis
    Analysis --> Debugging[Debugging & Monitoring]

    style Events fill:#e1f5ff
    style Claude fill:#fff3e0
    style Verbosity fill:#f3e5f5
    style Analysis fill:#e8f5e9
```

**Figure**: Prodigy's observability architecture showing event tracking, Claude logs, and verbosity control.

!!! tip "Quick Access"
    View the latest Claude execution log:
    ```bash
    prodigy logs --latest
    ```
    Follow live execution output:
    ```bash
    prodigy logs --latest --tail
    ```

## When to Use Each Feature

| Situation | Feature | Command/Location |
|-----------|---------|------------------|
| Understanding workflow execution flow | Event Tracking | `~/.prodigy/events/{repo}/{job_id}/` |
| Debugging Claude command failures | Claude Observability | `prodigy logs --latest` |
| Increasing output detail for troubleshooting | Verbosity Control | `-v`, `-vv`, or `-vvv` flags |
| Investigating MapReduce agent failures | Debugging | DLQ + JSON logs |
| Cleaning up old logs | Log Management | `prodigy logs clean` |

## Subpages

<div class="grid cards" markdown>

-   :material-format-list-bulleted-type: **[Event Tracking](event-tracking.md)**

    ---

    JSONL event streams capturing workflow lifecycle: AgentStarted, AgentCompleted, AgentFailed, and more. Query events with `jq` for custom analysis.

-   :material-robot: **[Claude Observability](claude-observability.md)**

    ---

    Detailed Claude execution logs with complete message history, tool invocations, and token usage. Access via `prodigy logs` command.

-   :material-bug: **[Debugging](debugging.md)**

    ---

    Debug MapReduce failures using DLQ integration, analyze performance metrics, and query events for monitoring.

-   :material-folder-cog: **[Log Management](log-management.md)**

    ---

    Log storage locations, cleanup strategies, retention policies, and practical examples for log analysis.

</div>
