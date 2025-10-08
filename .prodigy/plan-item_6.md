# Implementation Plan: Add Test Coverage for handle_commit_verification

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:WorkflowExecutor::handle_commit_verification:850
**Priority Score**: 31.60
**Debt Type**: TestingGap (Cognitive: 81, Cyclomatic: 21, Coverage: 0%)
**Current Metrics**:
- Lines of Code: 70
- Cyclomatic Complexity: 21
- Cognitive Complexity: 81
- Coverage: 0% (23 uncovered lines)
- Nesting Depth: 6

**Issue**: The `handle_commit_verification` function is a complex piece of business logic with 100% test coverage gap. With cyclomatic complexity of 21, it requires at least 21 test cases for full path coverage. The function has 7 downstream dependencies and handles critical commit verification logic including auto-commits, commit requirements, and change detection.

## Target State

**Expected Impact**:
- Complexity Reduction: 6.3
- Coverage Improvement: 50%
- Risk Reduction: 13.27

**Success Criteria**:
- [ ] Add comprehensive tests covering all 21 branches
- [ ] Extract 16 pure functions from complex conditional logic
- [ ] Achieve 80%+ test coverage for this function and its extracted helpers
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Foundation Tests for Main Scenarios

**Goal**: Establish baseline test coverage for the 4 main execution paths

**Changes**:
- Add test for successful commit scenario (commits created)
- Add test for auto-commit with changes
- Add test for auto-commit with no changes
- Add test for commit_required failure scenario

**Testing**:
- Run `cargo test handle_commit_verification` to verify new tests pass
- Run `cargo test --lib` to ensure no regressions
- Run `cargo tarpaulin --lib` to measure coverage improvement

**Success Criteria**:
- [ ] 4 new tests added and passing
- [ ] Coverage for handle_commit_verification increases to ~20%
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Add Edge Case Tests for Conditional Branches

**Goal**: Cover the complex nested conditional logic with 12 additional tests

**Changes**:
- Add tests for auto_commit disabled with commit_required
- Add tests for check_for_changes returning error
- Add tests for create_auto_commit failures
- Add tests for successful auto-commit with display_success
- Add tests for commit metadata tracking with multiple commits
- Add tests for commit metadata tracking with single commit
- Add tests for different file change counts in commits

**Testing**:
- Run `cargo test handle_commit_verification` to verify all tests pass
- Run `cargo tarpaulin --lib` to measure coverage (target: 50%+)
- Verify all 21 branches are being exercised

**Success Criteria**:
- [ ] 12 additional edge case tests added and passing
- [ ] Coverage for handle_commit_verification reaches 50%+
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Extract Pure Helper Functions (Part 1 - Decision Logic)

**Goal**: Extract 8 pure functions for decision-making logic to reduce complexity

**Changes**:
- Extract `should_auto_commit(step: &WorkflowStep, head_before: &str, head_after: &str) -> bool`
- Extract `should_require_commit(step: &WorkflowStep) -> bool`
- Extract `needs_commit_verification(has_changes: bool, auto_commit: bool, commit_required: bool) -> CommitAction` (enum: CreateCommit, RequireCommit, Skip)
- Extract `determine_commit_strategy(step: &WorkflowStep, has_changes: bool) -> CommitStrategy`
- Extract `format_commit_count_message(count: usize) -> String`
- Extract `format_file_count_message(count: usize) -> String`
- Extract `build_commit_success_message(commit_count: usize, file_count: usize, step_display: &str) -> String`
- Extract `extract_unique_files(commits: &[CommitInfo]) -> HashSet<String>`

**Testing**:
- Add 3-5 unit tests per extracted function
- Run `cargo test` to ensure all tests pass
- Run `cargo clippy` to verify no new warnings

**Success Criteria**:
- [ ] 8 pure helper functions extracted
- [ ] 24-40 new unit tests for helpers added
- [ ] Cyclomatic complexity of handle_commit_verification reduced by ~8
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Pure Helper Functions (Part 2 - Error Handling & State)

**Goal**: Extract 8 more pure functions for error handling and state management

**Changes**:
- Extract `handle_auto_commit_error(error: &Error, step: &WorkflowStep, commit_required: bool) -> Result<()>`
- Extract `handle_check_for_changes_error(error: &Error, step: &WorkflowStep) -> Result<()>`
- Extract `build_commit_context_vars(commits: &[CommitInfo]) -> HashMap<String, String>`
- Extract `extract_commit_hashes(commits: &[CommitInfo]) -> Vec<String>`
- Extract `format_commit_hashes(commits: &[CommitInfo]) -> String`
- Extract `parse_commits_metadata(commits: &[CommitInfo]) -> CommitMetadata` (struct with count, files)
- Extract `should_handle_commit_error(step: &WorkflowStep, auto_commit_failed: bool) -> bool`
- Extract `validate_commit_requirements(step: &WorkflowStep, has_commits: bool) -> Result<()>`

