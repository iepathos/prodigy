# Troubleshooting

## Common Issues

### 1. Variables not interpolating
**Symptom:** Variables appear as literal `${variable_name}` in output instead of their values.

**Causes:**
- Incorrect syntax (missing `${}` wrapper)
- Variable not defined or not in scope
- Typo in variable name

**Solutions:**
- Ensure proper `${}` syntax: `${workflow.name}`, not `$workflow.name`
- Check variable is defined before use
- Verify variable is available in current context (e.g., `${item.*}` only available in map phase)
- Use echo to debug: `- shell: "echo 'Variable value: ${my_var}'"`

---

### 2. Capture not working
**Symptom:** Captured variables are empty or contain unexpected data.

**Causes:**
- Incorrect `capture_format` for output type
- Command output not in expected format
- Missing or incorrect `capture_streams` configuration

**Solutions:**
- Match `capture_format` to output type and how it transforms output:
  - `string` - Captures raw text output as-is
  - `number` - Parses output as numeric value (int or float)
  - `json` - Parses JSON and allows JSONPath queries on the result
  - `lines` - Splits multi-line output into an array
  - `boolean` - Evaluates to true/false based on success status
- Test command output manually first
- Capture all streams for debugging:
  ```yaml
  - shell: "cargo test 2>&1"
    capture: "test_output"
    capture_streams:
      stdout: true      # Optional, default false - Capture standard output
      stderr: true      # Optional, default false - Capture error output
      exit_code: true   # Optional, default false - Capture exit code
      success: true     # Optional, default false - Capture success boolean
      duration: true    # Optional, default false - Capture execution duration
  ```
- **Note:** All capture_streams fields are optional and default to false. Only specify the streams you need to capture

---

### 3. Validation failing
**Symptom:** Goal-seeking or validation commands fail to recognize completion.

**Causes:**
- Validate command not outputting `score: N` format
- Threshold too high
- Score calculation incorrect
- Validation command not configured correctly

**Solutions:**
- Ensure validate command outputs exactly `score: N` (where N is 0-100)
- Validation is part of goal_seek commands with these fields:
  - `validate` - Command that outputs score
  - `threshold` - Minimum score to consider success (0-100)
  - `max_iterations` - Maximum attempts before giving up
  - `on_incomplete` - Commands to run when score below threshold
- Test validate command independently
- Lower threshold temporarily for debugging
- Example correct format:
  ```yaml
  - goal_seek:
      validate: |
        result=$(run-checks.sh | grep 'Percentage' | sed 's/.*: \([0-9]*\)%.*/\1/')
        echo "score: $result"
      threshold: 80
      max_iterations: 5
      on_incomplete:
        - claude: "/fix-issues"
  ```

---

### 4. MapReduce items not found
**Symptom:** Map phase finds zero items or wrong items.

**Causes:**
- Incorrect JSONPath expression
- Input file format doesn't match expectations
- Input file not generated in setup phase
- JSONPath syntax errors

**Solutions:**
- Test JSONPath expression with actual data.

  **⚠️ IMPORTANT: jq uses its own filter syntax, NOT JSONPath!**
  ```bash
  # jq uses its own syntax (not JSONPath)
  jq '.items[]' items.json

  # For filtering with jq
  jq '.items[] | select(.score >= 5)' items.json

  # To test actual JSONPath expressions, use an online JSONPath tester
  # or a tool that supports JSONPath directly
  ```
- Verify input file exists and contains expected structure
- Check setup phase completed successfully
- Use simpler JSONPath first: `$[*]` to get all items
- Common JSONPath mistakes:
  - Wrong bracket syntax: Use `$.items[*]` not `$.items[]`
  - Missing root `$`: Always start with `$`
  - Incorrect filter syntax: `$[?(@.score >= 5)]` for filtering
  - Nested paths: `$.data.items[*].field` for deep structures

---

### 5. Timeout errors
**Symptom:** Commands or workflows fail with timeout errors.

**Causes:**
- Commands take longer than expected
- Default timeout too short
- Infinite loops or hanging processes

**Solutions:**
- Increase timeout values using duration strings:
  ```yaml
  - shell: "slow-command.sh"
    timeout: "600s"  # or "10m" - uses humantime duration format
  ```
- For MapReduce, increase agent timeout (note: this uses seconds as a number):
  ```yaml
  map:
    agent_timeout_secs: 600  # Takes a number (seconds) not a duration string
  ```
