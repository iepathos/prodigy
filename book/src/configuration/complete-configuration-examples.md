## Complete Configuration Examples

This subsection provides comprehensive, production-ready workflow examples demonstrating all major Prodigy configuration features. Each example is extracted from real workflows in the repository and includes detailed annotations explaining configuration choices.

### Quick Reference

Complete workflow configurations include:

| Feature | Standard Workflow | MapReduce Workflow |
|---------|------------------|-------------------|
| **Basic Structure** | `commands: []` | `mode: mapreduce` with `setup`, `map`, `reduce` |
| **Environment Variables** | `env:`, `secrets:`, `profiles:` | Same + phase-specific overrides |
| **Command Types** | `claude:`, `shell:`, `goal_seek:`, `foreach:`, `write_file:` | Same + `agent_template` |
| **Error Handling** | `on_failure:`, `on_success:`, `retry:` | Same + `error_policy:`, `on_item_failure:` |
| **Validation** | `validate:` with `threshold`, `on_incomplete` | Per-step validation + gap filling |
| **Output Capture** | `capture_output:`, `outputs:` | `capture_outputs:` in setup phase |
| **Timeouts** | `timeout:` per command | `timeout:` per phase + `agent_timeout_secs` |
| **Merge Workflow** | `merge:` with custom commands | Same with `${merge.*}` variables |

---

### 1. Complete Standard Workflow Example

This example demonstrates a full standard workflow with all major configuration options.

**Source**: workflows/debtmap.yml (lines 1-56)

```yaml
# Sequential workflow for technical debt analysis and remediation
# Demonstrates: validation, goal-seeking, error handlers, output capture

# Phase 1: Generate coverage data
- shell: "just coverage-lcov"
  timeout: 300

# Phase 2: Analyze tech debt and capture baseline
- shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-before.json --format json"
  capture_output: true

# Phase 3: Create implementation plan with validation
- claude: "/prodigy-debtmap-plan --before .prodigy/debtmap-before.json --output .prodigy/IMPLEMENTATION_PLAN.md"
  commit_required: true
  validate:
    commands:
      - claude: "/prodigy-validate-debtmap-plan --before .prodigy/debtmap-before.json --plan .prodigy/IMPLEMENTATION_PLAN.md --output .prodigy/plan-validation.json"
    result_file: ".prodigy/plan-validation.json"
    threshold: 75  # Must achieve 75% completeness
    on_incomplete:
      commands:
        - claude: "/prodigy-revise-debtmap-plan --gaps ${validation.gaps} --plan .prodigy/IMPLEMENTATION_PLAN.md"
      max_attempts: 3
      fail_workflow: false

# Phase 4: Execute the plan with comprehensive validation
- claude: "/prodigy-debtmap-implement --plan .prodigy/IMPLEMENTATION_PLAN.md"
  commit_required: true
  validate:
    commands:
      - shell: "just coverage-lcov"
      - shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-after.json --format json"
      - shell: "debtmap compare --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --plan .prodigy/IMPLEMENTATION_PLAN.md --output .prodigy/comparison.json --format json"
      - claude: "/prodigy-validate-debtmap-improvement --comparison .prodigy/comparison.json --output .prodigy/debtmap-validation.json"
    result_file: ".prodigy/debtmap-validation.json"
    threshold: 75
    on_incomplete:
      commands:
        - claude: "/prodigy-complete-debtmap-fix --plan .prodigy/IMPLEMENTATION_PLAN.md --validation .prodigy/debtmap-validation.json --attempt ${validation.attempt_number}"
          commit_required: true
        - shell: "just coverage-lcov"
        - shell: "debtmap analyze . --lcov target/coverage/lcov.info --output .prodigy/debtmap-after.json --format json"
        - shell: "debtmap compare --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --plan .prodigy/IMPLEMENTATION_PLAN.md --output .prodigy/comparison.json --format json"
      max_attempts: 5
      fail_workflow: true

# Phase 5: Verify tests pass with error recovery
- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure --output ${shell.output}"
    max_attempts: 5
    fail_workflow: true

# Phase 6: Enforce code quality standards
- shell: "just fmt-check && just lint"
  on_failure:
    claude: "/prodigy-lint ${shell.output}"
    max_attempts: 5
    fail_workflow: true
```

