# Implementation Plan: Add Test Coverage for CookSessionAdapter::update_session

## Problem Summary

**Location**: ./src/unified_session/cook_adapter.rs:CookSessionAdapter::update_session:184
**Priority Score**: 48.64
**Debt Type**: ComplexityHotspot (cognitive: 17, cyclomatic: 5)
**Current Metrics**:
- Lines of Code: 26
- Cyclomatic Complexity: 5
- Cognitive Complexity: 17
- Coverage: 0% (missing coverage for critical branches)
- Upstream Callers: 22 different call sites

**Issue**: Add 5 tests for 100% coverage gap. NO refactoring needed (complexity 5 is acceptable). The function has good structure but lacks comprehensive test coverage for edge cases and error conditions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 2.5 (via test-driven clarity)
- Coverage Improvement: 0.0 (needs measurement after tests)
- Risk Reduction: 17.02 (significantly reduce regression risk)

**Success Criteria**:
- [ ] 5 new focused tests added (each <15 lines)
- [ ] Each test covers ONE specific code path
- [ ] All edge cases and error conditions tested
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Add Test for Multiple Sequential Updates

**Goal**: Verify that multiple updates can be applied in sequence and cached state remains consistent

**Changes**:
- Add test `test_update_session_multiple_sequential_updates` to verify:
  - Increment iteration
  - Add files changed
  - Update status
  - All updates are properly reflected in cached state

**Testing**:
```bash
cargo test test_update_session_multiple_sequential_updates
```

**Success Criteria**:
- [ ] Test passes and verifies sequential update behavior
- [ ] Cached state reflects all accumulated updates
- [ ] Test is <15 lines
- [ ] All tests pass: `cargo test --lib`

### Phase 2: Add Test for CompleteIteration Update Path

**Goal**: Verify that `CookSessionUpdate::CompleteIteration` is handled correctly (maps to empty vec)

**Changes**:
- Add test `test_update_session_complete_iteration` to verify:
  - CompleteIteration update is accepted
  - No error occurs
  - Cached state is still updated (even though unified_updates is empty)

**Testing**:
```bash
cargo test test_update_session_complete_iteration
```

**Success Criteria**:
- [ ] Test passes and verifies CompleteIteration handling
- [ ] No panic or error occurs
- [ ] Test is <15 lines
- [ ] All tests pass: `cargo test --lib`

### Phase 3: Add Test for Update After Session Completion

**Goal**: Verify behavior when trying to update an already-completed session

**Changes**:
- Add test `test_update_session_after_completion` to verify:
  - Start a session
  - Mark it as completed
  - Try to update it (should succeed gracefully)
  - Verify cached state reflects the final state

**Testing**:
```bash
cargo test test_update_session_after_completion
```

**Success Criteria**:
- [ ] Test passes and verifies post-completion update behavior
- [ ] No errors occur
- [ ] Test is <15 lines
- [ ] All tests pass: `cargo test --lib`

### Phase 4: Add Test for Files Changed Delta Accumulation

**Goal**: Verify that multiple AddFilesChanged updates accumulate correctly

**Changes**:
- Add test `test_update_files_changed_delta` to verify:
  - Add 3 files changed
  - Add 5 more files changed
  - Add 2 more files changed
  - Verify cumulative total in cached state

**Testing**:
```bash
cargo test test_update_files_changed_delta
```

**Success Criteria**:
- [ ] Test passes and verifies accumulation logic
- [ ] Delta values are properly accumulated
- [ ] Test is <15 lines
- [ ] All tests pass: `cargo test --lib`

### Phase 5: Add Test for Update Metadata Path

**Goal**: Verify custom metadata updates work correctly

**Changes**:
- Add test `test_update_metadata` to verify:
  - Custom metadata can be set via CookSessionUpdate variants
  - Metadata is preserved in cached state
  - Multiple metadata updates don't interfere with each other

**Testing**:
- Run all tests to ensure coverage is complete
```bash
cargo test --lib
cargo clippy
cargo fmt --check
```

**Success Criteria**:
- [ ] Test passes and verifies metadata handling
- [ ] All 5 new tests pass
- [ ] No clippy warnings
- [ ] Code is properly formatted
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write the test first (TDD approach)
2. Run `cargo test <test_name>` to verify the specific test
3. Run `cargo test --lib` to verify all tests still pass
4. Run `cargo clippy` to check for warnings
5. Run `cargo fmt` to ensure proper formatting
6. Commit the phase with a clear message

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Proper formatting
4. `cargo tarpaulin --lib` - Measure coverage improvement
5. Review test coverage report to confirm gaps are filled

## Test Design Guidelines

Each test should:
- Be <15 lines of code
- Test ONE specific code path
- Use descriptive assertion messages
- Follow the existing test pattern in the file
- Use `create_test_adapter()` helper for setup
- Clean up with temp dir drop (automatic)

## Rollback Plan

If a phase fails:
1. Review the test failure carefully
2. Check if the test expectation is correct
3. If test is wrong, fix the test
4. If code behavior is unexpected, investigate further
5. Do NOT refactor the production code (per debtmap recommendation)
6. If completely blocked, document findings and reassess

## Notes

- The function structure is clean and well-designed
- Cyclomatic complexity of 5 is acceptable and manageable
- Cognitive complexity of 17 is due to async/await patterns, not poor design
- Focus is purely on test coverage, NOT refactoring
- Existing tests cover happy paths; new tests cover edge cases
- The 22 upstream callers indicate this is a critical function
- High test coverage will significantly reduce regression risk
