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

#### Expression Syntax for When Clauses

The `when` clause supports a flexible expression syntax for conditional logic:

**Variable Interpolation:**
- Use `${variable}` to reference captured outputs or environment variables
- Variables are evaluated in the context of previous command results
- Boolean variables are evaluated as truthy/falsy values

**Comparison Operators:**
- `==` - Equality comparison (e.g., `${status == 'success'}`)
- `!=` - Inequality comparison (e.g., `${exit_code != 0}`)

**Logical Operators:**
- `&&` - Logical AND (e.g., `${tests_passed && build_succeeded}`)
- `||` - Logical OR (e.g., `${is_dev || is_staging}`)

**Type Coercion:**
- String values: Non-empty strings are truthy, empty strings are falsy
- Numeric values: Non-zero numbers are truthy, zero is falsy
- Boolean values: `true` is truthy, `false` is falsy

**Complex Expressions:**
```yaml
# Multiple conditions with logical operators
- shell: "deploy.sh"
  when: "${environment == 'production' && tests_passed && coverage >= 80}"

# Nested logic with parentheses
- shell: "run-checks.sh"
  when: "${(is_pr || is_main) && tests_passed}"

# Comparing captured outputs
- shell: "notify-team.sh"
  when: "${test-step.exit_code == 0 && build-step.success}"
```

### On Success Handlers

Execute follow-up commands when a command succeeds:

```yaml
- shell: "cargo test"
  on_success:
    shell: "cargo bench"
```

**Note**: The `on_success` field supports any workflow step command with all its features, including nested conditionals, output capture, validation, and error handlers. You can create complex success workflows by combining multiple handlers or using `when` clauses for sophisticated control flow.

### On Failure Handlers

Handle failures with automatic remediation:

```yaml
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings"
    max_attempts: 3
    fail_workflow: false
    commit_required: true
```

The `on_failure` configuration supports:
- `max_attempts`: Maximum retry attempts (default: 3)
- `fail_workflow`: Whether to fail entire workflow on final failure (default: false)
- `commit_required`: Whether the remediation command should create a git commit (default: true)

**Note**: These defaults come from the `TestDebugConfig` which provides sensible defaults for error recovery workflows.

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

The `capture_streams` field supports two formats:

**Simple String Format** - For basic stream selection:

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

Use the string format when you only need to capture output from specific streams.

**Advanced Struct Format** - For fine-grained control with metadata:

```yaml
- shell: "cargo test"
  capture_output: "test_results"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: true
    success: true
    duration: true
```

Use the struct format when you need additional metadata alongside the captured output:
- `exit_code`: Capture the command's exit code
- `success`: Capture whether the command succeeded (true/false)
- `duration`: Capture how long the command took to execute

This is particularly useful for validation workflows where you need to make decisions based on command success or timing information.

**Format Flexibility**: The `capture_streams` field is flexible—you can choose either format based on your needs. Use the simple string format (`"stdout"`, `"stderr"`, `"both"`) when you only need basic stream selection, or the struct format when you need metadata like `exit_code`, `success`, and `duration` alongside the output. Both formats are supported via Rust's untagged enum deserialization.

### Output File Redirection

Write command output directly to a file:

```yaml
# Redirect output to file
- shell: "cargo test --verbose"
  output_file: "test-results.txt"

# File is written to working directory
# Can be combined with capture_output to save and use output
```

### Working Directory Control

Control the working directory for individual commands using the `working_dir` field. This allows you to execute commands in different directories without changing your workflow's base directory:

```yaml
# Run frontend build in frontend directory
- shell: "npm install"
  working_dir: "./frontend"

# Run backend build in backend directory
- shell: "cargo build"
  working_dir: "./backend"

# Run tests in a subdirectory
- shell: "pytest"
  working_dir: "./tests"
```

The `working_dir` path is relative to the workflow's root directory. This is particularly useful for:
- **Monorepo workflows**: Building different packages in their respective directories
- **Multi-language projects**: Running language-specific tools in appropriate directories
- **Isolated testing**: Running tests in dedicated test directories
- **Subproject operations**: Working with nested projects without changing global context

**Note**: The `working_dir` field is also available with the alias `cwd` for compatibility with other tools.

---

## Step Identification

Assign unique IDs to steps for explicit output referencing. This is particularly useful in complex workflows where multiple steps produce outputs and you need to reference specific results.

### Basic Step IDs

```yaml
- shell: "cargo test"
  id: "test-step"
  capture_output: "test_results"

# Reference step output by ID
- shell: "echo 'Tests: ${test-step.output}'"
```

### When to Use Step IDs

**1. Complex Workflows with Multiple Parallel Paths**

When you have multiple steps producing similar outputs, IDs make references unambiguous:

```yaml
- shell: "cargo test --lib"
  id: "unit-tests"
  capture_output: "results"

- shell: "cargo test --test integration"
  id: "integration-tests"
  capture_output: "results"

# Clear reference to specific test results
- claude: "/analyze-failures '${unit-tests.output}'"
  when: "${unit-tests.exit_code} != 0"

- claude: "/analyze-failures '${integration-tests.output}'"
  when: "${integration-tests.exit_code} != 0"
```

**2. Debugging Specific Steps**

Step IDs help identify which step produced problematic output:

```yaml
- shell: "npm run build"
  id: "build"
  capture_output: "build_log"

- shell: "npm run lint"
  id: "lint"
  capture_output: "lint_log"

- shell: "npm test"
  id: "test"
  capture_output: "test_log"

# Reference specific logs for debugging
- claude: "/debug-build-failure '${build.output}'"
  when: "${build.exit_code} != 0"
```

**3. Conditional Execution Based on Specific Step Outputs**

Use step IDs to create complex conditional logic:

```yaml
- shell: "cargo clippy"
  id: "clippy-check"
  capture_output: "warnings"
  capture_format: "lines"

- shell: "cargo fmt --check"
  id: "format-check"
  capture_output: "format_issues"

# Only proceed if both checks pass
- shell: "cargo build --release"
  when: "${clippy-check.exit_code} == 0 && ${format-check.exit_code} == 0"

# Fix clippy warnings if present
- claude: "/fix-clippy-warnings '${clippy-check.output}'"
  when: "${clippy-check.exit_code} != 0"
  on_failure:
    claude: "/analyze-clippy-fix-failures"
```

**4. Combining with Validation and Error Handlers**

Step IDs enable sophisticated error handling patterns:

```yaml
- shell: "cargo test --format json"
  id: "test-run"
  capture_output: "test_results"
  capture_format: "json"
  validate:
    shell: "check-coverage.sh"
    threshold: 80
    on_incomplete:
      claude: "/improve-coverage '${test-run.output}'"
      max_attempts: 3
  on_failure:
    claude: "/debug-test-failures '${test-run.output}'"
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

**Convenience Syntax**: For simple cases, you can use an array format directly:

```yaml
- claude: "/refactor"
  validate:
    - shell: "cargo test"
    - shell: "cargo clippy"
    - shell: "cargo fmt --check"
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

**Convenience Syntax**: For simple cases, you can use an array format directly:

```yaml
- claude: "/implement-spec"
  validate:
    shell: "check-completeness.sh"
    threshold: 100
    on_incomplete:
      - claude: "/fill-gaps"
      - shell: "cargo fmt"
```

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
