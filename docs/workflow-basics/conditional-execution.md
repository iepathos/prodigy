# Conditional Execution

Prodigy supports conditional execution to create dynamic workflows that adapt to runtime conditions. Use when clauses, failure handlers, and success handlers to control workflow flow.

## Overview

Conditional execution enables:
- **When clauses** - Skip commands based on conditions
- **On failure handlers** - Execute recovery commands when errors occur
- **On success handlers** - Execute commands after successful completion
- **Boolean expressions** - Complex conditional logic with variables

## When Clauses

Skip commands based on runtime conditions:

```yaml
- shell: "cargo build --release"
  when: "${PROFILE} == 'prod'"

- shell: "cargo test"
  when: "${SKIP_TESTS} != 'true'"
```

### Conditional Operators

- `==` - Equality
- `!=` - Inequality
- `<`, `<=` - Less than, less than or equal
- `>`, `>=` - Greater than, greater than or equal
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
- shell: "cargo test"
  on_failure:
    - claude: "/fix-test-failures"
    - shell: "cargo test"  # Retry after fix
```

### AI-Assisted Recovery

Use Claude to automatically fix failures:

```yaml
- shell: "cargo clippy"
  on_failure:
    - claude: "/fix-clippy-errors"
    - shell: "cargo clippy"  # Verify fix
```

### Escalation Control

Configure how failures escalate:

```yaml
- shell: "risky-operation.sh"
  on_failure:
    - shell: "cleanup.sh"
  continue_on_error: true  # Don't fail workflow
```

## On Success Handlers

Execute commands after successful completion:

```yaml
- shell: "cargo build --release"
  on_success:
    - shell: "cp target/release/binary dist/"
    - shell: "echo 'Build successful!'"
```

### Post-Processing

Perform actions only after success:

```yaml
- claude: "/implement-feature '${item.name}'"
  on_success:
    - shell: "cargo test"
    - shell: "git add -A"
    - shell: "git commit -m 'Implement ${item.name}'"
```

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

## See Also

- [Error Handling](error-handling.md) - Comprehensive error handling strategies
- [Variables and Interpolation](variables.md) - Variable syntax and usage
- [Command Types](command-types.md) - Available command types