**Key Features Demonstrated**:
- **Validation with gap filling**: `validate:` block with `threshold` and `on_incomplete` handler
- **Error recovery**: `on_failure:` handlers with `max_attempts` for automatic fixing
- **Output capture**: Shell output captured and passed to Claude for debugging
- **Commit control**: `commit_required: true` ensures changes are tracked
- **Timeouts**: Per-command timeout to prevent hanging
- **Sequential orchestration**: Each phase builds on previous results

**Configuration Details** (from src/config/command.rs:WorkflowStepCommand):
- `commit_required: bool` - Whether step must create a git commit (default: false)
- `timeout: u64` - Maximum execution time in seconds
- `validate: ValidationConfig` - Validation specification with threshold and handlers
- `on_failure: TestDebugConfig` - Error handler with max_attempts and fail_workflow
- `capture_output: bool` - Capture command output for use in subsequent steps

---

### 2. Complete MapReduce Workflow Example

This example demonstrates a production MapReduce workflow with all phases and configuration options.

**Source**: workflows/book-docs-drift.yml (lines 1-101)

```yaml
name: prodigy-book-docs-drift-detection
mode: mapreduce

# Global environment configuration
env:
  # Project configuration
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/book-config.json"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"

  # Book-specific settings
  BOOK_DIR: "book"
  ANALYSIS_DIR: ".prodigy/book-analysis"
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"

  # Workflow settings
  MAX_PARALLEL: "3"

# Setup phase: Analyze codebase and prepare work items
setup:
  - shell: "mkdir -p $ANALYSIS_DIR"

  # Step 1: Analyze codebase features
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

  # Step 2: Detect gaps and generate work items
  - claude: "/prodigy-detect-documentation-gaps --project $PROJECT_NAME --config $PROJECT_CONFIG --features $FEATURES_PATH --chapters $CHAPTERS_FILE --book-dir $BOOK_DIR"

# Map phase: Process each documentation subsection in parallel
map:
  input: "${ANALYSIS_DIR}/flattened-items.json"
  json_path: "$[*]"

  agent_template:
    # Step 1: Analyze subsection for drift
    - claude: "/prodigy-analyze-subsection-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true

    # Step 2: Fix drift with validation
    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --json '${item}'"
      commit_required: true
      validate:
        claude: "/prodigy-validate-doc-fix --project $PROJECT_NAME --json '${item}' --output .prodigy/validation-result.json"
        result_file: ".prodigy/validation-result.json"
        threshold: 100  # Documentation must meet 100% quality standards
        on_incomplete:
          claude: "/prodigy-complete-doc-fix --project $PROJECT_NAME --json '${item}' --gaps ${validation.gaps}"
          max_attempts: 3
          fail_workflow: false
          commit_required: true

  max_parallel: ${MAX_PARALLEL}

# Reduce phase: Aggregate results and validate build
reduce:
  # Rebuild the book to ensure all chapters compile
  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
      commit_required: true

  # Clean up temporary analysis files
  - shell: "rm -rf ${ANALYSIS_DIR}"
  - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files for ${PROJECT_NAME}' || true"

# Error handling policy
error_policy:
  on_item_failure: dlq          # Send failures to Dead Letter Queue
  continue_on_failure: true     # Don't stop on individual item failures
  max_failures: 2               # Stop if more than 2 items fail
  error_collection: aggregate   # Collect errors for batch reporting

# Custom merge workflow
merge:
  commands:
    - shell: "git fetch origin"
    - claude: "/prodigy-merge-master --project ${PROJECT_NAME}"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Key Features Demonstrated**:
- **Environment parameterization**: All paths and settings in `env:` block for easy customization
- **Setup phase**: Generate work items before parallel processing
- **Agent template**: Commands execute in isolation per work item
- **Work item access**: `${item}` variable provides access to current item fields
- **Parallel execution**: `max_parallel` controls concurrency (can reference env vars)
- **Validation with gap filling**: Automatic quality improvement until threshold met
- **Error policy**: Comprehensive failure handling with DLQ and thresholds
- **Merge workflow**: Custom merge process with branch variables

**MapReduce Configuration Details** (from src/config/mapreduce.rs):

**SetupPhaseConfig**:
- `commands: Vec<WorkflowStep>` - Commands to execute during setup
- `timeout: Option<String>` - Phase timeout (supports env var references like `"$TIMEOUT"`)
- `capture_outputs: HashMap<String, CaptureConfig>` - Variables to capture from setup

**MapPhaseYaml**:
- `input: String` - Path to work items JSON or command to generate items
- `json_path: String` - JSONPath expression to extract items (default: `""` for array root)
- `agent_template: AgentTemplate` - Commands to execute per item
- `max_parallel: String` - Concurrency limit (supports env vars like `"${MAX_PARALLEL}"`)
- `filter: Option<String>` - Filter expression (e.g., `"item.priority >= 5"`)
- `sort_by: Option<String>` - Sort field with direction (`"item.priority DESC"`)
- `max_items: Option<usize>` - Limit number of items to process
- `offset: Option<usize>` - Skip first N items
- `agent_timeout_secs: Option<String>` - Per-agent timeout (supports env vars)

**Error Policy** (from src/cook/workflow/error_policy.rs:WorkflowErrorPolicy):
- `on_item_failure: ItemFailureAction` - Action on failure: `dlq`, `retry`, `skip`, `stop` (default: `dlq`)
- `continue_on_failure: bool` - Continue processing after failures (default: `true`)
- `max_failures: Option<usize>` - Stop after N failures
- `failure_threshold: Option<f64>` - Stop if failure rate exceeds threshold (0.0 to 1.0)
- `error_collection: ErrorCollectionStrategy` - Collection mode: `aggregate`, `immediate`, `batched` (default: `aggregate`)

**Merge Workflow Variables**:
- `${merge.worktree}` - Worktree name being merged
- `${merge.source_branch}` - Source branch (worktree branch)
- `${merge.target_branch}` - Target branch (original branch)
- `${merge.session_id}` - Session ID for correlation

---

### 3. Environment Variables and Secrets Example

This example demonstrates comprehensive environment configuration with static variables, dynamic values, secrets, and profiles.

**Source**: workflows/environment-example.yml (lines 1-70)

```yaml
# Global environment configuration
env:
  # Static environment variables
  NODE_ENV: production
  API_URL: https://api.example.com

  # Dynamic environment variable (computed from command)
  WORKERS:
    command: "nproc 2>/dev/null || echo 4"
    cache: true  # Cache the result for workflow duration

  # Conditional environment variable (based on git branch)
  DEPLOY_ENV:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"

