# Implementation Plan: Refactor GitChangeTracker::calculate_step_changes

## Problem Summary

**Location**: src/cook/workflow/git_context.rs:GitChangeTracker::calculate_step_changes:318
**Priority Score**: 23.465
**Debt Type**: ComplexityHotspot (Cognitive: 80, Cyclomatic: 27)

**Current Metrics**:
- Lines of Code: 104
- Cyclomatic Complexity: 27
- Cognitive Complexity: 80
- Nesting Depth: 6
- Function Role: PureLogic
- Purity: Not pure (0.76 confidence)

**Issue**: High complexity (27/80) makes the function hard to test and maintain. The function mixes I/O operations (git2 library calls) with business logic (change aggregation), has deep nesting (6 levels), and handles multiple responsibilities:
1. Opening repository and getting current HEAD
2. Calculating uncommitted changes via git status
3. Calculating committed changes via revwalk
4. Computing diff statistics
5. Processing file changes from commits
6. Normalizing and deduplicating results

## Target State

**Expected Impact**:
- Complexity Reduction: 13.5 (from 27 to ~13-14)
- Coverage Improvement: 0.0 (already well-tested)
- Risk Reduction: 8.21

**Success Criteria**:
- [ ] Cyclomatic complexity reduced to ≤14
- [ ] Cognitive complexity reduced to <40
- [ ] Nesting depth reduced to ≤4 levels
- [ ] All 24 existing tests continue to pass (git_context_*_tests.rs)
- [ ] No clippy warnings introduced
- [ ] Proper formatting maintained
- [ ] Function length reduced to <60 lines

## Implementation Phases

### Phase 1: Extract Uncommitted Changes Logic

**Goal**: Extract the git status processing into a focused helper function, reducing initial complexity.

**Changes**:
- Extract status collection and file change detection (lines 327-344) into `collect_uncommitted_changes(repo: &Repository) -> Result<StepChanges>`
- This pure function will:
  - Take a Repository reference
  - Configure StatusOptions
  - Iterate through statuses
  - Classify and collect changes
  - Return a StepChanges struct with uncommitted files

**Testing**:
- Run existing Phase 1 tests (git_context_uncommitted_tests.rs - 8 tests)
- Verify `test_calculate_step_changes_with_new_file` passes
- Verify `test_calculate_step_changes_with_modified_file` passes

**Success Criteria**:
- [ ] New function `collect_uncommitted_changes` extracted
- [ ] Function is <20 lines
- [ ] Nesting depth reduced by 1 level
- [ ] Cyclomatic complexity reduced by ~3-4
- [ ] All 8 Phase 1 tests pass
- [ ] `cargo clippy` shows no new warnings
- [ ] Ready to commit

### Phase 2: Extract Commit History Collection

**Goal**: Extract revwalk logic into a focused helper function for collecting commit SHAs.

**Changes**:
- Extract commit collection (lines 354-361) into `collect_commits_between(repo: &Repository, from_oid: Oid, to_oid: Oid) -> Result<Vec<String>>`
- This focused function will:
  - Take Repository reference and two OIDs
  - Create and configure revwalk
  - Collect commit SHAs between the two points
  - Return a vector of commit strings

**Testing**:
- Run existing Phase 2 tests (git_context_commit_tests.rs - 6 tests)
- Verify `test_calculate_step_changes_with_commits` passes
- Verify commit tracking tests pass

**Success Criteria**:
- [ ] New function `collect_commits_between` extracted
- [ ] Function is <15 lines
- [ ] Nesting depth reduced by 1 level
- [ ] Cyclomatic complexity reduced by ~2-3
- [ ] All 6 Phase 2 tests pass
- [ ] All 8 Phase 1 tests still pass
- [ ] `cargo clippy` shows no new warnings
- [ ] Ready to commit

### Phase 3: Extract Diff Statistics Calculation

**Goal**: Extract the deeply nested diff processing logic into a focused helper function.

**Changes**:
- Extract diff computation (lines 364-404) into `calculate_diff_stats(repo: &Repository, from_commit: git2::Commit, to_commit: git2::Commit) -> Result<(usize, usize, Vec<String>, Vec<String>, Vec<String>)>`
- This function will:
  - Take Repository reference and two Commits
  - Get trees from commits
  - Create diff between trees
  - Calculate insertions/deletions
  - Process diff deltas to collect file changes
  - Return tuple of (insertions, deletions, added_files, modified_files, deleted_files)

