## Step Identification

Assign unique IDs to steps for explicit output referencing. This is particularly useful in complex workflows where multiple steps produce outputs and you need to reference specific results.

### Available Step Reference Fields

When you assign an ID to a step, you can reference multiple fields from that step's execution:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `${step-id.output}` | string | Standard output (stdout) from step | `${test-step.output}` |
| `${step-id.exit_code}` | number | Process exit code (0 = success) | `${build.exit_code}` |
| `${step-id.success}` | boolean | Whether step succeeded (exit_code == 0) | `${lint.success}` |

**Source**: Field resolution implemented in `src/cook/expression/mod.rs:187-200`

These fields are automatically available for any step with an `id` field. They're commonly used in conditionals and error handling:

### Basic Step IDs with Auto-Captured Fields

```yaml
- shell: "cargo test"
  id: "test-step"

# Reference step's automatic fields
- shell: "echo 'Exit code: ${test-step.exit_code}'"
- shell: "echo 'Success: ${test-step.success}'"
- claude: "/analyze-test-output '${test-step.output}'"
  when: "${test-step.exit_code != 0}"
```

**Note**: The `.output`, `.exit_code`, and `.success` fields are automatically captured for any step with an `id`. You don't need to explicitly configure output capture for these standard fields.

### Custom Output Fields

For capturing specific files or structured data, use the `outputs` field:

```yaml
- claude: "/prodigy-code-review"
  id: "review"
  outputs:
    spec:
      file_pattern: "*-spec.md"

# Reference custom output field
- claude: "/prodigy-implement-spec ${review.spec}"
  id: "implement"
```

**Source**: Real example from `src/cook/mod_tests.rs:223-228` and `src/config/command.rs:1049`

Custom outputs are useful for:
- Capturing generated files (specs, reports, configs)
- Passing structured data between steps
- Referencing specific artifacts by name

### When to Use Step IDs

**1. Conditional Execution Based on Step Results**

Step IDs enable precise control flow using the auto-captured `.exit_code` and `.success` fields:

```yaml
- shell: "cargo test --lib"
  id: "unit-tests"

- shell: "cargo test --test integration"
  id: "integration-tests"

# Execute only if specific test suite failed
- claude: "/analyze-failures '${unit-tests.output}'"
  when: "${unit-tests.exit_code != 0}"

- claude: "/analyze-failures '${integration-tests.output}'"
  when: "${integration-tests.exit_code != 0}"
```

**Source**: Conditional evaluation from `src/cook/workflow/conditional_tests.rs:95-97`

**2. Error Handling with Step-Specific Outputs**

Use step IDs to pass the exact output from a failed step to error handlers:

```yaml
- shell: "npm run build"
  id: "build"

- shell: "npm run lint"
  id: "lint"

- shell: "npm test"
  id: "test"

# Pass step output to Claude for analysis
- claude: "/debug-build-failure '${build.output}'"
  when: "${build.exit_code != 0}"
```

**Source**: Real pattern from `src/cook/mod.rs:721` and `src/cook/execution/mapreduce_integration_tests.rs:230`

**3. Multi-Step Conditional Logic**

Combine multiple step results to control workflow execution:

```yaml
- shell: "cargo clippy"
  id: "clippy-check"

- shell: "cargo fmt --check"
  id: "format-check"

# Only proceed if both checks pass
- shell: "cargo build --release"
  when: "${clippy-check.exit_code == 0 && format-check.exit_code == 0}"

# Fix clippy warnings if present
- claude: "/fix-clippy-warnings '${clippy-check.output}'"
  when: "${clippy-check.exit_code != 0}"
```

**Source**: Boolean logic support from `src/cook/workflow/conditional_tests.rs:56`

**4. Passing Step Outputs to Subsequent Commands**

Reference earlier step outputs in later commands, including in `on_failure` handlers:

```yaml
- shell: "cargo test"
  id: "test"
  on_failure:
    claude: "/fix-failing-tests '${test.output}'"
    max_attempts: 3

# Use test results in summary
- shell: "generate-report.sh '${test.output}' ${test.exit_code}"
```

**Source**: Real `on_failure` pattern from `src/cook/mod.rs:721-722`

### When NOT to Use Step IDs

Step IDs add complexity, so skip them when:

- **Single command workflows**: If you only have one or two steps, `${shell.output}` is clear enough
- **No output references**: If you never reference a step's output, exit code, or success status
- **Simple sequential execution**: Steps that always run in order without conditionals

**Example of when step IDs are unnecessary:**

```yaml
- shell: "cargo build"
- shell: "cargo test"
- shell: "cargo clippy"
```

None of these steps reference each other, so IDs would add no value.

### Implementation Details

**How It Works:**

1. When a step with `id: "my-step"` executes, Prodigy automatically captures:
   - `${my-step.output}` - stdout from the command
   - `${my-step.exit_code}` - process exit code (0 = success, non-zero = failure)
   - `${my-step.success}` - boolean indicating success (computed as `exit_code == 0`)

2. Custom outputs (via `outputs:` field) are stored separately and accessed by their output name:
   - `${my-step.custom-output-name}` - file path matching the pattern

3. Variable resolution happens at runtime when constructing subsequent commands

**Source**: Implementation in `src/cook/expression/mod.rs:187-200` and `src/cook/orchestrator/execution_pipeline.rs:650-654`

### Troubleshooting

**"Variable not found" errors:**
- Ensure the step has completed before referencing its outputs
- Verify the step ID matches exactly (case-sensitive)
- Check that the step actually has an `id` field

**Empty output values:**
- Step output is only captured if the command writes to stdout
- Use `outputs:` for file-based artifacts instead of stdout
- Verify the command actually produces output

**Conditional not working:**
- The `.success` and `.exit_code` fields are only available for steps with `id` set
- Ensure your `when:` expression uses correct syntax: `"${step-id.exit_code != 0}"`
