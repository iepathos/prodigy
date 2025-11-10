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

Provide automatic remediation for common failure scenarios using `on_failure` handlers:

```yaml
# Provide automatic remediation
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
    max_retries: 2
    fail_workflow: false
```

**Source**: `on_failure` configuration from src/cook/workflow/on_failure.rs:68-115

**Available Fields:**
- `claude` or `shell`: Command to execute on failure
- `max_retries` (alias: `max_attempts`): Number of retry attempts
- `fail_workflow`: Whether to fail the workflow after handler execution (default: false)
- `retry_original`: Whether to retry the original command after handler

**Consider:**
- Whether to fail the workflow or continue
- How many retry attempts are reasonable
- Use [retry configuration](../retry-configuration/index.md) for advanced backoff strategies
- See [error handling](../error-handling.md) for comprehensive error management

### 4. Validate Critical Changes with Automatic Gap Filling

Ensure implementations meet requirements before proceeding, with automatic remediation for incomplete implementations:

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
      fail_workflow: true
      commit_required: true
```

**Source**: Validation with `on_incomplete` from src/cook/workflow/validation.rs:122-152

**Key Features:**
- **`threshold`**: Percentage (0-100) of validation criteria that must pass
- **`on_incomplete`**: Handler executed when validation score < threshold
- **`max_attempts`**: Maximum retry attempts for gap filling (default: 2)
- **`fail_workflow`**: Whether to fail after max attempts (default: true)
- **`commit_required`**: Whether handler must create git commits

**Multi-Command Recovery:**
```yaml
validate:
  commands:
    - shell: "cargo test"
    - shell: "debtmap analyze . --output after.json"
  threshold: 100
  on_incomplete:
    commands:
      - claude: "/fix-test-failures"
        commit_required: true
      - shell: "cargo fmt"
      - shell: "cargo clippy --fix --allow-dirty"
    max_attempts: 5
    fail_workflow: false
```

**Best Practices:**
- Validate after significant changes
- Use multiple validation steps for comprehensive coverage
- Set appropriate thresholds based on criticality (100 for critical paths, 80-90 for development)
- Use `on_incomplete` for automatic remediation instead of manual intervention
- Enable `commit_required: true` when fixes should be preserved
- See [implementation validation](implementation-validation.md) for detailed examples

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
- Ensure items are truly independent (no shared state or file conflicts)
- Test with sequential execution first (`parallel: 1`) to verify correctness
- Set appropriate parallel limits based on resources (CPU cores, memory)
- Use `continue_on_error` for fault tolerance
- Consider timeout implications with parallel execution

**Verifying Independence:**
```yaml
# STEP 1: Test with sequential execution
- foreach:
    foreach: ${modules}
    parallel: 1  # Start with sequential
    do:
      - shell: "test-module.sh ${item}"

# STEP 2: After verifying correctness, increase parallelism
- foreach:
    foreach: ${modules}
    parallel: 5  # Scale up after validation
    do:
      - shell: "test-module.sh ${item}"
```

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

### 8. Use Git Context Variables for Change-Aware Workflows

Leverage git context variables to make workflows respond intelligently to changes:

```yaml
# Run tests only on changed files
- shell: "git diff --stat"
  id: "detect-changes"

# Selective test execution based on file changes
- shell: "cargo test"
  when: "${step.files_changed}" != ""

# Pass changed files to linter (using shell for space-to-newline conversion)
- shell: |
    changed_files=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.rs$')
    if [ -n "$changed_files" ]; then
      echo "$changed_files" | xargs cargo clippy --
    fi
```

**Source**: Git context tracking from src/cook/workflow/git_context.rs:86-116

**Available Variables:**

**Step-Level (Current Step):**
- `${step.files_added}`: Files added in current step
- `${step.files_modified}`: Files modified in current step
- `${step.files_deleted}`: Files deleted in current step
- `${step.files_changed}`: All files changed (combined)
- `${step.commits}`: Commit SHAs from current step
- `${step.commit_count}`: Number of commits
- `${step.insertions}`: Lines added
- `${step.deletions}`: Lines deleted

**Workflow-Level (Cumulative):**
- `${workflow.files_added}`: All files added in workflow
- `${workflow.files_modified}`: All files modified in workflow
- `${workflow.files_deleted}`: All files deleted in workflow
- `${workflow.files_changed}`: All files changed in workflow
- `${workflow.commits}`: All commit SHAs
- `${workflow.commit_count}`: Total commits
- `${workflow.insertions}`: Total lines added
- `${workflow.deletions}`: Total lines deleted

**Format Note**: Variables are currently space-separated. Use shell commands for filtering and formatting:

```yaml
# Filter by extension and convert to newlines
- shell: "echo ${step.files_changed} | tr ' ' '\n' | grep '\.rs$'"