# Secret environment variables (masked in logs)
secrets:
  # Reference to environment variable
  API_KEY: "${env:SECRET_API_KEY}"

# Environment files to load (.env format)
env_files:
  - .env.production

# Environment profiles for different contexts
profiles:
  development:
    NODE_ENV: development
    API_URL: http://localhost:3000
    DEBUG: "true"

  testing:
    NODE_ENV: test
    API_URL: http://localhost:4000
    COVERAGE: "true"

# Workflow steps demonstrating environment features
commands:
  - name: "Show environment"
    shell: "echo NODE_ENV=$NODE_ENV API_URL=$API_URL WORKERS=$WORKERS"
    capture_output: true

  - name: "Build frontend"
    shell: "echo 'Building frontend with NODE_ENV='$NODE_ENV"
    env:
      BUILD_TARGET: production  # Step-specific environment override
      OPTIMIZE: "true"
    working_dir: ./frontend

  - name: "Run tests"
    shell: "echo 'Running tests in test environment'"
    env:
      PYTHONPATH: "./src:./tests"
      TEST_ENV: "true"
    working_dir: ./backend
    temporary: true  # Environment restored after this step

  - name: "Deploy application"
    shell: "echo 'Deploying to '$DEPLOY_ENV' environment'"
    working_dir: "${env.DEPLOY_DIR}"

  - name: "Cleanup"
    shell: "echo 'Cleaning up temporary files'"
    clear_env: true  # Clear all environment variables except step-specific
    env:
      CLEANUP_MODE: "full"
