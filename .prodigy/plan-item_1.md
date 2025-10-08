# Implementation Plan: Add Test Coverage for EventStore Index Functions

## Problem Summary

**Location**: `./src/cook/execution/events/event_store.rs:FileEventStore::index:389`
**Priority Score**: 45.3875
**Debt Type**: ComplexityHotspot (cognitive: 15, cyclomatic: 6)
**Current Metrics**:
- Lines of Code: 30
- Cyclomatic Complexity: 6
- Cognitive Complexity: 15
- Coverage: 0%

**Issue**: The `index` function and its helper functions lack direct test coverage. While integration tests exist for the main `index` function, the helper functions (`update_time_range`, `process_event_line`, `increment_event_count`, etc.) have no unit tests covering their individual branches and edge cases.

## Target State

**Expected Impact**:
- Complexity Reduction: 3.0
- Coverage Improvement: 0.0 (Note: This will improve once tests are added)
- Risk Reduction: 15.89

**Success Criteria**:
- [ ] 100% test coverage for helper functions
- [ ] Each test focuses on ONE decision branch
- [ ] All tests are <15 lines each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Test update_time_range Function

**Goal**: Achieve 100% coverage for the `update_time_range` pure function (lines 186-203)

**Changes**:
- Add 4 unit tests for `update_time_range` covering all branch combinations:
  1. Both start and end are None (first event)
  2. Event time is earlier than current start (updates start)
  3. Event time is later than current end (updates end)
  4. Event time is within current range (no update)

**Testing**:
- Run `cargo test update_time_range` to verify
- Each test should be <15 lines

**Success Criteria**:
- [ ] 4 tests added for `update_time_range`
- [ ] All 4 test cases pass
- [ ] 100% branch coverage for `update_time_range`
- [ ] Ready to commit

### Phase 2: Test increment_event_count Function

**Goal**: Achieve 100% coverage for the `increment_event_count` pure function (lines 207-209)

**Changes**:
- Add 2 unit tests for `increment_event_count`:
  1. First occurrence of event type (creates new entry)
  2. Subsequent occurrence (increments existing count)

**Testing**:
- Run `cargo test increment_event_count` to verify
- Each test should be <10 lines

**Success Criteria**:
- [ ] 2 tests added for `increment_event_count`
- [ ] Both test cases pass
- [ ] 100% branch coverage for `increment_event_count`
- [ ] Ready to commit

### Phase 3: Test create_file_offset Function

**Goal**: Achieve 100% coverage for the `create_file_offset` pure function (lines 212-225)

**Changes**:
- Add 1 unit test for `create_file_offset`:
  1. Verify FileOffset struct is correctly populated from EventRecord

**Testing**:
- Run `cargo test create_file_offset` to verify
- Test should be <10 lines

**Success Criteria**:
- [ ] 1 test added for `create_file_offset`
- [ ] Test case passes
- [ ] 100% coverage for `create_file_offset`
- [ ] Ready to commit

### Phase 4: Test process_event_line Function

**Goal**: Achieve 100% coverage for the `process_event_line` function (lines 228-252)

**Changes**:
- Add 2 unit tests for `process_event_line`:
  1. Valid JSON event (successful parse and index update)
  2. Invalid JSON event (graceful failure, no panic)

**Testing**:
- Run `cargo test process_event_line` to verify
- Each test should be <15 lines

**Success Criteria**:
- [ ] 2 tests added for `process_event_line`
- [ ] Both test cases pass
- [ ] 100% branch coverage for `process_event_line`
- [ ] Ready to commit

### Phase 5: Test save_index Function

**Goal**: Achieve 100% coverage for the `save_index` async function (lines 255-259)

**Changes**:
- Add 2 async unit tests for `save_index`:
  1. Successful index save with valid path
  2. Error handling for invalid/readonly path (if applicable)

**Testing**:
- Run `cargo test save_index` to verify
- Each test should be <12 lines

**Success Criteria**:
- [ ] 2 tests added for `save_index`
- [ ] Both test cases pass
- [ ] 100% coverage for `save_index`
- [ ] Ready to commit

### Phase 6: Test process_event_file Function

**Goal**: Achieve 100% coverage for the `process_event_file` async function (lines 262-287)

**Changes**:
- Add 3 async unit tests for `process_event_file`:
  1. File with multiple valid events
  2. Empty file (no events)
  3. File with mixed valid/invalid JSON lines

**Testing**:
- Run `cargo test process_event_file` to verify
- Each test should be <15 lines

**Success Criteria**:
- [ ] 3 tests added for `process_event_file`
- [ ] All test cases pass
- [ ] 100% branch coverage for `process_event_file`
- [ ] Ready to commit

### Phase 7: Verify Overall Coverage Improvement

**Goal**: Confirm that all helper functions now have 100% test coverage

**Changes**:
- Run full test suite
- Generate coverage report with `cargo tarpaulin`
- Verify coverage improvement for event_store.rs

**Testing**:
- Run `cargo test --lib` to ensure all tests pass
- Run `cargo clippy` to check for warnings
- Run `cargo fmt` to verify formatting
- Run `just ci` for full CI checks

**Success Criteria**:
- [ ] All existing tests pass
- [ ] All new tests pass (14 total new tests)
- [ ] Coverage report shows significant improvement
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Write focused unit tests for the specific function
2. Each test should test ONE branch/path only
3. Keep tests under 15 lines each
4. Run `cargo test <function_name>` to verify
5. Commit after each phase completes successfully

**Test patterns to follow**:
- Use existing test patterns from the codebase (e.g., `tests::test_index_*`)
- Use `tempfile::TempDir` for filesystem tests
- Use `tokio::test` for async tests
- Use descriptive test names: `test_<function>_<scenario>`

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Properly formatted
4. `cargo tarpaulin --lib` - Generate coverage report
5. `just ci` - Full CI checks

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure (test compilation, test logic, etc.)
3. Adjust the test implementation
4. Retry the phase

Since each phase only adds tests (no production code changes), rollback risk is minimal.

## Notes

**Why focus on helper functions?**
- The main `index` function already has 7 integration tests
- The debtmap 0% coverage likely refers to the helper functions
- Testing helper functions directly provides better branch coverage
- Pure functions are easier to test in isolation

**Test organization**:
- Add all new tests in the existing `#[cfg(test)] mod tests` section
- Group related tests together (e.g., all `update_time_range` tests)
- Use descriptive names to indicate what branch is being tested

**Expected outcome**:
- 14 new focused unit tests
- Significant coverage improvement for event_store.rs
- Better confidence in helper function behavior
- Easier debugging when issues arise