# Convert to JSON array
- shell: "echo ${step.files_added} | tr ' ' '\n' | jq -R | jq -s"

# Filter and pass to command
- shell: |
    ts_files=$(echo "${step.files_changed}" | tr ' ' '\n' | grep '\.ts$' | tr '\n' ' ')
    if [ -n "$ts_files" ]; then
      prettier --write $ts_files
    fi
```

See [git context advanced](../git-context-advanced.md) for detailed usage patterns.

### 9. Separate Concerns with Step-Level Environment

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

### 11. Design Idempotent MapReduce Work Items

For MapReduce workflows, design work items that can be safely retried and processed independently:

```yaml
name: tech-debt-elimination
mode: mapreduce

setup:
  - shell: "debtmap analyze . --output debt.json"

map:
  input: debt.json
  json_path: "$.items[*]"

  # Prevent duplicate processing
  distinct: "item.id"

  # Process high-priority items first
  filter: "item.severity == 'critical' || item.severity == 'high'"
  sort_by: "item.priority DESC"

  # Limit scope for initial run
  max_items: 20
  max_parallel: 5

  agent_template:
    - claude: "/fix-debt-item '${item.description}' --id ${item.id}"
      commit_required: true

    # Verify fix with tests
    - shell: "cargo test"
      on_failure:
        claude: "/debug-and-fix"
        max_retries: 2

reduce:
  - shell: "debtmap analyze . --output debt-after.json"
  - claude: "/compare-debt-reports --before debt.json --after debt-after.json"
```

**Source**: MapReduce architecture from src/cook/mapreduce/orchestrator.rs and book/src/mapreduce-worktree-architecture.md

**Key Principles:**

1. **Idempotency**: Use `distinct` field to prevent duplicate processing
   ```yaml
   map:
     distinct: "item.id"  # Deduplicates based on this field
   ```

2. **Work Item Independence**: Each item processes in isolated worktree
   - No shared state between agents
   - Independent git histories
   - Isolated file system changes
   - See [MapReduce worktree architecture](../mapreduce-worktree-architecture.md)

3. **Failure Isolation**: Failed items go to Dead Letter Queue (DLQ)
   ```yaml
   # Default error policy (configurable)
   error_policy:
     on_item_failure: dlq
     continue_on_failure: true
   ```

4. **Retry Strategy**: Configure backoff for transient failures
   ```yaml
   map:
     retry_config:
       attempts: 5
       backoff: exponential
       max_delay: "30s"
       jitter: true  # Prevents thundering herd
   ```

5. **DLQ Management**: Monitor and retry failed items
   ```bash
   # View failed items
   prodigy dlq show <job_id>

   # Retry with parallelism
   prodigy dlq retry <job_id> --max-parallel 10

   # View failure statistics
   prodigy dlq stats <job_id>
   ```

**Testing MapReduce Workflows:**
```yaml
# Start with small scope
map:
  max_items: 5        # Test with 5 items first
  max_parallel: 1     # Sequential to verify correctness

# After validation, scale up
map:
  max_items: 100
  max_parallel: 10    # Increase parallelism
```

**Best Practices:**
- Design work items to be retryable without side effects
- Avoid shared state or file dependencies between items
- Use DLQ for automatic failure collection and retry
- Test with `parallel: 1` before scaling to `parallel: N`
- Monitor DLQ statistics to identify systematic issues
- Enable jitter in retry config to prevent concurrent retry storms
- Use `distinct` to ensure exactly-once processing semantics

See also:
- [MapReduce overview](../mapreduce/index.md)
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md)
- [Retry configuration](../retry-configuration/index.md)

### 12. Monitor Resource Usage in Parallel Workflows

When running parallel operations, monitor system resources to avoid overload:

```yaml
# Conservative parallelism for resource-intensive tasks
- foreach:
    foreach: ${large_modules}
    parallel: 3  # Lower limit for CPU/memory intensive work
    do:
      - shell: "cargo build --release"

# Higher parallelism for I/O-bound tasks
- foreach:
    foreach: ${test_suites}
    parallel: 8  # Higher limit for I/O-bound operations
    do:
      - shell: "npm test"
```

**Guidelines:**
- CPU-bound: `parallel <= CPU cores`
- I/O-bound: `parallel = 1.5-2x CPU cores`
- Memory-intensive: Calculate based on available RAM per process
- Network-bound: Consider rate limits and connection pools