```

**Environment Configuration Details** (from src/cook/environment/config.rs):

**EnvValue Types**:
- **Static**: Simple string value
- **Dynamic**: Computed from command with optional caching
  - `command: String` - Command to execute for value
  - `cache: bool` - Cache result (default: false)
- **Conditional**: Value based on expression evaluation
  - `condition: String` - Expression to evaluate
  - `when_true: String` - Value when condition is true
  - `when_false: String` - Value when condition is false

**Secret Management**:
- Marked with `secret: true` or defined in `secrets:` block
- Automatically masked in logs, error messages, and event streams
- Supports environment variable references: `"${env:VAR_NAME}"`

**Profile Usage**:
```bash
# Activate a profile at runtime
prodigy run workflow.yml --profile development
prodigy run workflow.yml --profile testing
```

**Step-Level Environment** (from src/config/command.rs:WorkflowStepCommand):
- `env: HashMap<String, String>` - Step-specific environment variables
- `working_dir: Option<PathBuf>` - Working directory for this step
- `temporary: bool` - Restore environment after step (default: false)
- `clear_env: bool` - Clear parent environment before applying step env (default: false)

---

### 4. Error Handling and Retry Strategies Example

This example demonstrates comprehensive error handling patterns including retry strategies, backoff configurations, and circuit breakers.

**Source**: workflows/implement-with-tests.yml (lines 1-79) and workflows/debtmap.yml

```yaml
# Nested error handling with automatic recovery
commands:
  # Step 1: Implement specification
  - claude: "/prodigy-implement-spec $ARG"
    analysis:
      max_cache_age: 300

  # Step 2: Run tests with nested error recovery
  - shell: "cargo test"
    capture_output: "test_output"
    commit_required: false
    on_failure:
      # First attempt: Debug test failures
      claude: "/prodigy-debug-test-failures '${test_output}'"
      commit_required: true
      on_success:
        # Verify fixes work
        shell: "cargo test"
        commit_required: false
        on_failure:
          # Second attempt: Deep analysis if still failing
          claude: "/prodigy-fix-test-failures '${shell.output}' --deep-analysis"
          commit_required: true

  # Step 3: Run linting
  - claude: "/prodigy-lint"
    commit_required: false

  # Step 4: Run benchmarks (non-critical)
  - shell: "cargo bench --no-run"
    commit_required: false
    on_failure:
      shell: "echo 'Skipping benchmarks due to compilation issues'"
      commit_required: false

  # Step 5: Final verification with status reporting
  - shell: "cargo test --release"
    capture_output: "final_test_results"
    commit_required: false
    on_failure:
      # Report persistent failures
      claude: "/prodigy-report-test-status failed '${final_test_results}' --notify"
      commit_required: false
    on_success:
      shell: "echo '✅ All tests passing! Implementation complete.'"
      commit_required: false
```

**Error Handler Configuration** (from src/config/command.rs:TestDebugConfig):
- `claude: String` - Command to run on failure
- `max_attempts: u32` - Maximum retry attempts (default: 3)
- `fail_workflow: bool` - Stop workflow if max attempts exceeded (default: false)
- `commit_required: bool` - Whether handler must create commits (default: true)

**Backoff Strategy Types** (from src/cook/workflow/error_policy.rs:BackoffStrategy):

```yaml
# Fixed delay between retries
retry:
  backoff:
    fixed:
      delay: 5s

# Linear backoff (delay increases linearly)
retry:
  backoff:
    linear:
      initial: 1s
      increment: 2s

# Exponential backoff (default: 2x multiplier)
retry:
  backoff:
    exponential:
      initial: 1s
      multiplier: 2.0

# Fibonacci sequence delays
retry:
  backoff:
    fibonacci:
      initial: 1s
```

**Circuit Breaker Configuration** (from src/cook/workflow/error_policy.rs:CircuitBreakerConfig):

```yaml
error_policy:
  circuit_breaker:
    failure_threshold: 5       # Open circuit after 5 failures
    success_threshold: 3       # Close after 3 successes
    timeout: 30s              # Time before attempting to close
    half_open_requests: 3     # Requests allowed in half-open state
