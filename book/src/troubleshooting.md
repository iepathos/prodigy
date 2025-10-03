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
      stdout: true      # Capture standard output
      stderr: true      # Capture error output
      exit_code: true   # Capture exit code
      success: true     # Capture success boolean
      duration: true    # Capture execution duration
  ```

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
- Test JSONPath expression with actual data using `jq`:
  ```bash
  jq '$.items[*]' items.json
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
    timeout: "600s"  # Duration string format
  ```
- Use the full format when you need to set a custom timeout
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
- Valid BackoffStrategy values:
  - `constant` - Same delay every time
  - `linear` - Increases linearly
  - `exponential` - Doubles each time
  - `fibonacci` - Fibonacci sequence
- Circuit breaker requires both timeout and failure_threshold

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
# Manually test your JSONPath
jq '$.items[*]' items.json

# Test with filter
jq '$.items[] | select(.score >= 5)' items.json
```

### Validate workflow syntax
```bash
# Workflows are validated automatically when loaded
# Check for syntax errors by attempting to run
prodigy run workflow.yml

# View the validation result file (if workflow validation completed)
cat .prodigy/validation-result.json
```

### Check DLQ for failed items
```bash
# List failed items
prodigy dlq list <job_id>

# View failure details
prodigy dlq inspect <job_id>

# Retry failed items (primary recovery operation)
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 10

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run
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

---

## Common Error Messages

### MapReduceError Types
Prodigy uses structured errors to help diagnose issues:

**Job-level errors:**
- `JobNotFound` - Job ID doesn't exist, check job_id spelling or if job was cleaned up
- `InvalidJobConfiguration` - Workflow YAML has configuration errors
- `WorktreeSetupFailed` - Failed to create git worktree, check disk space and git status

**Agent-level errors:**
- `AgentFailed` - Individual agent execution failed, check DLQ for details
- `AgentTimeout` - Agent exceeded timeout, increase agent_timeout_secs
- `CommandExecutionFailed` - Shell or Claude command failed in agent

**Resource errors:**
- `WorktreeMergeConflict` - Git merge conflict when merging agent results
- `ResourceExhausted` - Out of disk space, memory, or other resources
- `StorageError` - Failed to read/write to storage, check permissions

**Recovery actions:**
- Check event logs: `prodigy events <job_id>`
- Review DLQ: `prodigy dlq list <job_id>`
- View detailed state: `cat ~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/checkpoint.json`

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
