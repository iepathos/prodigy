# Conditional Execution

Prodigy supports conditional execution to create dynamic workflows that adapt to runtime conditions. Use when clauses, failure handlers, and success handlers to control workflow flow.

## Overview

Conditional execution enables:
- **When clauses** - Skip commands based on conditions
- **On failure handlers** - Execute recovery commands when errors occur
- **On success handlers** - Execute commands after successful completion
- **Boolean expressions** - Complex conditional logic with variables

## When Clauses

Skip commands based on runtime conditions using boolean expressions.

!!! example "Basic When Clauses"
    ```yaml
    - shell: "cargo build --release"
      when: "${PROFILE} == 'prod'"

    - shell: "cargo test"
      when: "${SKIP_TESTS} != 'true'"
    ```

### Conditional Operators

!!! info "Supported Operators"
    **Comparison:**

    - `==` - Equality
    - `!=` - Inequality
    - `<`, `<=` - Less than, less than or equal
    - `>`, `>=` - Greater than, greater than or equal

    **Logical:**

    - `&&` - Logical AND
    - `||` - Logical OR
    - `!` - Logical NOT

### Complex Conditions

Combine multiple conditions:

```yaml
- shell: "deploy.sh"
  when: "${PROFILE} == 'prod' && ${TESTS_PASSED} == 'true'"

- claude: "/analyze ${item.path}"
  when: "${item.score} >= 5 || ${item.priority} == 'high'"
```

## On Failure Handlers

Execute commands when a previous command fails:

```yaml
# Source: workflows/coverage-simplified.yml
- shell: "just test"
  on_failure:
    claude: "/prodigy-debug-test-failure --spec ${coverage.spec} --output ${shell.output}"
    max_attempts: 3
    fail_workflow: false  # Continue workflow even if tests can't be fixed
```

### Simple Failure Handlers

Use a single command or list of commands:

```yaml
# Single command (shell or claude)
- shell: "cargo test"
  on_failure: "echo 'Tests failed'"

# Multiple commands
- shell: "cargo clippy"
  on_failure:
    - claude: "/fix-clippy-errors"
    - shell: "cargo clippy"  # Verify fix
```

### Advanced Failure Configuration

Control retry behavior and workflow escalation:

```yaml
# Source: src/cook/workflow/on_failure.rs:85-105
- shell: "cargo build"
  on_failure:
    shell: "cleanup.sh"
    max_attempts: 3        # Retry up to 3 times (alias: max_retries)
    fail_workflow: false   # Continue workflow even after all retries fail
    retry_original: true   # Retry original command after handler
```

!!! tip "Configuration Options"
    - **`max_attempts`** (or `max_retries`): Number of retry attempts (default: 1)
    - **`fail_workflow`**: Whether to fail the entire workflow after handling (default: false)
    - **`retry_original`**: Whether to retry the original command after handler (default: false)
    - **`shell`**: Shell command to execute on failure
    - **`claude`**: Claude command to execute on failure

### AI-Assisted Recovery

Use Claude to automatically diagnose and fix failures:

```yaml
# Source: workflows/complex-build-pipeline.yml
- shell: "cargo check"
  on_success:
    shell: "cargo build --release"
    on_success:
      shell: "cargo test --release"
      on_failure:
        claude: "/prodigy-debug-and-fix '${shell.output}'"
```

## On Success Handlers

Execute commands after successful completion:

```yaml
# Source: workflows/complex-build-pipeline.yml
- shell: "cargo bench"
  on_exit_code:
    0:
      shell: "echo 'Benchmarks completed successfully!'"
    101:
      claude: "/prodigy-fix-compilation-errors '${benchmark_results}'"
```

### Simple Success Handlers

Execute commands when a command succeeds:

```yaml
- shell: "cargo build --release"
  on_success:
    - shell: "cp target/release/binary dist/"
    - shell: "echo 'Build successful!'"
```

### Post-Processing Workflow

Perform actions only after success:

```yaml
- claude: "/implement-feature '${item.name}'"
  on_success:
    - shell: "cargo test"
    - shell: "git add -A"
    - shell: "git commit -m 'Implement ${item.name}'"
```

### Exit Code Handlers

Handle specific exit codes with custom actions:

```yaml
# Source: workflows/complex-build-pipeline.yml
- shell: "cargo bench"
  timeout: 600  # 10 minutes
  on_exit_code:
    0:
      shell: "echo 'Benchmarks completed successfully!'"
    101:
      claude: "/prodigy-fix-compilation-errors '${benchmark_results}'"
```

!!! note "Exit Code Convention"
    Exit code 0 indicates success. Non-zero exit codes indicate different types of failures. Common exit codes:

    - **0**: Success
    - **1**: General error
    - **101**: Rust compilation error
    - **127**: Command not found
    - Custom exit codes can be defined by your scripts

## Combining Conditions and Handlers

Use when clauses with handlers for complex workflows:

```yaml
- shell: "deploy.sh"
  when: "${PROFILE} == 'prod'"
  on_failure:
    - shell: "rollback.sh"
    - claude: "/notify-team 'Deployment failed'"
  on_success:
    - shell: "echo 'Deployment successful!'"
    - claude: "/notify-team 'Deployed to production'"
```

## Boolean Expressions with Variables

Reference captured variables in conditions:

```yaml
- shell: "git diff --name-only"
  capture: changed_files

- shell: "cargo test"
  when: "${changed_files} != ''"
  on_failure:
    - claude: "/fix-tests"
```

## Best Practices

!!! tip "Conditional Execution Guidelines"
    **When to Use When Clauses:**

    - Skip expensive operations in specific environments
    - Conditional deployments based on branch or environment
    - Feature flags and experimental workflows

    **When to Use Failure Handlers:**

    - Automated error recovery with Claude
    - Cleanup after failures
    - Retry transient failures with `max_attempts`
    - Graceful degradation with `fail_workflow: false`

    **When to Use Success Handlers:**

    - Post-build steps (deployment, artifact copying)
    - Notifications and status updates
    - Chaining dependent operations

!!! warning "Common Pitfalls"
    - **Infinite loops:** Be careful with `retry_original: true` and high `max_attempts`
    - **Variable scope:** Ensure variables exist before using them in `when` clauses
    - **Exit code conflicts:** Don't combine `on_failure` with `on_exit_code` for the same codes
    - **Handler complexity:** Keep handlers simple; complex recovery should be separate workflows

## See Also

- [Error Handling](error-handling.md) - Comprehensive error handling strategies
- [Variables and Interpolation](variables.md) - Variable syntax and usage
- [Workflow Structure](workflow-structure.md) - Basic workflow syntax and structure