```

---

### 5. Goal-Seeking and Validation Examples

This example demonstrates iterative refinement with validation and automatic gap filling.

**Source**: workflows/goal-seeking-examples.yml (lines 1-129)

```yaml
# Example 1: Test Coverage Improvement
- goal_seek:
    goal: "Achieve 90% test coverage"
    claude: "/prodigy-coverage --improve"
    validate: "cargo tarpaulin --print-summary 2>/dev/null | grep 'Coverage' | sed 's/.*Coverage=\\([0-9]*\\).*/score: \\1/'"
    threshold: 90
    max_attempts: 5
    timeout_seconds: 300
    fail_on_incomplete: true
  commit_required: true

# Example 2: Performance Optimization
- goal_seek:
    goal: "Optimize algorithm performance to under 100ms"
    claude: "/optimize-performance --target 100ms"
    validate: "cargo bench --bench main_bench 2>/dev/null | grep 'time:' | awk '{if ($2 < 100) print \"score: 95\"; else print \"score:\", int(10000/$2)}'"
    threshold: 90
    max_attempts: 4
    timeout_seconds: 600
  commit_required: true

# Example 3: Code Quality with Custom Validation
- goal_seek:
    goal: "Fix all clippy warnings and improve code quality"
    claude: "/fix-clippy-warnings"
    validate: |
      warnings=$(cargo clippy 2>&1 | grep -c warning || echo 0)
      if [ "$warnings" -eq 0 ]; then
        echo "score: 100"
      else
        score=$((100 - warnings * 5))
        echo "score: $score"
      fi
    threshold: 95
    max_attempts: 3
    fail_on_incomplete: false
  commit_required: true

# Example 4: Multi-stage Goal Seeking
- name: "Complete feature implementation with quality checks"
  goal_seek:
    goal: "Implement user profile feature"
    claude: "/implement-feature user-profile"
    validate: "test -f src/features/user_profile.rs && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 2

- name: "Add comprehensive tests"
  goal_seek:
    goal: "Add tests for user profile feature"
    claude: "/add-tests src/features/user_profile.rs"
    validate: |
      test_count=$(grep -c "#\\[test\\]" src/features/user_profile.rs || echo 0)
      if [ "$test_count" -ge 5 ]; then
        echo "score: 100"
      else
        score=$((test_count * 20))
        echo "score: $score"
      fi
    threshold: 100
    max_attempts: 3

- name: "Ensure tests pass"
  goal_seek:
    goal: "Make all user profile tests pass"
    claude: "/fix-tests user_profile"
    validate: "cargo test user_profile 2>&1 | grep -q 'test result: ok' && echo 'score: 100' || echo 'score: 0'"
    threshold: 100
    max_attempts: 4
    fail_on_incomplete: true
```

**Goal-Seeking Configuration** (from src/cook/goal_seek/mod.rs):
- `goal: String` - Human-readable description of the goal
- `claude: String` - Claude command to execute for improvement
- `validate: String` - Shell command that outputs "score: N" (0-100)
- `threshold: u32` - Minimum score required for success (0-100)
- `max_attempts: u32` - Maximum refinement iterations
- `timeout_seconds: u64` - Maximum time for all attempts
- `fail_on_incomplete: bool` - Fail workflow if threshold not reached

**Validation Output Format**:
The validation command must output a single line with the score:
```
score: 85
```

The score should be between 0 and 100, where 100 indicates complete success.

---

### 6. Foreach Parallel Iteration Example

This example demonstrates parallel iteration over work items with different input sources and concurrency controls.

**Configuration Details** (from src/config/command.rs:ForeachConfig):

```yaml
# Static list input with parallel execution
- foreach:
    input: ["file1.rs", "file2.rs", "file3.rs"]
    parallel: 3  # Process 3 files concurrently
    do:
      - claude: "/lint ${item}"
      - shell: "rustfmt ${item}"
    continue_on_error: true  # Don't stop on individual item failures

# Command input (output becomes items)
- foreach:
    input:
      command: "find src -name '*.rs' -type f"
    parallel: 5
    do:
      - claude: "/analyze ${item}"
      - shell: "cargo check --file ${item}"
    max_items: 50  # Limit to first 50 files

# Sequential execution (no parallelism)
- foreach:
    input: ["step1", "step2", "step3"]
    parallel: false  # Execute sequentially
    do:
      - claude: "/execute-step ${item}"
