# Implementation Plan: Test Coverage and Refactoring for ResumeExecutor::execute_from_checkpoint

## Problem Summary

**Location**: ./src/cook/workflow/resume.rs:ResumeExecutor::execute_from_checkpoint:337
**Priority Score**: 32.06
**Debt Type**: TestingGap (99% coverage gap with high complexity)

**Current Metrics**:
- Lines of Code: 546
- Cyclomatic Complexity: 54
- Cognitive Complexity: 227
- Coverage: 1.36% (only 7 of 546 lines covered)
- Function Role: PureLogic
- Nesting Depth: 6

**Issue**: This function has complex business logic with a 99% coverage gap. With a cyclomatic complexity of 54, it requires at least 54 test cases for full path coverage. The high cognitive complexity (227) indicates the function is doing too much and needs to be broken down into smaller, testable units.

## Target State

**Expected Impact**:
- Complexity Reduction: 16.2 (from 54 to ~38)
- Coverage Improvement: 49.3% (from 1.36% to ~50%)
- Risk Reduction: 13.5%

**Success Criteria**:
- [ ] Coverage increases from 1.36% to at least 50%
- [ ] All critical error paths are tested
- [ ] Complex logic extracted into pure functions (≤3 complexity each)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Foundation Testing - Critical Paths

**Goal**: Establish baseline test coverage for the most critical execution paths

**Changes**:
- Create comprehensive test module in `tests/resume_executor_test.rs`
- Test happy path: successful execution from checkpoint
- Test error paths: missing executors, invalid checkpoint
- Test checkpoint validation scenarios
- Test workflow context restoration

**Testing Strategy**:
- Use existing test patterns from `tests/checkpoint_resume_integration_test.rs`
- Create mock implementations for ClaudeExecutor, SessionManager, UserInteraction
- Use TempDir for isolated test environments
- Target 20% coverage improvement (~100 lines covered)

**Test Cases** (8-10 tests):
1. `test_execute_from_checkpoint_success` - Happy path execution
2. `test_execute_from_checkpoint_missing_claude_executor` - Error when executor not set
3. `test_execute_from_checkpoint_missing_session_manager` - Error when session manager not set
4. `test_execute_from_checkpoint_missing_user_interaction` - Error when user interaction not set
5. `test_execute_from_checkpoint_invalid_checkpoint` - Validation failure handling
6. `test_execute_from_checkpoint_already_completed` - Handle completed workflow
7. `test_execute_from_checkpoint_restore_context` - Context restoration
8. `test_execute_from_checkpoint_workflow_file_parsing` - YAML/JSON parsing

**Success Criteria**:
- [ ] 8-10 tests pass
- [ ] Coverage increases to ~20%
- [ ] All tests are deterministic
- [ ] Ready to commit

### Phase 2: Extract Workflow Loading Logic

**Goal**: Extract workflow file loading and parsing into testable pure functions

**Changes**:
- Extract `load_workflow_file(path) -> Result<WorkflowConfig>`
- Extract `parse_workflow_config(content, extension) -> Result<WorkflowConfig>`
- Extract `convert_to_workflow_steps(commands) -> Vec<WorkflowStep>`
- Reduce nesting in main function by 1 level

**Refactoring Pattern**:
```rust
// Before: Inline parsing (lines 421-435)
let workflow_content = tokio::fs::read_to_string(workflow_path).await?;
let workflow_config: WorkflowConfig = if workflow_path.extension()...

// After: Extracted function
let workflow_config = load_workflow_file(workflow_path).await?;
```

**Testing**:
- Add 3-4 unit tests for each extracted function
- Test YAML parsing, JSON parsing, invalid formats
- Test error cases (missing file, malformed content)

**Success Criteria**:
- [ ] 3 new pure functions with complexity ≤3
- [ ] 10-12 new tests added
- [ ] Coverage increases to ~30%
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Extract Progress Tracking Logic

**Goal**: Separate progress tracking and display from core execution logic

**Changes**:
- Extract `create_progress_tracker(checkpoint) -> SequentialProgressTracker`
- Extract `setup_progress_display(tracker) -> ProgressDisplay`
- Extract `update_progress_for_step(tracker, display, step_index, step_name)`
- Reduce cognitive load by isolating I/O from logic

**Refactoring Pattern**:
```rust
// Before: Inline progress setup (lines 389-413)
let mut progress_tracker = SequentialProgressTracker::for_resume(...);
let mut progress_display = ProgressDisplay::new();
progress_tracker.update_phase(...).await;
progress_display.force_update(...);

// After: Extracted function
let (mut progress_tracker, mut progress_display) =
    setup_progress_tracking(&checkpoint, workflow_id).await?;
```

**Testing**:
- Add 2-3 tests per extracted function
- Mock progress tracker and display
- Test state transitions

**Success Criteria**:
- [ ] 3 new functions extracted
- [ ] 6-9 new tests added
- [ ] Coverage increases to ~40%
- [ ] Nesting depth reduced to 5
- [ ] Ready to commit

### Phase 4: Extract Error Recovery Logic

**Goal**: Isolate complex error recovery branching into dedicated functions

**Changes**:
- Extract `handle_step_failure(step, error, context) -> Result<RecoveryOutcome>`
- Extract `execute_error_handler(handler, error, context) -> Result<bool>`
- Extract `apply_recovery_action(action, executor, context) -> Result<RecoveryOutcome>`
- Extract `execute_cleanup_actions(actions, executor, context) -> Result<()>`

