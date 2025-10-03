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
- Match `capture_format` to output type:
  - `string` for text output
  - `number` for numeric values
  - `json` for JSON output
  - `lines` for multi-line output as array
  - `boolean` for true/false values
- Test command output manually first
- Capture all streams for debugging:
  ```yaml
  - shell: "cargo test 2>&1"
    capture: "test_output"
    capture_streams:
      stdout: true
      stderr: true
      exit_code: true
  ```

---

### 3. Validation failing
**Symptom:** Goal-seeking or validation commands fail to recognize completion.

**Causes:**
- Validate command not outputting `score: N` format
- Threshold too high
- Score calculation incorrect

**Solutions:**
- Ensure validate command outputs exactly `score: N` (where N is 0-100)
- Test validate command independently
- Lower threshold temporarily for debugging
- Example correct format:
  ```yaml
  validate: |
    result=$(run-checks.sh | grep 'Percentage' | sed 's/.*: \([0-9]*\)%.*/\1/')
    echo "score: $result"
  ```

---

### 4. MapReduce items not found
**Symptom:** Map phase finds zero items or wrong items.

**Causes:**
- Incorrect JSONPath expression
- Input file format doesn't match expectations
- Input file not generated in setup phase

**Solutions:**
- Test JSONPath expression with actual data using `jq`:
  ```bash
  jq '$.items[*]' items.json
  ```
- Verify input file exists and contains expected structure
- Check setup phase completed successfully
- Use simpler JSONPath first: `$[*]` to get all items

---

### 5. Timeout errors
**Symptom:** Commands or workflows fail with timeout errors.

**Causes:**
- Commands take longer than expected
- Default timeout too short
- Infinite loops or hanging processes

**Solutions:**
- Increase timeout values:
  ```yaml
  - shell: "slow-command.sh"
    timeout: 600  # 10 minutes
  ```
- For MapReduce, increase agent timeout:
  ```yaml
  map:
    agent_timeout_secs: 600
  ```
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
- Use correct merge format:
  ```yaml
  merge:
    - shell: "git fetch origin"
    - claude: "/merge-worktree ${merge.source_branch}"
  ```
- Increase merge timeout:
  ```yaml
  merge:
    commands:
      - shell: "slow-merge-validation.sh"
    timeout: 600
  ```
- Check logs for merge execution errors

---

## Debug Tips

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
# Use prodigy to validate your workflow file
prodigy validate workflow.yml
```

### Check DLQ for failed items
```bash
# List failed items
prodigy dlq list <job_id>

# View failure details
prodigy dlq inspect <job_id>
```

### Monitor MapReduce progress
```bash
# View events
prodigy events <job_id>

# Check checkpoints
prodigy checkpoints list
```

---

## FAQ

### Q: Why are my changes not being committed?
**A:** Add `commit_required: true` to your command or use `auto_commit: true` for automatic commits when changes are detected.

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

### Q: What's the difference between `on_failure` and `on_exit_code`?
**A:** `on_failure` runs for any non-zero exit code. `on_exit_code` lets you handle specific exit codes differently.

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

## Best Practices for Debugging

1. **Start simple**: Test commands individually before adding to workflow
2. **Use echo liberally**: Debug variable values with echo statements
3. **Check logs**: Review Prodigy logs for detailed error messages
4. **Test incrementally**: Add commands one at a time and test after each
5. **Validate input data**: Ensure JSON files and data formats are correct
6. **Use dry runs**: Test workflows without actual execution when possible
7. **Monitor resources**: Check disk space, memory, and CPU during execution
8. **Version control**: Commit working workflows before making changes
9. **Read error messages**: Error messages often contain the solution
10. **Ask for help**: Include full error messages and workflow config when seeking support
