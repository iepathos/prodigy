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

## 5. Write File Commands

Create or overwrite files with content from variables or literals. Supports text, JSON, and YAML formats with automatic validation and formatting.

```yaml
# Write plain text file
- write_file:
    path: "output/result.txt"
    content: "Build completed at ${shell.output}"
    format: text
    mode: "0644"
    create_dirs: true

# Write JSON file with validation
- write_file:
    path: "config/generated.json"
    content: |
      {
        "version": "${version}",
        "timestamp": "${timestamp}",
        "items": ${items_json}
      }
    format: json

# Write YAML file with formatting
- write_file:
    path: ".prodigy/metadata.yml"
    content: |
      workflow: ${workflow.name}
      iteration: ${workflow.iteration}
      results:
        success: ${map.successful}
        total: ${map.total}
    format: yaml
```

**WriteFileConfig Fields:**
- `path` - File path to write (supports variable interpolation)
- `content` - Content to write (supports variable interpolation)
- `format` - Output format: `text` (default), `json`, `yaml`
- `mode` - File permissions in octal (default: "0644")
- `create_dirs` - Create parent directories if they don't exist (default: false)

**Format Validation:**
- `json` - Validates JSON syntax and pretty-prints output
- `yaml` - Validates YAML syntax and formats output
- `text` - Writes content as-is without validation

## 6. Validation Commands

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
- `max_attempts` - Maximum retry attempts (default: 2)
- `fail_workflow` - Whether to fail workflow if validation incomplete (default: true)
- `commit_required` - Whether to require commit after gap filling (default: false)
- `prompt` - Optional interactive prompt for user guidance

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
| `capture_streams` | string | Reserved for future use - currently stored as string in YAML |
| `on_success` | object | Command to run on success |
| `on_failure` | object | OnFailureConfig with nested command, max_attempts, fail_workflow, strategy |
| `validate` | object | Validation configuration |
| `output_file` | string | Redirect command output to a file |

**Note:** The following fields are used internally during workflow execution but are NOT part of the YAML configuration syntax. These are implementation details managed by Prodigy's execution engine, not user-facing configuration options:
- `handler` - Internal HandlerStep for execution routing
- `retry` - Internal RetryConfig for automatic retry logic
- `working_dir` - Not available (use shell `cd` command instead)
- `env` - Not available (use shell environment syntax: `ENV=value command`)
- `auto_commit` - Internal commit tracking
- `commit_config` - Internal commit configuration
- `step_validate` - Internal validation state
- `skip_validation` - Internal validation control
- `validation_timeout` - Internal validation timing
- `ignore_validation_failure` - Internal validation handling

### CaptureStreams Configuration

**Note:** The `capture_streams` field is currently stored as a string type in WorkflowStepCommand (line 120 of src/config/command.rs) and is reserved for future implementation. The CaptureStreams struct exists in src/cook/workflow/variables.rs for use by the execution engine, but the YAML configuration interface is not yet fully connected.

When implemented, `capture_streams` will control which output streams are captured:

```yaml
# Future implementation example:
- shell: "cargo test"
  capture: "test_results"
  capture_streams:
    stdout: true      # Capture standard output (default: true)
    stderr: false     # Capture standard error (default: false)
    exit_code: true   # Capture exit code (default: true)
    success: true     # Capture success boolean (default: true)
    duration: true    # Capture execution duration (default: true)
```

For now, use `capture` and `capture_format` to control output capture behavior.

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

The old `capture_output: true` syntax captured command output to a default variable name, making it unclear where the output was stored and harder to reference in later commands.

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

The modern `capture` field requires an explicit variable name, making output references clearer and more maintainable. You can then reference the captured value using `${file_count}` in subsequent commands.
