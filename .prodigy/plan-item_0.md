# Implementation Plan: Complete Test Coverage for GitRunnerImpl::status

## Problem Summary

**Location**: ./src/subprocess/git.rs:GitRunnerImpl::status:52
**Priority Score**: 54.7625
**Debt Type**: ComplexityHotspot (Cognitive: 30, Cyclomatic: 12)
**Current Metrics**:
- Lines of Code: 41
- Cyclomatic Complexity: 12
- Cognitive Complexity: 30
- Nesting Depth: 4 levels
- Coverage: 0% (based on debtmap analysis)

**Issue**: Add 12 tests for 100% coverage gap. NO refactoring needed (complexity 12 is acceptable)

**Rationale**: Complexity 12 is manageable. Coverage at 0%. Focus on test coverage, not refactoring.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.0
- Coverage Improvement: 0.0
- Risk Reduction: 19.166875

**Success Criteria**:
- [ ] 100% branch coverage for GitRunnerImpl::status function
- [ ] All edge cases tested (short lines, malformed input, various git status codes)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with rustfmt

## Implementation Phases

### Phase 1: Edge Case Coverage - Line Length Boundaries

**Goal**: Test the `line.len() > 2` condition and boundary cases for line parsing

**Changes**:
- Add test for lines with length exactly 2 (boundary case)
- Add test for lines with length 1 (should be ignored)
- Add test for empty lines in status output
- Add test for lines with only whitespace

**Testing**:
- Run `cargo test git_error_tests::test_status_line_length_*` for new tests
- Run `cargo test git_error_tests` to ensure all existing tests pass

**Success Criteria**:
- [ ] 4 new tests added covering line length edge cases
- [ ] Tests validate that short lines (<= 2 chars) are properly handled
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Git Status Code Coverage - Deleted and Renamed Files

**Goal**: Cover all git status codes, not just modified (M) and added (A)

**Changes**:
- Add test for deleted files (status code 'D')
- Add test for renamed files (status code 'R')
- Add test for copied files (status code 'C')
- Add test for files with both staged and unstaged changes (e.g., 'MM', 'AM')

**Testing**:
- Run `cargo test git_error_tests::test_status_*_status_code` for new tests
- Verify modified_files array correctly captures all status types

**Success Criteria**:
- [ ] 4 new tests added for D, R, C, and dual-status files
- [ ] All git status codes properly categorized as modified_files
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Branch Parsing Edge Cases

**Goal**: Ensure robust branch name parsing for unusual branch formats

**Changes**:
- Add test for branch line with no upstream but with spaces
- Add test for branch name containing special characters (slashes, dashes, dots)
- Add test for very long branch names (>100 chars)
- Add test for branch line with multiple "..." separators (malformed)

**Testing**:
- Run `cargo test git_error_tests::test_status_branch_*` for new tests
- Verify branch parsing handles edge cases gracefully

**Success Criteria**:
- [ ] 4 new tests added for branch parsing edge cases
- [ ] Branch parser handles unusual formats without panicking
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Final Coverage Verification

**Goal**: Achieve 100% branch coverage and verify all paths are tested

**Changes**:
- Run `cargo tarpaulin --out Html --output-dir coverage` to generate coverage report
- Identify any remaining uncovered branches in GitRunnerImpl::status (lines 52-92)
- Add focused tests for any remaining gaps
- Document test coverage in commit message

**Testing**:
- Review HTML coverage report
- Verify 100% line and branch coverage for GitRunnerImpl::status
- Run full test suite: `cargo test`
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] 100% branch coverage achieved for GitRunnerImpl::status
- [ ] Coverage report confirms all branches tested
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests following existing patterns in `git_error_tests` module
2. Use `MockProcessRunner` to simulate git command output
3. Keep each test focused on ONE specific edge case
4. Test name should clearly describe the scenario
5. Run `cargo test --lib git_error_tests` after each test
6. Commit after each phase with descriptive message

**Test Pattern to Follow**:
```rust
#[tokio::test]
async fn test_status_<specific_scenario>() {
    let mut mock_runner = MockProcessRunner::new();
    mock_runner
        .expect_command("git")
        .with_args(|args| args == ["status", "--porcelain", "--branch"])
        .returns_stdout("<specific_test_output>")
        .returns_success()
        .finish();

    let git = GitRunnerImpl::new(Arc::new(mock_runner));
    let temp_dir = TempDir::new().unwrap();
    let result = git.status(temp_dir.path()).await;

    assert!(result.is_ok());
    let status = result.unwrap();
    // Specific assertions for this scenario
}
```

**Final verification**:
1. `cargo test --lib` - All unit tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Proper formatting
4. `cargo tarpaulin --out Html` - Generate coverage report
5. Review coverage/index.html - Verify 100% coverage for GitRunnerImpl::status

## Rollback Plan

If a phase fails:
1. Review test failure output carefully
2. Check if MockProcessRunner expectations match actual function behavior
3. If test design is flawed, fix the test (not the production code)
4. If a genuine bug is found, document it separately (do not fix in this workflow)
5. For build failures: `git reset --hard HEAD~1` and revise approach

**Important**: This is a test-only workflow. Do NOT modify the `status` function itself. Only add tests.

## Notes

- The existing test suite already has 18 comprehensive tests
- The debtmap recommendation is to add 12 more tests for complete coverage
- Focus on edge cases and boundary conditions not covered by existing tests
- Each test should be < 15 lines and test ONE specific path
- The complexity of 12 is acceptable per the debtmap analysis
- DO NOT refactor the status function - only add tests
- Use early returns pattern in tests for clarity
- Follow existing test naming convention: `test_status_<scenario_description>`
