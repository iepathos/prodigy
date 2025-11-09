## Common Patterns

### Test-Fix-Verify Loop

Automatically fix test failures and verify the fixes:

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/fix-tests"
    on_success:
      shell: "cargo test --release"
```

This pattern:
1. Runs tests
2. If tests fail, uses Claude to fix them
3. If fixes succeed, runs tests again in release mode to verify

### Parallel Processing with Aggregation

Process multiple items in parallel, then aggregate results:

```yaml
- foreach:
    foreach: "find src -name '*.rs'"
    parallel: 10
    do:
      - shell: "analyze-file ${item}"
        capture_output: "analysis_${item}"

- shell: "aggregate-results.sh"
```

**Use Cases:**
- Analyzing multiple files independently
- Running tests in parallel across modules
- Processing data in batches

### Gradual Quality Improvement

Iteratively improve code quality until threshold is met:

```yaml
- goal_seek:
    goal: "Code quality score above 90"
    shell: "auto-improve.sh"
    validate: "quality-check.sh"
    threshold: 90
    max_attempts: 5
  on_success:
    shell: "git commit -m 'Improved code quality'"
```

This pattern:
- Runs quality improvements iteratively
- Validates after each attempt
- Commits when goal is achieved

### Conditional Deployment

Deploy only when all tests pass:

```yaml
- shell: "cargo test"
  capture_output: "test_results"
  capture_format: "json"

- shell: "deploy.sh"
  when: "${test_results.passed == test_results.total}"
  on_success:
    shell: "notify-success.sh"
  on_failure:
    shell: "rollback.sh"
```

**Pattern Components:**
- Capture test results as JSON
- Deploy only if all tests passed
- Send notification on success
- Rollback on failure

### Multi-Stage Validation

Validate through multiple quality gates:

```yaml
- claude: "/implement-feature"
  validate:
    commands:
      - shell: "cargo build"
      - shell: "cargo test"
      - shell: "cargo clippy"
      - shell: "cargo fmt --check"
    threshold: 100
    on_incomplete:
      commands:
        - claude: "/fix-build-errors"
        - claude: "/fix-test-failures"
        - claude: "/fix-clippy-warnings"
      max_attempts: 3
```

This ensures code passes all quality checks before proceeding.

### Progressive Enhancement

Build features incrementally with validation at each stage:

```yaml
# Stage 1: Basic implementation
- claude: "/implement-basic-feature"
  validate:
    shell: "basic-tests.sh"
    threshold: 100

# Stage 2: Add edge case handling
- claude: "/add-edge-cases"
  validate:
    shell: "comprehensive-tests.sh"
    threshold: 100

# Stage 3: Optimize performance
- claude: "/optimize-performance"
  validate:
    shell: "benchmark.sh"
    threshold: 95
```

### Fail-Fast with Early Validation

Validate quickly before expensive operations:

```yaml
# Quick syntax check first
- shell: "cargo check"
  on_failure:
    fail_workflow: true

# Then run expensive tests
- shell: "cargo test"
  timeout: 600

# Finally run benchmarks
- shell: "cargo bench"
  timeout: 1800
```

### Multi-Environment Workflow

Different behavior based on environment:

```yaml
- shell: "run-tests.sh"
  env:
    ENV: "${ENVIRONMENT}"
  capture_output: "test_results"

- shell: "deploy.sh"
  when: "${ENVIRONMENT == 'production' && test_results.exit_code == 0}"
  env:
    DEPLOY_TARGET: "production"

- shell: "deploy-staging.sh"
  when: "${ENVIRONMENT == 'staging' && test_results.exit_code == 0}"
  env:
    DEPLOY_TARGET: "staging"
```

### Retry with Backoff

Retry failed operations with different strategies:

```yaml
- shell: "flaky-operation.sh"
  on_failure:
    shell: "flaky-operation.sh --retry"
    on_failure:
      shell: "flaky-operation.sh --force"
      max_attempts: 3
```

### Data Pipeline

Transform data through multiple stages:

```yaml
- shell: "extract-data.sh"
  capture_output: "raw_data"

- shell: "transform-data.sh '${raw_data}'"
  capture_output: "transformed_data"
  capture_format: "json"

- shell: "validate-data.sh '${transformed_data}'"
  validate:
    shell: "schema-check.sh"
    threshold: 100

- shell: "load-data.sh '${transformed_data}'"
```

### Feature Flag Workflow

Enable features conditionally:

```yaml
- shell: "check-feature-flag.sh new-feature"
  capture_output: "feature_enabled"
  capture_format: "boolean"

- claude: "/implement-new-feature"
  when: "${feature_enabled}"

- shell: "run-tests.sh"
  env:
    FEATURE_NEW: "${feature_enabled}"
```
