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
- `fail_on_incomplete`: Whether to fail workflow if threshold not met (default: true)

**Troubleshooting:**
- **Threshold not met:** Check that validate command outputs exactly `score: N` format (0-100)
- **Not converging:** Use `fail_on_incomplete: false` for optional quality gates
- **Debug scores:** Run workflow with verbose mode (`-v`) to see validation scores each iteration
- **Max attempts reached:** Increase `max_attempts` or lower `threshold` if goal is too ambitious

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

**Variables and Error Handling:**
- **${item}**: Current item value available in loop body
- **continue_on_error: true** (default): Failed items don't stop the loop
- **Parallel execution caveat**: Output order is not guaranteed when using `parallel`
- **No built-in result aggregation**: Use `write_file` commands to collect results if needed

**Example with result collection:**
```yaml
- foreach:
    input: ["module1", "module2", "module3"]
    parallel: 3
    do:
      - shell: "cargo test --package ${item}"
      - write_file:
          path: "results/${item}.txt"
          content: "Test result: ${shell.output}"
          create_dirs: true
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

**Best Practices:**
- **Use format validation for config files**: Set `format: json` or `format: yaml` when generating configuration files to catch syntax errors early
- **Set appropriate permissions**: Use `mode` field to control file permissions (e.g., `"0600"` for sensitive files)
- **Handle nested paths**: Set `create_dirs: true` when writing to paths that may not exist
- **Combine with validation**: Use `validate` field to ensure generated files meet requirements before proceeding
- **For logs and documentation**: Use `format: text` to write content as-is without validation overhead

## 6. Validation Commands

Validate implementation completeness with automatic retry.

> **Deprecation Notice**: The `command` field in ValidationConfig is deprecated. Use `shell` instead for shell commands or `claude` for Claude commands. The `command` field is still supported for backward compatibility but will be removed in a future version.

```yaml
- claude: "/implement-auth-spec"
  validate:
    shell: "debtmap validate --spec auth.md --output result.json"
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
| `capture` | string | Variable name to capture command output (replaces deprecated `capture_output: true`) |
| `capture_format` | enum | Format: `string` (default), `number`, `json`, `lines`, `boolean` (see examples below) |
| `capture_streams` | string | Reserved for future YAML syntax - not yet available in workflows |
| `on_success` | object | Command to run on success |
| `on_failure` | object | OnFailureConfig with nested command, max_attempts, fail_workflow, strategy |
| `validate` | object | Validation configuration |
| `output_file` | string | Redirect command output to a file |

### CaptureStreams Configuration

**Note:** While `capture_streams` functionality is implemented internally in Prodigy's execution engine, it is not yet exposed in the YAML workflow syntax. The field exists in the configuration structs but is currently stored as a string placeholder.

**Current Approach:** Use the `capture` and `capture_format` fields to control output capture:

```yaml
# Capture stdout as string (most common use case)
- shell: "cargo test"
  capture: "test_output"
  capture_format: "string"

# Capture exit status as boolean
- shell: "cargo test"
  capture: "test_passed"
  capture_format: "boolean"

# Capture and parse JSON output
- shell: "cargo metadata --format-version 1"
  capture: "project_info"
  capture_format: "json"
```

**Future Enhancement:** A future version will expose `capture_streams` in YAML syntax to provide fine-grained control over which streams (stdout, stderr, exit_code, success, duration) are captured. Until then, use the `capture` and `capture_format` fields which cover most common use cases.

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

The old `capture_output: true/false` syntax is deprecated. It used a boolean value to enable/disable output capture, but didn't specify where the output was stored, making it unclear and harder to reference in later commands.

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

**Why the change?** The modern `capture` field requires an explicit variable name, making workflows more maintainable:
- **Explicit is better than implicit**: Variable names are self-documenting
- **Easier refactoring**: Clear what each command produces
- **Better error messages**: References to undefined variables are clearer

You can then reference the captured value using `${file_count}` in subsequent commands. The boolean `capture_output` field is retained for backward compatibility but should not be used in new workflows.

---

## Technical Notes

<details>
<summary>Internal Implementation Fields (for contributors)</summary>

The following fields are used internally during workflow execution but are NOT part of the YAML configuration syntax. These are implementation details managed by Prodigy's execution engine, not user-facing configuration options:

- `handler` - Internal HandlerStep for execution routing
- `retry` - Internal RetryConfig for automatic retry logic
- `env` - Not available (use shell environment syntax: `ENV=value command`)
- `auto_commit` - Internal commit tracking
- `commit_config` - Internal commit configuration
- `step_validate` - Internal validation state
- `skip_validation` - Internal validation control
- `validation_timeout` - Internal validation timing
- `ignore_validation_failure` - Internal validation handling

These fields are documented here for reference when working on Prodigy's source code, but should not be used in workflow YAML files.

</details>

---

## Cross-References

For more information on related topics:
- **Variable Interpolation**: See the Variables chapter for details on using captured outputs like `${variable_name}` in subsequent commands
- **Error Handling**: See the Error Handling chapter for advanced `on_failure` strategies and retry patterns
- **MapReduce Workflows**: See the MapReduce chapter for large-scale parallel command execution

**Example: Using Captured Output in Subsequent Commands**

```yaml
# Capture build output and use it in later commands
- shell: "cargo build --release 2>&1"
  capture: "build_output"
  capture_format: "string"

# Use the captured output in Claude command
- claude: "/analyze-warnings '${build_output}'"
  when: "${build_output contains 'warning'}"

# Store output to file for later analysis
- write_file:
    path: "logs/build-${workflow.iteration}.log"
    content: "${build_output}"
    create_dirs: true
```
