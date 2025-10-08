# Implementation Plan: Refactor execute_command with Functional Patterns

## Problem Summary

**Location**: ./src/abstractions/claude.rs:RealClaudeClient::execute_command:407
**Priority Score**: 33.74
**Debt Type**: ComplexityHotspot (Cognitive: 41, Cyclomatic: 16)
**Current Metrics**:
- Lines of Code: 99
- Cyclomatic Complexity: 16
- Cognitive Complexity: 41
- Nesting Depth: 4

**Issue**: This function has moderate cyclomatic complexity (16) and high cognitive complexity (41), combining retry logic, error classification, command building, and result handling in a single large function. The recommendation is to apply functional patterns by extracting 4 pure functions with Iterator chains.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 8.0
- Coverage Improvement: 0.0
- Risk Reduction: 11.81

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 16 to ≤8
- [ ] Cognitive complexity reduced from 41 to ≤25
- [ ] Pure functions extracted for predicates, transformations
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Pure Predicate Functions

**Goal**: Extract error classification and retry decision logic into pure, testable functions

**Changes**:
- Already exists: `is_transient_error(stderr: &str) -> bool` (line 326)
- Already exists: `should_retry_error(error_type, attempt, max_retries) -> bool` (line 376)
- Already exists: `classify_command_error(error, stderr) -> CommandErrorType` (line 357)
- Add unit tests for these three existing pure functions to ensure they're properly validated

**Testing**:
- Add tests for `is_transient_error` with various error patterns
- Add tests for `should_retry_error` with different retry scenarios
- Add tests for `classify_command_error` with ProcessError variants
- Run `cargo test --lib`

**Success Criteria**:
- [ ] Tests added for all three predicate functions
- [ ] All tests pass
- [ ] Coverage for predicates is >80%
- [ ] Ready to commit

### Phase 2: Extract Command Building Logic

**Goal**: Separate command construction into a pure function

**Changes**:
- Extract function: `build_claude_command(args: &[&str], env_vars: Option<&HashMap<String, String>>) -> ProcessCommand`
- This function takes arguments and environment variables and returns a configured ProcessCommand
- Remove command building logic from main retry loop
- Use the extracted function in `execute_command`

**Testing**:
- Add unit tests for `build_claude_command` with various argument combinations
- Test with and without environment variables
- Verify existing integration tests still pass
- Run `cargo test --lib`

**Success Criteria**:
- [ ] Command building extracted to pure function
- [ ] Unit tests for command building pass
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Extract Result Processing Logic

**Goal**: Create pure functions for processing command execution results

**Changes**:
- Extract function: `classify_output_result(output: &ProcessOutput) -> OutputClassification`
  - Returns enum: `Success`, `TransientFailure(String)`, `PermanentFailure`
- Extract function: `should_continue_retry(classification: &OutputClassification, attempt: u32, max_retries: u32) -> bool`
- Simplify main retry loop to use these classification functions
- Note: `convert_to_std_output` already exists (line 387)

**Testing**:
- Add tests for `classify_output_result` with various ProcessOutput states
- Add tests for `should_continue_retry` logic
- Verify retry behavior is preserved
- Run `cargo test --lib`

**Success Criteria**:
- [ ] Result classification functions extracted
- [ ] Unit tests for classification pass
- [ ] Retry behavior unchanged
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Refactor Main Loop with Functional Patterns

**Goal**: Simplify the main retry loop using the extracted pure functions

**Changes**:
- Refactor retry loop to use extracted functions
- Reduce nesting by early returns
- Use pattern matching more effectively
- Simplify control flow with the pure functions
- Goal: Reduce cyclomatic complexity to ≤8

**Testing**:
- Run full test suite: `cargo test`
- Run clippy: `cargo clippy`
- Verify verbose output behavior is preserved
- Test retry scenarios manually if needed

**Success Criteria**:
- [ ] Main function cyclomatic complexity ≤8
- [ ] Nesting depth ≤2
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Add Property-Based Tests

**Goal**: Verify function invariants with property-based testing

**Changes**:
- Add proptest tests for retry logic invariants:
  - Retry count never exceeds max_retries
  - Transient errors always retry (when attempts remain)
  - Permanent errors never retry
  - Delay increases exponentially
- Add proptest tests for error classification:
  - Known patterns always classified correctly
  - Classification is consistent

**Testing**:
- Run property-based tests: `cargo test`
- Ensure tests catch edge cases
- Verify no regressions

**Success Criteria**:
- [ ] Property-based tests added
- [ ] Tests discover and validate key invariants
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure consistent formatting
4. Verify new tests cover the extracted functions

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage improvements
3. Verify cognitive/cyclomatic complexity reduction
4. Manual testing of retry scenarios if needed

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or compilation errors
3. Adjust the implementation approach
4. Retry with refined strategy

## Notes

- The function already has several pure helper functions (`is_transient_error`, `should_retry_error`, `classify_command_error`, `convert_to_std_output`, `calculate_retry_delay`)
- The main complexity comes from the nested retry loop logic with multiple error handling paths
- Focus is on extracting the remaining impure logic into testable chunks and simplifying the main control flow
- Preserve existing behavior exactly - this is a refactoring, not a feature change
- The goal is to make the code more testable and reduce cognitive load, not to change functionality
- Property-based testing will help ensure retry invariants hold across all scenarios
