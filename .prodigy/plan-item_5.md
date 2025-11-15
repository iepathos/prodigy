# Implementation Plan: Reduce Complexity in WorkflowExecutor::handle_on_failure

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:WorkflowExecutor::handle_on_failure:147
**Priority Score**: 26.10
**Debt Type**: ComplexityHotspot (Cyclomatic: 19, Cognitive: 69)
**Current Metrics**:
- Function Length: 180 lines
- Cyclomatic Complexity: 19
- Cognitive Complexity: 69
- Nesting Depth: 5

**Issue**: High complexity 19/69 makes function hard to test and maintain. The function handles multiple responsibilities:
1. Error context injection/cleanup
2. Handler command construction
3. Handler command execution with error handling
4. Recovery strategy determination
5. Retry logic for original command
6. Legacy handler fallback support

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: ~9.5 points (from 19 to ~10)
- Coverage Improvement: 0.0% (already well-tested via integration tests)
- Risk Reduction: 6.79 points

**Success Criteria**:
- [ ] Cyclomatic complexity reduced to ≤10
- [ ] Cognitive complexity reduced to ≤35
- [ ] Maximum nesting depth ≤3
- [ ] Each extracted function ≤20 lines
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt

## Implementation Phases

### Phase 1: Extract Error Context Management

**Goal**: Isolate error context variable injection and cleanup into pure functions

**Changes**:
- Extract `inject_error_context()` function that creates error variable map
- Extract `cleanup_error_context()` function that removes error variables
- Both functions should be pure: take inputs, return outputs, no mutation

**Implementation**:
```rust
// Add to failure_handler.rs:

/// Inject error context variables into workflow context
pub fn create_error_context_variables(
    stderr: &str,
    exit_code: Option<i32>,
    step_name: &str,
) -> Vec<(String, String)> {
    vec![
        ("error.message".to_string(), stderr.to_string()),
        ("error.exit_code".to_string(), exit_code.unwrap_or(-1).to_string()),
        ("error.step".to_string(), step_name.to_string()),
        ("error.timestamp".to_string(), chrono::Utc::now().to_rfc3339()),
    ]
}

/// Get list of error context variable keys for cleanup
pub fn get_error_context_keys() -> Vec<&'static str> {
    vec![
        "error.message",
        "error.exit_code",
        "error.step",
        "error.timestamp",
    ]
}
```

- Update `handle_on_failure` to use these helper functions

**Testing**:
```bash
cargo test executor::tests --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Error context logic extracted to pure functions
- [ ] Complexity reduced by ~2 points
- [ ] All tests pass
- [ ] Ready to commit

---

### Phase 2: Extract Handler Step Construction

**Goal**: Extract WorkflowStep creation logic into a dedicated function

**Changes**:
- Extract `create_handler_step()` function in failure_handler.rs
- Takes: HandlerCommand, timeout config
- Returns: WorkflowStep
- Eliminates the 50+ line WorkflowStep construction inline

**Implementation**:
```rust
// Add to failure_handler.rs:

/// Create a WorkflowStep from a HandlerCommand
pub fn create_handler_step(
    cmd: &crate::cook::workflow::on_failure::HandlerCommand,
    timeout: Option<u64>,
) -> crate::cook::workflow::step::WorkflowStep {
    use crate::cook::workflow::step::{WorkflowStep, CaptureOutput};

    WorkflowStep {
        name: None,
        shell: cmd.shell.clone(),
        claude: cmd.claude.clone(),
        test: None,
        goal_seek: None,
        foreach: None,
        write_file: None,
        command: None,
        handler: None,
        capture: None,
        capture_format: None,
        capture_streams: Default::default(),
        auto_commit: false,
        commit_config: None,
        output_file: None,
        timeout,
        capture_output: CaptureOutput::Disabled,
        on_failure: None,
        retry: None,
        on_success: None,
        on_exit_code: Default::default(),
        commit_required: false,
        working_dir: None,
        env: Default::default(),
        validate: None,
        step_validate: None,
        skip_validation: false,
        validation_timeout: None,
        ignore_validation_failure: false,
        when: None,
    }
}
```

- Replace inline WorkflowStep construction with function call

**Testing**:
```bash
cargo test executor::tests --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Handler step construction extracted
- [ ] Complexity reduced by ~1 point
- [ ] No visual/structural changes to logic
- [ ] All tests pass
- [ ] Ready to commit

