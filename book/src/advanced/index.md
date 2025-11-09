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
- `>` - Greater than (e.g., `${score > 80}`)
- `<` - Less than (e.g., `${errors < 5}`)
- `>=` - Greater than or equal to (e.g., `${coverage >= 90}`)
- `<=` - Less than or equal to (e.g., `${warnings <= 10}`)
- `contains` - String matching (e.g., `${output contains 'success'}`)

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

**Complex On Success Example:**

```yaml
- shell: "cargo build --release"
  on_success:
    claude: "/verify-build-artifacts"
    validate:
      shell: "check-binary-size.sh"
      threshold: 100
    on_failure:
      claude: "/optimize-binary-size"
      max_attempts: 2
```

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

**Multi-Step Remediation:**

For complex error recovery, use the `commands` array to execute multiple remediation steps in sequence:

```yaml
- shell: "cargo test"
  on_failure:
    commands:
      - claude: "/analyze-test-failures"
      - shell: "cargo fmt"
      - claude: "/verify-fixes"
    max_attempts: 3
    fail_workflow: true
```

### Exit Code Handlers

Map specific exit codes to different actions using `on_exit_code`:

```yaml
- shell: "cargo test"
  on_exit_code:
    1: {claude: "/fix-test-failures"}
    2: {shell: "retry-flaky-tests.sh"}
    101: {claude: "/fix-compilation-errors"}
    255: {fail_workflow: true}
```

This allows fine-grained control over error handling based on the specific exit code returned by a command.

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

The `capture_streams` field supports two formats for flexible output capture.

**Simple String Format** - For basic stream selection:

```yaml
# Capture only stdout (default)
- shell: "cargo build"
  capture_output: "build_log"
  capture_streams: "stdout"

# Capture only stderr
- shell: "cargo test"
  capture_output: "error_log"
  capture_streams: "stderr"

# Capture both streams merged
- shell: "npm install"
  capture_output: "install_log"
  capture_streams: "both"
```

**Structured Object Format** - For advanced control with exit code and success status:

```yaml
# Structured format with all fields
- shell: "cargo test"
  capture_output: "test_result"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: true
    success: true

# Access individual fields
- shell: "echo 'Test exit code: ${test_result.exit_code}'"
- shell: "echo 'Test passed: ${test_result.success}'"
```

**Format Flexibility:**

Prodigy uses Rust's untagged enum deserialization, allowing you to choose either format based on your needs without any special syntax. Both formats are equally valid and you can mix them in the same workflow.

### Output File Redirection

Write output directly to a file instead of capturing it:

```yaml
# Redirect stdout to file
- shell: "cargo doc --no-deps"
  output_file: "docs/build.log"

# Combine with capture for dual output
- shell: "cargo test"
  capture_output: "test_status"
  output_file: "test-results.log"
```

### Step-Level Environment Overrides

Configure environment variables and working directory for individual steps:

```yaml
# Full step-level environment configuration
- shell: "npm test"
  env:
    NODE_ENV: "test"
    DEBUG: "*"
  working_dir: "./frontend"
  clear_env: false    # Don't clear parent environment
  inherit: true       # Inherit from workflow-level env

# Working directory with alias
- shell: "cargo build"
  cwd: "./backend"    # 'cwd' is an alias for 'working_dir'
```

**Environment Fields:**
- `env`: Map of environment variables to set for this step
- `working_dir` / `cwd`: Directory to execute command in
- `clear_env`: Clear parent environment before adding step env (default: false)
- `inherit`: Inherit workflow-level environment variables (default: true)

---

## Additional Topics

See also:
- [Step Identification](step-identification.md)
- [Timeout Configuration](timeout-configuration.md)
- [Implementation Validation](implementation-validation.md)
- [Parallel Iteration with Foreach](parallel-iteration-with-foreach.md)
- [Goal-Seeking Operations](goal-seeking-operations.md)
- [Best Practices](best-practices.md)
- [Common Patterns](common-patterns.md)
