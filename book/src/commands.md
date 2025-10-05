# Command Types

## 1. Shell Commands

```yaml
# Simple shell command
- shell: "cargo test"

# With output capture
- shell: "ls -la | wc -l"
  capture: "file_count"

# With failure handling
- shell: "cargo clippy"
  on_failure:
    claude: "/fix-warnings ${shell.output}"

# With timeout
- shell: "cargo bench"
  timeout: 600  # seconds

# With conditional execution
- shell: "cargo build --release"
  when: "${tests_passed}"
```

## 2. Claude Commands

```yaml
# Simple Claude command
- claude: "/prodigy-analyze"

# With arguments
- claude: "/prodigy-implement-spec ${spec_file}"

# With commit requirement
- claude: "/prodigy-fix-bugs"
  commit_required: true

# With output capture
- claude: "/prodigy-generate-plan"
  capture: "implementation_plan"
```

## 3. Goal-Seeking Commands

Iteratively refine code until a validation threshold is met.

```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/prodigy-coverage --improve"
    validate: "cargo tarpaulin --print-summary | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
    fail_on_incomplete: true
  commit_required: true
```

**Fields:**
- `goal`: Human-readable description
- `claude` or `shell`: Command to execute for refinement
- `validate`: Command that outputs `score: N` (0-100)
- `threshold`: Minimum score to consider complete
- `max_attempts`: Maximum refinement iterations
- `timeout_seconds`: Optional timeout per attempt
- `fail_on_incomplete`: Whether to fail workflow if threshold not met

## 4. Foreach Commands

Iterate over a list with optional parallelism.

```yaml
- foreach:
    input: "find . -name '*.rs' -type f"  # Command
    # OR
    # input: ["file1.rs", "file2.rs"]    # List

    parallel: 5  # Number of parallel executions (or true/false)

    do:
      - claude: "/analyze-file ${item}"
      - shell: "cargo check ${item}"

    continue_on_error: true
    max_items: 50
```

## 5. Validation Commands

Validate implementation completeness with automatic retry.

```yaml
- claude: "/implement-auth-spec"
  validate:
    shell: "debtmap validate --spec auth.md --output result.json"
    # DEPRECATED: 'command' field (use 'shell' instead)
    result_file: "result.json"
    threshold: 95  # Percentage completion required (default: 100.0)
    timeout: 60
    expected_schema: "validation-schema.json"  # Optional JSON schema

    # What to do if incomplete
    on_incomplete:
      claude: "/complete-implementation ${validation.gaps}"
      max_attempts: 3
      fail_workflow: true
      commit_required: true
      prompt: "Implementation incomplete. Continue?"  # Optional interactive prompt
```

**ValidationConfig Fields:**
- `shell` or `claude` - Single validation command (use `shell`, not deprecated `command`)
- `commands` - Array of commands for multi-step validation
- `result_file` - Path to JSON file with validation results
- `threshold` - Minimum completion percentage (default: 100.0)
- `timeout` - Timeout in seconds
- `expected_schema` - JSON schema for validation output structure

**OnIncompleteConfig Fields:**
- `shell` or `claude` - Single gap-filling command
- `commands` - Array of commands for multi-step gap filling
- `max_attempts` - Maximum retry attempts
- `fail_workflow` - Whether to fail workflow if validation incomplete
- `commit_required` - Whether to require commit after gap filling
- `prompt` - Optional interactive prompt for user guidance
- `retry_original` - Whether to retry the original command (default: false). When true, re-executes the original command instead of gap-filling commands
- `strategy` - Retry strategy configuration (similar to OnFailureConfig strategy)

**Alternative: Array format for multi-step validation**

```yaml
- claude: "/implement-feature"
  validate:
    # When using array format, ValidationConfig uses default threshold (100.0)
    # and creates a commands array
    - shell: "run-tests.sh"
    - shell: "check-coverage.sh"
    - claude: "/validate-implementation --output validation.json"
      result_file: "validation.json"
```

**Alternative: Multi-step gap filling**

```yaml
- claude: "/implement-feature"
  validate:
    shell: "validate.sh"
    result_file: "result.json"
    on_incomplete:
      commands:
        - claude: "/analyze-gaps ${validation.gaps}"
        - shell: "run-fix-script.sh"
        - claude: "/verify-fixes"
      max_attempts: 2
```

