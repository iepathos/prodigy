# Implementation Plan: Test Coverage for handle_step_validation Function

## Problem Summary

**Location**: ./src/cook/workflow/executor/validation.rs:WorkflowExecutor::handle_step_validation:224
**Priority Score**: 33.04
**Debt Type**: TestingGap (0% coverage, complexity 19, cognitive complexity 41)
**Current Metrics**:
- Lines of Code: 144
- Cyclomatic Complexity: 19
- Cognitive Complexity: 41
- Coverage: 0%
- Nesting Depth: 3

**Issue**: Complex business logic with 100% coverage gap. This function handles step validation with multiple execution paths including dry-run mode simulation, timeout handling, validation result display, and error message formatting. Cyclomatic complexity of 19 requires comprehensive test coverage to ensure reliability.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 5.7 (through extraction of pure functions)
- Coverage Improvement: 50.0%
- Risk Reduction: 13.87

**Success Criteria**:
- [ ] Achieve 80%+ test coverage for handle_step_validation
- [ ] Extract 8 pure functions (complexity ≤3 each)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Core Path Tests

**Goal**: Establish baseline test coverage for the primary execution paths

**Changes**:
- Add test for dry-run mode with single validation command
- Add test for dry-run mode with multiple validation commands
- Add test for dry-run mode with detailed validation config
- Add test for successful validation execution (passed result)
- Add test for failed validation execution (failed result)

**Testing**:
```bash
cargo test --lib handle_step_validation
cargo test --lib validation::tests
```

**Success Criteria**:
- [ ] 5 new tests covering main branches
- [ ] ~30% coverage achieved
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Add Timeout and Error Handling Tests

**Goal**: Cover timeout scenarios and error handling paths

**Changes**:
- Add test for validation with timeout (successful completion before timeout)
- Add test for validation timeout (timeout occurs)
- Add test for validation error/exception handling
- Add test for display_error when timeout occurs

**Testing**:
```bash
cargo test --lib handle_step_validation
```

**Success Criteria**:
- [ ] 4 new tests covering timeout branches
- [ ] ~50% coverage achieved
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Pure Display Formatting Functions

**Goal**: Extract pure functions for result message formatting

**Changes**:
- Extract `format_validation_passed_message(results_count: usize, attempts: usize) -> String`
- Extract `format_validation_failed_message(results_count: usize, attempts: usize) -> String`
- Extract `format_failed_validation_detail(idx: usize, message: &str, exit_code: i32) -> String`
- Add unit tests for each extracted function (3 tests per function = 9 tests)
- Update handle_step_validation to use extracted functions

**Testing**:
```bash
cargo test --lib format_validation_
cargo test --lib handle_step_validation
```

**Success Criteria**:
- [ ] 3 pure functions extracted (complexity ≤2 each)
- [ ] 9 new unit tests for pure functions
- [ ] handle_step_validation complexity reduced by ~3
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Pure Validation Step Name Logic

**Goal**: Extract pure logic for determining step names

**Changes**:
- Extract `determine_step_name(step: &WorkflowStep) -> &str` as pure function
- Move the if/else logic for step name determination (lines 283-291)
- Add unit tests covering:
  - Step with explicit name
  - Step with claude command (no name)
  - Step with shell command (no name)
  - Step with neither (fallback case)
- Update handle_step_validation to use extracted function

**Testing**:
```bash
cargo test --lib determine_step_name
cargo test --lib handle_step_validation
```

**Success Criteria**:
- [ ] 1 pure function extracted (complexity ≤3)
- [ ] 4 new unit tests for pure function
- [ ] handle_step_validation complexity reduced by ~2
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Extract Validation Result Display Logic

**Goal**: Separate validation result display into testable functions

**Changes**:
- Extract `should_display_validation_success(result: &StepValidationResult) -> bool`
- Extract `should_display_validation_details(result: &StepValidationResult) -> Vec<(usize, &ValidationItemResult)>` (returns failed validations)
- Add unit tests for extracted functions:
  - Test successful validation result
  - Test failed validation result with no failed items
  - Test failed validation result with multiple failed items
  - Test edge cases (empty results, all passed)
- Update handle_step_validation to use extracted functions

**Testing**:
```bash
cargo test --lib should_display_validation_
cargo test --lib handle_step_validation
```

**Success Criteria**:
- [ ] 2 pure functions extracted (complexity ≤3 each)
- [ ] 6 new unit tests for pure functions
- [ ] handle_step_validation complexity reduced by ~3
- [ ] All tests pass
- [ ] Ready to commit

### Phase 6: Final Coverage and Integration Tests

**Goal**: Achieve 80%+ total coverage and verify all paths

**Changes**:
- Add integration test for full validation workflow with retries
- Add test for validation with step name variations
- Add edge case tests:
  - Empty validation results
  - Very long messages
  - Multiple failed validation details
- Verify all uncovered lines from debtmap are now covered

**Testing**:
```bash
cargo test --lib handle_step_validation
cargo tarpaulin --lib --packages prodigy -- validation::tests
```

**Success Criteria**:
- [ ] 80%+ coverage for handle_step_validation achieved
- [ ] All edge cases covered
- [ ] Integration tests pass
- [ ] debtmap shows improvement
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run phase-specific tests listed above
4. Verify coverage improvement with `cargo tarpaulin --lib`

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage
3. `debtmap analyze` - Verify improvement in unified_score

**Test file location**: `src/cook/workflow/executor/validation.rs` (add `#[cfg(test)] mod tests` section)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure using `cargo test --lib -- --nocapture`
3. Check compilation errors with `cargo check`
4. Adjust the implementation approach
5. Retry the phase

## Notes

### Key Testing Considerations

1. **Mocking Strategy**: Use the existing `tests::test_mocks::MockUserInteraction` for display methods
2. **Async Testing**: Use `tokio::test` for async test functions
3. **Validation Executor Mocking**: May need to create mock for `StepValidationExecutor`
4. **Context Setup**: Create minimal `WorkflowContext` and `ExecutionEnvironment` for tests

### Pure Function Extraction Targets

The following functions are good candidates for extraction (from debtmap recommendation):
1. Message formatting functions (already planned in Phase 3)
2. Step name determination logic (already planned in Phase 4)
3. Validation result display logic (already planned in Phase 5)
4. Additional candidates:
   - Pluralization helper: `fn pluralize(count: usize, singular: &str, plural: &str) -> &str`
   - Result filtering: `fn filter_failed_validations(results: &[ValidationItemResult]) -> Vec<&ValidationItemResult>`

### Complexity Reduction Path

- Start: Cyclomatic complexity 19
- After Phase 3: ~16 (extract display formatting)
- After Phase 4: ~14 (extract step name logic)
- After Phase 5: ~11 (extract result display logic)
- Target: ≤13 (meets "extract 8 functions" goal with ~8 smaller helper functions)

### Coverage Strategy

- Phase 1: Cover main execution branches (30%)
- Phase 2: Cover error/timeout paths (50%)
- Phases 3-5: Add pure function tests (70%)
- Phase 6: Integration and edge cases (80%+)
