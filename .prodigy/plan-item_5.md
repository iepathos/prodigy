# Implementation Plan: Test and Refactor execute_setup_phase

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/coordination/executor.rs:MapReduceCoordinator::execute_setup_phase:261
**Priority Score**: 30.19
**Debt Type**: TestingGap (100% coverage gap, 0% current coverage)
**Current Metrics**:
- Lines of Code: 85
- Functions: 1 (monolithic)
- Cyclomatic Complexity: 13
- Cognitive Complexity: 42
- Coverage: 0%
- Nesting Depth: 5

**Issue**: Complex business logic with 100% coverage gap. Cyclomatic complexity of 13 requires at least 13 test cases for full path coverage. The function handles setup phase execution with complex error formatting, logging, and conditional logic that should be extracted into testable pure functions.

## Target State

**Expected Impact**:
- Complexity Reduction: 3.9 (target: ~9 cyclomatic complexity)
- Coverage Improvement: 50% (from 0% to 50%+)
- Risk Reduction: 12.68

**Success Criteria**:
- [ ] 8+ unit tests covering critical branches (error paths, Claude logs, exit codes)
- [ ] Extract 8 pure functions for error formatting, validation, and logging
- [ ] Each extracted function has cyclomatic complexity ≤ 3
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`
- [ ] Test coverage for execute_setup_phase reaches 50%+

## Implementation Phases

### Phase 1: Add Integration Tests for Current Behavior

**Goal**: Establish test coverage for the existing monolithic function to prevent regressions during refactoring.

**Changes**:
- Add test module `execute_setup_phase_tests` in executor.rs
- Create mock implementations for dependencies (like existing `handle_on_failure_tests`)
- Write 8 integration tests covering:
  1. Success path: all setup steps execute successfully
  2. Shell command failure with exit code
  3. Shell command failure with stderr output
  4. Shell command failure with stdout only
  5. Claude command failure with log hint
  6. Multiple setup steps with mixed success/failure
  7. Environment variable setup verification
  8. Debug logging context verification

**Testing**:
```bash
cargo test execute_setup_phase_tests --lib
```

**Success Criteria**:
- [ ] 8 new tests added and passing
- [ ] Tests use mocks to avoid external dependencies
- [ ] Tests cover main execution paths
- [ ] `cargo test --lib` passes
- [ ] Ready to commit

### Phase 2: Extract Error Message Building Logic

**Goal**: Extract error formatting into pure, testable functions.

**Changes**:
- Create new helper functions (around line 345, after execute_setup_phase):
  1. `fn format_setup_error(step_index: usize, exit_code: Option<i32>, stderr: Option<&str>, stdout: Option<&str>) -> String`
     - Pure function to build error message
     - Handles exit code formatting
     - Conditionally includes stderr/stdout
     - Complexity target: ≤3

  2. `fn should_include_stdout(stderr: Option<&str>) -> bool`
     - Pure predicate: includes stdout only if stderr is empty
     - Complexity: 1

  3. `fn format_claude_log_hint(project_root: &Path, job_id: &str) -> Option<String>`
     - Extracts repo name and formats log hint
     - Returns None if repo name extraction fails
     - Complexity: ≤2

- Refactor execute_setup_phase to use these helpers (lines 302-338)
- Add unit tests for each extracted function (6-8 tests per function)

**Testing**:
```bash
cargo test format_setup_error --lib
cargo test should_include_stdout --lib
cargo test format_claude_log_hint --lib
cargo test execute_setup_phase_tests --lib
```

**Success Criteria**:
- [ ] 3 new pure functions extracted
- [ ] Each function has cyclomatic complexity ≤3
- [ ] 15+ unit tests for extracted functions
- [ ] Integration tests still pass
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy` clean
- [ ] Ready to commit

### Phase 3: Extract Environment Setup Logic

**Goal**: Separate environment variable configuration into testable functions.

**Changes**:
- Create helper functions:
  1. `fn create_setup_env_vars() -> HashMap<String, String>`
     - Pure function to create standard env vars
     - Returns map with PRODIGY_CLAUDE_STREAMING and PRODIGY_AUTOMATION
     - Complexity: 1

  2. `fn format_debug_context(step: &WorkflowStep, working_dir: &Path, project_root: &Path, worktree: Option<&str>, session_id: &str) -> Vec<String>`
     - Pure function to format debug log lines
     - Returns vector of debug message strings
     - Complexity: 1

- Refactor execute_setup_phase to use these helpers (lines 287-297)
- Add unit tests for each function

