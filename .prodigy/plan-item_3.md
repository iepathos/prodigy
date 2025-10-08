# Implementation Plan: Add Test Coverage for CookSessionAdapter::update_session

## Problem Summary

**Location**: ./src/unified_session/cook_adapter.rs:CookSessionAdapter::update_session:184
**Priority Score**: 39.64
**Debt Type**: ComplexityHotspot (Cognitive: 17, Cyclomatic: 5)
**Current Metrics**:
- Lines of Code: 26
- Cyclomatic Complexity: 5
- Cognitive Complexity: 17
- Coverage: 0%
- Upstream Dependencies: 14 callers
- Downstream Dependencies: 3 callees

**Issue**: Add 5 tests for 100% coverage gap. NO refactoring needed (complexity 5 is acceptable)

**Rationale**: Complexity 5 is manageable. Coverage at 0%. Focus on test coverage, not refactoring. Current structure is clean and simple.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 2.5
- Coverage Improvement: 0.0
- Risk Reduction: 13.87

**Success Criteria**:
- [ ] 100% test coverage for `CookSessionAdapter::update_session` function
- [ ] All 5 uncovered branches tested
- [ ] Each test is focused (<15 lines) and tests ONE path
- [ ] Edge cases and boundary conditions covered
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Read and Understand Current Implementation

**Goal**: Analyze the `update_session` function to identify the 5 uncovered branches and understand the test scenarios needed.

**Changes**:
- Read `src/unified_session/cook_adapter.rs` starting at line 184
- Identify all conditional branches and error paths
- Review existing tests to understand testing patterns
- Map out the 5 specific test cases needed

**Testing**:
- Run `cargo test cook_adapter` to see existing test coverage
- Run `cargo tarpaulin --lib` to identify uncovered lines

**Success Criteria**:
- [ ] Identified all 5 uncovered branches/paths
- [ ] Understood what each branch does
- [ ] Documented the test scenarios needed

### Phase 2: Write Tests for Basic Update Scenarios

**Goal**: Cover the primary update paths with 2-3 focused tests.

**Changes**:
- Add test for successful session update with metadata changes
- Add test for successful session update with progress changes
- Add test for successful session update with file changes
- Each test should be <15 lines and test ONE specific update path

**Testing**:
- Run `cargo test cook_adapter::tests::test_update_*`
- Verify each new test passes independently
- Check coverage improvement with `cargo tarpaulin`

**Success Criteria**:
- [ ] 2-3 new tests added for basic update scenarios
- [ ] Each test is focused and <15 lines
- [ ] All new tests pass
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 3: Write Tests for Edge Cases

**Goal**: Cover edge cases and boundary conditions with 2-3 focused tests.

**Changes**:
- Add test for empty/minimal update (no changes)
- Add test for update with all fields changed simultaneously
- Add test for update with boundary values (e.g., max iterations, zero files)
- Each test should be <15 lines and test ONE edge case

**Testing**:
- Run `cargo test cook_adapter::tests::test_update_*`
- Verify edge case tests pass
- Check coverage improvement with `cargo tarpaulin`

**Success Criteria**:
- [ ] 2-3 new tests added for edge cases
- [ ] Each test is focused and <15 lines
- [ ] All new tests pass
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 4: Verify 100% Coverage Achievement

**Goal**: Ensure all branches are covered and cleanup any test issues.

**Changes**:
- Run full coverage analysis with `cargo tarpaulin`
- Identify any remaining uncovered lines
- Add any missing tests if gaps remain
- Refine existing tests if needed for clarity

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Run `cargo tarpaulin --lib` to verify 100% coverage of target function
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] 100% coverage achieved for `update_session` function
- [ ] Total of 5 new focused tests added
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Validation and Documentation

**Goal**: Run full CI checks and verify the debt item is resolved.

**Changes**:
- Run `just ci` to ensure all checks pass
- Update any relevant documentation if needed
- Verify the debtmap score has improved

**Testing**:
- Run `just ci` for full CI validation
- Run `cargo tarpaulin` to regenerate coverage report
- Run `debtmap analyze` to verify improvement in debt score

**Success Criteria**:
- [ ] All CI checks pass
- [ ] Coverage report shows improvement
- [ ] Debtmap analysis shows reduced debt score
- [ ] Code is ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test cook_adapter` to verify existing tests pass
2. Run `cargo test --lib` for full test suite validation
3. Run `cargo clippy` to check for warnings
4. Run `cargo tarpaulin --lib` to measure coverage improvement

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage
3. `debtmap analyze` - Verify improvement

**Test Structure Pattern** (following existing patterns):
```rust
#[test]
fn test_update_<scenario>() {
    // Setup: Create test session
    // Action: Call update_session with specific update
    // Assert: Verify expected changes
}
```

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or coverage gap
3. Adjust the test approach
4. Retry with refined tests

## Notes

- **NO REFACTORING**: The function has complexity 5, which is acceptable. Focus ONLY on adding tests.
- **Test Independence**: Each test should be completely independent and test ONE specific path.
- **Follow Existing Patterns**: Review tests in the same file to match the testing style.
- **Coverage Tool**: Use `cargo tarpaulin` to identify exact uncovered lines.
- **Function Role**: The function is classified as "PureLogic" with 14 upstream callers, so comprehensive testing is critical.
- **Keep Tests Simple**: Each test should be <15 lines as recommended by debtmap.
- **Edge Cases Matter**: With 0% current coverage, both happy paths AND edge cases need testing.