---

### Phase 3: Extract Handler Execution Loop

**Goal**: Extract the handler command execution loop into a separate function

**Changes**:
- Extract `execute_handler_commands()` async function
- Takes: handler_commands, on_failure_config, executor ref, env, ctx
- Returns: Result<(bool, Vec<String>)> (success flag + outputs)
- Encapsulates the loop over handler commands, error handling, and continue_on_error logic

**Implementation**:
```rust
// Add to executor.rs (needs access to self):

async fn execute_handler_commands(
    &mut self,
    handler_commands: &[crate::cook::workflow::on_failure::HandlerCommand],
    timeout: Option<u64>,
    env: &ExecutionEnvironment,
    ctx: &mut WorkflowContext,
) -> Result<(bool, Vec<String>)> {
    let mut handler_success = true;
    let mut handler_outputs = Vec::new();

    for (idx, cmd) in handler_commands.iter().enumerate() {
        self.user_interaction.display_progress(&format!(
            "Handler command {}/{}",
            idx + 1,
            handler_commands.len()
        ));

        let handler_step = failure_handler::create_handler_step(cmd, timeout);

        match Box::pin(self.execute_step(&handler_step, env, ctx)).await {
            Ok(handler_result) => {
                handler_outputs.push(handler_result.stdout.clone());
                if !handler_result.success && !cmd.continue_on_error {
                    handler_success = false;
                    self.user_interaction
                        .display_error(&format!("Handler command {} failed", idx + 1));
                    break;
                }
            }
            Err(e) => {
                self.user_interaction.display_error(&format!(
                    "Handler command {} error: {}",
                    idx + 1,
                    e
                ));
                if !cmd.continue_on_error {
                    handler_success = false;
                    break;
                }
            }
        }
    }

    Ok((handler_success, handler_outputs))
}
```

- Replace inline loop with function call

**Testing**:
```bash
cargo test executor::tests --lib
cargo test workflow::tests --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Handler execution loop extracted
- [ ] Complexity reduced by ~3 points
- [ ] Loop logic preserved exactly
- [ ] All tests pass
- [ ] Ready to commit

---

### Phase 4: Extract Retry-After-Handler Logic

**Goal**: Extract the "retry original command after handler" logic into a separate function

**Changes**:
- Extract `retry_original_command()` async function
- Takes: step, max_retries, executor ref, env, ctx
- Returns: Result<Option<StepResult>> (Some if retry succeeded, None if all failed)
- Eliminates duplicate retry loops (appears twice in handle_on_failure)

**Implementation**:
```rust
// Add to executor.rs:

async fn retry_original_command(
    &mut self,
    step: &WorkflowStep,
    max_retries: u32,
    env: &ExecutionEnvironment,
    ctx: &mut WorkflowContext,
) -> Result<Option<StepResult>> {
    for retry in 1..=max_retries {
        self.user_interaction.display_info(&format!(
            "Retrying original command (attempt {}/{})",
            retry, max_retries
        ));

        // Create a copy of the step without on_failure to avoid recursion
        let mut retry_step = step.clone();
        retry_step.on_failure = None;

        let retry_result = Box::pin(self.execute_step(&retry_step, env, ctx)).await?;
        if retry_result.success {
            return Ok(Some(retry_result));
        }
    }
    Ok(None)
}
```

- Replace both retry loops with calls to this function
- Use the result to update `result` if Some(retry_result) is returned

**Testing**:
```bash
cargo test executor::tests --lib
cargo test workflow::retry --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Retry logic extracted and deduplicated
- [ ] Both code paths (new handler + legacy handler) use same function
- [ ] Complexity reduced by ~2 points
- [ ] All tests pass
- [ ] Ready to commit

