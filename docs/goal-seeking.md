# Goal-Seeking Primitives

Goal-seeking provides iterative refinement capabilities for AI-assisted tasks, addressing the fundamental challenge that LLMs often fail on first attempts but succeed with validation and retry mechanisms.

## Overview

The goal-seeking system enables:
- **Iterative refinement** - Multiple attempts with validation feedback
- **Convergence detection** - Automatic stopping when no improvement
- **Flexible validation** - JSON or text-based score extraction
- **Context passing** - Previous results inform next attempts
- **Configurable thresholds** - Define success criteria

## CLI Usage

### Basic Command

```bash
prodigy goal-seek "Your goal description" \
  -c "command to execute" \
  --validate "validation command" \
  -t 80  # threshold score
```

### Full Options

```bash
prodigy goal-seek [OPTIONS] --command <COMMAND> --validate <VALIDATE> <GOAL>

Arguments:
  <GOAL>  What you want to achieve

Options:
  -c, --command <COMMAND>            Command to execute (gets validation context)
  --validate <VALIDATE>              Command to validate results (should output score: 0-100)
  -t, --threshold <THRESHOLD>        Minimum score to consider success [default: 80]
  -m, --max-attempts <MAX_ATTEMPTS>  Maximum attempts before giving up [default: 5]
  --timeout <TIMEOUT>                Overall timeout in seconds
  --fail-on-incomplete               Exit with error if goal not achieved
  -p, --path <PATH>                  Working directory for commands
```

### Examples

#### Simple File Creation
```bash
prodigy goal-seek "Create a Python hello world" \
  -c "echo 'print(\"Hello, World!\")' > hello.py" \
  --validate "python hello.py > /dev/null 2>&1 && echo 'score: 100' || echo 'score: 0'" \
  -t 100
```

#### Test Coverage Improvement
```bash
prodigy goal-seek "Achieve 90% test coverage" \
  -c "claude /improve-coverage" \
  --validate "pytest --cov 2>/dev/null | grep 'TOTAL' | awk '{print \"score:\", $4}' | sed 's/%//'" \
  -t 90 \
  -m 5
```

#### Code Quality Check
```bash
prodigy goal-seek "Fix all linting issues" \
  -c "claude /fix-linting" \
  --validate "ruff check . 2>&1 | grep -c 'error' | awk '{if ($1 == 0) print \"score: 100\"; else print \"score:\", 100-$1*10}'" \
  -t 100 \
  --fail-on-incomplete
```

## YAML Workflow Integration

### Basic Structure

```yaml
- goal_seek:
    goal: "Description of what to achieve"
    command: "Command to execute for each attempt"
    validate: "Command that outputs score: 0-100"
    threshold: 80           # Minimum score for success
    max_attempts: 5         # Maximum iterations
    timeout_seconds: 300    # Optional timeout
    fail_on_incomplete: false  # Whether to fail workflow if goal not met
```

### Validation Output Format

The validation command must output a score in one of these formats:

1. **Simple text**: `score: 85`
2. **Percentage**: `85%`
3. **Ratio**: `85/100`
4. **JSON**: `{"score": 85, "gaps": ["missing tests"]}`

### Context Variables

On attempts after the first, these environment variables are available:
- `PRODIGY_VALIDATION_SCORE` - Previous attempt's score
- `PRODIGY_VALIDATION_OUTPUT` - Full validation output
- `PRODIGY_VALIDATION_GAPS` - JSON gaps if provided

### Complete Example

```yaml
# Iteratively improve code until all tests pass
- goal_seek:
    goal: "Fix all failing tests"
    command: "claude: /debug-test-failures"
    validate: |
      if cargo test 2>&1 | grep -q "test result: ok"; then
        echo "score: 100"
      else
        passed=$(cargo test 2>&1 | grep -oP '\d+(?= passed)')
        failed=$(cargo test 2>&1 | grep -oP '\d+(?= failed)')
        total=$((passed + failed))
        score=$((passed * 100 / total))
        echo "score: $score"
      fi
    threshold: 100
    max_attempts: 4
    fail_on_incomplete: true
  commit_required: true
```

