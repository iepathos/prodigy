# Implementation Plan: Reduce Nesting Depth in list_resumable_jobs_internal

## Problem Summary

**Location**: ./src/cook/execution/state.rs:DefaultJobStateManager::list_resumable_jobs_internal:884
**Priority Score**: 60.102062072615965
**Debt Type**: ComplexityHotspot (cognitive: 56, cyclomatic: 10)
**Current Metrics**:
- Lines of Code: 59
- Cyclomatic Complexity: 10
- Cognitive Complexity: 56
- Nesting Depth: 6

**Issue**: The function has excessive nesting depth (6 levels) due to nested conditionals and loops. This high cognitive complexity (56) makes the code difficult to understand and maintain. The debtmap analysis recommends reducing nesting through guard clauses and early returns.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 5.0
- Coverage Improvement: 0.0
- Risk Reduction: 21.035721725415588

**Success Criteria**:
- [ ] Reduce nesting depth from 6 to 3 or fewer levels
- [ ] Reduce cognitive complexity from 56 to ~30 or below
- [ ] Extract nested logic into helper functions with clear names
- [ ] All existing 22 tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Extract Job Entry Validation

**Goal**: Extract the outer validation logic into a helper function to reduce initial nesting

**Changes**:
- Create a new helper function `is_valid_job_directory(&Path) -> Option<String>` that:
  - Checks if path is a directory (async metadata check)
  - Validates the job_id can be extracted from the directory name
  - Returns `Option<String>` with job_id if valid, None otherwise
- Replace the nested `if let Ok(metadata)... if metadata.is_dir()... if let Some(job_id)` chain with a single guard clause using the helper

**Testing**:
- Run `cargo test --lib tests::test_list_resumable_file_not_dir` to verify file rejection
- Run `cargo test --lib tests::test_list_resumable_invalid_filename` to verify invalid names are skipped
- Run `cargo test --lib tests::test_list_resumable_special_chars_in_name` to verify valid names work

**Success Criteria**:
- [ ] Nesting depth reduced by 2 levels in the main loop
- [ ] Helper function has clear, testable logic
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Extract Checkpoint Processing Logic

**Goal**: Extract the checkpoint loading and validation into a pure helper function

**Changes**:
- Create a new helper function `build_resumable_job(job_id: &str, state: MapReduceJobState, checkpoints: Vec<CheckpointInfo>) -> Option<ResumableJob>` that:
  - Takes a loaded state and checkpoint list
  - Returns `None` if job is complete (is_complete check)
  - Returns `Some(ResumableJob)` with calculated values if incomplete
- Replace the nested `if !state.is_complete` block with a call to this helper
- Use early continue in the match error arm instead of nested logic

**Testing**:
- Run `cargo test --lib tests::test_list_resumable_complete_job` to verify complete jobs are filtered
- Run `cargo test --lib tests::test_list_resumable_invalid_checkpoint` to verify invalid checkpoints are skipped
- Run `cargo test --lib tests::test_list_resumable_max_checkpoint_version` to verify version calculation

**Success Criteria**:
- [ ] Inner conditional nesting reduced by 1-2 levels
- [ ] Checkpoint processing logic is pure and independently testable
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Checkpoint Version Calculation

**Goal**: Simplify the checkpoint version extraction into a pure helper function

**Changes**:
- Create a new pure helper function `get_latest_checkpoint_version(checkpoints: Vec<CheckpointInfo>) -> u32` that:
  - Takes a list of checkpoints
  - Returns the maximum version number or 0 if empty
  - Handles the `unwrap_or_default()` and `unwrap_or(0)` logic
- Update `build_resumable_job` to use this helper instead of inline chain

**Testing**:
- Run `cargo test --lib tests::test_list_resumable_empty_checkpoint_list` to verify empty list handling
- Run `cargo test --lib tests::test_list_resumable_high_checkpoint_version` to verify high version numbers
- Run `cargo test --lib tests::test_list_resumable_mixed_checkpoint_versions` to verify max calculation

**Success Criteria**:
- [ ] Checkpoint version logic is pure and testable
- [ ] Code is more readable with clear intent
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Simplify Main Loop with Guard Clauses

**Goal**: Use guard clauses to flatten the remaining nesting in the main loop

**Changes**:
- Replace the nested `match` with early `continue` statements:
  - `let job_id = match is_valid_job_directory(&path).await { Some(id) => id, None => continue };`
  - `let state = match self.checkpoint_manager.load_checkpoint(&job_id).await { Ok(s) => s, Err(_) => continue };`
  - `let checkpoints = self.checkpoint_manager.list_checkpoints(&job_id).await.unwrap_or_default();`
  - `if let Some(job) = build_resumable_job(&job_id, state, checkpoints) { resumable_jobs.push(job); }`

**Testing**:
- Run full test suite: `cargo test --lib state::tests`
- Verify all 22 upstream caller tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] Main loop has linear flow with guard clauses
- [ ] Maximum nesting depth is 3 or fewer
- [ ] All 22 tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Verification and Cleanup

**Goal**: Verify all improvements and ensure code quality

**Changes**:
- Run full CI checks with `just ci` (or `cargo test && cargo clippy`)
- Review helper functions for documentation
- Add inline comments if any guard clauses need clarification
- Verify cognitive complexity reduction with complexity metrics

**Testing**:
- Run `cargo test` for full test suite
- Run `cargo clippy -- -D warnings` to ensure no warnings
- Run `cargo fmt` to ensure formatting
- Optionally: Run `debtmap analyze` to verify improvement in metrics

**Success Criteria**:
- [ ] All tests pass (22+ tests for this function)
- [ ] No clippy warnings
- [ ] Code is formatted correctly
- [ ] Nesting depth <= 3
- [ ] Cognitive complexity reduced by ~40%
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run targeted tests related to the specific change:
   - `cargo test --lib state::tests::test_list_resumable_<specific_test>`
2. Run all tests for the module after each phase:
   - `cargo test --lib state::tests`
3. Check for clippy warnings:
   - `cargo clippy -- -D warnings`

**Final verification**:
1. `cargo test` - All tests across the codebase
2. `cargo clippy` - No warnings
3. `cargo fmt -- --check` - Formatting is correct
4. Optionally: `debtmap analyze` - Verify debt score improvement

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure output
3. Identify what assumption was incorrect
4. Adjust the implementation approach
5. Retry the phase with corrections

## Notes

**Why This Approach Works**:
- Each helper function extracts a clear responsibility
- Pure functions are easier to test and reason about
- Guard clauses eliminate nesting without changing behavior
- Incremental refactoring allows verification at each step

**Key Insights**:
- The function is already covered by 22 comprehensive tests
- No behavior changes are needed - only structural improvements
- The async nature means we need to keep the checkpoint manager calls in the main function
- Helper functions should be private to the module (not public API changes)

**Potential Gotchas**:
- Must preserve the exact error handling behavior (Err(_) => continue)
- Must maintain the order of operations (metadata check before job_id extraction)
- Must not introduce new unwrap() calls (violates Spec 101)
- The `unwrap_or_default()` on line 910 is acceptable as it provides a safe fallback
