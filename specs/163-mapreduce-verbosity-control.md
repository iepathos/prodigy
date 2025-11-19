# Spec 163: MapReduce Event Log Verbosity Control

**Status**: Draft
**Category**: Optimization
**Created**: 2025-01-19

## Objective

Reduce noise in MapReduce event logging by controlling verbosity levels. Currently, MapReduce event logs are extremely verbose, displaying extensive details like hundreds of commit hashes, timestamps, and complete event structures for every agent operation. This creates log spam that obscures important information during normal workflow execution.

## Problem

MapReduce workflows produce excessive logging output that clutters the console:

```
Successfully merged agent mapreduce-20251119_182402_agent_21 (item item_21)
MapReduce event: AgentCompleted { job_id: "mapreduce-20251119_182402", agent_id: "mapreduce-20251119_182402_agent_21", item_id: "item_21", duration: 35s, commits: ["a1b2c3d", "e4f5g6h", ... (100+ more hashes)], cleanup_status: Success, json_log_location: Some("/path/to/log.json") }
MapReduce event: AgentStarted { job_id: "mapreduce-20251119_182402", agent_id: "mapreduce-20251119_182402_agent_22", item_id: "item_22", timestamp: 2025-01-19T18:24:37Z }
```

This verbose output:
- Makes it difficult to track workflow progress
- Obscures errors and important messages
- Creates unnecessarily large log files
- Reduces readability in CI/CD environments

## Requirements

### 1. Default Behavior (Verbosity = 0)

Without the `-v` flag, MapReduce event logging should be minimal:

**Show**:
- Workflow start/completion summary
- Error messages and failures
- Progress indicators (e.g., "Processing 10/100 items")
- Final results summary

**Hide**:
- Individual `AgentStarted` events
- Individual `AgentCompleted` events with full details
- Commit lists (unless failure occurs)
- Detailed event structures
- Merge success messages for individual agents

**Example Default Output**:
```
Starting MapReduce workflow: analyze-codebase
Setup phase completed
Map phase: Processing 100 items with 10 parallel agents
Progress: [##########] 100/100 items completed
Reduce phase: Aggregating results
✓ Workflow completed in 5m 32s
  - Successful: 98/100 items
  - Failed: 2/100 items (see DLQ)
```

### 2. Verbose Behavior (Verbosity >= 1, `-v` flag)

With the `-v` flag, show full event details:

**Show**:
- All default information
- Individual `AgentStarted` events
- Individual `AgentCompleted` events with details
- Commit lists (full or truncated based on size)
- Event structures with timestamps
- Merge operations per agent
- Cleanup status

**Example Verbose Output**:
```
Starting MapReduce workflow: analyze-codebase
Setup phase completed
Map phase: Processing 100 items with 10 parallel agents
MapReduce event: AgentStarted { job_id: "mapreduce-123", agent_id: "agent-1", item_id: "item-1", timestamp: 2025-01-19T18:24:35Z }
MapReduce event: AgentCompleted { job_id: "mapreduce-123", agent_id: "agent-1", item_id: "item-1", duration: 35s, commits: ["a1b2c3d", "e4f5g6h", ...], cleanup_status: Success }
Successfully merged agent agent-1 (item item-1)
...
```

### 3. Commit List Truncation

Even in verbose mode, commit lists should be truncated if they exceed a reasonable length:

- **Threshold**: 10 commits
- **Format**: `commits: ["a1b2c3d", "e4f5g6h", ... (95 more)]`
- **Rationale**: Prevent log spam even in verbose mode while preserving debuggability

### 4. Error Visibility

Errors and failures should ALWAYS be shown regardless of verbosity level:

- Failed agent details
- DLQ entries
- Checkpoint errors
- Merge conflicts
- Cleanup failures

## Implementation Guidance

### Affected Components

1. **MapReduce Orchestrator** (`src/mapreduce/orchestrator.rs`):
   - Modify event logging to check verbosity level
   - Suppress verbose events in default mode
   - Add progress indicators for default mode

2. **Event Logger** (if centralized):
   - Add verbosity parameter
   - Filter events based on verbosity level
   - Preserve error visibility

3. **CLI** (`src/cli.rs`):
   - Ensure `-v` flag is properly propagated to MapReduce execution
   - Document verbosity behavior in help text

### Event Categorization

**Always Show (Verbosity >= 0)**:
- Workflow lifecycle (start, complete, failed)
- Errors and failures
- Progress summaries
- Final statistics

**Show in Verbose Mode (Verbosity >= 1)**:
- `AgentStarted` events
- `AgentCompleted` events
- Individual merge operations
- Cleanup status
- Detailed event structures

**Show in Debug Mode (Verbosity >= 2, if implemented)**:
- Full commit lists (untruncated)
- Internal state transitions
- Resource allocation details

### Code Example

```rust
// In orchestrator or event handler
fn log_agent_completed(event: &AgentCompletedEvent, verbosity: u8) {
    if verbosity >= 1 {
        // Show full event details
        println!("MapReduce event: AgentCompleted {{ job_id: {:?}, agent_id: {:?}, ... }}",
                 event.job_id, event.agent_id);

        if event.commits.len() > 10 && verbosity < 2 {
            println!("  commits: [{:?}, {:?}, ... ({} more)]",
                     event.commits[0], event.commits[1], event.commits.len() - 2);
        } else if verbosity >= 2 {
            println!("  commits: {:?}", event.commits);
        }
    }
    // Always update progress counter (shown in default mode)
}
```

## Acceptance Criteria

- [ ] Default mode (no `-v` flag) shows minimal, clean output
- [ ] Verbose mode (`-v` flag) shows full event details
- [ ] Commit lists are truncated at 10 items in verbose mode (unless `-vv`)
- [ ] Errors are always visible regardless of verbosity level
- [ ] Progress indicators replace verbose event logs in default mode
- [ ] Existing tests pass with new verbosity controls
- [ ] Documentation updated to explain verbosity behavior
- [ ] CI/CD logs are cleaner and more readable in default mode

## Testing Strategy

1. **Unit Tests**:
   - Test event logging with different verbosity levels
   - Verify commit list truncation logic
   - Ensure error visibility at all levels

2. **Integration Tests**:
   - Run MapReduce workflow with default verbosity (clean output)
   - Run MapReduce workflow with `-v` (full details)
   - Verify progress indicators in default mode

3. **Manual Testing**:
   - Execute sample MapReduce workflow without `-v` flag
   - Verify output is clean and readable
   - Execute with `-v` and verify full details are shown

## Future Enhancements

- Support `-vv` for even more detailed logging (untruncated commit lists)
- Add `--quiet` flag to suppress all non-error output
- Implement structured logging (JSON) for machine parsing
- Allow per-event-type verbosity control via configuration

## Related Specifications

- Spec 121: Claude Command Observability (json_log_location tracking)
- Spec 127: Worktree Isolation (affects merge logging)
- Spec 134: MapReduce Checkpoint and Resume (affects checkpoint logging)
- Spec 140: Concurrent Resume Protection (affects lock logging)