**Testing**:
```bash
cargo test create_setup_env_vars --lib
cargo test format_debug_context --lib
cargo test execute_setup_phase_tests --lib
```

**Success Criteria**:
- [ ] 2 new pure functions extracted
- [ ] 8+ unit tests for extracted functions
- [ ] Integration tests still pass
- [ ] Execute_setup_phase is now ~40 lines (down from 85)
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy` clean
- [ ] Ready to commit

### Phase 4: Extract Step Validation and Result Handling

**Goal**: Create pure functions for result validation and decision logic.

**Changes**:
- Create helper functions:
  1. `fn is_step_failure(result: &StepResult) -> bool`
     - Pure predicate for failure detection
     - Complexity: 1

  2. `fn build_step_error(step_index: usize, total_steps: usize, result: &StepResult, is_claude: bool, log_hint: Option<String>) -> String`
     - Orchestrates error message building using Phase 2 functions
     - Complexity: ≤3

  3. `fn should_show_log_hint(step: &WorkflowStep) -> bool`
     - Pure predicate: returns true if step is a Claude command
     - Complexity: 1

- Refactor execute_setup_phase to use these helpers
- Add unit tests for each function

**Testing**:
```bash
cargo test is_step_failure --lib
cargo test build_step_error --lib
cargo test should_show_log_hint --lib
cargo test execute_setup_phase_tests --lib
```

**Success Criteria**:
- [ ] 3 new pure functions extracted
- [ ] 10+ unit tests for extracted functions
- [ ] Integration tests still pass
- [ ] Execute_setup_phase is now ~25-30 lines
- [ ] Total of 8+ extracted functions across all phases
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy` clean
- [ ] Ready to commit

### Phase 5: Final Verification and Coverage Check

**Goal**: Verify all targets are met and document improvements.

**Changes**:
- Run full test suite with coverage analysis
- Verify cyclomatic complexity reduction
- Update any documentation if needed
- Ensure all 8+ extracted functions have complexity ≤3

**Testing**:
```bash
just ci                          # Full CI checks
cargo tarpaulin --lib            # Coverage analysis
cargo test --lib                 # All tests pass
```

**Success Criteria**:
- [ ] Test coverage for execute_setup_phase ≥ 50%
- [ ] 8+ pure functions extracted, each with complexity ≤3
- [ ] 40+ total unit tests (integration + extracted function tests)
- [ ] Original function reduced from 85 to ~25-30 lines
- [ ] All tests passing
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run specific test module for the phase
4. Verify test coverage increases progressively

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Generate coverage report
3. Verify coverage improvement from 0% to 50%+
4. Verify function count increased from 1 to 8+
5. Verify cyclomatic complexity reduced from 13 to ~9 or better

**Test Organization**:
- Integration tests: `execute_setup_phase_tests` module
- Unit tests: Inline with extracted helper functions
- Follow existing patterns from `handle_on_failure_tests` module

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure:
   - Check test output for specific failures
   - Verify mock implementations are correct
   - Check for unintended behavior changes
3. Adjust the plan:
   - Break phase into smaller steps if needed
   - Add more tests to catch edge cases
   - Simplify extracted function signatures
4. Retry with adjustments

## Notes

### Key Architectural Decisions

1. **Test-First Approach**: Phase 1 establishes integration tests before refactoring to ensure no regressions
2. **Pure Function Extraction**: All extracted functions should be pure (no side effects) for easy testing
3. **Incremental Complexity Reduction**: Each phase reduces complexity and line count gradually
4. **Preserve Behavior**: All refactoring must maintain exact existing behavior

### Gotchas to Watch For

1. **Error Message Formatting**: Ensure extracted functions maintain exact error message format
2. **Log Hint Logic**: Claude command detection must work correctly for log hints
3. **Optional Values**: Handle None cases carefully in stderr/stdout logic
4. **Path Handling**: Repository name extraction can fail - handle gracefully
5. **Mock Dependencies**: Use same mock patterns as `handle_on_failure_tests`

### Functional Programming Principles Applied

1. **Pure Functions**: All extracted helpers are pure (input → output, no side effects)
2. **Single Responsibility**: Each extracted function does one thing
3. **Testability**: Pure functions are trivial to unit test
4. **Immutability**: Functions don't modify inputs
5. **Separation of Concerns**: I/O (execute_setup_step) separated from logic (error formatting)

### Expected Outcomes

- **Before**: 1 function, 85 lines, complexity 13, 0% coverage
- **After**: 8+ functions, main function ~25-30 lines, complexity ~9, 50%+ coverage
- **Test Count**: 0 → 40+ tests
- **Maintainability**: Significantly improved - each piece independently testable