**Testing**:
- Run existing Phase 3 tests (git_context_diff_tests.rs - 10 tests)
- Verify `test_calculate_step_changes_with_diff_stats` passes
- Verify file change detection tests pass

**Success Criteria**:
- [ ] New function `calculate_diff_stats` extracted
- [ ] Function is <30 lines
- [ ] Removes 3 levels of nesting from main function
- [ ] Cyclomatic complexity reduced by ~5-6
- [ ] All 10 Phase 3 tests pass
- [ ] All 14 Phase 1+2 tests still pass
- [ ] `cargo clippy` shows no new warnings
- [ ] Ready to commit

### Phase 4: Simplify Main Function Logic

**Goal**: Refactor the main function to compose the extracted helpers with clearer control flow.

**Changes**:
- Restructure `calculate_step_changes` to use the three extracted functions
- Simplify the conditional logic for commit detection (lines 347-405)
- Extract commit comparison into a focused predicate function `has_new_commits(last: &Option<String>, current: &Option<String>) -> bool`
- Reduce nesting by early returns where appropriate
- Main function should become a clear orchestration:
  1. Open repo and get HEAD
  2. Collect uncommitted changes
  3. If commits exist: collect commits and diff stats
  4. Merge changes
  5. Normalize and return

**Testing**:
- Run full test suite: `cargo test git_context`
- Verify all 24 tests pass (8 + 6 + 10)
- Run `cargo clippy` for complexity validation

**Success Criteria**:
- [ ] Main function is <50 lines
- [ ] Nesting depth ≤3 levels
- [ ] Cyclomatic complexity ≤14
- [ ] Cognitive complexity <40
- [ ] All 24 tests pass
- [ ] `cargo clippy` shows no new warnings
- [ ] Ready to commit

### Phase 5: Final Verification and Documentation

**Goal**: Verify improvements meet targets and update documentation.

**Changes**:
- Run full CI checks: `just ci`
- Verify complexity improvements with any available metrics
- Update function documentation if needed to reflect new structure
- Ensure all helper functions have clear doc comments

**Testing**:
- Full test suite: `cargo test --all`
- Clippy: `cargo clippy --all-targets`
- Format check: `cargo fmt --check`
- Coverage: `cargo tarpaulin` (optional, for verification)

**Success Criteria**:
- [ ] All CI checks pass
- [ ] Complexity reduction validated (target: 27→~13-14)
- [ ] All tests pass (24 git_context tests + full suite)
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Documentation updated
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run phase-specific tests first: `cargo test git_context_{uncommitted,commit,diff}_tests`
2. Run full git_context tests: `cargo test git_context`
3. Run clippy: `cargo clippy --lib`
4. Commit if all checks pass

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test --all` - Complete test suite
3. Review diff for unintended changes
4. Verify function complexity reduction

**Test Organization**:
- Phase 1 tests: `git_context_uncommitted_tests.rs` (8 tests)
- Phase 2 tests: `git_context_commit_tests.rs` (6 tests)
- Phase 3 tests: `git_context_diff_tests.rs` (10 tests)
- Helper tests: `git_utils.rs` (existing pure function tests)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or clippy warning
3. Adjust the extraction approach:
   - Consider smaller extraction
   - Check for missing error handling
   - Verify function signatures match usage
4. Retry with adjusted approach

If multiple phases fail:
1. Consider a different decomposition strategy
2. Re-evaluate which logic can be extracted
3. Consult with team or reassess approach

## Notes

**Why This Approach**:
- Follows existing pattern of extracting to `git_utils.rs` for pure functions
- Incremental extraction allows testing at each step
- Existing comprehensive test suite (24 tests) provides safety net
- Separates I/O (git2 operations) from logic (data aggregation)

**Existing Helpers to Leverage**:
- `classify_file_status()` - already extracted
- `classify_delta_status()` - already extracted
- `add_unique_file()` - already extracted
- `normalize_file_lists()` - already extracted

**Key Risks**:
- Deep nesting in diff processing (lines 367-404) is most complex to extract
- Error handling must be preserved correctly when extracting
- Function signatures need to balance simplicity vs. parameter count

**Not Changing**:
- Public API of `calculate_step_changes()` - remains the same
- Test files - no modifications needed
- Behavior - purely structural refactoring
