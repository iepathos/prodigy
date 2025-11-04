# Implementation Plan: Refactor git_context.rs Test Suite into Focused Modules

## Problem Summary

**Location**: ./src/cook/workflow/git_context.rs:file:0
**Priority Score**: 205.20
**Debt Type**: God Object (GodClass with 5 fields, 11 methods, 6 responsibilities)
**Current Metrics**:
- Lines of Code: 1383
- Functions: 42 (11 public methods + 31 test functions)
- Cyclomatic Complexity: 436 (max: 33)
- Coverage: 0.0% (test file)

**Issue**: This file contains 1383 lines with mixed production code (481 lines) and test code (901 lines). While the production code is reasonably well-structured, the massive test suite makes the file difficult to navigate and maintain. The test code should be extracted into a focused test module, and complex variable resolution logic could be simplified.

**Root Cause**: The file combines production implementation (GitChangeTracker with 6 responsibilities) with an extensive test suite testing 3 distinct phases of functionality, leading to poor navigability and maintenance overhead.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 87.2 points
- Maintainability Improvement: 20.5 points
- Test Effort Reduction: 138.3 points

**Success Criteria**:
- [ ] Test suite extracted to separate test module (tests/git_context_tests.rs or git_context/tests.rs)
- [ ] Production code remains in git_context.rs (under 500 lines)
- [ ] Variable resolution complexity reduced from cyclomatic complexity 33 to under 10
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting maintained

## Implementation Phases

### Phase 1: Extract Test Helper Functions to Separate Module

**Goal**: Move the test helper function `init_test_repo` and test setup utilities to a dedicated test_helpers submodule to reduce duplication and clarify test structure.

**Changes**:
- Create new file `src/cook/workflow/git_context/test_helpers.rs`
- Move `init_test_repo()` function (lines 487-503) to test_helpers module
- Add any other common test utilities (signature creation, file operations)
- Update git_context.rs to import from test_helpers in #[cfg(test)]

**Testing**:
- Run `cargo test --lib git_context` to verify all tests pass
- Verify helper function is accessible from tests

**Success Criteria**:
- [ ] Test helpers extracted to separate module
- [ ] All 31 tests pass without modification
- [ ] No clippy warnings

### Phase 2: Extract Test Suite to Separate Test File

**Goal**: Move the entire test suite (lines 482-1383) from git_context.rs to a dedicated test file, making the main module more focused and navigable.

**Changes**:
- Create new file `tests/git_context_tests.rs` or organize as `src/cook/workflow/git_context/tests.rs`
- Move all test functions (3 test phases with 31 test functions) to the new file
- Update imports to reference the production code correctly
- Ensure test_helpers module is accessible from the new test location

**Testing**:
- Run `cargo test --lib` to verify all tests still pass
- Run `cargo test git_context` specifically to confirm test discovery
- Verify git_context.rs is now under 500 lines

**Success Criteria**:
- [ ] All 31 tests moved to separate file
- [ ] Production code in git_context.rs reduced to ~481 lines
- [ ] All tests pass without modification
- [ ] Test organization maintained (Phase 1, 2, 3 sections)

### Phase 3: Simplify Variable Resolution Logic

**Goal**: Refactor the `resolve_variable()` method (lines 376-429) and `resolve_step_variable()` (lines 432-474) to reduce cyclomatic complexity from 33 to under 10 through functional decomposition.

**Changes**:
- Extract format parsing logic into pure function `parse_variable_format(modifier: Option<&str>) -> VariableFormat`
- Extract pattern filtering logic into pure function `extract_glob_pattern(modifier: Option<&str>) -> Option<&str>`
- Simplify match expressions by extracting common patterns
- Create helper function `resolve_file_list_variable()` for files_added/modified/deleted cases
- Add unit tests for the new pure functions

**Testing**:
- Run existing variable resolution tests to ensure behavior unchanged
- Add tests for new helper functions
- Verify cyclomatic complexity reduced with `cargo clippy`

**Success Criteria**:
- [ ] Variable resolution split into 3-4 pure functions
- [ ] Each function under 20 lines
- [ ] Cyclomatic complexity under 10 per function
- [ ] All tests pass
- [ ] New tests for extracted functions

### Phase 4: Add Module-Level Documentation

**Goal**: Improve module documentation to clearly describe the new structure, test organization, and usage examples.

**Changes**:
- Update module-level doc comment to reflect new structure
- Add documentation for test_helpers module
- Document the three test phases in test file comments
- Add examples of variable resolution patterns

**Testing**:
- Run `cargo doc --open` to verify documentation renders correctly
- Check for any rustdoc warnings

**Success Criteria**:
- [ ] Module documentation updated and accurate
- [ ] Test organization documented
- [ ] No rustdoc warnings
- [ ] Examples compile and run

### Phase 5: Final Verification and Cleanup

**Goal**: Ensure all changes integrate correctly, tests pass, and metrics show improvement.

**Changes**:
- Run full CI suite with `just ci`
- Verify code formatting with `cargo fmt --check`
- Check for clippy warnings with `cargo clippy`
- Run coverage analysis with `cargo tarpaulin` (if available)
- Run debtmap analysis to verify improvement

**Testing**:
- `cargo test --all` - All tests pass
- `cargo clippy -- -D warnings` - No clippy warnings
- `cargo fmt --check` - Proper formatting
- `debtmap analyze` - Reduced complexity score

**Success Criteria**:
- [ ] All 31+ tests pass
- [ ] No clippy warnings
- [ ] Properly formatted
- [ ] Debtmap shows complexity reduction
- [ ] File under 500 lines

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib git_context` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. Commit after successful verification

**Final verification**:
1. `just ci` - Full CI checks pass
2. `cargo tarpaulin` - Coverage maintained or improved
3. `debtmap analyze` - Verify complexity reduction and maintainability improvement
4. Manual review of file structure and readability

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and error messages
3. Identify root cause (imports, test discovery, logic error)
4. Adjust the plan based on findings
5. Retry with corrected approach

## Notes

**Why this approach**:
- The production code (lines 1-481) is already reasonably well-structured with clear separation of concerns
- The real problem is the 901-line test suite (65% of the file) making it hard to navigate
- Variable resolution has high cyclomatic complexity (33) that can be reduced through functional decomposition
- This approach maintains all existing functionality and tests while improving structure

**Test organization**:
- Phase 1 tests: Uncommitted changes detection (lines 586-840, 8 tests)
- Phase 2 tests: Commit history walking (lines 844-1039, 6 tests)
- Phase 3 tests: Diff statistics and file changes (lines 1043-1382, 17 tests)
- This organization should be preserved in the extracted test file

**Complexity hotspots**:
- `resolve_variable()` (lines 376-429): Cyclomatic complexity 15+
- `resolve_step_variable()` (lines 432-474): Cyclomatic complexity 10+
- `calculate_step_changes()` (lines 245-347): Large but sequential, complexity is acceptable

**Key constraints**:
- This is a test file with 0% coverage, so we're refactoring test code itself
- Must maintain all 31 existing tests without breaking them
- Focus on structure and navigability rather than changing test behavior
- Ensure test discovery works correctly after extraction
