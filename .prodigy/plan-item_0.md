# Implementation Plan: Reduce Cognitive Complexity in list_resumable_jobs_internal

## Problem Summary

**Location**: ./src/cook/execution/state.rs:DefaultJobStateManager::list_resumable_jobs_internal:962
**Priority Score**: 51.94
**Debt Type**: ComplexityHotspot (cognitive: 26, cyclomatic: 7)
**Current Metrics**:
- Lines of Code: 31
- Functions: 1
- Cyclomatic Complexity: 7
- Cognitive Complexity: 26
- Coverage: Good (22 test cases)

**Issue**: While cyclomatic complexity of 7 is manageable, the cognitive complexity of 26 is high due to nested async operations and inline logic. The function could benefit from extracting guard clauses and simplifying the control flow.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.5
- Coverage Improvement: 0.0
- Risk Reduction: 18.18

**Success Criteria**:
- [x] Reduce cognitive complexity below 15
- [x] Maintain cyclomatic complexity at 7 or below
- [x] All existing tests continue to pass
- [x] No clippy warnings
- [x] Proper formatting

## Implementation Phases

### Phase 1: Extract Early Return Guard Clause

**Goal**: Simplify the function by extracting the directory existence check into a clearer early return pattern.

**Changes**:
- Extract the metadata check into a dedicated helper function `ensure_jobs_dir_exists`
- Simplify the early return logic to be more explicit
- Reduce nesting by one level

**Testing**:
- Run `cargo test test_list_resumable_empty_no_jobs_dir` to verify empty directory handling
- Run `cargo test test_list_resumable_empty_dir` to verify empty directory case
- All existing tests should continue passing

**Success Criteria**:
- [x] Early return pattern is clearer
- [x] All tests pass
- [x] Ready to commit

### Phase 2: Extract Job Directory Processing Logic

**Goal**: Move the core directory processing logic into a dedicated helper function to reduce cognitive load.

**Changes**:
- Create `process_job_directory` helper that handles a single directory entry
- Moves the validation and job building logic out of the main loop
- Reduces complexity in the main function

**Testing**:
- Run `cargo test test_list_resumable_invalid_metadata` for invalid metadata handling
- Run `cargo test test_list_resumable_file_not_dir` for non-directory entries
- Run `cargo test test_list_resumable_many_jobs` for bulk processing

**Success Criteria**:
- [x] Main function is simpler
- [x] Helper function is focused and testable
- [x] All tests pass
- [x] Ready to commit

### Phase 3: Simplify Entry Collection Pattern

**Goal**: Use a more functional approach to collecting resumable jobs, reducing cognitive complexity.

**Changes**:
- Convert the while loop to use iterator combinators where appropriate
- Use `filter_map` pattern for cleaner optional handling
- Maintain async compatibility while improving readability

**Testing**:
- Run `cargo test test_list_resumable_multiple_mixed_jobs` for mixed job scenarios
- Run `cargo test test_list_resumable_partial_failures` for partial failure cases
- Run full test suite: `cargo test --lib state`

**Success Criteria**:
- [x] Cognitive complexity reduced below 15
- [x] Code is more readable and functional
- [x] All tests pass
- [x] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib state` to verify all state tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Run `cargo fmt --check` to ensure formatting

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test --lib` - All library tests
3. Verify cognitive complexity reduction using code analysis tools

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - likely an async/await issue or test expectation
3. Adjust the refactoring to maintain exact behavior
4. Retry

## Notes

- The function is already well-tested with 22 test cases covering various edge cases
- The main complexity comes from nested async operations and error handling
- Helper functions are already in place (`is_valid_job_directory`, `try_build_resumable_job`) which is good
- Focus on reducing nesting and improving readability without changing behavior
- The function is incorrectly marked as "PureLogic" when it performs I/O - this is a separate issue not addressed here