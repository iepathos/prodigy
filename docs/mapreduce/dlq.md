# Dead Letter Queue (DLQ)

The Dead Letter Queue (DLQ) is Prodigy's failure management system for MapReduce workflows. When work items fail repeatedly during map phase execution, they are automatically routed to the DLQ for analysis, debugging, and selective retry.

## Overview

The DLQ provides comprehensive failure tracking and recovery capabilities:

- **Automatic Routing**: Failed items exceeding retry limits are automatically moved to DLQ
- **Failure Analysis**: Pattern detection, temporal distribution, and error grouping
- **Selective Retry**: Filter and reprocess specific items with configurable parallelism
- **Debug Integration**: Preserves Claude JSON logs and worktree artifacts for troubleshooting
- **Capacity Management**: Automatic eviction of oldest items when limits are reached
- **Rich CLI**: Eight specialized commands for DLQ management

## Data Structures

### DeadLetteredItem

Each failed work item in the DLQ contains comprehensive failure information:

```rust
// Source: src/cook/execution/dlq.rs:32-42
pub struct DeadLetteredItem {
    pub item_id: String,
    pub item_data: Value,
    pub first_attempt: DateTime<Utc>,
    pub last_attempt: DateTime<Utc>,
    pub failure_count: u32,
    pub failure_history: Vec<FailureDetail>,
    pub error_signature: String,
    pub worktree_artifacts: Option<WorktreeArtifacts>,
    pub reprocess_eligible: bool,
    pub manual_review_required: bool,
}
```

**Field Descriptions:**

- `item_id`: Unique identifier for the work item
- `item_data`: Original work item JSON data from map phase input
- `first_attempt` / `last_attempt`: Timestamps tracking failure timespan
- `failure_count`: Number of failed attempts
- `failure_history`: Detailed history of each failure attempt (see below)
- `error_signature`: Normalized error pattern for grouping similar failures
- `worktree_artifacts`: Preserved worktree state for debugging
- `reprocess_eligible`: Whether item can be automatically retried
- `manual_review_required`: Flag indicating complex failures needing human intervention

### FailureDetail

Each attempt in `failure_history` captures detailed failure context:

```rust
// Source: src/cook/execution/dlq.rs:47-58
pub struct FailureDetail {
    pub attempt_number: u32,
    pub timestamp: DateTime<Utc>,
    pub error_type: ErrorType,
    pub error_message: String,
    pub stack_trace: Option<String>,
    pub agent_id: String,
    pub step_failed: String,
    pub duration_ms: u64,
    pub json_log_location: Option<String>,
}
```

**Key Fields:**

