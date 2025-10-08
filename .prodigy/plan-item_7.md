# Implementation Plan: Add Test Coverage and Refactor `handle_incomplete_validation`

## Problem Summary

**Location**: ./src/cook/workflow/executor/validation.rs:WorkflowExecutor::handle_incomplete_validation:160
**Priority Score**: 31.589442719099992
**Debt Type**: TestingGap (0% coverage, cognitive complexity: 56, cyclomatic complexity: 20)

**Current Metrics**:
- Lines of Code: 112
- Functions: 1 (monolithic function)
- Cyclomatic Complexity: 20 (high - requires ~20 test cases for full branch coverage)
- Cognitive Complexity: 56 (very high - difficult to understand)
- Coverage: 0.0% direct, 18.2% transitive
- Uncovered Lines: 35 ranges (lines 160-265)

**Issue**: Complex business logic with 100% coverage gap. This function handles validation retry logic with multiple branches for command execution, failure handling, and user interaction. The high cyclomatic complexity (20) requires at least 20 test cases for full path coverage. With 11 downstream dependencies, this is a critical orchestration point that needs comprehensive testing.

**Rationale**: Testing before refactoring ensures no regressions. After extracting 11 pure functions, each will need only 3-5 tests instead of 20 tests for the monolithic function.

## Target State

**Expected Impact**:
- Complexity Reduction: 6.0 (from 20 to ~14 cyclomatic complexity)
- Coverage Improvement: 50.0% (from 0% to 50%+)
- Risk Reduction: 13.27 points

**Success Criteria**:
- [ ] Direct test coverage increases from 0% to 80%+
- [ ] Extract at least 11 pure functions with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with rustfmt
- [ ] Each extracted function has 3-5 unit tests
- [ ] Integration tests cover the main orchestration flow

## Implementation Phases

### Phase 1: Add Foundational Integration Tests

**Goal**: Establish baseline test coverage for the main execution paths before refactoring.

**Changes**:
- Add test module with mock setup for `WorkflowExecutor`
- Create mock implementations for `UserInteraction`, `ValidationConfig`, `OnIncompleteConfig`
- Write 5 integration tests covering critical paths:
  1. Success after first retry attempt
  2. Failure after max attempts exhausted
  3. Multiple command execution success
  4. Single command handler execution
  5. Interactive prompt handling

**Testing**:
- Run `cargo test --lib -- handle_incomplete_validation` to verify tests pass
- Verify coverage improvement with `cargo tarpaulin --lib`
- Ensure no existing tests break

**Success Criteria**:
- [ ] 5 new integration tests added
- [ ] Coverage increases from 0% to ~25-30%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Pure Decision Functions

**Goal**: Extract decision logic into pure, testable functions.

**Changes**:
Extract these 4 pure functions to the top of the file:
1. `should_continue_retry(attempts: u32, max_attempts: u32, is_complete: bool) -> bool`
   - Determines if retry loop should continue
   - Complexity: 1 (simple boolean logic)

2. `determine_handler_type(on_incomplete: &OnIncompleteConfig) -> HandlerType`
   - Returns enum: `MultiCommand | SingleCommand | NoHandler`
   - Complexity: 2 (3 branches)

3. `calculate_retry_progress(attempts: u32, max_attempts: u32, completion: f64) -> RetryProgress`
   - Pure calculation of retry state
   - Complexity: 1

4. `should_fail_workflow(is_complete: bool, fail_workflow_flag: bool, attempts: u32) -> bool`
   - Determines if workflow should fail
   - Complexity: 2

**Testing**:
- Write 3-5 unit tests per function (16 tests total)
- Test edge cases: max_attempts=0, completion=0.0, completion=100.0
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] 4 pure functions extracted
- [ ] 16 unit tests added
- [ ] Coverage increases to ~40-45%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Command Execution Orchestration

**Goal**: Separate command execution logic into testable functions.

**Changes**:
Extract these 3 functions as methods on `WorkflowExecutor`:
1. `async fn execute_multi_commands(&mut self, commands: &[WorkflowCommand], env: &ExecutionEnvironment, ctx: &mut WorkflowContext) -> Result<bool>`
   - Executes array of recovery commands
   - Returns success/failure
   - Complexity: 3

2. `async fn execute_single_handler(&mut self, handler: &WorkflowStep, env: &ExecutionEnvironment, ctx: &mut WorkflowContext) -> Result<bool>`
   - Executes single command handler
   - Returns success/failure
   - Complexity: 2

3. `async fn execute_no_handler(&mut self) -> Result<bool>`
   - Displays error for missing handler
   - Returns false
   - Complexity: 1

**Testing**:
- Write integration tests for each function (9 tests total)
- Mock `execute_step` to return controlled results
- Test command array execution with partial failures
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] 3 execution functions extracted
- [ ] 9 integration tests added
- [ ] Coverage increases to ~55-60%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Display and User Interaction Logic

