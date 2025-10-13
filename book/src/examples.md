# Examples

## Example 1: Simple Build and Test

```yaml
- shell: "cargo build"
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
- shell: "cargo clippy"
```

---

## Example 2: Coverage Improvement with Goal Seeking

```yaml
- goal_seek:
    goal: "Achieve 80% test coverage"
    claude: "/improve-coverage"  # Can also use 'shell' for shell commands
    validate: |
      coverage=$(cargo tarpaulin | grep 'Coverage' | sed 's/.*: \([0-9.]*\)%.*/\1/')
      echo "score: ${coverage%.*}"
    threshold: 80
    max_attempts: 5
```

**Note:** The `goal_seek` command will automatically commit changes made by the Claude command. Commit behavior is controlled by the command execution, not by the `goal_seek` configuration.

---

## Example 3: Foreach Iteration

```yaml
# Test multiple configurations in sequence
- foreach:
    - rust-version: "1.70"
      profile: debug
    - rust-version: "1.71"
      profile: release
    - rust-version: "stable"
      profile: release
  do:
    - shell: "rustup install ${foreach.item.rust-version}"
    - shell: "cargo +${foreach.item.rust-version} build --profile ${foreach.item.profile}"
    - shell: "cargo +${foreach.item.rust-version} test"

# Parallel foreach with error handling
- foreach:
    - "web-service"
    - "api-gateway"
    - "worker-service"
  parallel: 3
  continue_on_error: true
  do:
    - shell: "cd services/${foreach.item} && cargo build"
    - shell: "cd services/${foreach.item} && cargo test"
      on_failure:
        claude: "/fix-service-tests ${foreach.item}"
```

---

## Example 4: Parallel Code Review

```yaml
name: parallel-code-review
mode: mapreduce

setup:
  - shell: "find src -name '*.rs' > files.txt"
  - shell: "jq -R -s -c 'split(\"\n\") | map(select(length > 0) | {path: .})' files.txt > items.json"

map:
  input: items.json
  json_path: "$[*]"  # Process all items in root array
  agent_template:
    - claude: "/review-file ${item.path}"
      id: "review"
      capture_output: "review_result"
      capture_format: "json"
    - shell: "cargo check ${item.path}"
  max_parallel: 5

reduce:
  - claude: "/summarize-reviews ${map.results}"
```

**Note:** JSONPath `"$[*]"` matches all items in the root array. Since the setup phase creates an array of `{path: ...}` objects, each map agent receives an `item` object with `item.path` available for use in commands.

---

## Example 5: Conditional Deployment

```yaml
- shell: "cargo test --quiet && echo true || echo false"
  id: "test"
  capture: "test_result"
  capture_format: "boolean"  # Supported: string, json, lines, number, boolean
  timeout: 300  # Timeout in seconds (5 minutes)

- shell: "cargo build --release"
  when: "${test_result} == true"

- shell: "docker build -t myapp ."
  when: "${test_result} == true"
  on_success:
    shell: "docker push myapp:latest"
```

---

## Example 6: Multi-Step Validation

```yaml
- claude: "/implement-feature auth"
  commit_required: true
  validate:
    commands:
      - shell: "cargo test auth"
      - shell: "cargo clippy -- -D warnings"
      - claude: "/validate-implementation --output validation.json"
    result_file: "validation.json"
    threshold: 90
    on_incomplete:
      claude: "/complete-gaps ${validation.gaps}"
      commit_required: true
      max_attempts: 2
```

---

## Example 7: Environment-Aware Workflow

```yaml
# Global environment variables
env:
  NODE_ENV: production
  API_URL: https://api.production.com

# Secrets (masked in logs)
secrets:
  API_KEY:
    value: "${SECRET_API_KEY}"
    secret: true

# Environment profiles for different contexts
profiles:
  production:
    env:
      API_URL: https://api.production.com
      LOG_LEVEL: error

  staging:
    env:
      API_URL: https://api.staging.com
      LOG_LEVEL: warn

# Load additional variables from .env files
# Note: Paths are relative to workflow file location
env_files:
  - .env
  - .env.production

# Workflow steps (no 'commands' wrapper in simple format)
- shell: "cargo build --release"

# Use environment variables in commands
- shell: "echo 'Deploying to ${NODE_ENV} at ${API_URL}'"

# Override environment for specific step using env field
- shell: "./deploy.sh"
  env:
    LOG_LEVEL: debug
```

