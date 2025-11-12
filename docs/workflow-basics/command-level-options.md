## Command-Level Options

All command types (`claude:`, `shell:`, `goal_seek:`, `foreach:`) support additional fields for advanced control and orchestration. These options enable timeout management, output capture, error handling, conditional execution, and more.

## Core Options

### timeout

Sets a maximum execution time for the command (in seconds). If the command exceeds this duration, it will be terminated.

**Type**: `Option<u64>` (optional, no default timeout)

**Source**: `src/config/command.rs:383`

```yaml
commands:
  # 5 minute timeout for test suite
  - shell: "npm test"
    timeout: 300

  # 10 minute timeout for Claude implementation
  - claude: "/implement feature"
    timeout: 600

  # No timeout (runs until completion)
  - shell: "cargo build --release"
```

**Real-world examples**:
- `workflows/debtmap-reduce.yml:6` - 15 minute timeout for coverage generation
- `workflows/complex-build-pipeline.yml:23` - 10 minute timeout for benchmarks
- `workflows/documentation-drift.yml:48` - 5 minute timeout for doc tests

### id

Assigns an identifier to the command for referencing its outputs in subsequent commands via workflow variables.

**Type**: `Option<String>` (optional)

**Source**: `src/config/command.rs:351-352`

```yaml
commands:
  - shell: "git rev-parse --short HEAD"
    id: "get_commit"
    capture_output: "commit_hash"

  # Use the captured output
  - shell: "echo 'Building commit ${commit_hash}'"
```

### commit_required

Specifies whether the command is expected to create git commits. Prodigy tracks commits for workflow provenance and rollback.

**Type**: `bool` (default: `false`)

**Source**: `src/config/command.rs:354-356`

```yaml
commands:
  # Claude commands that modify code should commit
  - claude: "/prodigy-coverage"
    commit_required: true

  # Test commands typically don't commit
  - shell: "cargo test"
    commit_required: false

  # Linting fixes may commit changes
  - claude: "/prodigy-lint"
    commit_required: true
```

**Real-world examples**:
- `workflows/coverage.yml:5,11` - Coverage and implementation commands
- `workflows/documentation-drift.yml:19,23,27` - Documentation update commands
- `workflows/implement-with-tests.yml:27,31` - Test vs implementation distinction

## Output Capture Options

### capture_output

Captures command output and stores it in a workflow variable. Supports both boolean mode (captures to default variable) and variable name mode (captures to named variable).

**Type**: `Option<CaptureOutputConfig>` where `CaptureOutputConfig` is:
```rust
enum CaptureOutputConfig {
    Boolean(bool),      // Simple capture (true/false)
    Variable(String),   // Capture to named variable
}
```

**Source**: `src/config/command.rs:367-368, 403-411`

```yaml
commands:
  # Capture to default variable name (shell.output)
  - shell: "echo 'Starting analysis...'"
    capture_output: true

  # Capture to custom variable name
  - shell: "ls -la | wc -l"
    capture_output: "file_count"

  # Use the captured variable
  - shell: "echo 'Found ${file_count} files'"

  # Disable capture explicitly
  - shell: "cargo build"
    capture_output: false
```

**Real-world examples**:
- `examples/capture-output-custom-vars.yml:10-48` - Custom variable names
- `workflows/implement-with-tests.yml:26,55` - Test output capture
- `workflows/complex-build-pipeline.yml:17,24` - Build diagnostics

### capture_format

Specifies how to parse captured output. Determines the data type and structure of the captured variable.

**Type**: `Option<String>` with values: `string` (default), `json`, `lines`, `number`, `boolean`

**Source**: `src/config/command.rs:391-392`, `src/cook/workflow/variables.rs:250-265`

**Supported formats**:
- `string` - Raw string output (default)
- `json` - Parse as JSON object/array
- `lines` - Split into array of lines
- `number` - Parse as numeric value
- `boolean` - Parse as boolean (`true`/`false`)

```yaml
commands:
  # Capture as JSON object
  - shell: "cat package.json"
    capture_output: "package_info"
    capture_format: "json"

  # Access JSON fields
  - shell: "echo 'Package: ${package_info.name} v${package_info.version}'"

  # Capture as boolean
  - shell: "cargo test --quiet && echo true || echo false"
    capture_output: "tests_passed"
    capture_format: "boolean"

  # Capture as number
  - shell: "find src -name '*.rs' | wc -l"
    capture_output: "file_count"
    capture_format: "number"

  # Capture as array of lines
  - shell: "git diff --name-only"
    capture_output: "changed_files"
    capture_format: "lines"
```

**Real-world examples**:
- `examples/capture-json-processing.yml:9-54` - JSON metadata extraction
- `examples/capture-conditional-flow.yml:9-37` - Boolean and number formats
- `examples/capture-parallel-analysis.yml:9-101` - Multi-format capture pipeline

