# Implementation Plan: Add Test Coverage for FileEventStore::index

## Problem Summary

**Location**: ./src/cook/execution/events/event_store.rs:FileEventStore::index:389
**Priority Score**: 48.3875
**Debt Type**: ComplexityHotspot (cognitive: 15, cyclomatic: 6)
**Current Metrics**:
- Lines of Code: 30
- Cyclomatic Complexity: 6
- Cognitive Complexity: 15
- Coverage: 0%

**Issue**: Add 6 tests for 100% coverage gap. NO refactoring needed (complexity 6 is acceptable)

**Rationale**: Complexity 6 is manageable. Coverage at 0%. Focus on test coverage, not refactoring.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.0
- Coverage Improvement: 0.0
- Risk Reduction: 16.935625

**Success Criteria**:
- [ ] 6 new focused tests added (each < 15 lines)
- [ ] Each test covers ONE specific decision branch
- [ ] Coverage for `index` function reaches near 100%
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Tests for Empty and Error Cases

**Goal**: Cover edge cases where the function handles empty inputs or error conditions

**Changes**:
- Add test for indexing a job with no event files (empty directory)
- Add test for indexing when event file directory doesn't exist
- Add test for error handling when save_index fails

**Testing**:
- Run `cargo test --lib test_index` to verify new tests pass
- Verify existing integration tests still pass

**Success Criteria**:
- [ ] 3 new tests added and passing
- [ ] Tests are focused (< 15 lines each)
- [ ] Each test covers ONE specific branch
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Add Tests for Time Range Calculation Paths

**Goal**: Cover the conditional logic for time range handling (Some vs None)

**Changes**:
- Add test for time range calculation with single event
- Add test for time range calculation with multiple events spanning time
- Add test for default time range when no events processed

**Testing**:
- Run `cargo test --lib test_index` to verify new tests pass
- Verify time range edge cases are covered

**Success Criteria**:
- [ ] 3 new tests added and passing
- [ ] Time range logic fully tested
- [ ] Each test covers ONE specific path
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Verify Coverage and Clean Up

**Goal**: Ensure coverage metrics show improvement and all tests are maintainable

**Changes**:
- Run `cargo tarpaulin` to verify coverage improvement
- Add any missing test documentation
- Ensure test names are descriptive
- Clean up any test code duplication

**Testing**:
- `cargo test --lib` - All tests pass
- `cargo clippy` - No warnings
- `cargo fmt` - Proper formatting
- `cargo tarpaulin` - Verify coverage improvement

**Success Criteria**:
- [ ] Coverage for `index` function significantly improved
- [ ] All 6 tests are clear and maintainable
- [ ] No test code smells
- [ ] Documentation updated
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Write one focused test at a time
3. Run the specific test: `cargo test --lib test_name`
4. Verify it passes before moving to the next test
5. Run `cargo clippy` to check for warnings

**Test Design Guidelines**:
- Each test should be < 15 lines
- Test ONE decision branch per test
- Use descriptive test names (e.g., `test_index_with_empty_directory`)
- Follow existing test patterns in the file
- Use `TempDir` for file system isolation
- Use `assert!` with clear failure messages

**Final verification**:
1. `cargo test --lib` - Full test suite passes
2. `cargo clippy` - No warnings
3. `cargo tarpaulin --lib` - Regenerate coverage report
4. Verify coverage improvement for `FileEventStore::index`

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure
3. Identify which branch wasn't covered correctly
4. Adjust the test to focus on ONE specific branch
5. Retry

## Notes

**Existing Test Coverage**:
- The file already has 8 comprehensive integration tests
- These tests verify end-to-end behavior but don't cover all branches
- The new tests should be unit-focused, testing specific decision points

**Key Decision Branches to Test**:
1. Empty files list (no event files found)
2. Non-empty files list
3. Time range is None after processing (no valid events)
4. Time range has Some(start, end) after processing
5. Error when saving index (directory doesn't exist)
6. Success path with valid events and successful save

**Test Naming Convention**:
Follow the pattern: `test_index_<specific_scenario>`
Examples:
- `test_index_with_empty_directory`
- `test_index_saves_correctly_with_valid_events`
- `test_index_handles_time_range_none`
