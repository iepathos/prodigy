## Best Practices

### 1. Use Meaningful Variable Names

Choose descriptive names for captured outputs to make workflows self-documenting:

```yaml
# Good - Clear and descriptive
- shell: "cargo test --format json"
  capture_output: "test_results"
  capture_format: "json"

# Avoid - Cryptic and unclear
- shell: "cargo test --format json"
  capture_output: "x"
```

### 2. Set Appropriate Timeouts

Protect workflows from hanging on long-running operations:

```yaml
# Set timeouts for potentially long-running operations
- shell: "npm install"
  timeout: 300  # 5 minutes

- claude: "/analyze-large-codebase"
  timeout: 1800  # 30 minutes
```

**Guidelines:**
- Set timeouts high enough for normal completion
- Consider worst-case scenarios (slow CI, cold caches)
- Use environment variables for configurable timeouts

### 3. Handle Failures Gracefully

Provide automatic remediation for common failure scenarios:

```yaml
# Provide automatic remediation
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
    max_attempts: 2
    fail_workflow: true
```

**Consider:**
- Whether to fail the workflow or continue
- How many retry attempts are reasonable
- Whether failures should create git commits

### 4. Validate Critical Changes

Ensure implementations meet requirements before proceeding:

```yaml
# Ensure implementation meets requirements
- claude: "/implement-feature"
  validate:
    commands:
      - shell: "cargo test"
      - shell: "cargo clippy -- -D warnings"
    threshold: 100
    on_incomplete:
      claude: "/fix-issues"
      max_attempts: 3
```

**Best Practices:**
- Validate after significant changes
- Use multiple validation steps for comprehensive coverage
- Set appropriate thresholds based on criticality
- Provide automatic remediation for incomplete implementations

### 5. Use Step IDs for Complex Workflows

Make output references explicit in complex workflows:

```yaml
# Make output references explicit
- shell: "git diff --stat"
  id: "git-changes"
  capture_output: "diff"

- claude: "/review-changes '${git-changes.output}'"
  id: "code-review"
```

**When to Use:**
- Multiple steps producing similar outputs
- Complex conditional logic based on specific steps
- Debugging specific step outputs
- Combining step metadata (exit_code, success, duration)

### 6. Leverage Parallel Execution

Use `foreach` with parallelism for independent operations:

```yaml
- foreach:
    foreach: ["frontend", "backend", "shared"]
    parallel: 3
    do:
      - shell: "cd ${item} && npm install"
      - shell: "cd ${item} && npm test"
```

**Guidelines:**
- Ensure items are truly independent
- Set appropriate parallel limits based on resources
- Use `continue_on_error` for fault tolerance
- Consider timeout implications with parallel execution

### 7. Structure Complex Conditionals

Use comparison operators and logical operators effectively:

```yaml
# Clear multi-condition logic
- shell: "deploy.sh"
  when: "${environment == 'production' && tests_passed && coverage >= 80}"

# Explicit step references
- shell: "notify-team.sh"
  when: "${test-step.exit_code == 0 && build-step.success}"
```

### 8. Separate Concerns with Step-Level Environment

Configure environment variables at the step level for isolation:

```yaml
- shell: "npm test"
  env:
    NODE_ENV: "test"
    DEBUG: "*"
  working_dir: "./frontend"

- shell: "npm run build"
  env:
    NODE_ENV: "production"
  working_dir: "./frontend"
```

### 9. Use Exit Code Handlers for Fine-Grained Control

Map specific exit codes to appropriate actions:

```yaml
- shell: "cargo test"
  on_exit_code:
    1: {claude: "/fix-test-failures"}
    101: {claude: "/fix-compilation-errors"}
    255: {fail_workflow: true}
```

### 10. Document Complex Workflows

Add comments to explain non-obvious logic:

```yaml
# Run tests with coverage, requiring 80% threshold
# Falls back to basic tests if coverage tooling unavailable
- shell: "cargo tarpaulin --out json"
  timeout: 600
  capture_output: "coverage"
  capture_format: "json"
  on_failure:
    shell: "cargo test"  # Fallback without coverage
```