- **Note:** `agent_timeout_secs` takes a number (seconds) while most other timeout fields use duration strings like "10m"
- Debug hanging commands by running them manually
- Add logging to identify slow steps

---

### 6. Environment variables not set
**Symptom:** Commands fail because required environment variables are missing.

**Causes:**
- Environment not inherited from parent process
- Typo in variable name
- Profile not activated
- Secret not loaded

**Solutions:**
- Ensure `inherit: true` in workflow config (default)
- Verify profile activation:
  ```yaml
  active_profile: "development"
  ```
- Check secrets are properly configured:
  ```yaml
  secrets:
    API_KEY: "${env:SECRET_API_KEY}"
  ```
- Debug with: `- shell: "env | grep VARIABLE_NAME"`

---

### 7. Merge workflow not running
**Symptom:** Custom merge commands not executed when merging worktree.

**Causes:**
- Merge block not properly formatted
- Syntax error in merge commands
- Merge workflow timeout too short

**Solutions:**
- Both merge formats are valid - choose based on needs:

  **Simplified format (direct list of commands):**
  ```yaml
  merge:
    - shell: "git fetch origin"
    - claude: "/merge-worktree ${merge.source_branch}"
  ```

  **Full format (with timeout configuration):**
  ```yaml
  merge:
    commands:
      - shell: "slow-merge-validation.sh"
    timeout: 600  # Timeout in seconds (plain number, not a duration string)
  ```
- Use the full format when you need to set a custom timeout
- **Note:** Unlike most other timeout fields in Prodigy which use duration strings ("10m", "600s"), the merge.timeout field takes a plain number representing seconds
- Check logs for merge execution errors

---

### 8. Commands failing without error handler
**Symptom:** Command fails and workflow stops immediately without recovery.

**Causes:**
- No `on_failure` handler configured
- Error not being caught by handler
- Handler itself failing

**Solutions:**
- Add `on_failure` handler to commands that might fail:
  ```yaml
  - shell: "risky-command.sh"
    on_failure:
      - shell: "echo 'Command failed, attempting recovery'"
      - claude: "/fix-issue"
  ```
- Commands without `on_failure` will stop the workflow on first error
- Check that your handler commands don't also fail
- Use shell exit codes to control failure: `command || exit 0` to ignore failures

---

### 9. Error policy configuration issues
**Symptom:** Retry, backoff, or circuit breaker not working as expected.

**Causes:**
- Incorrect Duration format for timeouts
- Wrong BackoffStrategy enum variant
- Invalid retry_config structure

**Solutions:**
- Use Duration strings for all timeout values:
  ```yaml
  error_policy:
    retry_config:
      max_attempts: 3
      initial_delay: "1s"    # Not 1000
      max_delay: "30s"       # Use duration strings
    circuit_breaker:
      timeout: "60s"         # Not 60
      failure_threshold: 5
  ```
- **Backoff Strategy:** Prodigy uses exponential backoff by default (base 2.0), which is not directly configurable in the workflow YAML. However, you CAN control the backoff behavior through the retry_config parameters:
  - `max_attempts` - Number of retry attempts before giving up
  - `initial_delay` - Starting delay between retries (e.g., "1s")
  - `max_delay` - Maximum delay between retries (e.g., "30s")
  - The delay doubles with each retry (exponential backoff with base 2.0) up to max_delay
  - Example: With `initial_delay: "1s"` and `max_delay: "30s"`, retries occur at 1s, 2s, 4s, 8s, 16s, 30s, 30s...
  - These parameters shape the exponential backoff curve to match your needs
- Circuit breaker requires both timeout and failure_threshold

---

### 10. Claude output visibility issues
**Symptom:** Can't see Claude's streaming output, or seeing too much output when you don't need it.

**Causes:**
- Default verbosity level hides Claude streaming output
- Running in CI/CD where streaming output clutters logs
- Need to debug Claude interactions but not seeing details

**Solutions:**
- **To see Claude streaming output**, use the `-v` flag:
  ```bash
  # Shows Claude streaming JSON output
  prodigy run workflow.yml -v
  ```
- **To force streaming output**, set environment variable:
  ```bash
  # Forces streaming regardless of verbosity
  PRODIGY_CLAUDE_CONSOLE_OUTPUT=true prodigy run workflow.yml
  ```
