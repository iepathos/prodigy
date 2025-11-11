## Goal-Seeking Operations

Iteratively refine implementations until they meet validation criteria. Goal-seeking addresses the fundamental challenge that AI often fails on first attempts but succeeds with validation feedback and retry mechanisms.

### Basic Goal Seek

Define a goal and validation command using either `shell` or `claude`:

**Using Shell Command:**

```yaml
- goal_seek:
    goal: "All tests pass"
    shell: "cargo fix"
    validate: "cargo test 2>&1 | grep -q 'test result: ok' && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
```

**Using Claude Command:**

```yaml
- goal_seek:
    goal: "Code quality improved"
    claude: "/fix-issues"
    validate: "quality-check.sh"  # Must output score in parseable format
    threshold: 95
```

**Important**: Validation commands must output a score in one of these formats:
- `score: 85` (simple text)
- `85%` (percentage)
- `85/100` (ratio)
- `85 out of 100` (natural language)
- `{"score": 85, "gaps": ["list of issues"]}` (JSON with optional gaps)

**Source**: Score extraction patterns defined in src/cook/goal_seek/validator.rs:37-62

The goal-seeking operation will:
1. Run the command (shell or claude)
2. Run the validation and extract numeric score (0-100)
3. Pass validation context to next attempt via environment variables
4. Retry if validation threshold not met
5. Stop when goal achieved, max attempts reached, or convergence detected

### Advanced Goal Seek Configuration

Control iteration behavior and convergence detection:

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

**Configuration Fields** (from src/cook/goal_seek/mod.rs:13-27):
- `goal`: Description of what you're trying to achieve
- `shell` or `claude`: The command to execute (use one or the other)
- `validate`: Shell command to validate progress (must output score 0-100)
- `threshold`: Minimum score to consider goal achieved (0-100)
- `max_attempts`: Maximum number of iterations (default: 3)
- `timeout_seconds`: Optional timeout for the entire goal-seeking operation
- `fail_on_incomplete`: Fail workflow if goal not achieved (default: true)

### Automatic Convergence Detection

Goal-seeking automatically detects when no progress is being made and stops early to prevent wasted iterations.

**Convergence Criteria** (src/cook/goal_seek/engine.rs:179-197):
- Triggers after 3+ attempts
- Last 3 scores are within 2 points of each other
- Returns `Converged` result with reason

**Example**: If attempts produce scores [82, 83, 82], the system detects convergence and stops instead of continuing to max_attempts.

**Benefits**:
- Saves time and computational resources
- Prevents infinite loops when stuck
- Provides clear feedback about why iteration stopped

```yaml
# Example: Convergence will stop early if no improvement
- goal_seek:
    goal: "Improve test coverage"
    claude: "/add-tests"
    validate: "coverage-check.sh"  # Returns score: 0-100
    threshold: 90
    max_attempts: 10  # May stop earlier if converged
```

### Validation Integration

Goal-seeking integrates with the validation system through flexible score extraction.

**Validation Output Formats** (src/cook/goal_seek/validator.rs:65-96):

**1. JSON Format with Score:**
```json
{"score": 85}
```

**2. JSON Format with Score and Gaps:**
```json
{
  "score": 75,
  "gaps": [
    "Missing test for user authentication",
    "No error handling tests",
    "Coverage below 80% in auth module"
  ]
}
```

**3. Simple Text Formats:**
- `score: 85`
- `85%`
- `85/100`
- `85 out of 100`

**Example Validation Command:**
```yaml
- goal_seek:
    goal: "100% test coverage"
    claude: "/add-tests"
    validate: |
      # Extract coverage percentage and format as score
      cargo tarpaulin --print-summary 2>/dev/null | \
        grep 'Coverage' | \
        sed 's/.*Coverage=\([0-9]*\).*/score: \1/'
    threshold: 100
    timeout_seconds: 600
    max_attempts: 10
```

**Real-World Example** (from workflows/goal-seeking-examples.yml:6-14):
```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/prodigy-coverage --improve"
    validate: "cargo tarpaulin --print-summary 2>/dev/null | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
    fail_on_incomplete: true
  commit_required: true
```

**Validation Context Environment Variables** (src/cook/goal_seek/engine.rs:128-153):

After the first attempt, refinement commands receive validation feedback via environment variables:
- `PRODIGY_VALIDATION_SCORE`: Previous attempt's numeric score (0-100)
- `PRODIGY_VALIDATION_OUTPUT`: Full text output from validation command
- `PRODIGY_VALIDATION_GAPS`: Parsed JSON gaps array (if validation returned JSON with gaps field)

This allows Claude or shell scripts to understand what failed and make targeted improvements.

### Progressive Refinement

Use goal-seeking for incremental improvements through multi-stage workflows.

**Multi-Stage Example** (from workflows/goal-seeking-examples.yml:99-129):

