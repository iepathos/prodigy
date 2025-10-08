# Implementation Plan: Add Test Coverage for GitRunnerImpl::status

## Problem Summary

**Location**: ./src/subprocess/git.rs:GitRunnerImpl::status:52
**Priority Score**: 47.2625
**Debt Type**: ComplexityHotspot (cognitive: 30, cyclomatic: 12)
**Current Metrics**:
- Function Length: 41 lines
- Cyclomatic Complexity: 12
- Cognitive Complexity: 30
- Coverage: 0%
- Nesting Depth: 4 levels

**Issue**: Add 12 tests for 100% coverage gap. NO refactoring needed (complexity 12 is acceptable). Complexity 12 is manageable. Coverage at 0%. Focus on test coverage, not refactoring.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.0
- Coverage Improvement: 0.0
- Risk Reduction: 16.541875

**Success Criteria**:
- [ ] 12 focused tests covering all branches in GitRunnerImpl::status
- [ ] 100% line coverage for the status function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Test Happy Path and Basic Error Cases

**Goal**: Establish baseline test coverage for successful status calls and basic error handling

**Changes**:
- Add test for clean repository (no untracked/modified files)
- Add test for repository with branch information
- Add test for non-zero exit code handling

**Testing**:
- Run `cargo test git_runner_status` to verify new tests pass
- Verify existing git_error_tests still pass

**Success Criteria**:
- [ ] 3 tests added and passing
- [ ] Tests cover: clean repo, branch parsing, exit code error
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Test File Status Variants

**Goal**: Cover all file status parsing branches (untracked, modified, edge cases)

**Changes**:
- Add test for untracked files (lines starting with "??")
- Add test for modified files (lines with length > 2)
- Add test for mixed status (both untracked and modified files)
- Add test for empty status output

**Testing**:
- Run `cargo test git_runner_status` to verify all file status parsing works
- Verify coverage increases for lines 71-84

**Success Criteria**:
- [ ] 4 tests added and passing
- [ ] Tests cover: untracked files, modified files, mixed status, empty output
- [ ] File parsing logic fully covered
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Test Branch Parsing Edge Cases

**Goal**: Cover all branch information parsing branches and edge cases

**Changes**:
- Add test for branch with upstream info ("## main...origin/main")
- Add test for branch without upstream ("## main")
- Add test for detached HEAD (no branch line)
- Add test for malformed branch line

**Testing**:
- Run `cargo test git_runner_status` to verify branch parsing edge cases
- Verify lines 72-75 have full coverage

**Success Criteria**:
- [ ] 4 tests added and passing
- [ ] Tests cover: branch with upstream, branch without upstream, no branch, malformed input
- [ ] Branch parsing logic fully covered
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Test Output Edge Cases and Integration

**Goal**: Cover remaining edge cases and ensure comprehensive coverage

**Changes**:
- Add test for status output with only branch line (no files)
- Add test for status output with files but no branch line
- Add test combining all scenarios (branch + untracked + modified)

**Testing**:
- Run `cargo test git_runner_status` to verify all tests pass
- Run `cargo tarpaulin --lib --packages prodigy -- git_runner` to verify 100% coverage for status function

**Success Criteria**:
- [ ] 3 tests added (total: 12 tests for status function)
- [ ] 100% line coverage for GitRunnerImpl::status achieved
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Validation and Cleanup

**Goal**: Ensure all quality gates pass and coverage is verified

**Changes**:
- Run full test suite
- Run clippy for warnings
- Format code
- Verify coverage metrics

**Testing**:
- `cargo test --lib` - All tests pass
- `cargo clippy` - No warnings
- `cargo fmt --check` - Properly formatted
- `cargo tarpaulin --lib` - Verify coverage improvement

**Success Criteria**:
- [ ] All 12 tests passing consistently
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Coverage metrics show improvement
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib subprocess::git` to verify git-specific tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Each test should be < 15 lines and test ONE path

**Test pattern to follow** (using existing git_error_tests as reference):
```rust
#[tokio::test]
async fn test_status_<scenario>() {
    let mut mock_runner = MockProcessRunner::new();
    mock_runner
        .expect_command("git")
        .with_args(|args| args == ["status", "--porcelain", "--branch"])
        .returns_stdout("<scenario_output>")
        .returns_success()  // or .returns_exit_code(128) for error cases
        .finish();

    let git = GitRunnerImpl::new(Arc::new(mock_runner));
    let temp_dir = TempDir::new().unwrap();
    let result = git.status(temp_dir.path()).await;

    // Assert specific expectations
    assert!(result.is_ok());
    // ... more assertions
}
```

**Final verification**:
1. `cargo test --lib` - Full library test suite
2. `cargo tarpaulin --lib` - Verify coverage increase
3. `cargo clippy` - No warnings

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or clippy warning
3. Adjust the test implementation
4. Retry the phase

## Notes

- The existing test module `git_error_tests` provides a good pattern for using `MockProcessRunner`
- Focus on testing the parsing logic in lines 71-84 (file status parsing)
- Focus on testing the branch parsing logic in lines 72-75
- The function has 0% coverage despite being called by multiple upstream callers, indicating it's only tested indirectly
- Each test should use `MockProcessRunner` to control git output and test specific parsing paths
- Avoid testing implementation details; focus on behavior and output correctness
- The complexity (12) is acceptable; the issue is purely lack of direct test coverage
