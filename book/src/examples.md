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
    claude: "/improve-coverage"
    validate: |
      coverage=$(cargo tarpaulin | grep 'Coverage' | sed 's/.*: \([0-9.]*\)%.*/\1/')
      echo "score: ${coverage%.*}"
    threshold: 80
    max_attempts: 5
  commit_required: true
```

---

## Example 3: Parallel Code Review

```yaml
name: parallel-code-review
mode: mapreduce

setup:
  - shell: "find src -name '*.rs' > files.txt"
  - shell: "jq -R -s -c 'split(\"\n\") | map(select(length > 0) | {path: .})' files.txt > items.json"

map:
  input: items.json
  json_path: "$.[:1]"
  agent_template:
    - claude: "/review-file ${item.path}"
    - shell: "cargo check ${item.path}"
  max_parallel: 5

reduce:
  - claude: "/summarize-reviews ${map.results}"
```

---

## Example 4: Conditional Deployment

```yaml
- shell: "cargo test --quiet && echo true || echo false"
  capture: "tests_passed"
  capture_format: boolean

- shell: "cargo build --release"
  when: "${tests_passed}"

- shell: "docker build -t myapp ."
  when: "${tests_passed}"
  on_success:
    shell: "docker push myapp:latest"
```

---

## Example 5: Multi-Step Validation

```yaml
- claude: "/implement-feature auth"
  commit_required: true
  validate:
    - shell: "cargo test auth"
    - shell: "cargo clippy -- -D warnings"
    - claude: "/validate-implementation --output validation.json"
      result_file: "validation.json"
      threshold: 90
      on_incomplete:
        - claude: "/complete-gaps ${validation.gaps}"
          commit_required: true
        max_attempts: 2
```

---

## Example 6: Environment-Aware Workflow

```yaml
env:
  DEPLOY_ENV:
    condition: "${branch} == 'main'"
    when_true: "production"
    when_false: "staging"

profiles:
  production:
    env:
      API_URL: https://api.production.com
  staging:
    env:
      API_URL: https://api.staging.com

# Activate profile based on DEPLOY_ENV
active_profile: "${DEPLOY_ENV}"

commands:
  - shell: "cargo build --release"
  - shell: "echo 'Deploying to ${DEPLOY_ENV} at ${API_URL}'"
  - shell: "deploy.sh ${DEPLOY_ENV}"
```

---

## Example 7: Complex MapReduce with Error Handling

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
        max_attempts: 2

reduce:
  - shell: "debtmap analyze . --output debt-after.json"
  - claude: "/compare-debt-reports --before debt.json --after debt-after.json"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 5
  failure_threshold: 0.3
```
