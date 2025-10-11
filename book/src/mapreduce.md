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
    timeout: "60s"           # Duration before attempting half-open (e.g., "60s", "1m", "5m")
    half_open_requests: 3    # Test requests in half-open state

  # Retry configuration with backoff
  retry_config:
    max_attempts: 3
    # BackoffStrategy is an enum - use one variant:
    backoff:
      exponential:
        initial: "1s"
        multiplier: 2.0

# Convenience fields (alternative to nested error_policy)
# These top-level fields map to error_policy for simpler syntax
on_item_failure: dlq
continue_on_failure: true
max_failures: 5
```

## Backoff Strategies

The `retry_config.backoff` field uses an enum-based configuration. Choose ONE of the following strategies:

### Fixed Delay
Retry with a constant delay between attempts:
```yaml
retry_config:
  max_attempts: 3
  backoff:
    fixed:
      delay: "1s"  # Constant delay (e.g., "1s", "500ms", "2m")
```

### Linear Backoff
Increase delay linearly with each attempt:
```yaml
retry_config:
  max_attempts: 5
  backoff:
    linear:
      initial: "1s"      # First retry delay
      increment: "500ms" # Add this much for each subsequent retry
```
Example delays: 1s, 1.5s, 2s, 2.5s, 3s

### Exponential Backoff
Double (or multiply) the delay with each attempt:
```yaml
retry_config:
  max_attempts: 4
  backoff:
    exponential:
      initial: "1s"    # First retry delay
      multiplier: 2.0  # Multiply delay by this factor each time
```
Example delays (multiplier=2.0): 1s, 2s, 4s, 8s

### Fibonacci Backoff
Use Fibonacci sequence for delays (gradual exponential growth):
```yaml
retry_config:
  max_attempts: 6
  backoff:
    fibonacci:
      initial: "1s"  # Base delay unit
```
Example delays: 1s, 1s, 2s, 3s, 5s, 8s

**Important Notes:**
- All duration fields use [humantime format](https://docs.rs/humantime/latest/humantime/): "1s", "500ms", "2m", "1h30m"
- The `backoff` field is an **enum** - use ONE variant, not a `type` discriminator
- Use `max_attempts` to limit total retries (there is no `max_delay` field)
- Choose strategy based on your use case:
  - **Fixed**: Predictable timing, good for known transient issues
  - **Linear**: Gradual increase, good for slowly-recovering services
  - **Exponential**: Fast backoff, good for rate limiting and congestion
  - **Fibonacci**: Balanced growth, good for general-purpose retries

## Error Collection Strategies

The `error_collection` field controls how errors are reported during workflow execution:

```yaml
error_policy:
  # Collect all errors and report at workflow end (default)
  error_collection: aggregate

  # OR: Report each error immediately as it occurs
  error_collection: immediate

  # OR: Report errors in batches of N items
  error_collection: batched:10
```

**Strategy Behaviors:**
- `aggregate` - Collect all errors in memory and report at the end
  - Use for: Final summary reports, batch processing where individual failures don't need immediate attention
  - Trade-off: Low noise, but you won't see errors until completion
- `immediate` - Log/report each error as soon as it happens
  - Use for: Debugging, development, real-time monitoring
  - Trade-off: More verbose, but immediate visibility
- `batched:N` - Report errors in batches of N items
  - Use for: Progress updates without spam, monitoring long-running jobs
  - Trade-off: Balance between noise and visibility (e.g., `batched:10` reports every 10 failures)

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
    # Simple format (shorthand - captures stdout with defaults)
    file_count: 0  # Capture stdout from command at index 0

    # Detailed CaptureConfig format
    analysis_result:
      command_index: 1
      source: stdout           # stdout, stderr, both, combined
      json_path: "$.result"    # Extract JSON field
      max_size: 1048576        # Max bytes (1MB)
      default: "{}"            # Fallback if extraction fails
      multiline: preserve      # preserve, join, first_line, last_line, array
```

**Setup Phase Fields:**
- `commands` - Array of commands to execute (or use simple array format at top level)
- `timeout` - Timeout for entire setup phase in seconds
- `capture_outputs` - Map of variable names to command outputs (Simple or Detailed format)

