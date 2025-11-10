## Implementation Validation

Validate that implementations meet requirements using the `validate` field.

### Basic Validation

Run validation commands after a step completes:

```yaml
- claude: "/implement-feature"
  validate:
    shell: "cargo test"
    threshold: 100  # Require 100% completion (default)
```

**Note**: The `threshold` field defaults to **100** if not specified, requiring full implementation completion.

**Source**: [src/cook/workflow/validation.rs:280-282](../../../src/cook/workflow/validation.rs)

### Validation with Claude

Use Claude to validate implementation quality:

```yaml
- shell: "generate-code.sh"
  validate:
    claude: "/verify-implementation"
    threshold: 95
```

### Multi-Step Validation

Run multiple validation commands in sequence using the `commands` array:

```yaml
- claude: "/refactor"
  validate:
    commands:
      - shell: "cargo test"
      - shell: "cargo clippy"
      - shell: "cargo fmt --check"
    threshold: 100
```

**Convenience Array Syntax**: For simple cases, you can use an array format directly:

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

**When to Use result_file:**

The `result_file` option is useful when you need to separate validation output from command logs:

- **Complex JSON Output**: Validation produces structured JSON that shouldn't be mixed with logs
- **Separate Concerns**: Keep validation results separate from command stdout/stderr
- **Additional Logging**: Validation command produces diagnostic output alongside results
- **Debugging**: Preserve validation output in a file for later inspection

The file must contain valid JSON matching the ValidationResult schema. When the validation command completes, Prodigy reads the specified file and parses it as JSON. If the file doesn't exist or contains invalid JSON, the validation fails.

**Source**: [src/cook/workflow/executor/validation.rs:700-715](../../../src/cook/workflow/executor/validation.rs)

#### Advanced: Result Files with Commands Array

You can use `result_file` with the `commands` array for multi-step validation where the final result is written to a file:

```yaml
- claude: "/implement-spec $ARG"
  validate:
    commands:
      - claude: "/prodigy-validate-spec $ARG --output .prodigy/validation-result.json"
    result_file: ".prodigy/validation-result.json"
    threshold: 100
    on_incomplete:
      claude: "/prodigy-complete-spec $ARG --gaps ${validation.gaps}"
      max_attempts: 5
      commit_required: true
```

In this pattern, the validation command writes its results to a JSON file, and Prodigy reads that file after all commands complete.

**Source**: Real-world example from [workflows/implement.yml:6-16](../../../workflows/implement.yml)

### Handling Incomplete Implementations

Automatically remediate when validation fails to meet the threshold.

**Convenience Array Syntax** - For simple remediation workflows:

```yaml
- claude: "/implement-spec"
  validate:
    shell: "check-completeness.sh"
    threshold: 100
    on_incomplete:
      - claude: "/fill-gaps"
      - shell: "cargo fmt"
```

**Verbose Configuration** - For complex cases requiring additional control:

```yaml
- claude: "/implement-spec"
  validate:
    shell: "check-completeness.sh"
    threshold: 100
    on_incomplete:
      claude: "/fill-gaps"
      max_attempts: 2          # Default: 2 (not 3)
      fail_workflow: true      # Default: true
      commit_required: false   # Default: false
```

