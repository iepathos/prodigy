# Implementation Plan: Add Test Coverage for SetupPhaseExecutor::execute_step

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/phases/setup.rs:SetupPhaseExecutor::execute_step:60
**Priority Score**: 29.6
**Debt Type**: TestingGap (0% coverage)
**Current Metrics**:
- Lines of Code: 42
- Function Length: 42 lines
- Cyclomatic Complexity: 5
- Cognitive Complexity: 11
- Coverage: 0.0%
- Uncovered Lines: 60, 67, 75, 80-81, 85-89, 94, 96-97

**Issue**: Business logic with 100% coverage gap, currently 0% covered. Needs 5 test cases to cover all 5 execution paths. The function has 14 upstream callers (including multiple tests) but is itself untested, creating a significant testing gap in the execution pipeline.

**Function Role**: PureLogic (purity confidence: 0.95)

## Target State

**Expected Impact**:
- Complexity Reduction: 0.0 (function is already reasonably simple)
- Coverage Improvement: 100.0% (from 0% to 100%)
- Risk Reduction: 12.432

**Success Criteria**:
- [ ] All 13 uncovered lines have test coverage (lines 60, 67, 75, 80-81, 85-89, 94, 96-97)
- [ ] All 5 execution paths tested (happy path, success, error, non-shell command, failure status)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Test coverage reaches 100% for execute_step function

## Implementation Phases

### Phase 1: Add Happy Path Test

**Goal**: Test the successful execution of a shell command through execute_step

**Changes**:
- Add `test_execute_step_success` that tests:
  - Creating a simple shell command (e.g., "echo 'test output'")
  - Calling execute_step with the command
  - Verifying the output is captured correctly
  - Covering lines 60, 67, 75, 80-81, 94

**Testing**:
- Run `cargo test --lib test_execute_step_success`
- Verify test passes and covers happy path
- Check coverage with `cargo tarpaulin --out Stdout` for the function

**Success Criteria**:
- [ ] Test passes consistently
- [ ] Happy path (lines 67, 75, 80-81, 94) covered
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 2: Add Command Failure Test

**Goal**: Test handling of shell commands that exit with non-zero status

**Changes**:
- Add `test_execute_step_command_failure` that tests:
  - Creating a shell command that fails (e.g., "exit 1" or "false")
  - Calling execute_step with the failing command
  - Verifying proper error handling and error message format
  - Covering lines 85-89 (error handling path)

**Testing**:
- Run `cargo test --lib test_execute_step_command_failure`
- Verify error is properly caught and formatted
- Check that PhaseError::ExecutionFailed is returned with correct message

**Success Criteria**:
- [ ] Test passes and properly captures error
- [ ] Error handling path (lines 85-89) covered
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 3: Add Non-Shell Command Test

**Goal**: Test error handling for unsupported command types

**Changes**:
- Add `test_execute_step_non_shell_command` that tests:
  - Creating a WorkflowStep without a shell command (e.g., with claude or other command types)
  - Calling execute_step with the non-shell step
  - Verifying appropriate error is returned
  - Covering lines 96-97 (else branch for non-shell commands)

**Testing**:
- Run `cargo test --lib test_execute_step_non_shell_command`
- Verify error message indicates only shell commands are supported
- Confirm PhaseError::ExecutionFailed with appropriate message

**Success Criteria**:
- [ ] Test passes and validates error handling
- [ ] Non-shell command path (lines 96-97) covered
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 4: Add Edge Case Tests

**Goal**: Cover remaining edge cases and ensure 100% line coverage

**Changes**:
- Add `test_execute_step_with_stderr_output` that tests:
  - Shell command that produces stderr output
  - Verifying stderr is included in error messages
- Add `test_execute_step_with_empty_output` that tests:
  - Shell command that produces no output (e.g., "true")
  - Verifying empty string is returned successfully
- Ensure all previously uncovered lines are now tested

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Run `cargo tarpaulin --out Stdout` to verify 100% coverage for execute_step
- Verify lines 60, 67, 75, 80-81, 85-89, 94, 96-97 are all covered

**Success Criteria**:
- [ ] All edge case tests pass
- [ ] 100% line coverage achieved for execute_step
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 5: Final Validation and Documentation

**Goal**: Verify complete coverage and add test documentation

**Changes**:
- Run full test suite with `cargo test`
- Run clippy with `cargo clippy`
- Run formatting with `cargo fmt`
- Verify coverage improvement with `cargo tarpaulin`
- Add module-level documentation comments to test file explaining coverage strategy

**Testing**:
- `cargo test --lib` - All tests pass
- `cargo clippy` - No warnings
- `cargo fmt --check` - Properly formatted
- `cargo tarpaulin` - Verify coverage improvement from 0% to 100% for execute_step

**Success Criteria**:
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Coverage for execute_step is 100%
- [ ] Test documentation added
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run specific new test with `cargo test --lib test_name`
3. Check coverage with `cargo tarpaulin --out Stdout | grep execute_step`
4. Verify no clippy warnings with `cargo clippy`

**Final verification**:
1. `cargo test` - Full test suite passes
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Formatting correct
4. `cargo tarpaulin --out Stdout` - Verify 100% coverage for execute_step
5. Review coverage report to confirm all 13 previously uncovered lines are now covered

**Coverage Validation**:
- Baseline: 0% coverage, 13 uncovered lines
- Target: 100% coverage, 0 uncovered lines
- Verify improvement with side-by-side comparison

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or error message
3. Adjust the test implementation:
   - Check subprocess manager mock configuration
   - Verify test environment setup
   - Review error message assertions
4. Retry the phase

**Common Issues**:
- **Subprocess execution fails**: Ensure SubprocessManager is properly configured in test context
- **Error message mismatch**: Check exact error message format in assertions
- **Coverage not improving**: Verify tests are actually calling execute_step, not just public methods

## Notes

**Key Implementation Details**:
- The function delegates to subprocess manager for actual execution
- Tests need to use a real SubprocessManager (production mode) since there's no mock needed for simple shell commands
- Focus on testing the error handling paths and edge cases
- The function is private, so tests will be in the existing setup_test.rs file
- Current tests only test public API (execute, can_skip, validate_context) but not the private execute_step method

**Testing Approach**:
- Direct unit tests of execute_step function (it's private but accessible from test module)
- Use real subprocess execution for simple commands (echo, true, false)
- Mock environment setup with PhaseContext for isolation
- Focus on the 5 execution paths identified in the debt analysis

**Why This Matters**:
- execute_step is core execution logic with 14 upstream callers
- 0% coverage creates significant risk in the execution pipeline
- High cognitive complexity (11) and cyclomatic complexity (5) increase bug risk
- Testing gap despite having many callers suggests integration testing only, no unit tests
