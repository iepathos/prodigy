# Implementation Plan: Add Comprehensive Test Coverage for list_resumable_jobs_internal

## Problem Summary

**Location**: ./src/cook/execution/state.rs:DefaultJobStateManager::list_resumable_jobs_internal:884
**Priority Score**: 33.102062072615965
**Debt Type**: ComplexityHotspot (cognitive: 56, cyclomatic: 10)
**Current Metrics**:
- Lines of Code: 59
- Cyclomatic Complexity: 10
- Cognitive Complexity: 56
- Nesting Depth: 6
- Coverage: 0%

**Issue**: Add 10 tests for 100% coverage gap. NO refactoring needed (complexity 10 is acceptable)

**Rationale**: Complexity 10 is manageable. Coverage at 0%. Focus on test coverage, not refactoring.

## Target State

**Expected Impact**:
- Complexity Reduction: 5.0
- Coverage Improvement: 0.0 (test coverage, not production code coverage metric)
- Risk Reduction: 11.585721725415588

**Success Criteria**:
- [ ] 10+ focused tests covering all decision branches
- [ ] Each test is < 15 lines and tests ONE path
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] 100% coverage of list_resumable_jobs_internal function

## Implementation Phases

### Phase 1: Test Infrastructure Setup

**Goal**: Set up test helpers and infrastructure for comprehensive testing

**Changes**:
- Create helper function to set up test job directories
- Create helper function to write checkpoint files
- Add test utilities for creating various job states

**Testing**:
- Verify helpers can create valid test scenarios
- Run `cargo test --lib state` to ensure no regressions

**Success Criteria**:
- [ ] Test helpers compile and work correctly
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Test Empty/Missing Directory Cases (3 tests)

**Goal**: Cover edge cases when jobs directory doesn't exist or is empty

**Changes**:
- Test 1: jobs_dir doesn't exist (line 888-890)
- Test 2: jobs_dir exists but is empty
- Test 3: jobs_dir exists but contains only non-directory entries

**Testing**:
- Each test verifies correct empty Vec<ResumableJob> return
- Run `cargo test test_list_resumable_empty` pattern

**Success Criteria**:
- [ ] All 3 tests pass
- [ ] Cover lines 888-890 (early return path)
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Test Directory Entry Processing (3 tests)

**Goal**: Cover directory iteration and metadata check branches

**Changes**:
- Test 4: Directory entry with invalid metadata (line 898 Err path)
- Test 5: Directory entry that's a file (line 899 false path)
- Test 6: Directory entry with invalid filename (line 900 None path)

**Testing**:
- Verify each edge case is handled gracefully
- Run `cargo test test_list_resumable_dir` pattern

**Success Criteria**:
- [ ] All 3 tests pass
- [ ] Cover lines 898-900 edge cases
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 4: Test Checkpoint Loading Branches (2 tests)

**Goal**: Cover checkpoint loading success and failure paths

**Changes**:
- Test 7: Valid job directory but load_checkpoint fails (line 930-933)
- Test 8: Valid checkpoint but job is complete (line 905 false path)

**Testing**:
- Test 7: Verify job is skipped when checkpoint invalid
- Test 8: Verify complete job is not added to resumable list
- Run `cargo test test_list_resumable_checkpoint` pattern

**Success Criteria**:
- [ ] Both tests pass
- [ ] Cover lines 905 and 930-933
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 5: Test Checkpoint Version Processing (2 tests)

**Goal**: Cover checkpoint list retrieval and max calculation

**Changes**:
- Test 9: list_checkpoints returns empty (line 910 unwrap_or_default path)
- Test 10: Multiple checkpoints, verify max version selected (lines 912-916)

**Testing**:
- Test 9: Verify default checkpoint version 0 used
- Test 10: Verify highest version checkpoint is selected
- Run `cargo test test_list_resumable_version` pattern

**Success Criteria**:
- [ ] Both tests pass
- [ ] Cover lines 910-916 checkpoint version logic
- [ ] All existing tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write focused tests (< 15 lines each)
2. Run `cargo test --lib state::tests::test_list_resumable` after each test
3. Verify test covers exactly ONE decision branch
4. Run `cargo clippy` to check for warnings
5. Commit after each phase completes

**Test Design Principles**:
- ONE assertion per test when possible
- Clear test names describing the scenario
- Use existing test utilities (TempDir, tokio::test)
- Tests should be deterministic
- Focus on behavior, not implementation

**Final verification**:
1. `cargo test --lib state` - All state module tests pass
2. `cargo clippy -- -D warnings` - No clippy warnings
3. `cargo fmt --check` - Proper formatting
4. `cargo tarpaulin --out Stdout -- --test-threads=1 state::tests` - Verify coverage improvement

## Decision Branch Coverage Map

| Line(s) | Branch | Test # | Scenario |
|---------|--------|--------|----------|
| 888-890 | jobs_dir doesn't exist | 1 | Empty directory returns empty vec |
| 893-895 | read_dir succeeds, no entries | 2 | Empty jobs dir |
| 895-896 | next_entry returns None | 3 | Only non-directories present |
| 898 | metadata().await.is_err() | 4 | Invalid metadata |
| 899 | !metadata.is_dir() | 5 | Entry is file not dir |
| 900 | file_name().is_none() | 6 | Invalid filename |
| 902-933 | load_checkpoint() fails | 7 | Skip invalid checkpoint |
| 905 | !state.is_complete (false) | 8 | Complete job excluded |
| 905 | !state.is_complete (true) | existing | Incomplete job included |
| 910 | list_checkpoints unwrap_or | 9 | Empty checkpoint list |
| 912-916 | max_by_key version | 10 | Multiple checkpoints |

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure output
3. Check if test helpers need adjustment
4. Verify async test setup is correct
5. Retry with adjusted test

## Notes

- The function has high cognitive complexity (56) but manageable cyclomatic complexity (10)
- Nesting depth of 6 makes it harder to read but doesn't require refactoring per debtmap guidance
- Focus is purely on test coverage - NO production code changes
- All tests should be async (`#[tokio::test]`)
- Use TempDir for isolated test environments
- Follow existing test patterns in the module (lines 1040-1102)
- The existing test_list_resumable_jobs covers the happy path, we need edge cases
