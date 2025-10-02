# Spec 116: Improve Error Output for Failed Commands

## Problem

When a command fails in a Prodigy workflow, the error message only shows:
```
❌ Session failed: Failed to execute step: <command> (working directory: <path>)
```

This doesn't include the actual error output from the underlying command, forcing users to:
1. Re-run the command manually to see the error
2. Check logs to understand what went wrong
3. Guess at the failure reason

## Example

Running `debtmap compare` with incorrect plan format fails with:
```
❌ Session failed: Failed to execute step: shell: debtmap compare --before .prodigy/debtmap-before.json --after .prodigy/debtmap-after.json --plan .prodigy/IMPLEMENTATION_PLAN.md --output .prodigy/comparison.json --format json (working directory: /Users/glen/.prodigy/worktrees/prodigy/session-a53231c1-55f1-4f46-b8ea-4a50ef7a1ef4)
```

But the actual error is:
```
Error: Could not find **Location**: in plan file. Expected format: **Location**: ./file.rs:function:line
```

This critical information is hidden in the stderr but not displayed.

## Solution

Enhance error reporting to include:
1. **Command stderr output** - Show the actual error message from the failed command
2. **Exit code** - Include the exit code for debugging
3. **Structured logging** - Log full error details at ERROR level for later analysis

### Error Message Format

```
❌ Session failed: Failed to execute step: <command>
Working directory: <path>
Exit code: <code>
Error output:
<stderr content>
```

### Implementation Details

**Location**: `src/cook/workflow/executor.rs:3437-3460` (`finalize_step_result`)

**Current code**:
```rust
fn finalize_step_result(&self, step: &WorkflowStep, mut result: StepResult) -> Result<StepResult> {
    let should_fail = Self::should_fail_workflow_for_step(&result, step);

    if should_fail {
        let error_msg = Self::build_step_error_message(step, &result);
        anyhow::bail!(error_msg);
    }

    // ...
}
```

**Proposed change**:
```rust
fn finalize_step_result(&self, step: &WorkflowStep, mut result: StepResult) -> Result<StepResult> {
    let should_fail = Self::should_fail_workflow_for_step(&result, step);

    if should_fail {
        let error_msg = Self::build_step_error_message(step, &result);

        // Log full error details for debugging
        tracing::error!(
            "Step failed - Command: {:?}, Exit code: {:?}, Stderr: {}",
            self.get_step_display_name(step),
            result.exit_code,
            result.stderr
        );

        anyhow::bail!(error_msg);
    }

    // ...
}
```

**Update `build_step_error_message`**:
```rust
fn build_step_error_message(step: &WorkflowStep, result: &StepResult) -> String {
    let step_desc = /* existing step description logic */;

    let mut error_msg = format!("Step failed: {}", step_desc);

    // Add exit code if available
    if let Some(code) = result.exit_code {
        error_msg.push_str(&format!("\nExit code: {}", code));
    }

    // Add stderr if not empty
    if !result.stderr.trim().is_empty() {
        error_msg.push_str(&format!("\nError output:\n{}", result.stderr));
    }

    error_msg
}
```

## Success Criteria

- [ ] Failed shell commands show stderr in error message
- [ ] Failed validation steps show actual validation error
- [ ] Exit codes are included in error output
- [ ] Full error details are logged at ERROR level
- [ ] Error output is properly formatted and readable
- [ ] All existing tests pass
- [ ] No clippy warnings

## Test Cases

1. **Shell command failure**
   - Run workflow with failing shell command
   - Verify stderr is displayed in error message
   - Verify exit code is shown

2. **Validation failure**
   - Run workflow where validation step fails
   - Verify actual error from validation command is shown
   - Verify user can understand failure without re-running

3. **Claude command failure**
   - Run workflow with failing Claude command
   - Verify error details are captured and displayed

## Implementation Notes

- Keep backward compatibility with existing error handling
- Ensure error messages are well-formatted (not cluttered)
- Truncate very long stderr output (>1000 lines) with indication
- Preserve full error in logs even if truncated in display
