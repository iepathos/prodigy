## Output Capture Options

Options for capturing, formatting, and storing command output.

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
