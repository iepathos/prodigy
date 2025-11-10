## Event Tracking

All MapReduce execution events are logged to `~/.prodigy/events/{repo_name}/{job_id}/` for debugging and monitoring. Prodigy provides comprehensive event tracking with correlation IDs, metadata enrichment, buffering, and powerful CLI tools for querying and analysis.

### Event Types

Prodigy tracks 25+ event types across different categories:

#### Job Lifecycle Events
- `JobStarted` - Job begins with config and total items
- `JobCompleted` - Job finishes with success/failure counts and duration
- `JobFailed` - Job fails with error and partial results count
- `JobPaused` - Job paused with checkpoint version
- `JobResumed` - Job resumed from checkpoint with pending items

#### Agent Lifecycle Events
- `AgentStarted` - Agent begins processing work item (includes worktree and attempt number)
- `AgentProgress` - Agent reports progress percentage and current step
- `AgentCompleted` - Agent finishes successfully (includes commits and Claude JSON log location)
- `AgentFailed` - Agent fails with error and retry eligibility
- `AgentRetrying` - Agent retries with backoff delay

#### Checkpoint Events
- `CheckpointCreated` - Checkpoint saved with version and completed agent count
- `CheckpointLoaded` - Checkpoint loaded for resume
- `CheckpointFailed` - Checkpoint operation failed

#### Worktree Events
- `WorktreeCreated` - Git worktree created for agent (includes branch name)
- `WorktreeMerged` - Agent changes merged to target branch
- `WorktreeCleaned` - Worktree removed after agent completion

#### Performance Monitoring Events
- `QueueDepthChanged` - Work queue status (pending/active/completed counts)
- `MemoryPressure` - Resource usage monitoring (used vs limit in MB)

#### Dead Letter Queue Events
- `DLQItemAdded` - Failed item added to DLQ with error signature
- `DLQItemRemoved` - Item removed from DLQ (successful retry)
- `DLQItemsReprocessed` - Batch reprocessing of DLQ items
- `DLQItemsEvicted` - Old DLQ items evicted per retention policy
- `DLQAnalysisGenerated` - Error pattern analysis completed

#### Claude Observability Events
- `ClaudeToolInvoked` - Claude tool use with name, ID, and parameters
- `ClaudeTokenUsage` - Token consumption (input/output/cache tokens)
- `ClaudeSessionStarted` - Claude session initialized with model and available tools
- `ClaudeMessage` - Claude message with content and JSON log location

### Event Record Structure

Each event is wrapped in an `EventRecord` with rich metadata:

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2024-01-01T12:00:00Z",
  "correlation_id": "workflow-abc123",
  "event": {
    "event_type": "agent_completed",
    "job_id": "mapreduce-xyz789",
    "agent_id": "agent-1",
    "duration": { "secs": 45, "nanos": 0 },
    "commits": ["a1b2c3d"],
    "json_log_location": "/home/user/.local/state/claude/logs/session-xyz.json"
  },
  "metadata": {
    "host": "worker-01",
    "pid": 12345,
    "thread": "tokio-runtime-worker"
  }
}
```

**Fields:**
- `id` - Unique UUID for this event
- `timestamp` - When the event occurred (UTC)
- `correlation_id` - Links related events across agents and phases
- `event` - The actual event data (varies by type)
- `metadata` - Runtime context (host, process ID, thread). This field is extensible and supports custom fields via the `log_with_metadata` method for application-specific tracking needs.

**Source**: EventRecord definition in src/cook/execution/events/event_logger.rs:17-25

### Event Storage

**Location:**
`~/.prodigy/events/{repo_name}/{job_id}/events-{timestamp}.jsonl`

**Format:**
Events are stored in JSONL (JSON Lines) format with one event per line.

**Buffering:**
- Events are buffered in memory before being written to disk
- **Default buffer size**: 1000 events
- **Default flush interval**: 5 seconds
- Background flush task runs automatically
- Buffer size and flush interval are configurable

**Source**: EventLogger configuration in src/cook/execution/events/event_logger.rs:44-45

**File Rotation:**
- Log files automatically rotate at 100MB (configurable)
- Optional compression for archived files
- Cross-worktree event aggregation for parallel jobs

### Correlation IDs

Each workflow run has a unique `correlation_id` that links all related events:

```bash
# All events from the same workflow share correlation_id
# Makes it easy to trace execution flow across agents
```

Use correlation IDs to:
- Trace work item through multiple retries
- Link agent execution to parent job
- Track checkpoint saves and resumes
- Debug cross-agent dependencies

### Viewing Events with CLI

#### List Events

```bash
# List all events for a job
prodigy events ls --job-id <job_id>