- `json_log_location`: Path to Claude JSON log file for Claude command failures (see [Observability](../advanced/observability.md#claude-json-logs))
- `error_type`: Categorized error classification (see below)
- `step_failed`: Which workflow step caused the failure
- `stack_trace`: Full error stack trace when available

### ErrorType

The DLQ categorizes failures into specific error types:

```rust
// Source: src/cook/execution/dlq.rs:63-70
pub enum ErrorType {
    Timeout,
    CommandFailed { exit_code: i32 },
    WorktreeError,
    MergeConflict,
    ValidationFailed,
    ResourceExhausted,
    Unknown,
}
```

**Error Type Descriptions:**

| Error Type | Description | Common Causes |
|------------|-------------|---------------|
| `Timeout` | Operation exceeded time limit | Long-running commands, hanging processes |
| `CommandFailed` | Shell/Claude command returned non-zero exit code | Test failures, build errors, invalid syntax |
| `WorktreeError` | Git worktree operation failed | Disk space, permissions, corrupted repo |
| `MergeConflict` | Failed to merge agent changes back to parent | Concurrent modifications, conflicting edits |
| `ValidationFailed` | Post-execution validation failed | Output doesn't meet requirements |
| `ResourceExhausted` | System resources depleted | Out of memory, disk full, too many processes |
| `Unknown` | Unclassified error | Unexpected failures, system errors |

## Automatic DLQ Routing

Items are automatically moved to the DLQ when they exceed the configured `max_retries` limit during map phase execution. The routing logic:

1. Agent fails during work item processing
2. Failure count increments
3. If `failure_count > max_retries`, item moves to DLQ
4. Otherwise, item is requeued for another attempt

This ensures persistent failures don't block workflow progress while preserving them for later analysis.

## Storage Structure

DLQ items are stored in the global Prodigy storage directory:

```
~/.prodigy/dlq/{repo_name}/{job_id}/
├── index.json              # DLQ metadata and item index
└── items/
    ├── item-1.json         # Individual failed items
    ├── item-2.json
    └── ...
```

**Organization:**

- **Repository-scoped**: DLQ data grouped by repository name
- **Job-scoped**: Each MapReduce job has its own DLQ subdirectory
- **Indexed**: `index.json` provides fast lookup and filtering
- **Persistent**: DLQ survives worktree cleanup and job completion

## CLI Commands

The DLQ system provides eight specialized commands for management and recovery.

### List Items

List items in the Dead Letter Queue with optional filtering:

```bash
prodigy dlq list [--job-id <ID>] [--eligible] [--limit <N>]
```

**Options:**

- `--job-id`: Filter to specific MapReduce job
- `--eligible`: Show only reprocess-eligible items
- `--limit`: Maximum number of items to display (default: 50)

**Example:**

```bash
# List all DLQ items
prodigy dlq list

# List eligible items for specific job
prodigy dlq list --job-id mapreduce-20240111-123456 --eligible

# List first 10 items
prodigy dlq list --limit 10
```

### Inspect Item

View detailed information about a specific DLQ item:

```bash
prodigy dlq inspect <item_id> [--job-id <ID>]
```

**Example:**

```bash
prodigy dlq inspect item-42 --job-id mapreduce-20240111-123456
```

**Output includes:**

- Complete item data
- All failure attempts with timestamps
- Error messages and stack traces
- Claude JSON log locations
- Worktree artifact paths
- Reprocess eligibility status

### Analyze Patterns

Analyze failure patterns across DLQ items:

```bash
prodigy dlq analyze [--job-id <ID>] [--export <file>]
```

**Features:**

- **Pattern Grouping**: Groups failures by error signature
- **Temporal Distribution**: Shows failure rates over time
- **Error Classification**: Breaks down by ErrorType
- **Sample Items**: Provides representative examples for each pattern

**Example:**

```bash
# Analyze failures for a job
prodigy dlq analyze --job-id mapreduce-20240111-123456

# Export analysis to file
prodigy dlq analyze --job-id mapreduce-20240111-123456 --export analysis.json
```

**Sample Output:**

```
Failure Analysis for mapreduce-20240111-123456
==============================================

Error Patterns:
  Pattern 1: "cargo test failed in tests/integration" (15 items, 45%)
    Error Type: CommandFailed
    Sample Items: item-5, item-12, item-23

  Pattern 2: "timeout after 300s" (8 items, 24%)
    Error Type: Timeout
    Sample Items: item-7, item-19

  Pattern 3: "merge conflict in src/lib.rs" (6 items, 18%)
    Error Type: MergeConflict
    Sample Items: item-3, item-14

Temporal Distribution:
  2024-01-11 10:00-11:00: 12 failures
  2024-01-11 11:00-12:00: 8 failures
  2024-01-11 12:00-13:00: 9 failures
```

!!! tip "Error Signatures"
    Error signatures are normalized versions of error messages with variable parts (paths, numbers, timestamps) removed. This allows grouping of similar failures for batch analysis and resolution.

### Export Items

Export DLQ items for external analysis:

```bash
prodigy dlq export <output> [--job-id <ID>] [--format <json|csv>]
```

**Options:**

- `output`: Output file path
- `--job-id`: Export specific job's items
- `--format`: Export format (json or csv, default: json)

**Example:**

```bash
# Export as JSON
prodigy dlq export failures.json --job-id mapreduce-20240111-123456

# Export as CSV for spreadsheet analysis
prodigy dlq export failures.csv --format csv
```

### Purge Old Items

Remove old DLQ items based on retention policy:

```bash
prodigy dlq purge --older-than-days <N> [--job-id <ID>] [--yes]
```

**Options:**

- `--older-than-days`: Delete items older than N days
- `--job-id`: Purge specific job only
- `--yes`: Skip confirmation prompt

**Example:**

```bash
# Purge items older than 30 days
prodigy dlq purge --older-than-days 30

# Purge old items for specific job without prompt
prodigy dlq purge --older-than-days 7 --job-id mapreduce-20240111-123456 --yes
```

!!! warning "Data Loss"
    Purging permanently deletes DLQ items. Ensure you've exported or resolved items before purging.

### Retry Failed Items

Reprocess failed items from the DLQ:

```bash
prodigy dlq retry <workflow_id> [--filter <expr>] [--max-retries <N>] [--parallel <N>] [--force]
```

**Options:**

- `workflow_id`: MapReduce job/workflow to retry
- `--filter`: JSONPath filter expression for selective retry
- `--max-retries`: Maximum retry attempts per item (default: 3)
- `--parallel`: Number of concurrent retry workers (default: 10)
- `--force`: Retry items even if not marked reprocess-eligible

**Examples:**

```bash
# Retry all eligible items for a job
prodigy dlq retry mapreduce-20240111-123456

# Retry only timeout errors with custom parallelism
prodigy dlq retry mapreduce-20240111-123456 \
  --filter "$.error_type == 'Timeout'" \
  --parallel 5

# Force retry all items regardless of eligibility
prodigy dlq retry mapreduce-20240111-123456 --force

# Retry with limited attempts
prodigy dlq retry mapreduce-20240111-123456 --max-retries 1
```

**Retry Behavior:**

- Creates new agent executions for each item
- Uses original workflow configuration
- Updates DLQ: removes successful items, keeps failed items
- Preserves correlation IDs for tracking
- Supports interruption and resumption

!!! tip "Incremental Retry Strategy"
    Start with a small filtered subset to validate fixes before retrying all items. Use `--parallel 1` for debugging to see detailed logs.

### Show Statistics

Display DLQ statistics and health metrics:

```bash
prodigy dlq stats [--workflow-id <ID>]
```

**Metrics:**

- Total items in DLQ
- Items eligible for reprocessing
- Items requiring manual review
- Oldest and newest item timestamps
- Error category breakdown

**Example:**

```bash
prodigy dlq stats --workflow-id mapreduce-20240111-123456
```

**Sample Output:**

```
DLQ Statistics for mapreduce-20240111-123456
============================================

Total Items: 29
Reprocess Eligible: 21
Manual Review Required: 8

Age Range:
  Oldest: 2024-01-11 10:23:45 UTC (2 hours ago)
  Newest: 2024-01-11 12:15:32 UTC (8 minutes ago)

Error Categories:
  CommandFailed: 15 (52%)
  Timeout: 8 (28%)
  MergeConflict: 6 (20%)
```

### Clear Processed Items

Remove successfully reprocessed items from the DLQ:

```bash
prodigy dlq clear <workflow_id> [--yes]
```

**Example:**

```bash
# Clear with confirmation
prodigy dlq clear mapreduce-20240111-123456

# Clear without prompt
prodigy dlq clear mapreduce-20240111-123456 --yes
```

!!! note "Clear vs Purge"
    `clear` removes items that have been successfully reprocessed. `purge` removes items based on age retention policy.

## Capacity Management

The DLQ has a configurable capacity limit (default: 1000 items) to prevent unbounded growth.

**Eviction Policy:**

When the DLQ reaches capacity:

1. System identifies the 10% oldest items (by `first_attempt` timestamp)
2. Oldest items are evicted automatically
3. Evicted items are logged in DLQ events
4. Warning is logged indicating capacity reached

**Configuration:**

```rust
// Source: src/cook/execution/dlq.rs:139-159
let dlq_config = DLQConfig {
    max_items: 1000,
    eviction_percentage: 10,
};
```

!!! warning "Capacity Planning"
    Monitor DLQ growth with `prodigy dlq stats`. If approaching capacity regularly, either increase `max_items` or implement more aggressive purging policies.

## Error Signature Generation

Error signatures enable pattern grouping by normalizing error messages:

```rust
// Source: src/cook/execution/dlq.rs:419-430
fn generate_error_signature(error_message: &str) -> String {
    // Remove variable parts: file paths, line numbers, timestamps
    let normalized = error_message
        .replace(|c: char| c.is_numeric(), "N")
        .replace("/home/user/", "/*/")
        .replace("at line ", "at line N");
    normalized
}
```

**Example:**

| Original Error | Error Signature |
|----------------|-----------------|
| `cargo test failed at tests/integration.rs:42` | `cargo test failed at tests/integration.rs:NN` |
| `/home/alice/project/src/lib.rs not found` | `/*/project/src/lib.rs not found` |
| `timeout after 305 seconds` | `timeout after NNN seconds` |

This allows the `analyze` command to group similar failures together.

## Worktree Artifacts Preservation

For failed items, the DLQ can preserve worktree state for debugging:

```rust
// Source: src/cook/execution/dlq.rs:73-80
pub struct WorktreeArtifacts {
    pub worktree_path: PathBuf,
    pub branch_name: String,
    pub uncommitted_changes: bool,
    pub error_logs: Vec<PathBuf>,
}
```

**Preserved Information:**

- `worktree_path`: Full path to the failed agent's worktree
- `branch_name`: Git branch where failure occurred
- `uncommitted_changes`: Whether work was partially completed
- `error_logs`: Paths to relevant log files

!!! tip "Debugging Failed Items"
    Use `prodigy dlq inspect <item_id>` to get the worktree path, then navigate to it to examine the exact state when the failure occurred.

## Integration with Observability

### Claude JSON Logs

Failed Claude commands include `json_log_location` in `FailureDetail`, linking to the complete Claude execution log:

```bash
# Get JSON log location from DLQ item
prodigy dlq inspect item-42 --job-id mapreduce-20240111-123456 | \
  jq '.failure_history[0].json_log_location'

# View the Claude JSON log
cat ~/.local/state/claude/logs/session-abc123.json | jq
```

See [Claude Command Observability](../advanced/observability.md#claude-json-logs) for details.

### DLQ Events

The DLQ emits events for observability:

```rust
// Source: src/cook/execution/dlq.rs:127-135
pub enum DLQEvent {
    ItemAdded { item_id: String, job_id: String },
    ItemRemoved { item_id: String, reason: String },
    ItemsReprocessed { count: usize, successful: usize },
    ItemsEvicted { count: usize, oldest_timestamp: DateTime<Utc> },
    AnalysisGenerated { job_id: String, pattern_count: usize },
}
```

Events are written to `~/.prodigy/events/{repo_name}/{job_id}/` for audit trails.

## Filtering

The DLQ supports filtering for selective operations:

```rust
// Source: src/cook/execution/dlq.rs:117-125
pub struct DLQFilter {
    pub error_type: Option<ErrorType>,
    pub reprocess_eligible: Option<bool>,
    pub after: Option<DateTime<Utc>>,
    pub before: Option<DateTime<Utc>>,
    pub error_signature: Option<String>,
}
```

**Filter Examples:**

```bash
# Filter by error type (using JSONPath in retry)
prodigy dlq retry job-123 --filter "$.error_type == 'Timeout'"

# Filter by eligibility
prodigy dlq list --eligible

# Filter by time range (in analyze command)
prodigy dlq analyze --job-id job-123 --after "2024-01-11T10:00:00Z"
```

## Workflow Integration

The DLQ integrates seamlessly with MapReduce workflow lifecycle:

```
┌─────────────────┐
│  Map Phase      │
│  Work Items     │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Agent Execution│
└────────┬────────┘
         │
    Failed? ───────────────┐
         │                 │
         │ Yes             │ No
         ▼                 ▼
┌─────────────────┐  ┌──────────────┐
│ Retry Logic     │  │ Success Path │
│ (max_retries)   │  └──────────────┘
└────────┬────────┘
         │
  Still Failing?
         │
         ▼
┌─────────────────┐
│  DLQ Routing    │
│  (automatic)    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  DLQ Storage    │
│  - Analyze      │
│  - Debug        │
│  - Retry        │
└─────────────────┘
```

## Best Practices

### When to Use Manual Review

Mark items for manual review when:

- Errors require code changes or workflow modifications
- Failure patterns indicate systematic issues
- Multiple retry attempts consistently fail
- Error messages are unclear or contradictory

### Retry Strategies

**Incremental Approach:**

1. Analyze patterns first: `prodigy dlq analyze --job-id <ID>`
2. Fix systematic issues (code bugs, resource limits)
3. Test with small subset: `prodigy dlq retry <ID> --parallel 1 --limit 5`
4. Scale up: `prodigy dlq retry <ID> --parallel 10`

**Filter-Based Retry:**

```bash
# Retry only specific error types after fixing root cause
prodigy dlq retry job-123 --filter "$.error_type == 'CommandFailed'"

# Retry items from specific time period
prodigy dlq retry job-123 --filter "$.last_attempt > '2024-01-11T12:00:00Z'"
```

### Retention Management

Establish retention policies to prevent unbounded DLQ growth:

```bash
# Weekly purge of items older than 30 days
prodigy dlq purge --older-than-days 30 --yes

# Export before purging for long-term records
prodigy dlq export archive-$(date +%Y%m%d).json
prodigy dlq purge --older-than-days 30 --yes
```

### Monitoring DLQ Health

Regularly check DLQ statistics:

```bash
# Check overall DLQ size
prodigy dlq stats

# Check specific job health
prodigy dlq stats --workflow-id <ID>

# Alert if manual review items exceed threshold
MANUAL_REVIEW=$(prodigy dlq stats --workflow-id <ID> | grep "Manual Review Required" | awk '{print $4}')
if [ "$MANUAL_REVIEW" -gt 10 ]; then
  echo "Warning: High manual review count: $MANUAL_REVIEW"
fi
```

## Troubleshooting

### High Failure Rates

**Symptoms:**

- Large number of items entering DLQ rapidly
- Same error signature repeated across many items

**Solutions:**

1. Use `prodigy dlq analyze` to identify common patterns
2. Fix systematic issues (resource limits, code bugs)
3. Consider increasing timeout values in workflow config
4. Check for environmental issues (disk space, permissions)

### Capacity Issues

**Symptoms:**

- Warning messages about DLQ capacity reached
- Automatic eviction events in logs

**Solutions:**

1. Increase `max_items` configuration
2. Implement more aggressive purge policy
3. Export and archive old items
4. Review workflow for excessive failure rates

### Reprocessing Failures

**Symptoms:**

- Retry command fails repeatedly
- Items remain in DLQ after retry

**Solutions:**

1. Use `--parallel 1` to see detailed error logs
2. Inspect individual items: `prodigy dlq inspect <item_id>`
3. Check Claude JSON logs via `json_log_location`
4. Verify workflow configuration hasn't changed
5. Test with single item using `--filter` before batch retry

### Worktree Artifacts Missing

**Symptoms:**

- `worktree_artifacts` is null in DLQ item
- Cannot access failed worktree for debugging

**Cause:**

Worktrees may be cleaned up automatically after timeout or manual cleanup.

**Solutions:**

1. Check worktree cleanup policies
2. Use Claude JSON logs for debugging instead
3. Retry item to reproduce failure in new worktree
4. Adjust worktree retention settings

## Related Topics

- [MapReduce Resume Guide](../mapreduce-resume-guide.md) - Recovery mechanisms for interrupted workflows
- [Error Handling](../workflow-basics/error-handling.md) - Workflow-level error handling strategies
- [Observability](../advanced/observability.md) - Claude JSON logs and debugging
- [Storage Architecture](../advanced/storage.md) - Global storage structure