```

**ForeachConfig Structure**:
- `input: ForeachInput` - Source of items (command or static list)
  - `command: String` - Command whose output (one item per line) becomes items
  - `list: Vec<String>` - Static list of items
- `parallel: ParallelConfig` - Concurrency control
  - `boolean: bool` - Enable/disable parallelism (true = default count, false = sequential)
  - `count: usize` - Specific number of concurrent items
- `do: Vec<WorkflowStepCommand>` - Commands to execute per item
- `continue_on_error: bool` - Continue if individual item fails (default: false)
- `max_items: Option<usize>` - Limit number of items to process

**Item Access**:
- Use `${item}` to reference the current item in commands
- Each iteration runs in a clean environment
- Failures are isolated to individual items

---

### 7. Write File Command Example

This example demonstrates the `write_file` command for generating files during workflow execution.

**Configuration Details** (from src/config/command.rs:WriteFileConfig):

```yaml
# Write plain text file
- write_file:
    path: "reports/summary.txt"
    content: |
      Workflow Summary
      ================
      Project: ${PROJECT_NAME}
      Completed: ${map.successful}/${map.total} items
      Duration: ${workflow.duration}
    format: text
    create_dirs: true  # Create parent directories if needed

# Write JSON file with validation
- write_file:
    path: "config/generated.json"
    content: |
      {
        "version": "${VERSION}",
        "timestamp": "${timestamp}",
        "items_processed": ${map.total}
      }
    format: json  # Validates JSON syntax and pretty-prints
    mode: "0644"

# Write YAML configuration
- write_file:
    path: "config/deploy.yml"
    content: |
      environment: ${DEPLOY_ENV}
      version: ${VERSION}
      features:
        - feature1
        - feature2
    format: yaml  # Validates YAML syntax and formats
    create_dirs: true
```

**WriteFileConfig Structure**:
- `path: String` - File path (supports variable interpolation)
- `content: String` - Content to write (supports variable interpolation)
- `format: WriteFileFormat` - Output format (default: `text`)
  - `text` - Plain text (no processing)
  - `json` - JSON with validation and pretty-printing
  - `yaml` - YAML with validation and formatting
- `mode: String` - File permissions in octal format (default: `"0644"`)
- `create_dirs: bool` - Create parent directories (default: false)

---

### 8. Advanced Timeout Configuration Example

This example demonstrates timeout configuration at multiple levels.

**Configuration Details**:

```yaml
# Global timeout for workflow
timeout: 3600  # 1 hour for entire workflow

commands:
  # Command-level timeout
  - shell: "long-running-task.sh"
    timeout: 600  # 10 minutes for this command

# MapReduce with phase-specific timeouts
setup:
  - shell: "setup-task.sh"
  timeout: 300  # 5 minutes for setup phase

map:
  agent_template:
    - claude: "/process ${item}"
      timeout: 180  # 3 minutes per command
  agent_timeout_secs: 600  # 10 minutes total per agent
  timeout_config:
    total_timeout_secs: 3600     # Max time for entire map phase
    idle_timeout_secs: 300       # Kill agent if idle for 5 minutes
    per_item_timeout_secs: 180   # Max time per work item

merge:
  commands:
    - claude: "/merge"
  timeout: 600  # 10 minutes for merge phase
```

**Timeout Hierarchy** (most specific wins):
1. Command-level `timeout:` - Per command
2. Agent-level `agent_timeout_secs:` - Per MapReduce agent
3. Phase-level `timeout:` - Per workflow phase (setup, reduce, merge)
4. Global-level `timeout:` - Entire workflow

---

### Cross-References

For more detailed information on specific features:

- **Workflow Structure**: See [../workflow-basics/full-workflow-structure.md](../workflow-basics/full-workflow-structure.md)
- **Environment Variables**: See [environment-variables.md](environment-variables.md)
- **Error Handling**: See [../error-handling.md](../error-handling.md)
- **MapReduce Basics**: See [../mapreduce/index.md](../mapreduce/index.md)
- **Validation**: See [../advanced/implementation-validation.md](../advanced/implementation-validation.md)
- **Goal Seeking**: See [../advanced/goal-seeking-operations.md](../advanced/goal-seeking-operations.md)

---
