# Implementation Plan: Split Oversized Test File

## Problem Summary

**Location**: ./src/cook/workflow/git_context_tests.rs:file:0
**Priority Score**: 205.03
**Debt Type**: God Object (test file)
**Current Metrics**:
- Lines of Code: 914
- Functions: 25 test functions
- Cyclomatic Complexity: 354 total (14.16 avg, 33 max)
- Coverage: 0% (test code itself)

**Issue**: URGENT - 914 lines, 25 functions in a single test file! The file has low cohesion with all functions grouped under a generic "Utilities" responsibility. This makes the test suite hard to navigate, maintain, and understand. The tests are logically organized into 3 phases (uncommitted changes, commit history, diff statistics) but physically located in one monolithic file.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 70.8 points
- Maintainability Improvement: 20.5 points
- Test Effort Reduction: 91.4 points

**Success Criteria**:
- [ ] Test file split into 3 focused modules (<30 functions each)
- [ ] Each module has clear responsibility (Phase 1, 2, 3)
- [ ] Shared test utilities extracted to common module
- [ ] All 25 tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Extract Test Utilities Module

**Goal**: Create a shared test utilities module with the `init_test_repo` helper and common imports.

**Changes**:
- Create `src/cook/workflow/git_context_test_utils.rs`
- Move `init_test_repo()` function to utilities module
- Make it public for use by test submodules
- Include common imports (anyhow::Result, git2::Repository, tempfile::TempDir, etc.)

**Testing**:
- Run `cargo test --lib git_context` to verify existing tests still pass
- Ensure the utility module compiles

**Success Criteria**:
- [ ] New utilities module created
- [ ] `init_test_repo()` extracted and public
- [ ] All existing tests still pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Phase 1 Tests (Uncommitted Changes)

**Goal**: Move Phase 1 tests (uncommitted changes detection) into a dedicated submodule.

**Changes**:
- Create `src/cook/workflow/git_context_uncommitted_tests.rs`
- Move 9 tests from Phase 1 (lines 118-371):
  - `test_calculate_step_changes_with_new_file`
  - `test_calculate_step_changes_with_modified_file`
  - `test_calculate_step_changes_with_deleted_file`
  - `test_calculate_step_changes_with_staged_new_file`
  - `test_calculate_step_changes_with_staged_modification`
  - `test_calculate_step_changes_with_staged_deletion`
  - `test_calculate_step_changes_with_mixed_changes`
- Import `init_test_repo` from utilities module
- Update module documentation

**Testing**:
- Run `cargo test --lib git_context_uncommitted` to verify Phase 1 tests pass
- Verify all 7 uncommitted change tests execute correctly

**Success Criteria**:
- [ ] Phase 1 tests moved to dedicated module
- [ ] Tests use shared utilities
- [ ] All Phase 1 tests pass (7 tests)
- [ ] Original file reduced by ~250 lines
- [ ] Ready to commit

### Phase 3: Extract Phase 2 Tests (Commit History)

**Goal**: Move Phase 2 tests (commit history walking) into a dedicated submodule.

**Changes**:
- Create `src/cook/workflow/git_context_commit_tests.rs`
- Move 8 tests from Phase 2 (lines 375-570):
  - `test_calculate_step_changes_with_new_commit`
  - `test_calculate_step_changes_with_multiple_commits`
  - `test_calculate_step_changes_with_commit_stats`
  - `test_calculate_step_changes_with_no_new_commits`
  - `test_calculate_step_changes_tracks_commit_shas`
- Import `init_test_repo` from utilities module
- Update module documentation

**Testing**:
- Run `cargo test --lib git_context_commit` to verify Phase 2 tests pass
- Verify all 5 commit history tests execute correctly

**Success Criteria**:
- [ ] Phase 2 tests moved to dedicated module
- [ ] Tests use shared utilities
- [ ] All Phase 2 tests pass (5 tests)
- [ ] Original file reduced by another ~200 lines
- [ ] Ready to commit