- **To disable streaming in CI/CD**, set environment variable:
  ```bash
  # Disables streaming for cleaner logs
  PRODIGY_CLAUDE_STREAMING=false prodigy run workflow.yml
  ```
- **For more detailed logs**, increase verbosity:
  ```bash
  prodigy run workflow.yml -vv   # Debug logs
  prodigy run workflow.yml -vvv  # Trace logs
  ```

**When to Use Each Mode:**
- **Use streaming (default)**: For debugging Claude interactions, maintaining an audit trail, and local development
- **Disable streaming (`PRODIGY_CLAUDE_STREAMING=false`)**: In CI/CD environments where disk space is constrained or when streaming logs aren't needed

**Streaming logs are saved to:** `~/.prodigy/logs/claude-streaming/` with format `{timestamp}-{uuid}.jsonl`. The log path is displayed before execution starts with a 📁 emoji for easy reference.

---

## Debug Tips

### Use verbosity flags for debugging
Prodigy supports multiple verbosity levels for debugging:
```bash
# Default: Clean output, no Claude streaming
prodigy run workflow.yml

# -v: Shows Claude streaming JSON output (useful for debugging Claude interactions)
prodigy run workflow.yml -v

# -vv: Adds debug-level logs
prodigy run workflow.yml -vv

# -vvv: Adds trace-level logs (very detailed)
prodigy run workflow.yml -vvv

# Force Claude streaming regardless of verbosity
PRODIGY_CLAUDE_CONSOLE_OUTPUT=true prodigy run workflow.yml
```

### Enable verbose output in shell commands
```yaml
- shell: "set -x; your-command"
```

### Inspect variables
```yaml
- shell: "echo 'Variable value: ${my_var}'"
- shell: "echo 'Item fields: path=${item.path} name=${item.name}'"
```

### Capture all streams for debugging
```yaml
- shell: "cargo test 2>&1"
  capture: "test_output"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: true
    success: true
    duration: true

# Then inspect
- shell: "echo 'Exit code: ${test_output.exit_code}'"
- shell: "echo 'Success: ${test_output.success}'"
- shell: "echo 'Duration: ${test_output.duration}s'"
```

### Test JSONPath expressions
```bash
# Note: jq uses its own filter syntax, not JSONPath
# Use jq for quick testing with equivalent expressions

# Get all items (equivalent to JSONPath $.items[*])
jq '.items[]' items.json

# Test with filter (equivalent to JSONPath $[?(@.score >= 5)])
jq '.items[] | select(.score >= 5)' items.json

# For actual JSONPath testing, use an online JSONPath tester
# or a tool that supports JSONPath directly
```

### Validate workflow syntax
```bash
# Workflows are validated automatically when loaded
# Check for syntax errors by attempting to run
prodigy run workflow.yml

# View the validation result file (if workflow validation completed)
cat .prodigy/validation-result.json
```

### Access Claude JSON logs for debugging

Prodigy maintains two types of Claude logs for comprehensive debugging:

1. **Prodigy's streaming logs** (JSONL format): `~/.prodigy/logs/claude-streaming/{timestamp}-{uuid}.jsonl`
   - Real-time streaming output during command execution
   - One JSON object per line (JSONL format)
   - Log path displayed before execution with 📁 emoji
   - Controlled by `PRODIGY_CLAUDE_STREAMING` environment variable

2. **Claude's native session logs** (JSON format): `~/.local/state/claude/logs/session-{id}.json`
   - Complete session history created by Claude Code CLI
   - Full message history and tool invocations
   - Token usage statistics and error details
   - Location displayed with verbose mode (`-v`)

**Primary method - Use the `prodigy logs` command:**
```bash
# View the most recent Claude log
prodigy logs --latest

# View with summary of activity
prodigy logs --latest --summary

# Follow the latest log in real-time
prodigy logs --latest --tail

# List recent logs
prodigy logs
```

**Alternative - Manual inspection:**

When using verbose mode (`-v`), Prodigy displays the location of Claude's native session logs:
```bash
prodigy run workflow.yml -v
# Output: Claude JSON log: ~/.local/state/claude/logs/session-abc123.json
```

Claude's native logs contain:
- Complete message history (user messages and Claude responses)
- All tool invocations with parameters and results
- Token usage statistics
- Error details and stack traces

