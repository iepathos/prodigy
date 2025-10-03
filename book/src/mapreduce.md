# MapReduce Workflows

## Complete Structure

```yaml
name: parallel-processing
mode: mapreduce

# Optional setup phase
setup:
  - shell: "generate-work-items.sh"
  - shell: "debtmap analyze . --output items.json"

# Map phase: Process items in parallel
map:
  # Input source (JSON file or command)
  input: "items.json"

  # JSONPath expression to extract items
  json_path: "$.items[*]"

  # Agent template (commands run for each item)
  # Modern syntax: Commands directly under agent_template
  agent_template:
    - claude: "/process '${item}'"
    - shell: "test ${item.path}"
      on_failure:
        claude: "/fix-issue '${item}'"

  # DEPRECATED: Nested 'commands' syntax (still supported)
  # agent_template:
  #   commands:
  #     - claude: "/process '${item}'"

  # Maximum parallel agents
  max_parallel: 10

  # Optional: Filter items
  filter: "item.score >= 5"

  # Optional: Sort items
  sort_by: "item.priority DESC"

  # Optional: Limit number of items
  max_items: 100

  # Optional: Skip items
  offset: 10

  # Optional: Deduplicate by field
  distinct: "item.id"

  # Optional: Agent timeout in seconds
  agent_timeout_secs: 300

# Reduce phase: Aggregate results
# Modern syntax: Commands directly under reduce
reduce:
  - claude: "/summarize ${map.results}"
  - shell: "echo 'Processed ${map.successful}/${map.total} items'"

# DEPRECATED: Nested 'commands' syntax (still supported)
# reduce:
#   commands:
#     - claude: "/summarize ${map.results}"

# Optional: Custom merge workflow (supports two formats)
merge:
  # Simple array format
  - shell: "git fetch origin"
  - claude: "/merge-worktree ${merge.source_branch}"
  - shell: "cargo test"

# OR full format with timeout
# merge:
#   commands:
#     - shell: "git fetch origin"
#     - claude: "/merge-worktree ${merge.source_branch}"
#   timeout: 600  # Timeout in seconds

# Error handling policy
error_policy:
  on_item_failure: dlq  # dlq, retry, skip, stop, or custom handler name
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.2  # 20% failure rate
  error_collection: aggregate  # aggregate, immediate, or batched:N

  # Circuit breaker configuration
  circuit_breaker:
    failure_threshold: 5      # Open circuit after N failures
    success_threshold: 2      # Close circuit after N successes
    timeout: 60              # Seconds before attempting half-open
    half_open_requests: 3    # Test requests in half-open state

  # Retry configuration with backoff
  retry_config:
    max_attempts: 3
    backoff:
      type: exponential      # fixed, linear, exponential, fibonacci
      initial: 1000          # Initial delay in ms
      multiplier: 2          # For exponential
      max_delay: 30000       # Maximum delay in ms

# Convenience fields (alternative to nested error_policy)
# These top-level fields map to error_policy for simpler syntax
on_item_failure: dlq
continue_on_failure: true
max_failures: 5
```

## Setup Phase (Advanced)

The setup phase supports two formats: simple array OR full configuration object.

```yaml
# Simple array format
setup:
  - shell: "prepare-data.sh"
  - shell: "analyze-codebase.sh"

# Full configuration format with timeout and capture
setup:
  commands:
    - shell: "prepare-data.sh"
    - shell: "analyze-codebase.sh"

  # Timeout for entire setup phase (seconds)
  timeout: 300

  # Capture outputs from setup commands
  capture_outputs:
    # Simple format (legacy - just index)
    file_count: 0  # Capture from command at index 0

    # Full CaptureConfig format
    analysis_result:
      command_index: 1
      format: json  # string, number, json, lines, boolean
```

**Setup Phase Fields:**
- `commands` - Array of commands to execute (or use simple array format at top level)
- `timeout` - Timeout for entire setup phase in seconds
- `capture_outputs` - Map of variable names to command outputs (supports Simple(index) or full CaptureConfig)

## Global Storage Architecture

MapReduce workflows use a global storage architecture located in `~/.prodigy/` (not `.prodigy/` in your project). This enables:

- **Cross-worktree event aggregation**: Multiple worktrees working on the same job share event logs
- **Persistent state management**: Job checkpoints survive worktree cleanup
- **Centralized monitoring**: All job data accessible from a single location
- **Efficient storage**: Deduplication across worktrees

### Storage Locations