---

### Phase 5: Restructure Main Function Flow

**Goal**: Simplify the main `handle_on_failure` function to be a high-level orchestrator

**Changes**:
- Use early returns to reduce nesting
- Simplify conditional branches
- Add clear comments for each major section

**Implementation**:
```rust
async fn handle_on_failure(
    &mut self,
    step: &WorkflowStep,
    mut result: StepResult,
    on_failure_config: &OnFailureConfig,
    env: &ExecutionEnvironment,
    ctx: &mut WorkflowContext,
) -> Result<StepResult> {
    // 1. Inject error context
    let step_name = self.get_step_display_name(step);
    let error_vars = failure_handler::create_error_context_variables(
        &result.stderr,
        result.exit_code,
        &step_name,
    );
    for (key, value) in error_vars {
        ctx.variables.insert(key, value);
    }

    // 2. Execute handler (new or legacy)
    let handler_commands = on_failure_config.handler_commands();
    if !handler_commands.is_empty() {
        result = self.handle_new_style_failure(
            step, result, on_failure_config, &handler_commands, env, ctx
        ).await?;
    } else if let Some(handler) = on_failure_config.handler() {
        result = self.handle_legacy_failure(
            step, result, on_failure_config, &handler, env, ctx
        ).await?;
    }

    // 3. Cleanup error context
    for key in failure_handler::get_error_context_keys() {
        ctx.variables.remove(key);
    }

    Ok(result)
}

// New helper methods:
async fn handle_new_style_failure(/* ... */) -> Result<StepResult> {
    // Current logic for new-style handlers
}

async fn handle_legacy_failure(/* ... */) -> Result<StepResult> {
    // Current logic for legacy handlers
}
```

**Testing**:
```bash
cargo test executor::tests --lib
cargo test workflow::integration --lib
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Main function is <30 lines
- [ ] Nesting depth ≤2
- [ ] Clear separation between new and legacy paths
- [ ] All tests pass
- [ ] Ready to commit

---

## Testing Strategy

**For each phase**:
1. `cargo test --lib` - Verify existing tests pass
2. `cargo clippy -- -D warnings` - Check for warnings
3. `cargo fmt` - Ensure proper formatting
4. Manual review of complexity reduction

**Final verification**:
1. `just ci` - Full CI checks
2. Compare cyclomatic complexity before/after (should be ≤10)
3. Verify all extraction functions are pure where possible
4. Ensure no behavioral changes (integration tests should pass unchanged)

**Regression Testing**:
```bash
# Run full workflow test suite
cargo test workflow::executor::tests
cargo test workflow::integration
cargo test on_failure

# Check specific failure handling scenarios
cargo test retry
cargo test handler
```

## Rollback Plan

If a phase fails:
1. Review git log to identify last good commit
2. `git reset --hard HEAD~1` to revert the failing phase
3. Review test failures and clippy warnings
4. Adjust implementation approach
5. Retry the phase with corrections

Each phase is independently committable, so rollback is granular.

## Notes

**Complexity Sources Identified**:
- Deep nesting (5 levels) - Addressed in Phase 5
- Large inline structures (WorkflowStep construction) - Addressed in Phase 2
- Duplicate retry loops - Addressed in Phase 4
- Mixed concerns (error handling + execution + recovery) - Addressed across all phases

**Functional Programming Opportunities**:
- Error context creation is pure (Phase 1)
- Handler step construction is pure (Phase 2)
- Recovery strategy determination already pure (in failure_handler)

**No Behavior Changes**:
This refactoring is purely structural. All logic is preserved exactly, just reorganized into smaller, focused functions.

**Dependencies**:
- Phases 1-4 can be done in sequence
- Phase 5 depends on Phases 1-4 being complete
- Each phase is independently testable

**Expected Final Metrics**:
- Cyclomatic Complexity: 19 → ~9
- Cognitive Complexity: 69 → ~30
- Function Length: 180 → ~40 lines
- Max Nesting: 5 → 2