### Phase 4: Extract Phase 3 Tests (Diff Statistics)

**Goal**: Move Phase 3 tests (diff statistics and file changes) into a dedicated submodule.

**Changes**:
- Create `src/cook/workflow/git_context_diff_tests.rs`
- Move 6 tests from Phase 3 (lines 574-913):
  - `test_calculate_step_changes_tracks_added_files_from_commits`
  - `test_calculate_step_changes_tracks_modified_files_from_commits`
  - `test_calculate_step_changes_tracks_deleted_files_from_commits`
  - `test_calculate_step_changes_deduplicates_files`
  - `test_calculate_step_changes_sorts_file_lists`
  - `test_calculate_step_changes_calculates_insertions_deletions`
  - `test_calculate_step_changes_handles_mixed_commit_and_uncommitted`
- Import `init_test_repo` from utilities module
- Update module documentation

**Testing**:
- Run `cargo test --lib git_context_diff` to verify Phase 3 tests pass
- Verify all 7 diff statistics tests execute correctly

**Success Criteria**:
- [ ] Phase 3 tests moved to dedicated module
- [ ] Tests use shared utilities
- [ ] All Phase 3 tests pass (7 tests)
- [ ] Original file reduced by another ~340 lines
- [ ] Ready to commit

### Phase 5: Finalize and Clean Up Original File

**Goal**: Keep only basic unit tests in the original file and set up module structure.

**Changes**:
- Keep only the simple unit tests in original file:
  - `test_step_changes` (tests StepChanges struct)
  - `test_filter_files` (tests filter functionality)
  - `test_format_file_list` (tests formatting)
  - `test_tracker_initialization` (tests basic init)
  - `test_non_git_directory` (tests non-git handling)
- Add module declarations for the new test modules
- Update file documentation to reference the test phase organization
- Ensure original file is now ~100 lines (down from 914)

**Testing**:
- Run `cargo test --lib git_context` to verify all tests pass
- Verify test count is still 25 total
- Run `cargo clippy` to ensure no warnings

**Success Criteria**:
- [ ] Original file reduced to ~100 lines
- [ ] Module structure properly declared
- [ ] All 25 tests still pass
- [ ] Test execution time unchanged
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run specific test module: `cargo test --lib git_context_<phase>`
3. Run `cargo clippy` to check for warnings
4. Verify test count matches expected (use `cargo test --lib -- --list`)

**Final verification**:
1. `cargo test --lib` - All tests pass (25 total)
2. `cargo clippy` - No warnings
3. `just ci` - Full CI checks pass
4. Verify file sizes: each new module <300 lines, original <150 lines

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or compilation errors
3. Check that imports and module declarations are correct
4. Verify `init_test_repo` is properly exported from utilities
5. Retry with corrections

## Notes

**Module Organization**:
- `git_context_test_utils.rs` - Shared test helpers (~30 lines)
- `git_context_uncommitted_tests.rs` - Phase 1: Uncommitted changes (~250 lines, 7 tests)
- `git_context_commit_tests.rs` - Phase 2: Commit history (~200 lines, 5 tests)
- `git_context_diff_tests.rs` - Phase 3: Diff statistics (~340 lines, 7 tests)
- `git_context_tests.rs` - Basic unit tests (~100 lines, 6 tests)

**Benefits**:
- Clear separation of concerns (uncommitted vs committed vs diff analysis)
- Easier to navigate and find relevant tests
- Reduced cognitive load per file
- Better alignment with the 3-phase test organization in comments
- Shared utilities eliminate duplication

**Potential Gotchas**:
- Ensure all test modules are properly declared in parent module
- Verify imports work correctly across module boundaries
- Check that `init_test_repo` visibility is correct (pub vs pub(crate))
- Test discovery should work the same (cargo test finds all #[test] functions)
