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

## Subpages

- [Event Tracking](event-tracking.md) - JSONL event streams and event types
- [Claude Observability](claude-observability.md) - Claude execution logs and verbosity control
- [Debugging](debugging.md) - Debugging MapReduce failures, performance metrics, and event queries
- [Log Management](log-management.md) - Log locations, cleanup, and practical examples