**Goal**: Isolate all user interaction into pure formatting + display calls.

**Changes**:
Extract these 4 pure formatting functions:
1. `format_retry_attempt_message(attempt: u32, max_attempts: u32) -> String`
   - Pure string formatting
   - Complexity: 1

2. `format_validation_status_message(percentage: f64, threshold: f64, is_complete: bool) -> String`
   - Pure string formatting with conditional
   - Complexity: 2

3. `format_recovery_step_progress(step_idx: usize, total_steps: usize, step_name: &str) -> String`
   - Pure string formatting
   - Complexity: 1

4. `format_workflow_failure_message(attempts: u32, completion: f64) -> String`
   - Pure string formatting
   - Complexity: 1

**Testing**:
- Write 3-4 unit tests per function (14 tests total)
- Test edge cases: attempt=0, percentage=0.0, percentage=100.0
- Test string interpolation correctness
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] 4 formatting functions extracted
- [ ] 14 unit tests added
- [ ] Coverage increases to ~70-75%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Refactor Main Function and Complete Coverage

**Goal**: Simplify `handle_incomplete_validation` by using extracted functions and achieve 80%+ coverage.

**Changes**:
- Refactor `handle_incomplete_validation` to use all extracted functions:
  - Replace inline decision logic with pure decision functions
  - Replace command execution blocks with orchestration functions
  - Replace inline formatting with pure formatting functions
- Add final integration tests for:
  1. Prompt confirmation flow
  2. Result context updates
  3. Edge case: max_attempts=1
  4. Edge case: validation passes immediately after first retry
  5. Error propagation from command failures

**Testing**:
- Write 5 comprehensive integration tests
- Run full test suite: `cargo test --lib`
- Run `cargo tarpaulin --lib` to verify coverage ≥80%
- Run `cargo clippy` to ensure no warnings
- Run `cargo fmt --check` to ensure formatting

**Success Criteria**:
- [ ] `handle_incomplete_validation` reduced from 112 lines to ~60 lines
- [ ] Cyclomatic complexity reduced from 20 to ~14
- [ ] Cognitive complexity reduced from 56 to ~30
- [ ] 5 integration tests added
- [ ] Coverage reaches 80%+
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib -- validation` to verify existing tests pass
2. Run `cargo test --lib -- <new_test_module>` for new tests
3. Run `cargo clippy -- -D warnings` to check for warnings
4. Run `cargo fmt --check` to verify formatting

**Phase-specific verification**:
- **Phase 1-2**: Focus on unit tests for pure functions
- **Phase 3-4**: Integration tests with mocked dependencies
- **Phase 5**: End-to-end integration tests

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib --exclude-files 'tests/*'` - Verify coverage ≥80%
3. `debtmap analyze` - Verify improvement in complexity and coverage metrics

**Coverage Targets by Phase**:
- Phase 1: ~25-30% (baseline integration tests)
- Phase 2: ~40-45% (pure decision functions)
- Phase 3: ~55-60% (command execution)
- Phase 4: ~70-75% (formatting functions)
- Phase 5: 80%+ (complete coverage)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure:
   - Check test output for specific failure
   - Run `cargo clippy` to identify issues
   - Verify mock setup is correct
3. Adjust the plan:
   - Break phase into smaller sub-phases if needed
   - Add missing test cases
   - Fix mock implementations
4. Retry with adjusted approach

**Common failure scenarios**:
- **Test compilation errors**: Check that extracted functions are properly scoped (pub(crate) vs private)
- **Mock setup issues**: Ensure all required traits are implemented on mocks
- **Coverage not improving**: Verify tests actually exercise the target function

## Notes

**Key Considerations**:
- This function is an async orchestration function with many side effects (user interaction, command execution)
- Pure functions should be extracted for ALL decision logic and formatting
- Command execution functions will remain async but should be thin wrappers
- Mock `UserInteraction` and `ExecutionEnvironment` for testing
- Focus on testing business logic, not implementation details

**Dependencies**:
- Downstream callees (11): All must continue to work unchanged
- Upstream callers (1): `WorkflowExecutor::handle_validation` must not break
- Transitive coverage comes from 2 functions - ensure they remain testable

**Refactoring Patterns**:
- **Decision logic** → Pure functions (testable with unit tests)
- **Command execution** → Thin async wrappers (testable with integration tests + mocks)
- **User interaction** → Pure formatting + display calls (format functions are pure and testable)

**Expected Outcome**:
After all phases, the codebase will have:
- 1 simplified orchestration function (~60 lines, complexity ~14)
- 11 extracted pure functions (each ≤3 complexity)
- 63+ unit and integration tests
- 80%+ test coverage
- Significantly improved maintainability and readability