**Refactoring Pattern**:
```rust
// Before: Large match statement (lines 698-840)
match recovery_action {
    RecoveryAction::Retry { delay, .. } => { ... }
    RecoveryAction::Continue => { ... }
    // ... many branches
}

// After: Extracted function
let outcome = apply_recovery_action(
    recovery_action,
    &mut executor,
    &env,
    &mut workflow_context
).await?;
```

**Testing**:
- Add 3-4 tests per extracted function
- Test each recovery action type
- Test cleanup execution
- Test error handler integration

**Success Criteria**:
- [ ] 4 new functions extracted
- [ ] 12-16 new tests added
- [ ] Coverage increases to ~50%
- [ ] Cyclomatic complexity reduced to ~40
- [ ] Ready to commit

### Phase 5: Extract Step Execution Loop

**Goal**: Break down the main execution loop into manageable, testable pieces

**Changes**:
- Extract `should_skip_step(step_index, start_from) -> bool`
- Extract `prepare_step_execution(step, step_index, total_steps) -> StepExecutionContext`
- Extract `execute_step_with_recovery(step, executor, env, context) -> Result<StepResult>`
- Extract `finalize_execution(progress_tracker, steps_executed, total_steps) -> ExecutionSummary`

**Refactoring Pattern**:
```rust
// Before: Large for loop (lines 609-843)
for (step_index, step) in extended_workflow.steps.iter().enumerate() {
    if step_index < start_from { ... }
    // Complex execution logic
}

// After: Extracted functions
for (step_index, step) in extended_workflow.steps.iter().enumerate() {
    if should_skip_step(step_index, start_from) {
        skip_and_continue(&mut progress_tracker, step_index).await;
        continue;
    }
    let result = execute_step_with_recovery(...).await?;
    handle_step_result(result, &mut progress_tracker).await?;
}
```

**Testing**:
- Add 2-3 tests per extracted function
- Test step skipping logic
- Test execution context preparation
- Test result handling

**Success Criteria**:
- [ ] 4 new functions extracted
- [ ] 8-12 new tests added
- [ ] Coverage reaches 50%+
- [ ] Cyclomatic complexity reduced to ~38
- [ ] Nesting depth reduced to 4
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

### Test Structure
```rust
// tests/resume_executor_test.rs
mod execute_from_checkpoint_tests {
    // Phase 1: Foundation tests
    mod happy_path { ... }
    mod error_handling { ... }
    mod checkpoint_validation { ... }

    // Phase 2: Workflow loading tests
    mod workflow_loading { ... }

    // Phase 3: Progress tracking tests
    mod progress_tracking { ... }

    // Phase 4: Error recovery tests
    mod error_recovery { ... }

    // Phase 5: Execution loop tests
    mod execution_loop { ... }
}
```

### Mock Helpers
Create reusable test helpers:
- `MockClaudeExecutor` - Simulates Claude command execution
- `MockSessionManager` - Simulates session state
- `MockUserInteraction` - Simulates user prompts
- `create_test_checkpoint()` - Standard checkpoint builder
- `create_test_workflow()` - Standard workflow builder

### Coverage Verification
After each phase:
```bash
cargo tarpaulin --lib --out Html --output-dir coverage
open coverage/index.html  # Verify coverage improvements
```

## Rollback Plan

If a phase fails:

1. **Identify the issue**:
   ```bash
   cargo test --lib  # See which tests fail
   cargo clippy      # Check for warnings
   ```

2. **Revert the phase**:
   ```bash
   git reset --hard HEAD~1
   ```

3. **Review and adjust**:
   - Analyze test failures
   - Check if extracted functions are truly pure
   - Verify mock implementations are correct
   - Adjust complexity targets if needed

4. **Retry with adjustments**:
   - Smaller extraction units
   - More focused test cases
   - Better separation of I/O and logic

## Notes

### Function Complexity Guidelines
- **Target**: Each extracted function ≤3 cyclomatic complexity
- **Pattern**: Prefer early returns over nested conditionals
- **Principle**: Single responsibility - one clear purpose per function

### Testing Guidelines
- **Deterministic**: No flaky tests - use fixed timestamps, controlled randomness
- **Isolated**: Each test sets up its own environment (TempDir)
- **Fast**: Mock I/O operations, avoid real file system where possible
- **Clear**: Test names describe scenario and expected outcome

### Error Handling Reminder
- **CRITICAL**: Replace `unwrap()` with proper error propagation using `?`
- **CRITICAL**: Use `.context()` or `.with_context()` to add error context
- **CRITICAL**: Return `Result<T>` from all fallible operations
- **Pattern**: `value.ok_or_else(|| anyhow!("error message"))?`

### Key Uncovered Areas (Priority for Testing)
Based on the 270+ uncovered lines, focus on:
1. **Lines 348-355**: Executor validation and error handling
2. **Lines 367-386**: Checkpoint validation and completion check
3. **Lines 421-435**: Workflow file parsing (YAML/JSON)
4. **Lines 476-521**: Command parsing and step conversion
5. **Lines 609-843**: Main execution loop with error recovery
6. **Lines 651-840**: Step failure and recovery action handling

### Integration with Existing Code
- Follow patterns from `tests/checkpoint_resume_integration_test.rs`
- Reuse checkpoint creation helpers where available
- Maintain consistency with existing error messages
- Preserve all existing behavior (no breaking changes)

### Performance Considerations
- Extracted functions are primarily synchronous transformations
- Async only where necessary (I/O operations)
- Progress tracking remains async for real-time updates
- No performance degradation expected from extraction

### Documentation
After each phase, update:
- Function doc comments with `///` syntax
- Module-level documentation for test organization
- Inline comments only for non-obvious logic