### Capture Configuration Formats

Prodigy supports two CaptureConfig formats:

**Simple Format** - Shorthand for common cases:
```yaml
capture_outputs:
  file_count: 0  # Captures stdout from command at index 0 with default settings
```

**Detailed Format** - Full control over capture behavior:
```yaml
capture_outputs:
  analysis_result:
    command_index: 1                # Which command to capture from
    source: stdout                  # stdout, stderr, both, combined
    pattern: "version: (\\d+\\.\\d+)" # Regex extraction (optional)
    json_path: "$.result"           # JSON path extraction (optional)
    max_size: 1048576               # Max bytes to capture (optional, default 1MB)
    default: "{}"                   # Fallback value if extraction fails (optional)
    multiline: preserve             # How to handle multiple lines (optional)
```

**CaptureConfig Fields:**
- `command_index` - Zero-based index of command to capture from (required)
- `source` - Where to capture from (optional, default: stdout):
  - `stdout` - Capture standard output only
  - `stderr` - Capture standard error only
  - `both` - Capture both with labels (stdout:\n...\nstderr:\n...)
  - `combined` - Interleave stdout and stderr in order
- `pattern` - Regex pattern for extraction (optional, use with capture groups)
- `json_path` - JSONPath expression for JSON extraction (optional, e.g., "$.items[*].name")
- `max_size` - Maximum bytes to capture (optional, default: 1MB)
- `default` - Fallback value if extraction fails (optional)
- `multiline` - How to handle multiple output lines (optional, default: preserve):
  - `preserve` - Keep all lines with newlines
  - `join` - Join lines with spaces (useful for single-line summaries)
  - `first_line` - Take only first line (useful for version strings)
  - `last_line` - Take only last line (useful for final status)
  - `array` - Return as JSON array of lines (useful for lists)

**Capture Configuration Examples:**

```yaml
# Extract version number from output
capture_outputs:
  version:
    command_index: 0
    pattern: "version: (\\d+\\.\\d+\\.\\d+)"
    multiline: first_line

# Parse JSON result
capture_outputs:
  items:
    command_index: 1
    source: stdout
    json_path: "$.items[*]"

# Capture error messages for debugging
capture_outputs:
  errors:
    command_index: 2
    source: stderr
    multiline: array

# Get file count as single number
capture_outputs:
  count:
    command_index: 3
    pattern: "(\\d+) files"
    multiline: first_line
    default: "0"
```

**When to use Simple vs Detailed:**
- Use **Simple** when you only need stdout with default settings
- Use **Detailed** when you need:
  - Pattern extraction with regex
  - JSON parsing with JSONPath
  - Custom source (stderr, both, combined)
  - Multiline handling options
  - Fallback values with `default`

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

## Common Pitfalls

### Incorrect CaptureConfig Fields

**Problem:** Using `format: json` in capture_outputs configuration.

```yaml
# ❌ WRONG - 'format' field doesn't exist
capture_outputs:
  result:
    command_index: 0
    format: json
```

**Solution:** Use the correct CaptureConfig fields:

```yaml
# ✅ CORRECT - Use json_path for JSON extraction
capture_outputs:
  result:
    command_index: 0
    json_path: "$.result"
    source: stdout
    multiline: preserve
```

### Incorrect Backoff Enum Syntax

**Problem:** Using flat `type` discriminator for backoff strategy.

```yaml
# ❌ WRONG - Using 'type' discriminator
retry_config:
  max_attempts: 3
  backoff:
    type: exponential
    initial: "1s"
    multiplier: 2
```

**Solution:** Use the correct enum variant syntax:

```yaml
# ✅ CORRECT - Enum variant with nested fields
retry_config:
  max_attempts: 3
  backoff:
    exponential:
      initial: "1s"
      multiplier: 2.0
```

### Duration Format Errors

**Problem:** Using numeric values instead of humantime strings.

```yaml
# ❌ WRONG - Numbers without units
backoff:
  fixed:
    delay: 1000  # Unclear: milliseconds? seconds?
```

**Solution:** Always use humantime format strings:

```yaml
# ✅ CORRECT - Explicit units
backoff:
  fixed:
    delay: "1s"  # or "1000ms", "1m", "1h30m"
```