```yaml
# Stage 1: Implement feature
- name: "Complete feature implementation"
  goal_seek:
    goal: "Implement user profile feature"
    claude: "/implement-feature user-profile"
    validate: "test -f src/features/user_profile.rs && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 2

# Stage 2: Add comprehensive tests
- name: "Add tests for user profile"
  goal_seek:
    goal: "Add tests for user profile feature"
    claude: "/add-tests src/features/user_profile.rs"
    validate: |
      test_count=$(grep -c "#\[test\]" src/features/user_profile.rs || echo 0)
      if [ "$test_count" -ge 5 ]; then
        echo "score: 100"
      else
        score=$((test_count * 20))
        echo "score: $score"
      fi
    threshold: 100
    max_attempts: 3

# Stage 3: Ensure tests pass
- name: "Make all tests pass"
  goal_seek:
    goal: "Make all user profile tests pass"
    claude: "/fix-tests user_profile"
    validate: "cargo test user_profile 2>&1 | grep -q 'test result: ok' && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 4
    fail_on_incomplete: true
```

**Progressive Quality Improvement:**

```yaml
# First pass: Get to 80% quality
- goal_seek:
    goal: "Basic quality standards met"
    shell: "quick-fix.sh"
    validate: "quality-check.sh"  # Outputs score: 0-100
    threshold: 80
    max_attempts: 3

# Second pass: Polish to 100%
- goal_seek:
    goal: "Perfect quality"
    claude: "/polish-code"
    validate: "quality-check.sh"  # Same validator, higher threshold
    threshold: 100
    max_attempts: 5
    timeout_seconds: 600
```

**Benefits of Progressive Refinement:**
- Each stage has clear, achievable goals
- Earlier stages provide foundation for later ones
- Context from previous attempts helps Claude make targeted improvements
- Convergence detection prevents wasted effort at each stage

### Error Handling

Goal-seeking operations can terminate in several ways (src/cook/goal_seek/mod.rs:44-76):

**Result Types:**
- `Success`: Threshold reached within max_attempts
- `MaxAttemptsReached`: Exhausted retries without reaching threshold
- `Timeout`: Time limit exceeded
- `Converged`: No improvement detected in last 3 attempts
- `Failed`: Command execution error

**Handling Incomplete Goals:**

```yaml
- goal_seek:
    goal: "All tests pass"
    shell: "cargo fix"
    validate: "cargo test 2>&1 | grep -q 'test result: ok' && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 3
    fail_on_incomplete: false  # Continue workflow even if goal not achieved

# Next step proceeds regardless of goal-seek result
- shell: "echo 'Continuing workflow despite incomplete goal'"
```

**Using Goal-Seeking in on_failure Handlers** (from workflows/goal-seeking-examples.yml:76-95):

```yaml
- shell: "cargo test"
  on_failure:
    goal_seek:
      goal: "Fix all failing tests"
      claude: "/debug-test-failures"
      validate: |
        cargo test 2>&1 | grep -q "test result: ok" && echo "score: 100" || {
          passed=$(cargo test 2>&1 | grep -oP '\d+(?= passed)' | head -1)
          failed=$(cargo test 2>&1 | grep -oP '\d+(?= failed)' | head -1)
          total=$((passed + failed))
          if [ "$total" -gt 0 ]; then
            score=$((passed * 100 / total))
            echo "score: $score"
          else
            echo "score: 0"
          fi
        }
      threshold: 100
      max_attempts: 3
      fail_on_incomplete: true
```

**Note**: Goal-seeking operations return `GoalSeekResult` variants, not shell exit codes. The workflow executor converts these to step results based on `fail_on_incomplete` configuration (src/cook/workflow/executor/commands.rs).

# Good: Explicit score output
validate: "cargo test 2>&1 | grep -q 'ok' && echo 'score: 100' || echo 'score: 0'"

# Better: Proportional score based on actual results
validate: |
  passed=$(cargo test 2>&1 | grep -oP '\d+(?= passed)')
  total=$(cargo test 2>&1 | grep -oP '\d+ tests')
  score=$((passed * 100 / total))
  echo "score: $score"

# Best: JSON with actionable feedback
validate: |
  result=$(cargo test --format json)
  passed=$(echo "$result" | jq '.passed')
  total=$(echo "$result" | jq '.total')
  score=$((passed * 100 / total))
  gaps=$(echo "$result" | jq '.failures')
  echo "{\"score\": $score, \"gaps\": $gaps}"
```

**Limit Iterations:**
- Set reasonable max_attempts (3-10)
- Use timeout_seconds for long-running operations (per-operation timeout)
- Consider fail_on_incomplete based on criticality
- Trust convergence detection to prevent wasted attempts

**Leverage Validation Context:**
- Claude commands can access previous scores via `PRODIGY_VALIDATION_SCORE`
- Use gaps information (`PRODIGY_VALIDATION_GAPS`) for targeted fixes
- Design validation to provide actionable feedback, not just scores

**Troubleshooting Common Issues:**

| Issue | Cause | Solution |
|-------|-------|----------|
| Validation always returns 0 | Score not in parseable format | Test validation command, ensure it outputs `score: N` format |
| Convergence happens too early | Score variance within 2 points | Ensure validation is precise enough to detect small improvements |
| Timeout vs MaxAttemptsReached | timeout_seconds too short | Increase timeout or reduce max_attempts |
| Context not available to Claude | Environment variables not passed | Verify command executor supports env vars (src/cook/goal_seek/engine.rs:128-153) |

**See Also:**
- [Implementation Validation](implementation-validation.md) - Using goal-seeking for spec validation
- [Error Handling](../workflow-basics/error-handling.md) - Workflow-level error handling
- Technical documentation: docs/goal-seeking.md in repository