**Advanced analysis with jq:**
```bash
# View complete message history
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages'

# Check tool invocations
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages[].content[] | select(.type == "tool_use")'

# Analyze token usage
cat ~/.local/state/claude/logs/session-abc123.json | jq '.usage'

# Extract error details
cat ~/.local/state/claude/logs/session-abc123.json | jq '.messages[] | select(.role == "assistant") | .content[] | select(.type == "error")'
```

**For MapReduce jobs**, check the DLQ for json_log_location:
```bash
# Get log location from DLQ
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Inspect the Claude JSON log
cat /path/from/above/session-xyz.json | jq '.messages[-3:]'
```

This is especially valuable for debugging MapReduce agent failures, as you can see exactly what Claude was doing when the agent failed.

### Check DLQ for failed items
```bash
# List failed items
prodigy dlq list <job_id>

# View failure details (inspect/show are aliases)
prodigy dlq inspect <job_id>
prodigy dlq show <job_id>

# Retry failed items (primary recovery operation)
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run

# Analyze failure patterns across items
prodigy dlq analyze <job_id>

# Export DLQ items to file for external analysis
prodigy dlq export output.json --job-id <job_id>

# Show DLQ statistics for workflow
prodigy dlq stats --workflow-id <workflow_id>

# Purge old DLQ items
prodigy dlq purge --older-than-days 30

# Clear processed items from DLQ
prodigy dlq clear <workflow_id>
```

### Monitor MapReduce progress
```bash
# View events
prodigy events <job_id>

# Check checkpoints
prodigy checkpoints list

# View event logs directly
ls ~/.prodigy/events/

# Check session state
cat .prodigy/session_state.json
```

### Inspect checkpoint files for job state
Checkpoint files contain the complete state of a MapReduce job and are crucial for debugging:

```bash
# View checkpoint to understand job state
cat ~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/checkpoint.json

# Check checkpoint version and completed items count
jq '.version, .completed_items | length' ~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/checkpoint.json

# List all pending items (items not yet processed)
jq '.pending_items' ~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/checkpoint.json

# View job progress summary
jq '{version, total: (.completed_items | length) + (.pending_items | length), completed: (.completed_items | length), pending: (.pending_items | length)}' ~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/checkpoint.json
```

**Key checkpoint fields:**
- `version` - Checkpoint format version (for migration compatibility)
- `completed_items` - Work items that have been successfully processed
- `pending_items` - Work items still waiting to be processed
- `job_id` - Unique identifier for the MapReduce job
- `timestamp` - When the checkpoint was last saved

---

## FAQ

### Q: Why are my changes not being committed?
**A:** Add `commit_required: true` to your command or use `auto_commit: true` for automatic commits when changes are detected. Note: `auto_commit` can be set at the workflow level (applies to all steps) or per-step. When true, Prodigy creates commits automatically when git diff detects changes.

### Q: How do I retry failed MapReduce items?
**A:** Use the DLQ retry command:
```bash
prodigy dlq retry <job_id>
```

### Q: Can I use environment variables in JSONPath expressions?
**A:** No, JSONPath expressions are evaluated against the input data, not the environment. Use variables in command arguments instead.

### Q: How do I skip items in MapReduce?
**A:** Use the `filter` field:
```yaml
map:
  filter: "item.score >= 5"
```

### Q: What's the difference between `on_failure` and `on_incomplete`?
**A:** `on_failure` runs when a command exits with a non-zero code. `on_incomplete` is used in goal_seek commands and runs when the validation score is below the threshold.

### Q: How do I run commands in parallel?
**A:** Use MapReduce mode with `max_parallel`:
```yaml
mode: mapreduce
map:
  max_parallel: 5
```

### Q: Can I nest workflows?
**A:** Not directly, but you can use `shell` commands to invoke other workflows:
```yaml
- shell: "prodigy run other-workflow.yml"
```

### Q: How do I clean up old worktrees?
**A:** Use the `prodigy worktree clean` command to remove completed worktrees:
```bash
# List all worktrees
prodigy worktree ls

# Clean up completed worktrees
prodigy worktree clean

# Force cleanup of all worktrees (use with caution)
prodigy worktree clean -f
```

Old worktrees can consume disk space, so periodic cleanup is recommended. The `clean` command safely removes worktrees that are no longer in use, while `-f` forces removal of all worktrees including those that may still be active.

---

## Common Error Messages

### MapReduceError Types
Prodigy uses structured errors to help diagnose issues:

