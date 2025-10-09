# Implementation Plan: Reduce Cognitive Complexity in GitHandler::execute

## Problem Summary

**Location**: ./src/commands/handlers/git.rs:GitHandler::execute:147
**Priority Score**: 32.57
**Debt Type**: ComplexityHotspot (Cognitive: 31, Cyclomatic: 9)
**Current Metrics**:
- Lines of Code: 90
- Cyclomatic Complexity: 9
- Cognitive Complexity: 31
- Upstream Callers: 86 (heavily used across tests and handlers)

**Issue**: While cyclomatic complexity (9) is manageable, cognitive complexity (31) is high. The function has multiple responsibilities: validation, auto-staging logic, dry-run handling, and command execution. This makes the code harder to understand and maintain despite being well-structured.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.5
- Coverage Improvement: 0.0 (already well-tested)
- Risk Reduction: 11.4

**Success Criteria**:
- [ ] Cognitive complexity reduced from 31 to ~20 or below
- [ ] Guard clauses extracted into separate validation functions
- [ ] Auto-staging logic isolated into a pure function
- [ ] All 86 existing tests continue to pass without modification
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Extract Validation Guard Clauses

**Goal**: Extract the operation validation logic into a separate pure function to reduce nesting and improve readability.

**Changes**:
- Create pure function `validate_operation(attributes: &HashMap<String, AttributeValue>) -> Result<String, String>`
- Move operation extraction and validation logic (lines 156-161) into this function
- Replace inline validation with call to `validate_operation()`
- Function should be pure and testable in isolation

**Testing**:
- Run `cargo test --lib handlers::git` to verify all git handler tests pass
- Verify error message for missing operation is unchanged
- Check that all 86 upstream callers continue to work correctly

**Success Criteria**:
- [ ] New `validate_operation` function is pure (no side effects)
- [ ] All tests pass without modification
- [ ] Code compiles without warnings
- [ ] Ready to commit with message: "refactor: extract operation validation in GitHandler::execute"

### Phase 2: Extract Auto-Staging Logic

**Goal**: Isolate the auto-staging decision and execution logic into a dedicated function to reduce the cognitive load in the main execute function.

**Changes**:
- Create async function `execute_auto_staging(context: &ExecutionContext, operation: &str, attributes: &HashMap<String, AttributeValue>) -> Result<(), String>`
- Move auto-staging logic (lines 172-191) into this function
- The function should encapsulate both the decision (`should_auto_stage`) and the execution
- Replace inline auto-staging block with call to `execute_auto_staging()`

**Testing**:
- Run `cargo test --lib handlers::git::test_git_commit_with_auto_stage`
- Run `cargo test --lib handlers::git::test_git_commit_auto_stage_failure`
- Run `cargo test --lib handlers::git::test_git_commit_auto_stage_custom_files`
- Verify all auto-staging behavior remains identical

**Success Criteria**:
- [ ] Auto-staging logic is isolated and clearly separated
- [ ] Error messages remain unchanged
- [ ] All auto-staging tests pass without modification
- [ ] Ready to commit with message: "refactor: extract auto-staging logic in GitHandler::execute"

### Phase 3: Extract Dry-Run Response Building

**Goal**: Separate the dry-run response building logic into a pure function to further reduce nesting in the main execute function.

**Changes**:
- Create pure function `build_dry_run_response(operation: &str, git_args: &[String], duration: u64) -> CommandResult`
- Move dry-run response building (lines 196-200) into this function
- Replace inline dry-run block with call to `build_dry_run_response()`
- Function should be pure and independently testable

**Testing**:
- Run `cargo test --lib handlers::git::test_git_commit_dry_run`
- Run all tests with `dry_run` flag to ensure behavior is unchanged
- Verify JSON response format is identical

**Success Criteria**:
- [ ] Dry-run response building is pure function
- [ ] All dry-run tests pass without modification
- [ ] Response format unchanged
- [ ] Ready to commit with message: "refactor: extract dry-run response building in GitHandler::execute"

### Phase 4: Extract Command Execution and Result Processing

**Goal**: Simplify the main execute function by extracting the command execution and result processing logic into a dedicated function.