### capture_streams

Controls which output streams to capture from command execution. By default, only stdout is captured.

**Type**: `Option<String>` or structured configuration

**Source**: `src/config/command.rs:394-396`, `src/cook/workflow/variables.rs:267-292`

**Captured fields**:
- `stdout` - Standard output (default: `true`)
- `stderr` - Standard error (default: `false`)
- `exit_code` - Process exit code (default: `true`)
- `success` - Whether command succeeded (default: `true`)
- `duration` - Execution duration (default: `true`)

```yaml
commands:
  # Capture all streams and metadata
  - shell: "cargo build --release"
    capture_output: "build_result"
    capture_streams:
      stdout: true
      stderr: true
      exit_code: true
      success: true
      duration: true

  # Access captured fields
  - shell: |
      echo "Build Success: ${build_result.success}"
      echo "Exit Code: ${build_result.exit_code}"
      echo "Duration: ${build_result.duration}s"
```

**Real-world examples**:
- `examples/capture-conditional-flow.yml:44-51` - Multi-stream capture with conditionals

### output_file

Redirects command output to a file. This option is defined in the type definition but not yet widely used in the codebase.

**Type**: `Option<String>` (file path)

**Source**: `src/config/command.rs:398-400`

```yaml
commands:
  - shell: "cargo test --verbose"
    output_file: "test-results.txt"

  - shell: "cargo doc --no-deps"
    output_file: "docs/api-output.log"
```

**Note**: This feature is defined but no production examples currently exist. Consider contributing examples if you use this option.

## Error Handling

### on_failure

Specifies commands to execute when the main command fails. Supports automatic retry with configurable attempts and workflow failure control.

**Type**: `Option<TestDebugConfig>` with fields:
- `claude: String` - Claude command to run on failure
- `max_attempts: u32` - Maximum retry attempts (default: 3)
- `fail_workflow: bool` - Whether to fail workflow after max attempts (default: `false`)
- `commit_required: bool` - Whether debug command should commit (default: `true`)

**Source**: `src/config/command.rs:370-372, 166-183`

```yaml
commands:
  # Retry tests with automated fixing
  - shell: "cargo test"
    on_failure:
      claude: "/prodigy-debug-test-failure --output ${shell.output}"
      max_attempts: 3
      fail_workflow: false

  # Critical check that must pass
  - shell: "just fmt-check && just lint"
    on_failure:
      claude: "/prodigy-lint ${shell.output}"
      max_attempts: 5
      fail_workflow: true

  # Doc test failures with commit requirement
  - shell: "cargo test --doc"
    on_failure:
      claude: "/prodigy-fix-doc-tests --output ${shell.output}"
      max_attempts: 2
      fail_workflow: false
      commit_required: true
```

**Real-world examples**:
- `workflows/coverage-with-test-debug.yml:13-23` - Test debugging with retries
- `workflows/debtmap-reduce.yml:58-70` - Critical quality gates
- `workflows/documentation-drift.yml:47-53` - Doc test recovery

### on_success

Executes another command when the main command succeeds. Enables chaining of dependent operations.

**Type**: `Option<Box<WorkflowStepCommand>>` (nested command)

**Source**: `src/config/command.rs:374-376`

```yaml
commands:
  # Chain successful operations
  - shell: "cargo check"
    on_success:
      shell: "cargo build --release"
      on_success:
        shell: "cargo test --release"

  # Nested success/failure handlers
  - shell: "cargo test"
    capture_output: "test_output"
    on_failure:
      claude: "/prodigy-debug-test-failures '${test_output}'"
      commit_required: true
      on_success:
        # After fixing, verify tests pass
        shell: "cargo test"
        on_failure:
          claude: "/prodigy-fix-test-failures '${shell.output}' --deep-analysis"

  # Success notification
  - shell: "cargo test --release"
    capture_output: "final_results"
    on_success:
      shell: "echo '✅ All tests passing!'"
```

**Real-world examples**:
- `workflows/implement-with-tests.yml:28-40,61-63` - Nested test-fix-verify loops
- `workflows/complex-build-pipeline.yml:7-13` - Build pipeline chaining

## Conditional Execution

### when

Evaluates a boolean expression to determine whether to execute the command. Supports variable interpolation and boolean logic.

**Type**: `Option<String>` (boolean expression)

**Source**: `src/config/command.rs:387-388`

```yaml
commands:
  # Capture test status
  - shell: "cargo test --quiet && echo true || echo false"
    capture_output: "tests_passed"
    capture_format: "boolean"

  # Conditional execution based on test results
  - shell: "echo 'Running coverage analysis...'"
    when: "${tests_passed}"

  # Multiple conditions
  - shell: "cargo build --release"
    when: "${tests_passed} && ${lint_passed}"
    capture_output: "build_output"

  # Conditional deployment
  - shell: |
      if [ "${tests_passed}" = "true" ] && [ "${build_output.success}" = "true" ]; then
        echo "✅ Deployment ready!"
      fi
    when: "${tests_passed}"
```

