## Common Patterns

### Test-Fix-Verify Loop

Automatically fix test failures and verify the fixes:

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/fix-tests"
    commit_required: true  # Ensure fixes create commits for audit trail
    on_success:
      shell: "cargo test --release"
```

**Source**: Pattern structure from src/config/command.rs:320-401 (WorkflowStepCommand)

This pattern:
1. Runs tests
2. If tests fail, uses Claude to fix them
3. Creates a commit for each fix (important for tracking changes)
4. If fixes succeed, runs tests again in release mode to verify

**Best Practices:**
- Use `commit_required: true` for automated fixes to maintain audit trail
- Consider adding timeout to prevent indefinite hanging
- Capture test output for debugging: `capture_output: "test_results"`

### MapReduce: Massive Parallel Processing

For large-scale parallel processing, use MapReduce workflows instead of foreach:

```yaml
name: parallel-analysis
mode: mapreduce

setup:
  - shell: "find src -name '*.rs' | jq -R -s 'split(\"\n\")[:-1] | map({file: .})' > files.json"

map:
  input: "files.json"
  json_path: "$[*]"

  agent_template:
    - shell: "analyze-file ${item.file}"
      capture_output: "analysis"

    - shell: "echo '${analysis}' > results/${item.file}.json"

  max_parallel: 10

reduce:
  - shell: "jq -s '.' results/*.json > aggregated-results.json"
  - claude: "/summarize-analysis aggregated-results.json"
```

**Source**: workflows/mapreduce-example.yml:1-39

**Use Cases:**
- Processing hundreds of files in parallel
- Running tests across large codebases
- Batch data processing with aggregation
- Distributed code analysis

**When to Use MapReduce vs Foreach:**
- **MapReduce**: 50+ items, need isolation, resumable, complex aggregation
- **Foreach**: <50 items, simple iteration, linear processing acceptable

See the [MapReduce](../mapreduce/index.md) chapter for comprehensive details.

### Parallel Iteration with Foreach

For simpler parallel iteration over small lists:

```yaml
- foreach: "ls *.txt"
  parallel: 5
  do:
    - shell: "process-file ${item}"
      capture_output: "result_${item}"
```

**Source**: src/config/command.rs:191-211 (ForeachConfig)

**Note**: The foreach feature exists in the configuration but has limited examples. For robust parallel processing with work isolation and resume capabilities, prefer MapReduce patterns.

**Use Cases:**
- Processing small batches (<50 items)
- Simple transformations without complex aggregation
- Quick parallel operations in standard workflows

### Gradual Quality Improvement

Iteratively improve code quality until threshold is met:

```yaml
- goal_seek:
    goal: "Code quality score above 90"
    shell: "auto-improve.sh"
    validate: "quality-check.sh"
    threshold: 90
    max_attempts: 5
  commit_required: true
```

**Source**: src/cook/goal_seek/mod.rs:14-41 (GoalSeekConfig), workflows/goal-seeking-examples.yml:29-43

**How It Works:**
1. Executes the improvement command (`auto-improve.sh`)
2. Runs validation command (`quality-check.sh`)
3. Validation must output `score: N` where N is 0-100
4. If score >= threshold (90), goal is achieved
5. Otherwise, repeats up to max_attempts times
6. Creates commit when goal is achieved (due to `commit_required: true`)

**Validation Output Format** (src/cook/goal_seek/validator.rs:37-62):

Your validation command must output one of these formats:
```bash
score: 85           # Primary format (recommended)
85%                 # Percentage format
85/100              # Fraction format
85 out of 100       # Verbose format
```

Or JSON format:
```json
{
  "score": 85,
  "gaps": ["missing tests", "undocumented functions"]
}
```

**Real Example** (workflows/goal-seeking-examples.yml:6-14):
```yaml
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/prodigy-coverage --improve"
    validate: "cargo tarpaulin --print-summary 2>/dev/null | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
  commit_required: true
```

### Conditional Deployment

Deploy only when all tests pass:

```yaml
- shell: "cargo test --format json"
  capture_output: "test_results"
  capture_format: "json"  # Required to parse output as JSON for field access

- shell: "deploy.sh"
  when: "${test_results.passed == test_results.total}"
  on_success:
    shell: "notify-success.sh"
  on_failure:
    shell: "rollback.sh"
