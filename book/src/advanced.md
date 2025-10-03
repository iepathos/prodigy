# Advanced Features

This chapter covers advanced workflow features for building sophisticated automation pipelines. These features enable conditional execution, parallel processing, validation, and complex control flow.

---

## Conditional Execution

Control when commands execute based on expressions or previous command results.

### Expression-Based Conditions

Use the `when` field to conditionally execute commands based on variable values:

```yaml
# Execute only when variable is true
- shell: "cargo build --release"
  when: "${tests_passed}"

# Execute based on complex expression
- shell: "deploy.sh"
  when: "${environment == 'production' && tests_passed}"
```

### On Success Handlers

Execute follow-up commands when a command succeeds:

```yaml
- shell: "cargo test"
  on_success:
    shell: "cargo bench"
```

### On Failure Handlers

Handle failures with automatic remediation:

```yaml
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings"
    max_attempts: 3
    fail_workflow: false
```

The `on_failure` configuration supports:
- `max_attempts`: Maximum retry attempts (default: 1)
- `fail_workflow`: Whether to fail entire workflow on final failure (default: true)

### Nested Conditionals

Chain multiple levels of conditional execution:

```yaml
- shell: "cargo check"
  on_success:
    shell: "cargo build --release"
    on_success:
      shell: "cargo test --release"
      on_failure:
        claude: "/debug-failures '${shell.output}'"
```

---

## Output Capture and Variable Management

Capture command output in different formats for use in subsequent steps.

### Capture Variable

Capture output to a named variable using the `capture_output` field:

```yaml
# Capture as string (backward compatible)
- shell: "git rev-parse HEAD"
  capture_output: "commit_hash"

# Reference in later steps
- shell: "echo 'Commit: ${commit_hash}'"
```

### Capture Formats

Control how output is parsed with `capture_format`:

```yaml
# String (default) - trimmed output as single string
- shell: "git rev-parse HEAD"
  capture_output: "commit_hash"
  capture_format: "string"

# Number - parse output as number
- shell: "wc -l < file.txt"
  capture_output: "line_count"
  capture_format: "number"

# JSON - parse output as JSON object
- shell: "cargo metadata --format-version 1"
  capture_output: "metadata"
  capture_format: "json"

# Lines - split output into array of lines
- shell: "find . -name '*.rs'"
  capture_output: "rust_files"
  capture_format: "lines"

# Boolean - parse "true"/"false" as boolean
- shell: "test -f README.md && echo true || echo false"
  capture_output: "has_readme"
  capture_format: "boolean"
```

### Stream Capture Control

Control which streams to capture using `capture_streams`:

```yaml
# Capture only stdout (default)
- shell: "cargo build"
  capture_output: "build_log"
  capture_streams: "stdout"

# Capture only stderr
- shell: "cargo test"
  capture_output: "errors"
  capture_streams: "stderr"

# Capture both streams
- shell: "npm install"
  capture_output: "full_output"
  capture_streams: "both"
```

### Output File Redirection

Write command output directly to a file:

```yaml
# Redirect output to file
- shell: "cargo test --verbose"
  output_file: "test-results.txt"

# File is written to working directory
# Can be combined with capture_output to save and use output
```

---

## Step Identification

Assign unique IDs to steps for referencing their outputs:

```yaml
- shell: "cargo test"
  id: "test-step"
  capture_output: "test_results"

# Reference step output by ID
- shell: "echo 'Tests: ${test-step.output}'"
```

---

## Timeout Configuration

Set execution timeouts at the command level:

```yaml
# Command-level timeout (in seconds)
- shell: "cargo bench"
  timeout: 600  # 10 minutes

# Timeout for long-running operations
- claude: "/analyze-codebase"
  timeout: 1800  # 30 minutes
```

**Note**: Timeouts are only supported at the individual command level, not for MapReduce agents.

---

## Implementation Validation

Validate that implementations meet requirements using the `validate` field.

### Basic Validation

Run validation commands after a step completes:

```yaml
- claude: "/implement-feature"
  validate:
    shell: "cargo test"
    threshold: 100  # Require 100% completion
```

### Validation with Claude

Use Claude to validate implementation quality:

```yaml
- shell: "generate-code.sh"
  validate:
    claude: "/verify-implementation"
    threshold: 95
```

### Multi-Step Validation

Run multiple validation commands in sequence:

```yaml
- claude: "/refactor"
  validate:
    commands:
      - shell: "cargo test"
      - shell: "cargo clippy"
      - shell: "cargo fmt --check"
    threshold: 100
```

### Validation with Result Files

Read validation results from a file instead of stdout:

```yaml
- claude: "/implement-feature"
  validate:
    shell: "run-validator.sh"
    result_file: "validation-results.json"
    threshold: 95
```

### Handling Incomplete Implementations

Automatically remediate when validation fails:

```yaml
- claude: "/implement-spec"
  validate:
    shell: "check-completeness.sh"
    threshold: 100
    on_incomplete:
      claude: "/fill-gaps"
      max_attempts: 3
      fail_workflow: true
```