**Real-world examples**:
- `examples/capture-conditional-flow.yml:20-51` - Multi-stage conditional pipeline

## Advanced Options

### validate

Configures implementation completeness validation with automatic gap detection and filling.

**Type**: `Option<ValidationConfig>` (validation configuration)

**Source**: `src/config/command.rs:378-380`

```yaml
commands:
  - claude: "/implement-feature"
    validate:
      force_refresh: false
      max_cache_age: 300
```

**Note**: See validation documentation for comprehensive coverage of this feature.

### analysis

Specifies per-step analysis requirements for code quality and coverage tracking.

**Type**: `Option<AnalysisConfig>` with fields:
- `force_refresh: bool` - Force fresh analysis even if cached (default: `false`)
- `max_cache_age: u64` - Maximum cache age in seconds (default: 300)

**Source**: `src/config/command.rs:116-126`

```yaml
commands:
  - claude: "/implement-feature"
    analysis:
      force_refresh: true
      max_cache_age: 600
```

### env

Sets command-specific environment variables that override workflow-level and system environment variables.

**Type**: `HashMap<String, String>` (key-value pairs)

**Source**: `src/config/command.rs:143-145`

```yaml
commands:
  - shell: "cargo build"
    env:
      RUST_BACKTRACE: "1"
      CARGO_INCREMENTAL: "0"

  - claude: "/analyze-code"
    env:
      DEBUG_MODE: "true"
      LOG_LEVEL: "trace"
```

## Option Combinations

### Test-Fix-Verify Pattern

Combines output capture, error handling, and conditionals for robust test workflows:

```yaml
commands:
  # Initial test run
  - shell: "cargo test"
    capture_output: "test_output"
    commit_required: false
    on_failure:
      # Automated fix on failure
      claude: "/prodigy-debug-test-failures '${test_output}'"
      commit_required: true
      max_attempts: 3
      on_success:
        # Verify fix worked
        shell: "cargo test"
        commit_required: false
```

### Conditional Pipeline Pattern

Uses capture formats and conditionals for decision-making workflows:

```yaml
commands:
  # Capture test status as boolean
  - shell: "cargo test --quiet && echo true || echo false"
    capture_output: "tests_passed"
    capture_format: "boolean"

  # Capture coverage as number
  - shell: "cargo tarpaulin --output-dir coverage | grep -oP '\\d+\\.\\d+(?=%)' | head -1"
    capture_output: "coverage_percent"
    capture_format: "number"
    when: "${tests_passed}"

  # Deploy only if quality gates pass
  - shell: "echo 'Deploying to production...'"
    when: "${tests_passed} && ${coverage_percent} >= 80"
```

### Parallel Capture Pattern

Captures multiple metrics in parallel for aggregation:

```yaml
commands:
  # Capture metadata
  - shell: "find src -name '*.rs' | wc -l"
    capture_output: "total_files"
    capture_format: "number"

  # Build summary JSON
  - shell: |
      echo '{
        "repository": "'$(basename $(pwd))'",
        "total_files": ${total_files},
        "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
      }'
    capture_output: "metadata"
    capture_format: "json"
```

## Troubleshooting

### Command Times Out

**Problem**: Command exceeds timeout and is terminated

**Solutions**:
- Increase `timeout` value
- Optimize command performance
- Split into multiple smaller commands
- Remove timeout for commands with unpredictable duration

### Capture Format Mismatch

**Problem**: Error parsing captured output with specified format

**Solutions**:
- Verify command output matches expected format (use `echo` to inspect)
- Add format validation to command output
- Use `capture_format: "string"` as fallback
- Check for extra whitespace or unexpected characters

### on_failure Not Triggering

**Problem**: Failure handler doesn't execute when command fails

**Solutions**:
- Verify command actually returns non-zero exit code
- Check `max_attempts` hasn't been exceeded
- Ensure `on_failure` syntax is correct (nested under command)
- Review workflow logs for execution details

### Variable Not Available

**Problem**: `${variable}` not found or empty in subsequent command

**Solutions**:
- Verify `capture_output` is set with correct variable name
- Check command actually produces output (not empty)
- Ensure `capture_format` matches output type
- Use default values: `${var|default:fallback}`

## See Also

- [Environment Configuration](environment-configuration.md) - Workflow-level environment configuration
- [Timeout Configuration](../advanced/timeout-configuration.md) - Advanced timeout strategies
- [Parallel Iteration with Foreach](../advanced/parallel-iteration-with-foreach.md) - Foreach command with parallel options
- [Examples](../examples.md) - Complete workflow examples

