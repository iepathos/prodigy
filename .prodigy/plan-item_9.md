# Implementation Plan: Add Tests and Refactor `execute_shell_with_retry`

## Problem Summary

**Location**: ./src/cook/workflow/executor/commands.rs:WorkflowExecutor::execute_shell_with_retry:601
**Priority Score**: 31.78
**Debt Type**: TestingGap (100% coverage gap)
**Current Metrics**:
- Lines of Code: 132
- Cyclomatic Complexity: 17
- Cognitive Complexity: 45
- Coverage: 0.0% (direct), 22.2% (transitive)

**Issue**: Complex business logic with 100% coverage gap. Cyclomatic complexity of 17 requires at least 17 test cases for full path coverage. The function handles shell command execution with retry logic, failure handling, temp file management, and Claude debugging commands - all untested.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 5.1
- Coverage Improvement: 50.0%
- Risk Reduction: 13.3%

**Success Criteria**:
- [ ] 80%+ code coverage for `execute_shell_with_retry` and extracted functions
- [ ] Cyclomatic complexity ≤10 for main function
- [ ] 9+ pure helper functions with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Integration Tests for Core Paths

**Goal**: Achieve 40%+ coverage by testing the main execution paths without refactoring

**Changes**:
- Add test module `tests::workflow_executor_shell_retry`
- Test successful execution on first attempt
- Test successful execution after retry
- Test max attempts exceeded with `fail_workflow=true`
- Test max attempts exceeded with `fail_workflow=false`
- Test execution without `on_failure` config

**Testing**:
```bash
cargo test execute_shell_with_retry
cargo tarpaulin --out Html --output-dir coverage
```

**Success Criteria**:
- [ ] 5+ integration tests passing
- [ ] Coverage ≥40% for the function
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Extract Pure Functions - Output Management

**Goal**: Extract temp file and output handling logic into testable pure functions

**Changes**:
- Extract `should_use_temp_file(stdout_len: usize, stderr_len: usize) -> bool`
- Extract `format_shell_output(stdout: &str, stderr: &str) -> String`
- Extract `create_output_temp_file(stdout: &str, stderr: &str) -> Result<NamedTempFile>`
- Update main function to use extracted helpers
- Add unit tests for each extracted function (3-5 tests per function)

**Testing**:
```bash
cargo test output_management
cargo clippy
```

**Success Criteria**:
- [ ] 3 pure functions extracted
- [ ] 10+ unit tests added
- [ ] Complexity reduced by ~3
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Pure Functions - Context Management

**Goal**: Extract variable context setup into testable pure functions

**Changes**:
- Extract `build_shell_context_vars(attempt: u32, exit_code: Option<i32>, output: String) -> HashMap<String, String>`
- Extract `prepare_debug_command(template: &str, ctx_vars: &HashMap<String, String>) -> String`
- Update main function to use extracted helpers
- Add unit tests for each extracted function (3-5 tests per function)

**Testing**:
```bash
cargo test context_management
cargo clippy
```

**Success Criteria**:
- [ ] 2 pure functions extracted
- [ ] 8+ unit tests added
- [ ] Complexity reduced by ~2
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Pure Functions - Retry Logic

**Goal**: Extract retry decision logic into testable pure functions

**Changes**:
- Extract `should_continue_retry(attempt: u32, max_attempts: u32) -> bool`
- Extract `handle_max_attempts_exceeded(max_attempts: u32, fail_workflow: bool, last_result: StepResult) -> Result<StepResult>`
- Extract `should_execute_debug_command(on_failure: Option<&TestDebugConfig>, attempt: u32) -> bool`
- Update main function to use extracted helpers
- Add unit tests for each extracted function (3-5 tests per function)

**Testing**:
```bash
cargo test retry_logic
cargo clippy
```

**Success Criteria**:
- [ ] 3 pure functions extracted
- [ ] 10+ unit tests added
- [ ] Complexity reduced by ~3
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Integration Tests and Coverage Verification

**Goal**: Achieve 80%+ total coverage and verify complexity targets

**Changes**:
- Add edge case tests (large output handling, temp file errors, etc.)
- Add property-based tests for retry logic
- Add tests for debug command failure paths
- Verify all uncovered lines from debtmap are now covered

**Testing**:
```bash
cargo test --lib
cargo tarpaulin --out Html --output-dir coverage
cargo clippy
just ci
```

**Success Criteria**:
- [ ] 80%+ coverage achieved
- [ ] All 39 previously uncovered lines now covered
- [ ] Cyclomatic complexity ≤10
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. Run `cargo tarpaulin` to verify coverage improvements

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --out Html` - Generate coverage report
3. Verify coverage ≥80% for target function
4. Verify complexity ≤10 for main function

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure in test output
3. Adjust the plan (may need smaller extraction steps)
4. Retry with revised approach

## Notes

- The function currently has 0% direct coverage but 22.2% transitive coverage from callers
- 39 uncovered lines identified in debtmap analysis
- Main complexity sources: nested conditionals, retry loop, temp file handling, context variable management
- Follow existing test patterns in `tests::test_mocks` module
- Use `MockUserInteraction` for interaction testing
- Preserve all existing behavior - this is refactoring + testing, not feature changes
