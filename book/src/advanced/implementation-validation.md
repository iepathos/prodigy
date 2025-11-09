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

The file should contain JSON matching the validation result schema with fields like `completion_percentage`, `status`, `gaps`, etc.

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
      max_attempts: 3
      fail_workflow: true
      commit_required: true
```

The `on_incomplete` configuration supports:
- `claude`: Claude command to execute for gap-filling
- `shell`: Shell command to execute for gap-filling
- `commands`: Array of commands to execute in sequence
- `max_attempts`: Maximum remediation attempts (default: 1)
- `fail_workflow`: Whether to fail workflow if remediation fails (default: true)
- `commit_required`: Whether remediation command should create a commit (default: false)

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