The `on_incomplete` configuration supports:
- `claude`: Claude command to execute for gap-filling
- `shell`: Shell command to execute for gap-filling
- `commands`: Array of commands to execute
- `max_attempts`: Maximum remediation attempts (default: 1)
- `fail_workflow`: Whether to fail workflow if remediation fails (default: true)
- `commit_required`: Whether remediation command should create a commit (default: false)

---

## Parallel Iteration with Foreach

Process multiple items in parallel using the `foreach` command.

### Basic Foreach

Iterate over a list of items:

```yaml
- foreach:
    foreach: ["a", "b", "c"]
    do:
      - shell: "process ${item}"
```

### Dynamic Item Lists

Generate items from a command:

```yaml
- foreach:
    foreach: "find . -name '*.rs'"
    do:
      - shell: "rustfmt ${item}"
```

### Parallel Execution

Control parallelism with the `parallel` field:

```yaml
- foreach:
    foreach: "ls *.txt"
    parallel: 5  # Process 5 items concurrently
    do:
      - shell: "analyze ${item}"
```

### Error Handling

Continue processing remaining items on failure:

```yaml
- foreach:
    foreach: ["test1", "test2", "test3"]
    continue_on_error: true
    do:
      - shell: "run-test ${item}"
```

### Limiting Items

Process only a subset of items:

```yaml
- foreach:
    foreach: "find . -name '*.log'"
    max_items: 10  # Process first 10 items only
    do:
      - shell: "compress ${item}"
```

---

## Goal-Seeking Operations

Iteratively refine implementations until they meet validation criteria.

### Basic Goal Seek

Define a goal and validation command:

```yaml
- goal_seek:
    goal: "All tests pass"
    command: "cargo fix"
    validate: "cargo test"
    threshold: 100
```

The goal-seeking operation will:
1. Run the command
2. Run the validation
3. Retry if validation threshold not met
4. Stop when goal achieved or max attempts reached

### Advanced Goal Seek Configuration

Control iteration behavior:

```yaml
- goal_seek:
    goal: "Code passes all quality checks"
    command: "auto-fix.sh"
    validate: "quality-check.sh"
    threshold: 95
    max_attempts: 5
    timeout: 300
    fail_on_incomplete: true
```

---

## Best Practices

### 1. Use Meaningful Variable Names

```yaml
# Good
- shell: "cargo test --format json"
  capture_output: "test_results"
  capture_format: "json"

# Avoid
- shell: "cargo test --format json"
  capture_output: "x"
```

### 2. Set Appropriate Timeouts

```yaml
# Set timeouts for potentially long-running operations
- shell: "npm install"
  timeout: 300

- claude: "/analyze-large-codebase"
  timeout: 1800
```

### 3. Handle Failures Gracefully

```yaml
# Provide automatic remediation
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
    max_attempts: 2
    fail_workflow: true
```

### 4. Validate Critical Changes

```yaml
# Ensure implementation meets requirements
- claude: "/implement-feature"
  validate:
    commands:
      - shell: "cargo test"
      - shell: "cargo clippy -- -D warnings"
    threshold: 100
    on_incomplete:
      claude: "/fix-issues"
      max_attempts: 3
```

### 5. Use Step IDs for Complex Workflows

```yaml
# Make output references explicit
- shell: "git diff --stat"
  id: "git-changes"
  capture_output: "diff"

- claude: "/review-changes '${git-changes.output}'"
  id: "code-review"
```

---

## Common Patterns

### Test-Fix-Verify Loop

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/fix-tests"
    on_success:
      shell: "cargo test --release"
```

### Parallel Processing with Aggregation

```yaml
- foreach:
    foreach: "find src -name '*.rs'"
    parallel: 10
    do:
      - shell: "analyze-file ${item}"
        capture_output: "analysis_${item}"

- shell: "aggregate-results.sh"
```

### Gradual Quality Improvement

```yaml
- goal_seek:
    goal: "Code quality score above 90"
    command: "auto-improve.sh"
    validate: "quality-check.sh"
    threshold: 90
    max_attempts: 5
  on_success:
    shell: "git commit -m 'Improved code quality'"
```

### Conditional Deployment

```yaml
- shell: "cargo test"
  capture_output: "test_results"
  capture_format: "json"

- shell: "deploy.sh"
  when: "${test_results.passed == test_results.total}"
  on_success:
    shell: "notify-success.sh"
  on_failure:
    shell: "rollback.sh"
```

### Multi-Stage Validation

```yaml
- claude: "/implement-feature"
  validate:
    commands:
      - shell: "cargo build"
      - shell: "cargo test"
      - shell: "cargo clippy"
      - shell: "cargo fmt --check"
    threshold: 100
    on_incomplete:
      commands:
        - claude: "/fix-build-errors"
        - shell: "cargo fmt"
      max_attempts: 3
      fail_workflow: true
```