## Advanced Features

### Convergence Detection

The system automatically detects convergence when:
- Last 3 attempts show scores within 2 points of each other
- No improvement trend is detected
- Prevents wasted attempts when stuck

### Multi-Stage Goal Seeking

Chain multiple goal-seeking operations:

```yaml
# Stage 1: Implement feature
- goal_seek:
    goal: "Implement authentication"
    command: "claude: /implement-auth"
    validate: "test -f src/auth.rs && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 2

# Stage 2: Add tests
- goal_seek:
    goal: "Add auth tests"
    command: "claude: /add-tests auth"
    validate: "grep -c '#\[test\]' src/auth.rs | awk '{print \"score:\", $1 * 10}'"
    threshold: 80
    max_attempts: 3

# Stage 3: Ensure tests pass
- goal_seek:
    goal: "Make auth tests pass"
    command: "claude: /fix-tests auth"
    validate: "cargo test auth 2>&1 | grep -q 'ok' && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 5
```

### Integration with Error Handling

Use goal-seeking in `on_failure` handlers:

```yaml
- shell: "cargo build"
  on_failure:
    goal_seek:
      goal: "Fix compilation errors"
      command: "claude: /fix-build-errors"
      validate: "cargo build 2>&1 | grep -q 'Finished' && echo 'score: 100' || echo 'score: 0'"
      threshold: 100
      max_attempts: 3
```

## Built-in Validators

The system includes validators for common scenarios:

### SpecCoverageValidator
Validates implementation against specifications:
- Analyzes codebase for implemented features
- Compares against spec requirements
- Returns coverage percentage

### TestPassValidator
Validates test execution results:
- Parses test output
- Calculates pass rate
- Identifies failing tests

### OutputQualityValidator
Validates code quality metrics:
- Complexity analysis
- Documentation coverage
- Linting violations
- Performance metrics

## Best Practices

1. **Clear Goals**: Write specific, measurable goal descriptions
2. **Reliable Validation**: Ensure validation commands are deterministic
3. **Appropriate Thresholds**: Set realistic success criteria
4. **Timeout Protection**: Use timeouts for long-running operations
5. **Context Usage**: Leverage validation context in refinement commands
6. **Progressive Improvement**: Start with lower thresholds and increase gradually

## Troubleshooting

### Common Issues

**Validation Always Returns 0**
- Check validation command syntax
- Ensure proper output format (e.g., `score: 85`)
- Test validation command separately

**Convergence Too Early**
- Increase score variance threshold
- Add randomization to command
- Check for deterministic failures

**Context Not Available**
- Verify environment variable names
- Check command executor supports env vars
- Ensure context passing is enabled

### Debug Mode

Use verbose flags to debug goal-seeking:
```bash
prodigy goal-seek "Goal" -c "cmd" --validate "val" -vv
```

This shows:
- Command execution details
- Validation output parsing
- Score extraction process
- Context variable values

## Implementation Details

### Architecture

```
GoalSeekEngine
├── CommandExecutor (pluggable)
│   ├── ShellCommandExecutor
│   └── MockCommandExecutor (testing)
├── Validator
│   ├── ScoreExtractor
│   └── Built-in Validators
└── AttemptHistory
    └── Convergence Detection
```

### Result Types

Goal-seeking can result in:
- **Success**: Threshold reached
- **MaxAttemptsReached**: Exhausted retries
- **Timeout**: Time limit exceeded
- **Converged**: No improvement detected
- **Failed**: Execution error

### Performance Considerations

- Commands are executed sequentially
- Validation runs after each attempt
- Context grows with each iteration
- Memory usage scales with attempt history

## Future Enhancements

Planned improvements include:
- Parallel attempt exploration
- Machine learning-based convergence prediction
- Validation result caching
- Interactive refinement mode
- Custom validator plugins
- Attempt strategy configuration