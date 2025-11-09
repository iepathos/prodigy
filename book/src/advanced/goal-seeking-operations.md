## Goal-Seeking Operations

Iteratively refine implementations until they meet validation criteria.

### Basic Goal Seek

Define a goal and validation command using either `shell` or `claude`:

**Using Shell Command:**

```yaml
- goal_seek:
    goal: "All tests pass"
    shell: "cargo fix"
    validate: "cargo test"
    threshold: 100
```

**Using Claude Command:**

```yaml
- goal_seek:
    goal: "Code quality improved"
    claude: "/fix-issues"
    validate: "quality-check.sh"
    threshold: 95
```

The goal-seeking operation will:
1. Run the command (shell or claude)
2. Run the validation
3. Retry if validation threshold not met
4. Stop when goal achieved or max attempts reached

### Advanced Goal Seek Configuration

Control iteration behavior:

```yaml
- goal_seek:
    goal: "Code passes all quality checks"
    shell: "auto-fix.sh"
    validate: "quality-check.sh"
    threshold: 95
    max_attempts: 5
    timeout_seconds: 300
    fail_on_incomplete: true
```

**Configuration Fields:**
- `goal`: Description of what you're trying to achieve
- `shell` or `claude`: The command to execute (use one or the other)
- `validate`: Shell command to validate progress
- `threshold`: Minimum completion percentage to consider goal achieved (0-100)
- `max_attempts`: Maximum number of iterations (default: 3)
- `timeout_seconds`: Timeout for each iteration in seconds
- `fail_on_incomplete`: Fail workflow if goal not achieved (default: true)

### Validation Integration

Goal-seeking integrates with the validation system:

```yaml
- goal_seek:
    goal: "100% test coverage"
    claude: "/add-tests"
    validate: "coverage-check.sh"
    threshold: 100
    timeout_seconds: 600
    max_attempts: 10
```

The validation command should output JSON with a `completion_percentage` field, or Prodigy will parse the output to determine progress.

### Progressive Refinement

Use goal-seeking for incremental improvements:

```yaml
# First pass: Get to 80% quality
- goal_seek:
    goal: "Basic quality standards met"
    shell: "quick-fix.sh"
    validate: "quality-check.sh"
    threshold: 80
    max_attempts: 3

# Second pass: Polish to 100%
- goal_seek:
    goal: "Perfect quality"
    claude: "/polish-code"
    validate: "quality-check.sh"
    threshold: 100
    max_attempts: 5
    timeout_seconds: 600
```

### Error Handling

Handle goal-seeking failures:

```yaml
- goal_seek:
    goal: "All tests pass"
    shell: "cargo fix"
    validate: "cargo test"
    threshold: 100
    max_attempts: 3
    fail_on_incomplete: false  # Don't fail workflow, continue to next step

# Handle incomplete goal
- shell: "echo 'Some tests still failing, proceeding anyway'"
  when: "${shell.exit_code != 0}"
```

### Best Practices

**Set Realistic Thresholds:**
- Start with achievable thresholds (80-90%)
- Increase gradually in multiple goal-seeking steps
- Consider diminishing returns on higher thresholds

**Provide Good Validation:**
- Validation should be fast and deterministic
- Clear completion percentage in output
- Specific feedback on what's missing

**Limit Iterations:**
- Set reasonable max_attempts (3-10)
- Use timeouts to prevent infinite loops
- Consider fail_on_incomplete based on criticality