**Testing**:
- Add 3-5 unit tests per extracted function
- Run `cargo test` to ensure all tests pass
- Verify coverage is now 80%+

**Success Criteria**:
- [ ] 8 additional pure helper functions extracted
- [ ] 24-40 new unit tests for helpers added
- [ ] Cyclomatic complexity of handle_commit_verification reduced to ≤8
- [ ] Coverage reaches 80%+
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Refactor Main Function Using Extracted Helpers

**Goal**: Simplify handle_commit_verification by using the 16 extracted pure functions

**Changes**:
- Refactor handle_commit_verification to use extracted helpers
- Reduce nesting depth from 6 to 3 or less
- Improve readability with descriptive function calls
- Ensure all error paths use proper error handling (no unwrap/panic)
- Update existing tests if needed to work with refactored structure

**Testing**:
- Run `cargo test handle_commit_verification` to verify all scenarios still work
- Run `cargo clippy` to ensure no warnings
- Run `cargo tarpaulin --lib` to verify coverage maintained at 80%+
- Run `just ci` to verify full CI passes

**Success Criteria**:
- [ ] handle_commit_verification now uses 16 extracted helpers
- [ ] Cyclomatic complexity reduced to ≤8
- [ ] Nesting depth reduced to ≤3
- [ ] Coverage maintained at 80%+
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests first for the specific functionality being added/refactored
2. Run `cargo test --lib handle_commit_verification` to verify new tests
3. Run `cargo test --lib` to ensure no regressions
4. Run `cargo clippy` to check for warnings
5. Run `cargo fmt` to ensure proper formatting

**Final verification**:
1. `just ci` - Full CI checks including all tests, clippy, and formatting
2. `cargo tarpaulin --lib` - Verify 80%+ coverage for the function
3. Review debtmap output - Verify score reduction and complexity improvement

**Test Coverage Targets by Phase**:
- Phase 1: 20% coverage (4 main paths)
- Phase 2: 50%+ coverage (all 21 branches)
- Phase 3: 60% coverage (helpers add coverage)
- Phase 4: 80%+ coverage (comprehensive helper tests)
- Phase 5: 80%+ coverage maintained (refactored code)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures and error messages
3. Identify the issue (incorrect test assumptions, missing mocks, logic errors)
4. Adjust the approach:
   - For test failures: Review mock setup and test assertions
   - For refactoring issues: Extract smaller functions first
   - For integration issues: Add integration tests before refactoring
5. Retry the phase with fixes

## Notes

### Key Testing Challenges:
- The function uses async git operations (mocked via MockGitOperations)
- User interaction display methods need mocking (MockUserInteraction)
- Multiple error paths require careful error injection
- Commit metadata extraction needs realistic commit data

### Mock Setup Required:
- MockGitOperations for get_current_head responses
- MockGitOperations for check_for_changes responses
- MockGitOperations for get_commits_between responses
- MockUserInteraction to verify display_success calls
- Mock commit creation and verification

### Function Dependencies to Mock:
- `get_current_head()` - Returns git HEAD hash
- `check_for_changes()` - Returns bool for git status
- `create_auto_commit()` - Creates commit, can fail
- `get_commits_between()` - Returns commit metadata
- `handle_no_commits_error()` - Raises error for commit_required
- `generate_commit_message()` - Generates commit message (needs implementation or mock)

### Existing Test Patterns:
The codebase uses comprehensive mocking infrastructure:
- `create_test_executor_with_git_mock()` sets up executor with all mocks
- MockGitOperations provides `add_success_response()` and `add_error_response()`
- Tests use TempDir for isolated working directories
- ExecutionEnvironment provides context for command execution

### Coverage Measurement:
Use `cargo tarpaulin --lib` to measure coverage. Focus on:
- Line coverage for all 23 uncovered lines
- Branch coverage for all 21 conditional branches
- Integration coverage through end-to-end tests

### Refactoring Principles:
- Extract pure functions (no side effects) where possible
- Keep I/O operations at the boundaries
- Use Result types for error handling (no unwrap/panic per Spec 101)
- Make functions testable in isolation
- Prefer composition over complexity
