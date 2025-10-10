# Implementation Plan: Reduce Cognitive Complexity in list_resumable_jobs_internal

## Problem Summary

**Location**: ./src/cook/execution/state.rs:DefaultJobStateManager::list_resumable_jobs_internal:926
**Priority Score**: 54.34330127018922
**Debt Type**: ComplexityHotspot
**Current Metrics**:
- Lines of Code: 42
- Cognitive Complexity: 33
- Cyclomatic Complexity: 8
- Coverage: Well tested (22 test cases)

**Issue**: The function has high cognitive complexity (33) despite moderate cyclomatic complexity (8). This is due to nested async operations, error handling with continue statements, and inline logic that obscures the function's intent. The recommendation is to extract guard clauses and maintain simplicity while reducing cognitive load.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.0
- Coverage Improvement: 0.0
- Risk Reduction: 19.020155444566228

**Success Criteria**:
- [ ] Cognitive complexity reduced from 33 to ~29 or below
- [ ] Cyclomatic complexity maintained at 8 or reduced
- [ ] All 22 existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`
- [ ] Function length kept under 42 lines (ideally reduced)

## Implementation Phases

### Phase 1: Extract Pure Validation Helper

**Goal**: Extract the job directory validation logic into a pure helper function to reduce nesting and clarify intent.

**Changes**:
- Move `is_valid_job_directory` implementation to top-level or make it more explicit
- Already exists, but ensure it's doing single responsibility

**Testing**:
- Run existing tests: `cargo test list_resumable`
- Verify all 22 test cases pass

**Success Criteria**:
- [ ] Validation logic is clear and testable
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Checkpoint Processing Logic

**Goal**: Extract the checkpoint loading and validation into a separate pure function to reduce cognitive nesting.

**Changes**:
- Create new helper function: `load_job_checkpoint(checkpoint_manager, job_id) -> Option<MapReduceJobState>`
- This function encapsulates the try-load-skip-on-error pattern
- Move the checkpoint manager interaction out of the main loop

**Testing**:
- Run `cargo test list_resumable_invalid_checkpoint`
- Run `cargo test list_resumable_metadata_missing`
- Verify error cases still skip correctly

**Success Criteria**:
- [ ] Checkpoint loading is in separate function
- [ ] Error handling is clear (returns Option)
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract ResumableJob Building Logic

**Goal**: Simplify the main loop by delegating the entire "try to build a resumable job" logic to a single helper.

**Changes**:
- Create helper: `try_build_resumable_job(checkpoint_manager, job_id, path) -> Option<ResumableJob>`
- This function orchestrates: validation, checkpoint loading, checkpoint listing, and building
- Main loop becomes: `while let Some(entry) => if let Some(job) = try_build... => jobs.push(job)`

**Testing**:
- Run full test suite: `cargo test list_resumable`
- Verify all 22 test cases pass
- Check edge cases: empty dirs, invalid files, complete jobs

**Success Criteria**:
- [ ] Main loop is simplified to just iteration and collection
- [ ] Building logic is extracted and testable
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Simplify Main Function Flow

**Goal**: Refactor the main function to use the extracted helpers, reducing cognitive complexity.

**Changes**:
- Refactor `list_resumable_jobs_internal` to:
  1. Early return if jobs_dir doesn't exist
  2. Create result vector
  3. Iterate entries and collect results from helper
  4. Return results
- Main function should be ~15-20 lines max
- All complexity pushed into well-named helper functions

**Testing**:
- Run full test suite: `cargo test list_resumable`
- Run clippy: `cargo clippy`
- Run formatting: `cargo fmt --check`

**Success Criteria**:
- [ ] Main function is clear and linear
- [ ] Cognitive complexity reduced to target (~29 or below)
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Verification and Documentation

**Goal**: Verify the refactoring achieved the target improvements and add documentation.

**Changes**:
- Add doc comments to new helper functions
- Verify cognitive complexity improvement
- Run full CI: `just ci` (if available)
- Regenerate coverage: `cargo tarpaulin` (if needed)

**Testing**:
- Full test suite: `cargo test`
- Full CI checks
- Manual review of code clarity

**Success Criteria**:
- [ ] All success criteria from Problem Summary met
- [ ] Documentation is clear
- [ ] Code is more maintainable
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run targeted tests: `cargo test list_resumable` to verify existing behavior
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting

**Final verification**:
1. `cargo test` - Full test suite
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Proper formatting
4. `just ci` - Full CI checks (if available)
5. Manual code review for clarity and maintainability

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or clippy warnings
3. Analyze what went wrong:
   - Did we break error handling behavior?
   - Did we change the function signature incorrectly?
   - Did we introduce new complexity?
4. Adjust the approach:
   - Consider smaller extraction steps
   - Review helper function signatures
   - Ensure we're not adding complexity
5. Retry with adjusted approach

## Notes

**Key Insights**:
- The function has 22 test cases covering edge cases - these are our safety net
- The cognitive complexity comes from nested async/await, error handling with continue, and inline logic
- The function is already well-structured with helper functions (`is_valid_job_directory`, `build_resumable_job`)
- Goal is to reduce nesting depth and make the main loop more linear

**Functional Programming Approach**:
- Extract pure functions that transform data without side effects
- Separate I/O (checkpoint loading) from logic (validation, building)
- Use Option types for clean error handling (no continue statements)
- Make the main loop a simple iterator-map-filter pattern

**Potential Gotchas**:
- Must preserve exact error handling behavior (skip invalid entries silently)
- Must maintain async behavior correctly
- Cannot change function signatures used by tests
- Need to be careful with async/await in helper functions
