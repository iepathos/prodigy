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

The `on_success` field accepts a complete workflow step command with all its features:

```yaml
- shell: "cargo build --release"
  on_success:
    shell: "check-binary-size.sh"
    validate:
      threshold: 100
    on_failure:
      claude: "/optimize-binary-size"
      max_attempts: 2
```

**Source**: Based on WorkflowStepCommand structure (src/config/command.rs:376)

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

**Source**: TestDebugConfig struct definition (src/config/command.rs:168-183)

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

**Note**: For multi-step error recovery, nest individual `on_failure` handlers at each step rather than using a commands array. The `TestDebugConfig` supports only a single `claude` command per handler.

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

### Command-Agnostic Capture

The `last.*` variables capture output from any command type without needing explicit `capture_output`:

```yaml
# Shell command output
- shell: "cargo test"
  # Output automatically available as ${last.output} and ${last.exit_code}

# Use in next command (any type)
- claude: "/analyze ${last.output}"

# Or reference in conditional
- shell: "notify-failure.sh"
  when: "${last.exit_code != 0}"
```

**Available Variables:**
- `${last.output}` - Output from the last command of any type (shell, claude, etc.)
- `${last.exit_code}` - Exit code from the last command

These variables work across all command types, making them ideal for generic workflows where you don't want to hard-code command-specific variables like `${shell.output}` or `${claude.output}`.

**Source**: Variable constants defined in src/cook/workflow/variables.rs:35-36

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

> **Error Handling:** If parsing fails (e.g., non-numeric output with `capture_format: number`), the command will fail with a descriptive error. Use `capture_format: string` (default) when output format is unreliable.

**Source**: CaptureFormat enum (src/cook/workflow/variables.rs:260-265)

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

**Structured Object Format** - For advanced control with exit code, success status, and duration:

```yaml
# Structured format with all fields
- shell: "cargo test"
  capture_output: "test_result"
  capture_streams:
    stdout: true      # Capture stdout stream (default: true)
    stderr: true      # Capture stderr stream (default: false)
    exit_code: true   # Capture exit code (default: true)
    success: true     # Capture success status (default: true)
    duration: true    # Capture execution duration in seconds (default: true)

# Access individual fields
- shell: "echo 'Test exit code: ${test_result.exit_code}'"
- shell: "echo 'Test passed: ${test_result.success}'"
- shell: "echo 'Duration: ${test_result.duration}s'"
```

**Source**: CaptureStreams struct definition (src/cook/workflow/variables.rs:268-292)

**Format Flexibility:**

The `capture_streams` field accepts two formats:

1. **Simple string** (`"stdout"`, `"stderr"`, `"both"`) - Stored as `Option<String>` in YAML config, best for basic stream selection
2. **Structured object** - Parsed into `CaptureStreams` struct during execution, enables fine-grained control over `exit_code`, `success`, and `duration` capture

Use simple format for basic cases, structured format when you need detailed execution metadata.

**Source**: WorkflowStepCommand.capture_streams field (src/config/command.rs:396), CaptureStreams struct (src/cook/workflow/variables.rs:268-292)

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

### Execution Context

Configure where and how commands execute in your workflow.

**Note**: Step-level environment variable configuration (`env`, `clear_env`, `inherit`) and working directory (`working_dir`, `cwd`) are internal features available in the execution layer but not currently exposed in the YAML configuration layer (WorkflowStepCommand). These features exist in the runtime `WorkflowStep` type for internal use.

For workflow-level environment configuration, see the [Environment Variables](../workflow-basics/environment-configuration.md) section in Workflow Basics.

---

## Additional Topics

See also:
- [Step Identification](step-identification.md)
- [Timeout Configuration](timeout-configuration.md)
- [Implementation Validation](implementation-validation.md)
- [Parallel Iteration with Foreach](parallel-iteration-with-foreach.md)
- [Goal-Seeking Operations](goal-seeking-operations.md)