**Alternative: Retry original command on incomplete**

```yaml
- claude: "/implement-auth"
  validate:
    shell: "validate-auth.sh"
    result_file: "result.json"
    threshold: 95
    on_incomplete:
      retry_original: true  # Re-run "/implement-auth" instead of gap-filling
      max_attempts: 3
      fail_workflow: true
```

---

## Command Reference

### Command Fields

All command types support these common fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier for referencing outputs |
| `timeout` | number | Command timeout in seconds |
| `commit_required` | boolean | Whether command should create a git commit |
| `when` | string | Conditional execution expression |
| `capture` | string | Variable name to capture output (replaces deprecated `capture_output: true/false`) |
| `capture_format` | enum | Format: `string` (default), `number`, `json`, `lines`, `boolean` (see examples below) |
| `capture_streams` | object | Configure which streams to capture (see CaptureStreams section below) |
| `on_success` | object | Command to run on success |
| `on_failure` | object | OnFailureConfig with nested command, max_attempts, fail_workflow, strategy |
| `on_exit_code` | map | Maps exit codes to full WorkflowStep objects (e.g., `101: {claude: "/fix"}`) |
| `validate` | object | Validation configuration |
| `output_file` | string | Redirect command output to a file |

**Note:** The following fields exist in internal structs but are NOT exposed in WorkflowStepCommand YAML:
- `handler` - Internal HandlerStep (not user-facing)
- `retry` - Internal RetryConfig (not user-facing)
- `working_dir` - Not available (use shell `cd` command instead)
- `env` - Not available (use shell environment syntax: `ENV=value command`)
- `auto_commit` - Not in WorkflowStepCommand
- `commit_config` - Not in WorkflowStepCommand
- `step_validate` - Not in WorkflowStepCommand
- `skip_validation` - Not in WorkflowStepCommand
- `validation_timeout` - Not in WorkflowStepCommand
- `ignore_validation_failure` - Not in WorkflowStepCommand

### CaptureStreams Configuration

The `capture_streams` field controls which output streams are captured:

```yaml
- shell: "cargo test"
  capture: "test_results"
  capture_streams:
    stdout: true      # Capture standard output (default: true)
    stderr: false     # Capture standard error (default: false)
    exit_code: true   # Capture exit code (default: true)
    success: true     # Capture success boolean (default: true)
    duration: true    # Capture execution duration (default: true)
```

**Examples:**

```yaml
# Capture only stdout and stderr
- shell: "build.sh"
  capture: "build_output"
  capture_streams:
    stdout: true
    stderr: true
    exit_code: false
    success: false
    duration: false

# Capture only timing information
- shell: "benchmark.sh"
  capture: "bench_time"
  capture_streams:
    stdout: false
    stderr: false
    exit_code: false
    success: false
    duration: true
```

### Capture Format Examples

The `capture_format` field controls how captured output is parsed:

```yaml
# String format (default) - raw text output
- shell: "git rev-parse HEAD"
  capture: "commit_hash"
  capture_format: "string"

# Number format - parses numeric output
- shell: "wc -l < file.txt"
  capture: "line_count"
  capture_format: "number"

# JSON format - parses JSON output
- shell: "cargo metadata --format-version 1"
  capture: "project_metadata"
  capture_format: "json"

# Lines format - splits output into array of lines
- shell: "git diff --name-only"
  capture: "changed_files"
  capture_format: "lines"

# Boolean format - true if command succeeds, false otherwise
- shell: "grep -q 'pattern' file.txt"
  capture: "pattern_found"
  capture_format: "boolean"
```

### Deprecated Fields

These fields are deprecated but still supported for backward compatibility:

- `test:` - Use `shell:` with `on_failure:` instead
- `command:` in ValidationConfig - Use `shell:` instead
- Nested `commands:` in `agent_template` and `reduce` - Use direct array format instead
- Legacy variable aliases (`$ARG`, `$ARGUMENT`, `$FILE`, `$FILE_PATH`) - Use modern `${item.*}` syntax

**Migration: capture_output to capture**

Old syntax (deprecated):
```yaml
- shell: "ls -la | wc -l"
  capture_output: true
```

New syntax (recommended):
```yaml
- shell: "ls -la | wc -l"
  capture: "file_count"
```

The modern `capture` field allows you to specify a variable name, making output references clearer and more maintainable
