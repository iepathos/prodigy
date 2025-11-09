## Step Identification

Assign unique IDs to steps for explicit output referencing. This is particularly useful in complex workflows where multiple steps produce outputs and you need to reference specific results.

### Available Step Reference Fields

When you assign an ID to a step, you can reference multiple fields from that step's execution:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `${step-id.output}` | string | Captured output | `${test-step.output}` |
| `${step-id.exit_code}` | number | Exit code | `${build.exit_code}` |
| `${step-id.success}` | boolean | Success status | `${lint.success}` |
| `${step-id.duration}` | number | Execution time (seconds) | `${bench.duration}` |

### Basic Step IDs

```yaml
- shell: "cargo test"
  id: "test-step"
  capture_output: "test_results"

# Reference step output by ID
- shell: "echo 'Tests: ${test-step.output}'"

# Reference step exit code
- shell: "echo 'Exit code: ${test-step.exit_code}'"
```

### When to Use Step IDs

**1. Complex Workflows with Multiple Parallel Paths**

When you have multiple steps producing similar outputs, IDs make references unambiguous:

```yaml
- shell: "cargo test --lib"
  id: "unit-tests"
  capture_output: "results"

- shell: "cargo test --test integration"
  id: "integration-tests"
  capture_output: "results"

# Clear reference to specific test results
- claude: "/analyze-failures '${unit-tests.output}'"
  when: "${unit-tests.exit_code != 0}"

- claude: "/analyze-failures '${integration-tests.output}'"
  when: "${integration-tests.exit_code != 0}"
```

**2. Debugging Specific Steps**

Step IDs help identify which step produced problematic output:

```yaml
- shell: "npm run build"
  id: "build"
  capture_output: "build_log"

- shell: "npm run lint"
  id: "lint"
  capture_output: "lint_log"

- shell: "npm test"
  id: "test"
  capture_output: "test_log"

# Reference specific logs for debugging
- claude: "/debug-build-failure '${build.output}'"
  when: "${build.exit_code != 0}"
```

**3. Conditional Execution Based on Specific Step Outputs**

Use step IDs to create complex conditional logic:

```yaml
- shell: "cargo clippy"
  id: "clippy-check"
  capture_output: "warnings"
  capture_format: "lines"

- shell: "cargo fmt --check"
  id: "format-check"
  capture_output: "format_issues"

# Only proceed if both checks pass
- shell: "cargo build --release"
  when: "${clippy-check.exit_code == 0 && format-check.exit_code == 0}"

# Fix clippy warnings if present
- claude: "/fix-clippy-warnings '${clippy-check.output}'"
  when: "${clippy-check.exit_code != 0}"
  on_failure:
    claude: "/analyze-clippy-fix-failures"
```

**4. Combining with Validation and Error Handlers**

Step IDs enable sophisticated error handling patterns:

```yaml
- shell: "cargo test --format json"
  id: "test-run"
  capture_output: "test_results"
  capture_format: "json"
  validate:
    shell: "check-test-coverage.sh"
    threshold: 80
  on_failure:
    claude: "/fix-failing-tests '${test-run.output}'"
    max_attempts: 3

# Use test results in summary
- shell: "generate-report.sh '${test-run.output}'"
```

**When Not to Use Step IDs:**
- Single-step workflows where `${shell.output}` is unambiguous
- Simple sequential workflows with no branching
- Steps where output isn't referenced later