```
~/.prodigy/
├── events/
│   └── {repo_name}/          # Events grouped by repository
│       └── {job_id}/         # Job-specific events
│           └── events-{timestamp}.jsonl  # Event log files
├── dlq/
│   └── {repo_name}/          # DLQ grouped by repository
│       └── {job_id}/         # Job-specific failed items
└── state/
    └── {repo_name}/          # State grouped by repository
        └── mapreduce/        # MapReduce job states
            └── jobs/
                └── {job_id}/ # Job-specific checkpoints
```

## Event Tracking

All MapReduce execution events are logged to `~/.prodigy/events/{repo_name}/{job_id}/` for debugging and monitoring:

**Events Tracked:**
- Agent lifecycle events (started, completed, failed)
- Work item processing status
- Checkpoint saves for resumption
- Error details with correlation IDs
- Cross-worktree event aggregation for parallel jobs

**Event Log Format:**
Events are stored in JSONL (JSON Lines) format, with each line representing a single event:

```json
{"timestamp":"2024-01-01T12:00:00Z","event_type":"agent_started","agent_id":"agent-1","item_id":"item-001"}
{"timestamp":"2024-01-01T12:05:00Z","event_type":"agent_completed","agent_id":"agent-1","item_id":"item-001","status":"success"}
```

**Viewing Events:**
```bash
# View all events for a job
prodigy events <job_id>

# Stream events in real-time
prodigy events <job_id> --follow
```

## Checkpoint and Resume

MapReduce workflows automatically save checkpoints to enable resumption after interruption.

### Checkpoint Structure

Checkpoints are stored in `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/` and contain:

```json
{
  "job_id": "mapreduce-1234567890",
  "workflow_file": "workflow.yml",
  "phase": "map",
  "items_processed": 45,
  "items_total": 100,
  "items_remaining": ["item-046", "item-047", "..."],
  "successful_items": 43,
  "failed_items": 2,
  "started_at": "2024-01-01T12:00:00Z",
  "last_checkpoint_at": "2024-01-01T12:30:00Z"
}
```

### Resume Behavior

When resuming a MapReduce job:

1. **Checkpoint Loading**: Prodigy loads the most recent checkpoint from `~/.prodigy/state/`
2. **Work Item Recovery**: Items marked as "in progress" are reset to "pending"
3. **Failed Item Handling**: Previously failed items are moved to DLQ (not retried automatically)
4. **Partial Results**: Successfully processed items are preserved
5. **Phase Continuation**: Job resumes from the phase it was interrupted in

**Resume Command:**
```bash
# Resume from checkpoint
prodigy resume-job <job_id>

# Resume with different parallelism
prodigy resume-job <job_id> --max-parallel 20

# Resume and show detailed logs
prodigy resume-job <job_id> -v
```

## Dead Letter Queue (DLQ)

Failed work items are automatically stored in the DLQ for review and retry.

### DLQ Storage

Failed items are stored in `~/.prodigy/dlq/{repo_name}/{job_id}/` with this structure:

```json
{
  "item_id": "item-047",
  "item_data": {
    "path": "src/module.rs",
    "score": 8,
    "priority": "high"
  },
  "failure_reason": "Command failed: cargo test",
  "error_details": "test failed: expected X but got Y",
  "failed_at": "2024-01-01T12:15:00Z",
  "attempt_count": 3,
  "correlation_id": "agent-7-item-047"
}
```

### DLQ Retry

The `prodigy dlq retry` command allows you to reprocess failed items:

```bash
# Retry all failed items for a job
prodigy dlq retry <job_id>

# Retry with custom parallelism (default: 5)
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run

# Verbose output for debugging
prodigy dlq retry <job_id> -v
```

**DLQ Retry Features:**
- Streams items to avoid memory issues with large queues
- Respects original workflow's `max_parallel` setting
- Preserves correlation IDs for tracking
- Updates DLQ state (removes successful, keeps failed)
- Supports interruption and resumption
- Retried items inherit original workflow configuration

**DLQ Retry Workflow:**
1. Load failed items from `~/.prodigy/dlq/{repo_name}/{job_id}/`
2. Process items using original workflow's agent template
3. Successfully processed items are removed from DLQ
4. Still-failing items remain in DLQ with updated attempt count
5. New failures during retry are logged and added to DLQ

### Viewing DLQ Contents

```bash
# List all failed items
prodigy dlq list <job_id>

# Show details for specific item
prodigy dlq show <job_id> <item_id>

# Clear DLQ after manual fixes
prodigy dlq clear <job_id>
```
