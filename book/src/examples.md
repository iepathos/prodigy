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
  commit_required: true
```

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
  json_path: "$[*]"  # Process all items
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

# Environment profiles for different contexts
profiles:
  production:
    env:
      API_URL: https://api.production.com
      LOG_LEVEL: error
    description: "Production environment"

  staging:
    env:
      API_URL: https://api.staging.com
      LOG_LEVEL: warn
    description: "Staging environment"

# Secrets (masked in logs)
secrets:
  API_KEY:
    value: "${SECRET_API_KEY}"
    secret: true

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

**Note:** Profile activation with `active_profile` is managed internally and not currently exposed in WorkflowConfig YAML. Use `--profile` CLI flag to activate profiles.

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