**Default Values**:
- `max_attempts`: **2** (maximum remediation attempts before giving up)
- `fail_workflow`: **true** (workflow fails if remediation doesn't reach threshold)
- `commit_required`: **false** (remediation command doesn't need to create a commit)

**Source**: [src/cook/workflow/validation.rs:284-289](../../../src/cook/workflow/validation.rs)

The `on_incomplete` configuration supports:
- `claude`: Claude command to execute for gap-filling
- `shell`: Shell command to execute for gap-filling
- `commands`: Array of commands to execute in sequence
- `max_attempts`: Maximum remediation attempts (default: **2**)
- `fail_workflow`: Whether to fail workflow if remediation fails (default: **true**)
- `commit_required`: Whether remediation command should create a commit (default: **false**)

**Source**: [src/cook/workflow/validation.rs:123-152](../../../src/cook/workflow/validation.rs)

### Timeout Configuration

Set a timeout for validation commands to prevent hanging:

```yaml
- claude: "/implement-feature"
  validate:
    shell: "long-running-test.sh"
    threshold: 100
    timeout: 300  # 5 minutes timeout
```

The `timeout` field specifies the maximum number of seconds the validation command can run. If the command exceeds this time, it's terminated and the validation fails.

**Source**: [src/cook/workflow/validation.rs:37-39](../../../src/cook/workflow/validation.rs)

### ValidationResult Schema

When using `result_file`, the JSON file must match this structure:

```json
{
  "completion_percentage": 95.5,
  "status": "incomplete",
  "implemented": [
    "Feature A is fully implemented",
    "Feature B includes unit tests"
  ],
  "missing": [
    "Feature C lacks error handling",
    "Feature D needs integration tests"
  ],
  "gaps": {
    "error_handling": {
      "description": "Missing error handling in parser",
      "location": "src/parser.rs:45",
      "severity": "high",
      "suggested_fix": "Add Result<T, E> return type and handle parse errors"
    }
  }
}
```

**Fields**:
- `completion_percentage`: Float (0-100) indicating implementation completeness
- `status`: Enum - `"complete"`, `"incomplete"`, `"failed"`, or `"skipped"`
- `implemented`: Array of strings describing completed requirements
- `missing`: Array of strings describing incomplete requirements
- `gaps`: Object mapping gap IDs to GapDetail objects with description, location, severity, and suggested_fix

**Source**: [src/cook/workflow/validation.rs:216-239](../../../src/cook/workflow/validation.rs)

### Validation Patterns

**Progressive Validation** - Validate in stages:

```yaml
- claude: "/implement-feature"
  validate:
    commands:
      - shell: "cargo check"     # Fast syntax check first
      - shell: "cargo test"      # Then run tests
      - shell: "cargo bench"     # Finally benchmarks
    threshold: 100
    timeout: 600  # 10 minute timeout for all commands
    on_incomplete:
      - claude: "/analyze-failures"
      - claude: "/fix-issues"
```

**Conditional Validation** - Validate based on previous results:

```yaml
- claude: "/optimize-code"
  id: "optimization"
  validate:
    shell: "benchmark.sh"
    threshold: 90

- shell: "verify-performance.sh"
  when: "${optimization.success}"
  validate:
    shell: "stress-test.sh"
    threshold: 100
```

**Complex Multi-Step Validation with Result Files** - Real-world pattern from Prodigy's debtmap workflow:

```yaml
- claude: "/implement-changes"
  commit_required: true
  validate:
    commands:
      - shell: "just coverage-lcov"
      - shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-after.json"
      - shell: "debtmap compare --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --output .prodigy/comparison.json"
      - claude: "/validate-improvement --comparison .prodigy/comparison.json --output .prodigy/validation.json"
    result_file: ".prodigy/validation.json"
    threshold: 75
    on_incomplete:
      commands:
        - claude: "/fix-remaining-gaps --validation .prodigy/validation.json"
          commit_required: true
        - shell: "just coverage-lcov"
        - shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-after.json"
      max_attempts: 5
      fail_workflow: true
```

**Source**: [workflows/debtmap.yml:26-42](../../../workflows/debtmap.yml)

This pattern demonstrates:
- Multiple validation commands executed in sequence
- Reading results from a file after all commands complete
- Multi-command remediation with commit requirements
- Iterative validation and fixing

### Configuration Reference

Complete list of validation configuration fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `shell` | String | None | Shell command to run for validation |
| `claude` | String | None | Claude command to run for validation |
| `commands` | Array | None | Array of commands for multi-step validation |
| `threshold` | Number | **100** | Completion percentage required (0-100) |
| `timeout` | Number | None | Timeout in seconds for validation commands |
| `result_file` | String | None | File path to read validation results from |
| `on_incomplete` | Object | None | Configuration for handling validation failures |

**Source**: [src/cook/workflow/validation.rs:11-49](../../../src/cook/workflow/validation.rs)