**Job-level errors:**
- `JobInitializationFailed` - Job failed to initialize, check configuration and permissions
- `JobAlreadyExists` - Job ID already exists, choose a different job ID or clean up old job
- `JobNotFound` - Job ID doesn't exist, check job_id spelling or if job was cleaned up

**Agent-level errors:**
- `AgentFailed` - Individual agent execution failed, check DLQ for details
- `AgentTimeout` - Agent exceeded timeout, increase agent_timeout_secs
- `CommandExecutionFailed` - Shell or Claude command failed in agent
- `CommandFailed` - Command execution failed with non-zero exit code

**Resource errors:**
- `ResourceExhausted` - Out of disk space, memory, or other resources

**Worktree errors:**
- `WorktreeCreationFailed` - Failed to create git worktree, check disk space and git status
- `WorktreeMergeConflict` - Git merge conflict when merging agent results

**Configuration and validation errors:**
- `InvalidConfiguration` - Workflow YAML has configuration errors
- `InvalidJsonPath` - JSONPath expression syntax error, check your $.path syntax
- `ValidationFailed` - Validation check failed, review validation criteria
- `ShellSubstitutionFailed` - Variable substitution failed, check ${variable} references
- `EnvironmentError` - Environment validation failed, check required env vars

**Checkpoint errors:**
- `CheckpointCorrupted` - Checkpoint file corrupted at specific version, may need to restart job
- `CheckpointLoadFailed` - Failed to load checkpoint, check file permissions and format
- `CheckpointSaveFailed` - Failed to save checkpoint, check disk space and permissions
- `CheckpointPersistFailed` - Failed to persist checkpoint to disk, check disk space

**I/O errors:**
- `WorkItemLoadFailed` - Failed to load work items from input file, check file format and path

**Concurrency errors:**
- `DeadlockDetected` - Deadlock in job execution, reduce parallelism or check for circular dependencies
- `ConcurrentModification` - Concurrent modification of job state, retry operation

**Other errors:**
- `DlqError` - DLQ operation failed, check DLQ storage and permissions
- `ProcessingError` - General processing error, check logs for details
- `Timeout` - Operation timed out, increase timeout values
- `General` - General error for migration compatibility

**Recovery actions:**
- Check event logs: `prodigy events <job_id>`
- Review DLQ: `prodigy dlq list <job_id>`
- View detailed state: `cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/checkpoint.json`
- Check Claude JSON logs (see Debug Tips section below)

### Checkpoint and Resume Errors

**"Checkpoint not found"**
- Cause: No checkpoint file exists for this job
- Solution: Job may have completed or checkpoint was deleted, start fresh

**"Failed to resume from checkpoint"**
- Cause: Checkpoint file is corrupted or format changed
- Solution: Check checkpoint JSON syntax, may need to start over

**"Worktree conflicts during merge"**
- Cause: Git merge conflicts when combining agent results
- Solution: Resolve conflicts manually in worktree, then retry merge

### Variable and Capture Errors

**"Variable not found: ${variable_name}"**
- Cause: Variable not defined or out of scope
- Solution: Check variable is defined before use, verify scope (workflow vs item vs capture)

**"Failed to parse capture output as {format}"**
- Cause: Command output doesn't match capture_format
- Solution: Check output manually, adjust capture_format or command output

**"JSONPath expression failed"**
- Cause: Invalid JSONPath syntax or doesn't match data structure
- Solution: Test with `jq` command, simplify expression, check input data format

---

## Best Practices for Debugging

1. **Start simple**: Test commands individually before adding to workflow
2. **Use verbosity flags**: Use `-v` to see Claude interactions, `-vv` for debug logs, `-vvv` for trace
3. **Use echo liberally**: Debug variable values with echo statements
4. **Check logs and state**: Review event logs (`~/.prodigy/events/`) and session state (`.prodigy/session_state.json`)
5. **Test incrementally**: Add commands one at a time and test after each
6. **Validate input data**: Ensure JSON files and data formats are correct before MapReduce
7. **Check DLQ regularly**: Monitor failed items with `prodigy dlq list` and retry when appropriate
8. **Monitor resources**: Check disk space, memory, and CPU during execution
9. **Version control**: Commit working workflows before making changes
10. **Read error messages carefully**: MapReduceError types indicate specific failure modes
11. **Ask for help**: Include full error messages, workflow config, and verbosity output when seeking support