# Filter by event type
prodigy events ls --job-id <job_id> --event-type agent_completed

# Filter by agent
prodigy events ls --job-id <job_id> --agent-id agent-1

# Recent events only (last N minutes)
prodigy events ls --job-id <job_id> --since 30

# Limit results
prodigy events ls --job-id <job_id> --limit 50
```

#### Event Statistics

```bash
# Show statistics grouped by event type
prodigy events stats

# Group by job ID
prodigy events stats --group-by job_id

# Group by agent ID
prodigy events stats --group-by agent_id
```

#### Search Events

```bash
# Search by pattern (regex supported)
prodigy events search "error|failed"

# Search in specific fields only
prodigy events search "timeout" --fields error,description
```

#### Follow Events Live

```bash
# Stream events in real-time (tail -f style)
prodigy events follow --job-id <job_id>

# Filter while following
prodigy events follow --job-id <job_id> --event-type agent_failed
```

#### Clean Old Events

```bash
# Preview cleanup (dry run)
prodigy events clean --older-than 30d --dry-run

# Keep only recent events
prodigy events clean --max-events 10000

# Size-based retention
prodigy events clean --max-size 100MB

# Archive instead of delete
prodigy events clean --older-than 7d --archive --archive-path /backup/events
```

**Note**: Cleanup operations require user confirmation unless running in automation mode (`PRODIGY_AUTOMATION=true`) or using `--dry-run` to preview changes.

**Source**: Cleanup confirmation logic in src/cli/events/mod.rs:591-627

### Debugging with Events

**Common debugging scenarios:**

**Track failed agent:**
```bash
# Find all events for failed agent
prodigy events ls --job-id <job_id> --agent-id <agent_id>

# Check Claude JSON log from AgentCompleted event
cat <json_log_location>
```

**Analyze performance:**
```bash
# Monitor queue depth changes
prodigy events ls --event-type queue_depth_changed

# Check memory pressure events
prodigy events ls --event-type memory_pressure
```

**Debug checkpoint issues:**
```bash
# Find checkpoint events
prodigy events ls --event-type checkpoint_created
prodigy events ls --event-type checkpoint_failed
```

**Review DLQ patterns:**
```bash
# See DLQ additions
prodigy events ls --event-type dlq_item_added

# Check error pattern analysis
prodigy events ls --event-type dlq_analysis_generated
```

**Trace specific workflow run:**
```bash
# Filter events by correlation_id to trace entire workflow execution
prodigy events search "<correlation_id>"

# Find correlation_id from recent job
prodigy events ls --job-id <job_id> --limit 1
```

### Troubleshooting

**Events not being written:**
- Check event file permissions in `~/.prodigy/events/{repo_name}/{job_id}/`
- Verify directory exists and is writable
- Check disk space availability
- Review buffer configuration if events are delayed

**Missing events:**
- Events may be buffered (default: 5 second flush interval)
- Check if logger was properly shut down (ensures buffer flush)
- Verify event type filter isn't excluding events

### Cross-References

- See [Checkpoint and Resume](checkpoint-and-resume.md) for checkpoint events
- See [Dead Letter Queue (DLQ)](dead-letter-queue-dlq.md) for DLQ event details
- See [Retry Metrics and Observability](../retry-configuration/retry-metrics-and-observability.md) for retry-specific monitoring