```

**Source**: src/config/command.rs:WorkflowStepCommand.when field, src/config/capture.rs

**Pattern Components:**
- Capture test results as JSON
- `capture_format: "json"` is **required** to enable JSON field access in `when` clauses
- Deploy only if all tests passed (conditional execution via `when`)
- Send notification on success
- Rollback on failure

**Important**: Without `capture_format: "json"`, the `when` clause cannot access object fields like `test_results.passed`. The output would be treated as a plain string.

### Multi-Stage Validation

Validate through multiple quality gates:

```yaml
- claude: "/implement-feature"
  validate:
    result_file: ".prodigy/validation-result.json"
    threshold: 100
    on_incomplete:
      claude: "/fix-validation-gaps"
      max_attempts: 3
      commit_required: true
```

**Source**: src/cook/workflow/validation.rs:11-49 (ValidationConfig), workflows/spec.yml:12-19

**How Validation Works:**

The validation command must write a JSON file with this schema (src/cook/workflow/validation.rs:216-239):

```json
{
  "completion_percentage": 85.0,
  "status": "Incomplete",
  "implemented": ["feature_a", "feature_b"],
  "missing": ["feature_c"],
  "gaps": {
    "feature_c": {
      "description": "Missing error handling",
      "location": "src/main.rs:45",
      "severity": "High",
      "suggested_fix": "Add try-catch block"
    }
  }
}
```

**Required Fields:**
- `completion_percentage`: 0-100 (compared against threshold)
- `status`: "Complete" | "Incomplete" | "Failed" | "Skipped"
- `gaps`: Object mapping gap IDs to GapDetail objects

**If completion_percentage < threshold:**
- `on_incomplete` commands execute
- Validation variables available: `${validation.gaps}`, `${validation.incomplete_specs}`
- Process repeats up to `max_attempts` times

**Real Example** (workflows/spec.yml:12-19):
```yaml
validate:
  result_file: ".prodigy/spec-validation.json"
  threshold: 100
  on_incomplete:
    claude: "/prodigy-refine-specs ${validation.incomplete_specs} --gaps ${validation.gaps}"
    max_attempts: 5
    commit_required: true
```

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

### Environment Profile Pattern

Use environment profiles for multi-environment workflows:

```yaml
env:
  DATABASE_URL:
    default: "postgres://localhost/dev"
    staging: "postgres://staging-server/db"
    prod: "postgres://prod-server/db"

  API_KEY:
    secret: true
    value: "${API_KEY_FROM_ENV}"

- shell: "migrate-database"
  env:
    DB_URL: "${DATABASE_URL}"

- shell: "deploy-app"
  when: "${profile == 'prod'}"
```

**Source**: Environment variables specification (Spec 120), src/config/environment.rs

**Key Features:**
- **Profile-specific values**: Different configurations per environment (dev/staging/prod)
- **Secret masking**: Mark sensitive values with `secret: true` to mask in logs
- **Activation**: Run with `prodigy run workflow.yml --profile prod`

**Use Cases:**
- Multi-environment deployments
- Secrets management
- Environment-specific behavior

See [Environment Variables](../configuration/environment-variables.md) for comprehensive details.

### Circuit Breaker and DLQ Retry

Handle failures gracefully with Dead Letter Queue retry:

```yaml
# Initial MapReduce workflow with automatic DLQ for failures
name: resilient-processing
mode: mapreduce

map:
  input: "items.json"
  json_path: "$[*]"

  agent_template:
    - shell: "process-item ${item.id}"
      timeout: 60
      on_failure:
        # Failures automatically go to DLQ
        shell: "log-failure ${item.id}"

  max_parallel: 10
```

**After initial run, retry failed items:**

```bash
# Retry all failed items from the job
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 5

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run
```

**Source**: MapReduce DLQ implementation, src/mapreduce/dlq/mod.rs

**How It Works:**
1. Failed work items automatically go to Dead Letter Queue
2. DLQ stores failure reason, timestamp, and retry count
3. Use `prodigy dlq retry` to reprocess failed items
4. Supports partial success (some items succeed, others remain in DLQ)

**Use Cases:**
- Handling transient failures (network issues, timeouts)
- Incremental retry of failed operations
- Production resilience patterns

See [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) for comprehensive details.

## See Also

- [Goal-Seeking Operations](goal-seeking-operations.md) - Deep dive into goal_seek patterns
- [Implementation Validation](implementation-validation.md) - Validation command details
- [MapReduce](../mapreduce/index.md) - Comprehensive parallel processing guide
- [Environment Variables](../configuration/environment-variables.md) - Environment configuration
- [Error Handling](../error-handling.md) - Comprehensive error handling strategies
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) - DLQ and retry patterns
