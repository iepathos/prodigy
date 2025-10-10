# Implementation Plan: Extract Pure Parsing Logic from GitRunnerImpl::status

## Problem Summary

**Location**: ./src/subprocess/git.rs:GitRunnerImpl::status:79
**Priority Score**: 48.625
**Debt Type**: ComplexityHotspot (cognitive: 14, cyclomatic: 8)
**Current Metrics**:
- Lines of Code: 43
- Cyclomatic Complexity: 8
- Cognitive Complexity: 14
- Nesting Depth: 2
- Upstream Dependencies: 41 callers
- Downstream Dependencies: 10 callees

**Issue**: While cyclomatic complexity of 8 is manageable, the cognitive complexity of 14 indicates the function mixes I/O orchestration with parsing logic. The debtmap recommends prioritizing test coverage and extracting guard clauses, but more importantly, the parsing loop (lines 98-113) can be extracted as a pure function to improve testability and reduce cognitive load.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.0
- Risk Reduction: 17.02%
- Coverage Improvement: 0.0 (maintaining existing coverage)

**Success Criteria**:
- [ ] Extract git status output parsing into a pure function
- [ ] Reduce cyclomatic complexity of `status` method by 4 points (from 8 to ~4)
- [ ] Maintain all 40+ existing test cases without modification
- [ ] Add new unit tests for the extracted pure parsing function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Pure Parsing Function

**Goal**: Create a pure function `parse_git_status_output` that takes the git status output string and returns parsed data, separating I/O from parsing logic.

**Changes**:
- Create new pure function `parse_git_status_output(output: &str) -> (Option<String>, Vec<String>, Vec<String>)`
- Function returns tuple of (branch, untracked_files, modified_files)
- Move the parsing loop (lines 98-113) into this new function
- Keep the existing helper functions (`parse_branch_line`, `parse_untracked_line`, `parse_modified_line`) as they are
- Update `status` method to call the new parsing function

**Testing**:
- Verify all existing integration tests still pass (they test via the public API)
- Run `cargo test --lib git_error_tests` to verify all 40+ test cases pass
- Run `cargo test --lib` to verify no regressions

**Success Criteria**:
- [ ] New pure function compiles without warnings
- [ ] All existing tests pass unchanged
- [ ] `status` method now only handles I/O and delegates parsing
- [ ] Cyclomatic complexity reduced in `status` method
- [ ] Ready to commit

### Phase 2: Add Unit Tests for Pure Parsing Function

**Goal**: Add comprehensive unit tests directly for the pure parsing function to improve test coverage and make edge cases more explicit.

**Changes**:
- Create new test module `parse_git_status_output_tests` for unit tests
- Add tests that directly call `parse_git_status_output` with various input strings
- Cover edge cases that are currently only tested through integration tests:
  - Empty output
  - Branch-only output
  - Files without branch
  - Mixed status scenarios
  - Edge cases (empty lines, whitespace, malformed input)
- Tests should be simpler than existing integration tests (no MockProcessRunner needed)

**Testing**:
- Run `cargo test --lib parse_git_status_output_tests` for new unit tests
- Run `cargo test --lib git_error_tests` to verify existing tests still pass
- All new tests should pass

**Success Criteria**:
- [ ] 10-15 new unit tests added for the pure function
- [ ] Tests cover all parsing logic branches
- [ ] All tests pass (new and existing)
- [ ] Test coverage for parsing logic increases
- [ ] Ready to commit

### Phase 3: Extract Guard Clause as Pure Function (Optional Enhancement)

**Goal**: Extract the error checking logic into a named function to make the intent more explicit and further reduce complexity.

**Changes**:
- Create pure function `check_git_command_success(status: ExitStatus) -> Result<(), ProcessError>`
- Replace the inline error check (lines 90-92) with call to this function
- Makes error handling more explicit and testable

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Error handling behavior should be identical

**Success Criteria**:
- [ ] Guard clause extracted into named function
- [ ] All tests pass
- [ ] Error handling behavior unchanged
- [ ] Code intent is clearer
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib subprocess` to verify subprocess module tests pass
2. Run `cargo clippy -- -D warnings` to check for any new warnings
3. Run `cargo fmt --check` to verify formatting
4. Run individual test modules to verify specific functionality

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test` - All tests pass
3. Review complexity metrics - should show reduction from baseline

**Integration Test Preservation**:
- All 40+ existing tests in `git_error_tests` module must pass unchanged
- These tests provide regression coverage through the public API
- No modifications to existing test assertions

## Rollback Plan

If a phase fails:
1. Check git status: `git status`
2. Review the error messages and test failures
3. If tests fail: Review the specific test and ensure parsing logic is equivalent
4. If needed: Revert the phase with `git reset --hard HEAD~1`
5. Review what went wrong
6. Adjust the implementation approach
7. Retry with fixes

Common issues and solutions:
- **Test failures**: Ensure the new parsing function produces identical output to the original loop
- **Type mismatches**: Verify the function signature matches expected types
- **Clippy warnings**: Address any new warnings about complexity or style
- **Edge case handling**: Check that empty strings, whitespace, and special characters are handled correctly

## Notes

### Why This Refactoring?

1. **Separation of Concerns**: Currently the `status` method mixes:
   - I/O execution (running the git command)
   - Error handling (checking exit code)
   - Parsing logic (processing output line by line)

   Extracting the parsing reduces the method to just I/O and error handling.

2. **Improved Testability**: The parsing logic can be tested with simple unit tests instead of requiring MockProcessRunner setup for every edge case.

3. **Reduced Cognitive Load**: The main method becomes a simple orchestration: run command, check error, parse output, return result.

4. **Functional Programming**: Aligns with the functional programming guidelines - pure functions for logic, I/O at the boundaries.

### Current Structure
```
status() {
  run git command          // I/O
  check exit code         // Guard clause
  loop through output {   // Parsing (complexity hotspot)
    parse branch
    parse untracked
    parse modified
  }
  return result
}
```

### Target Structure (After Phase 1)
```
status() {
  run git command                    // I/O
  check exit code                   // Guard clause
  (branch, untracked, modified) =   // Parsing (delegated)
    parse_git_status_output()
  return result
}

parse_git_status_output() {         // Pure function
  loop through output {
    parse branch
    parse untracked
    parse modified
  }
  return (branch, untracked, modified)
}
```

### Impact on Callers

No impact - this is an internal refactoring. The public API (`GitRunner::status`) remains unchanged, and all 41 upstream callers will continue to work without modification.

### Complexity Reduction Calculation

Current complexity sources in `status`:
- Branch statement (if/else for exit code): +1
- Loop over lines: +1
- Branch line check: +1
- Continue: +1
- Untracked line check: +1
- Continue: +1
- Modified line check: +1
- Total: ~8

After extraction:
- Branch statement (if/else for exit code): +1
- Call to pure function: 0 (no branching)
- Total: ~4 (reduction of 4, matching debtmap prediction)

The extracted function will have complexity ~4-5, but being pure and focused, it will be much easier to test and understand.