### Confusing Simple vs Detailed Capture

**Problem:** Treating simple capture format as "legacy" or not knowing when to use it.

**Solution:**
- Use **Simple** (`variable: 0`) when you only need stdout with defaults
- Use **Detailed** when you need pattern/json extraction, custom source, or multiline handling
- Simple format is NOT deprecated - it's a valid shorthand

### Missing Multiplier Decimal Point

**Problem:** Using integer multiplier in exponential backoff.

```yaml
# ⚠️  POTENTIAL ISSUE - Integer might work but float is safer
backoff:
  exponential:
    multiplier: 2
```

**Solution:** Use float values explicitly:

```yaml
# ✅ CORRECT - Float value
backoff:
  exponential:
    multiplier: 2.0
```

### Incorrect Variable References

**Problem:** Using wrong variable names in map/reduce phases.

```yaml
# ❌ WRONG - These variables don't exist
reduce:
  - shell: "echo ${results}"  # Should be ${map.results}
  - shell: "echo ${total}"    # Should be ${map.total}
```

**Solution:** Use the correct variable namespaces (see [Variables chapter](./variables.md)):

```yaml
# ✅ CORRECT - Proper variable names
reduce:
  - shell: "echo ${map.results}"
  - shell: "echo ${map.successful}/${map.total} items"
```

### Max Delay Field (Doesn't Exist)

**Problem:** Trying to use `max_delay` field in backoff configuration.

```yaml
# ❌ WRONG - max_delay is not supported
backoff:
  exponential:
    initial: "1s"
    multiplier: 2.0
    max_delay: "60s"  # This field doesn't exist
```

**Solution:** Use `max_attempts` to limit retries instead:

```yaml
# ✅ CORRECT - Limit via max_attempts
retry_config:
  max_attempts: 5  # Limits total retry attempts
  backoff:
    exponential:
      initial: "1s"
      multiplier: 2.0
```

### Nested Commands Syntax

**Problem:** Mixing modern and deprecated syntax.

```yaml
# ⚠️  DEPRECATED but still supported
agent_template:
  commands:
    - shell: "test.sh"
```

**Solution:** Use modern flat syntax:

```yaml
# ✅ MODERN - Commands directly under agent_template
agent_template:
  - shell: "test.sh"
  - claude: "/process '${item}'"
```

## Troubleshooting

### Workflow Validation Errors

If you see validation errors when running a MapReduce workflow:

1. **Check backoff syntax**: Ensure you're using enum variants (e.g., `exponential: { initial: "1s" }`), not `type` discriminators
2. **Check duration formats**: All duration fields must use humantime format (e.g., `"1s"`, `"500ms"`)
3. **Check CaptureConfig fields**: Don't use `format` - use `json_path` or `pattern` instead
4. **Check variable references**: Use `${map.*}`, `${item.*}`, `${merge.*}` namespaces

### Common Error Messages

**"unknown field `format`"**
- You're using `format` in CaptureConfig
- Solution: Remove `format`, use `json_path` or `pattern`

**"missing field `delay`" (in Fixed backoff)**
- You're using `initial` instead of `delay` for Fixed strategy
- Solution: Fixed uses `delay`, not `initial`

**"data did not match any variant"**
- Your backoff configuration doesn't match any enum variant
- Solution: Check the exact field names for your chosen strategy

**"invalid value: integer, expected a string"**
- You're using a number for a duration field
- Solution: Use quoted humantime strings (e.g., `"1s"` instead of `1`)

### Performance Issues

**Too many parallel agents overwhelming system:**
- Reduce `max_parallel` value
- Use circuit breaker to prevent cascading failures
- Monitor with `prodigy events <job_id> --follow`

**DLQ filling up with same errors:**
- Check DLQ contents: `prodigy dlq list <job_id>`
- Fix root cause before retrying
- Use `error_collection: immediate` for faster debugging

### Resume Not Working

**Checkpoint not found:**
- Check `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`
- Ensure you're using the correct job_id
- Run `prodigy resume-job <job_id> -v` for detailed logs

**Resume starts from beginning:**
- Checkpoints may be corrupted
- Check event logs: `prodigy events <job_id>`
- Consider using offset to skip already-processed items