**Note:** Profiles are activated using the `--profile <name>` CLI flag when running workflows. For example: `prodigy run workflow.yml --profile production`

---

## Example 8: Complex MapReduce with Error Handling

```yaml
name: tech-debt-elimination
mode: mapreduce

setup:
  - shell: "debtmap analyze . --output debt.json"

map:
  input: debt.json
  json_path: "$.items[*]"
  filter: "item.severity == 'critical'"
  sort_by: "item.priority DESC"
  max_items: 20
  max_parallel: 5

  agent_template:
    - claude: "/fix-debt-item '${item.description}'"
      commit_required: true
    - shell: "cargo test"
      on_failure:
        claude: "/debug-and-fix"

reduce:
  - shell: "debtmap analyze . --output debt-after.json"
  - claude: "/compare-debt-reports --before debt.json --after debt-after.json"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.3
  error_collection: aggregate  # Options: aggregate, immediate, batched:{size}
```

**Note:** The `error_policy` configuration is optional. If not specified, sensible defaults are used: `on_item_failure: dlq` (failed items go to Dead Letter Queue), `continue_on_failure: true` (workflow continues despite failures), and `error_collection: aggregate` (errors are collected and reported at the end).

---

## Example 9: Generating Configuration Files

```yaml
# Generate a JSON configuration file
- write_file:
    path: "config/deployment.json"
    format: json  # Options: text, json, yaml
    create_dirs: true  # Create parent directories if they don't exist
    content:
      environment: production
      api_url: "${API_URL}"
      features:
        - auth
        - analytics
        - notifications
      timeout: 30

# Generate a YAML configuration file
- write_file:
    path: "config/services.yml"
    format: yaml
    content:
      services:
        web:
          image: "myapp:latest"
          ports:
            - "8080:8080"
        database:
          image: "postgres:15"
          environment:
            POSTGRES_DB: "${DB_NAME}"

# Generate a plain text report
- write_file:
    path: "reports/summary.txt"
    format: text
    mode: "0644"  # File permissions (optional)
    content: |
      Deployment Summary
      ==================
      Environment: ${NODE_ENV}
      API URL: ${API_URL}
      Timestamp: $(date)
```

---

## Example 10: Advanced Features

```yaml
# Nested error handling with retry configuration
- shell: "cargo build --release"
  on_failure:
    shell: "cargo clean"
    on_success:
      shell: "cargo build --release"
      max_attempts: 2
  on_success:
    shell: "cargo test --release"

# Working directory control
- shell: "npm install"
  cwd: "frontend/"

- shell: "npm run build"
  cwd: "frontend/"

# Git context variables (available after git operations)
- shell: "git add ."
- shell: "git commit -m 'Update implementation'"
  id: "commit"

# Access git context from previous step
- shell: "echo 'Modified files: ${step.commit.files_modified}'"
- shell: "echo 'Files added: ${step.commit.files_added}'"
- shell: "echo 'Rust files changed: ${step.commit.files_modified:*.rs}'"

# Git context with pattern filtering
- shell: |
    for file in ${step.commit.files_modified:*.rs}; do
      echo "Running clippy on $file"
      cargo clippy --file "$file"
    done

# Format modifiers for output capture
- claude: "/analyze-codebase"
  id: "analysis"
  capture: "analysis_result"
  capture_format: "json"

# Use captured output with format modifiers
- shell: "echo 'Issues found: ${analysis_result:json:.issues | length}'"
- shell: "echo 'File list:' && echo '${analysis_result:lines}'"
- shell: "echo '${analysis_result:csv}' > report.csv"

# Complex conditional execution
- shell: "cargo test"
  id: "test"
  capture: "test_output"

- claude: "/fix-tests"
  when: "${test_output} contains 'FAILED'"
  max_attempts: 3
```

**Note:** Advanced features include:
- **Nested handlers**: Chain `on_failure` and `on_success` handlers for complex error recovery
- **Working directory**: Use `cwd` to run commands in specific directories
- **Git context**: Access `files_modified`, `files_added`, `files_deleted` from git operations
- **Pattern filtering**: Use `:*.rs` syntax to filter file lists by pattern
- **Format modifiers**: Apply `:json`, `:lines`, `:csv` modifiers to captured output
- **Max attempts**: Combine with conditional execution for retry logic
