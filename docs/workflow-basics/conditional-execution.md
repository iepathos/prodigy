# Conditional Execution

Prodigy provides flexible conditional execution mechanisms to control workflow behavior based on runtime conditions, command results, and variable values.

## Overview

Conditional execution enables:
- Skipping commands based on conditions (`when` clauses)
- Running commands on success (`on_success`)
- Running commands on failure (`on_failure`)
- Complex boolean logic with variables
- Dynamic workflow paths based on state

## When Clauses

Skip commands based on boolean expressions:

```yaml
- shell: "npm run build"
  when: "${ENVIRONMENT} == 'production'"

- shell: "cargo test --release"
  when: "${RUN_TESTS} == true"

- claude: "/deploy"
  when: "${workflow.iteration} > 0"
```

### Syntax

Boolean expressions support:
- **Comparison operators**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical operators**: `&&` (and), `||` (or), `!` (not)
- **Variable references**: `${variable}`, `$variable`
- **String literals**: `"value"`
- **Number literals**: `42`, `3.14`
- **Boolean literals**: `true`, `false`

### Examples

```yaml
# Simple comparison
- shell: "deploy.sh"
  when: "${environment} == 'prod'"

# Logical AND
- shell: "cargo test"
  when: "${RUN_TESTS} == true && ${ENVIRONMENT} != 'prod'"

# Logical OR
- shell: "notify.sh"
  when: "${workflow.status} == 'failed' || ${FORCE_NOTIFY} == true"

# Negation
- shell: "cleanup.sh"
  when: "!${SKIP_CLEANUP}"

# Numeric comparison
- shell: "scale-up.sh"
  when: "${item.priority} >= 8"
```

## On Failure Handlers

Execute commands when a step fails:

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/debug-test-failure --output ${shell.stderr}"

- claude: "/implement-feature"
  commit_required: true
  on_failure:
    claude: "/fix-implementation-errors"
```

### Use Cases

- **Automated debugging**: Analyze failures with AI
- **Retry with fixes**: Attempt recovery automatically
- **Logging and notification**: Record failure details
- **Cleanup**: Remove partial changes on error

### Advanced Patterns

```yaml
- shell: "cargo build --release"
  on_failure:
    # Multiple recovery steps
    claude: "/analyze-build-error ${shell.stderr}"
    shell: "cargo clean"
    shell: "cargo build"  # Retry after clean
```

## On Success Handlers

Execute commands after successful completion:

```yaml
- shell: "cargo test"
  on_success:
    shell: "echo 'All tests passed!'"
    shell: "git tag v${VERSION}"

- claude: "/implement-spec spec.md"
  commit_required: true
  on_success:
    shell: "cargo fmt"
    shell: "cargo clippy -- -D warnings"
```

### Use Cases

- **Post-processing**: Format or validate after implementation
- **Notifications**: Alert on successful completion
- **Deployment**: Trigger deployment after successful tests
- **Documentation**: Generate docs after code changes

## Combining Conditions

Combine `when`, `on_failure`, and `on_success`:

```yaml
- shell: "cargo test --release"
  when: "${ENVIRONMENT} == 'prod'"
  on_failure:
    claude: "/debug-failure ${shell.stderr}"
  on_success:
    shell: "deploy.sh"
```

## Conditional Logic in MapReduce

### Map Phase Filtering

Filter work items with boolean expressions:

```yaml
mode: mapreduce

map:
  input: "items.json"
  json_path: "$[*]"
  filter: "item.priority >= 5"  # Only process high-priority items

  agent_template:
    - claude: "/process ${item.path}"
      when: "${item.type} == 'feature'"  # Additional per-command filtering
```

### Conditional Error Handling

```yaml
map:
  agent_template:
    - shell: "cargo check ${item.file}"
      on_failure:
        claude: "/fix-errors ${item.file}"
        when: "${item.auto_fix} == true"  # Only auto-fix if enabled
```

## Advanced Patterns

### Nested Conditionals

```yaml
- shell: "integration-tests.sh"
  when: "${ENVIRONMENT} == 'staging' || ${ENVIRONMENT} == 'prod'"
  on_success:
    - shell: "load-test.sh"
      when: "${ENVIRONMENT} == 'prod'"  # Only load test in production
  on_failure:
    - claude: "/analyze-integration-failure"
      when: "${AUTO_DEBUG} == true"
```

### Variable-Based Branching

```yaml
- shell: "detect-language.sh"
  capture_output: language

- shell: "cargo build"
  when: "${language} == 'rust'"

- shell: "npm run build"
  when: "${language} == 'javascript'"

- shell: "go build"
  when: "${language} == 'go'"
```

### Iteration-Based Logic

```yaml
- shell: "cargo test"
  on_failure:
    claude: "/fix-tests"
    when: "${workflow.iteration} < 3"  # Only auto-fix first 3 iterations

- shell: "notify-failure.sh"
  when: "${workflow.iteration} >= 3"  # Alert after 3 failed attempts
```

## Expression Evaluation

### Type Coercion

Prodigy automatically coerces types in comparisons:
- Strings to numbers: `"42" == 42` → `true`
- Strings to booleans: `"true" == true` → `true`
- Numbers to strings: `42 == "42"` → `true`

### Undefined Variables

If a variable is undefined:
- Comparison returns `false`
- Conditional command is skipped
- No error is raised

Provide defaults to handle undefined variables:
```yaml
when: "${variable|default:false} == true"
```

## Examples

### Environment-Specific Deployment

```yaml
env:
  ENVIRONMENT: "staging"

- shell: "cargo build"

- shell: "cargo test"
  when: "${ENVIRONMENT} != 'prod'"  # Skip tests in prod

- shell: "docker build -t app:${VERSION} ."
  when: "${ENVIRONMENT} == 'prod'"

- shell: "kubectl apply -f k8s/deployment.yml"
  on_success:
    shell: "kubectl rollout status deployment/app"
  on_failure:
    shell: "kubectl rollout undo deployment/app"
```

### Progressive Enhancement

```yaml
- shell: "cargo test"
  capture_output: test_results

- shell: "echo 'Basic tests passed'"
  when: "${test_results.exit_code} == 0"

- shell: "cargo test --release"
  when: "${test_results.exit_code} == 0"
  capture_output: release_tests

- shell: "deploy.sh"
  when: "${test_results.exit_code} == 0 && ${release_tests.exit_code} == 0"
```

## See Also

- [Variables and Interpolation](variables.md) - Variable system used in conditions
- [Error Handling](error-handling.md) - Comprehensive error handling strategies
- [Command Types](command-types.md) - Commands that support conditional execution
- [MapReduce Workflows](../mapreduce/index.md) - Filtering and conditional logic at scale