**Changes**:
- Create async function `execute_git_command(context: &ExecutionContext, operation: String, git_args: Vec<String>, start: Instant) -> CommandResult`
- Move command execution and result processing (lines 204-235) into this function
- This function handles the actual git execution and result transformation
- Replace inline execution block with call to `execute_git_command()`

**Testing**:
- Run `cargo test --lib handlers::git` to verify all tests pass
- Pay special attention to tests for success cases, failure cases, and error handling
- Verify duration tracking works correctly
- Ensure stdout/stderr handling is unchanged

**Success Criteria**:
- [ ] Command execution logic is isolated
- [ ] All execution tests pass without modification
- [ ] Error messages and result format unchanged
- [ ] Duration tracking works correctly
- [ ] Ready to commit with message: "refactor: extract command execution in GitHandler::execute"

### Phase 5: Final Verification and Documentation

**Goal**: Verify the refactoring achieved its goals and ensure code quality standards are met.

**Changes**:
- Verify the main `execute` function now has clear, linear flow with minimal nesting
- Add doc comments to new helper functions explaining their purpose
- Ensure all helper functions follow functional programming principles (pure where possible)
- Run comprehensive test suite

**Testing**:
- Run `cargo test` - Full test suite
- Run `cargo clippy` - No new warnings
- Run `cargo fmt --check` - Proper formatting
- Optional: Run debtmap analysis to verify complexity reduction

**Success Criteria**:
- [ ] Main execute function has reduced cognitive complexity (~20 or below)
- [ ] All 86 upstream callers work correctly
- [ ] All tests pass (no test modifications required)
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Helper functions have clear documentation
- [ ] Ready to commit with message: "refactor: finalize GitHandler::execute complexity reduction"

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib handlers::git` to verify git handler tests pass
2. Run `cargo clippy -- -D warnings` to ensure no new warnings
3. Verify error messages and behavior are unchanged from baseline

**Final verification**:
1. `cargo test` - Full test suite (all 86+ tests must pass)
2. `cargo clippy` - Zero warnings
3. `cargo fmt --check` - Verify formatting
4. Optional: `debtmap analyze` - Verify complexity metrics improved

**Critical Constraint**: NO test modifications are allowed. All 86 upstream callers must continue to work without changes.

## Rollback Plan

If any phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or compilation error
3. Identify what behavior changed unexpectedly
4. Adjust the extraction to preserve exact behavior
5. Retry the phase

If multiple phases fail, consider:
- Are we changing behavior inadvertently?
- Do we need to preserve more context in extracted functions?
- Should we adjust the phase boundaries?

## Notes

### Key Considerations

1. **No Behavioral Changes**: This is a pure refactoring. All 86 existing tests must pass without modification.

2. **Functional Programming**: Follow the codebase guidelines:
   - Pure functions where possible (validation, dry-run response building)
   - Clear separation between I/O (command execution) and logic (validation, decision-making)
   - Single responsibility per function

3. **Cognitive Complexity Reduction**: The goal is to reduce mental overhead by:
   - Eliminating nested conditionals through guard clauses
   - Extracting distinct responsibilities into named functions
   - Creating a linear, easy-to-follow flow in the main execute function

4. **High Test Coverage**: The function has 86 upstream callers (mostly tests), which provides excellent safety for refactoring. Any behavioral changes will be immediately detected.

5. **Incremental Progress**: Each phase is independently valuable and moves us toward the goal. Commit after each successful phase.

### Expected Final Structure

After refactoring, `execute` should have a clear linear flow:
```rust
async fn execute(...) -> CommandResult {
    // Phase 1: Validation
    let operation = validate_operation(&attributes)?;

    // Build git args
    let git_args = Self::build_git_args(&operation, &attributes)?;

    // Phase 2: Auto-staging (if needed)
    execute_auto_staging(context, &operation, &attributes).await?;

    // Phase 3: Dry-run handling
    if context.dry_run {
        return build_dry_run_response(&operation, &git_args, duration);
    }

    // Phase 4: Command execution
    execute_git_command(context, operation, git_args, start).await
}
```

This structure is much easier to understand and maintain while preserving all existing behavior.
