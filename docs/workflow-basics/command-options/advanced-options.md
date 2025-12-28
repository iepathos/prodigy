## Advanced Options

Additional configuration options for specialized use cases.

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

Common patterns that combine multiple options for robust workflows.

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

Common issues and their solutions.

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
