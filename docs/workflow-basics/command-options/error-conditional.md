## Error Handling

Options for handling command failures and implementing recovery strategies.

### on_failure

Specifies commands to execute when the main command fails. Supports automatic retry with configurable attempts and workflow failure control.

**Type**: `Option<TestDebugConfig>` with fields:
- `claude: String` - Claude command to run on failure
- `max_attempts: u32` - Maximum retry attempts (default: 3)
- `fail_workflow: bool` - Whether to fail workflow after max attempts (default: `false`)
- `commit_required: bool` - Whether debug command should commit (default: `true`)

**Source**: `src/config/command.rs:370-372, 166-183`

```yaml
commands:
  # Retry tests with automated fixing
  - shell: "cargo test"
    on_failure:
      claude: "/prodigy-debug-test-failure --output ${shell.output}"
      max_attempts: 3
      fail_workflow: false

  # Critical check that must pass
  - shell: "just fmt-check && just lint"
    on_failure:
      claude: "/prodigy-lint ${shell.output}"
      max_attempts: 5
      fail_workflow: true

  # Doc test failures with commit requirement
  - shell: "cargo test --doc"
    on_failure:
      claude: "/prodigy-fix-doc-tests --output ${shell.output}"
      max_attempts: 2
      fail_workflow: false
      commit_required: true
```

**Real-world examples**:
- `workflows/coverage-with-test-debug.yml:13-23` - Test debugging with retries
- `workflows/debtmap-reduce.yml:58-70` - Critical quality gates
- `workflows/documentation-drift.yml:47-53` - Doc test recovery

### on_success

Executes another command when the main command succeeds. Enables chaining of dependent operations.

**Type**: `Option<Box<WorkflowStepCommand>>` (nested command)

**Source**: `src/config/command.rs:374-376`

```yaml
commands:
  # Chain successful operations
  - shell: "cargo check"
    on_success:
      shell: "cargo build --release"
      on_success:
        shell: "cargo test --release"

  # Nested success/failure handlers
  - shell: "cargo test"
    capture_output: "test_output"
    on_failure:
      claude: "/prodigy-debug-test-failures '${test_output}'"
      commit_required: true
      on_success:
        # After fixing, verify tests pass
        shell: "cargo test"
        on_failure:
          claude: "/prodigy-fix-test-failures '${shell.output}' --deep-analysis"

  # Success notification
  - shell: "cargo test --release"
    capture_output: "final_results"
    on_success:
      shell: "echo 'All tests passing!'"
```

**Real-world examples**:
- `workflows/implement-with-tests.yml:28-40,61-63` - Nested test-fix-verify loops
- `workflows/complex-build-pipeline.yml:7-13` - Build pipeline chaining

## Conditional Execution

Options for controlling when commands execute based on runtime conditions.

### when

Evaluates a boolean expression to determine whether to execute the command. Supports variable interpolation and boolean logic.

**Type**: `Option<String>` (boolean expression)

**Source**: `src/config/command.rs:387-388`

```yaml
commands:
  # Capture test status
  - shell: "cargo test --quiet && echo true || echo false"
    capture_output: "tests_passed"
    capture_format: "boolean"

  # Conditional execution based on test results
  - shell: "echo 'Running coverage analysis...'"
    when: "${tests_passed}"

  # Multiple conditions
  - shell: "cargo build --release"
    when: "${tests_passed} && ${lint_passed}"
    capture_output: "build_output"

  # Conditional deployment
  - shell: |
      if [ "${tests_passed}" = "true" ] && [ "${build_output.success}" = "true" ]; then
        echo "Deployment ready!"
      fi
    when: "${tests_passed}"
```

**Real-world examples**:
- `examples/capture-conditional-flow.yml:20-51` - Multi-stage conditional pipeline
