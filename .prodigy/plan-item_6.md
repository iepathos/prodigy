# Implementation Plan: Add Tests and Refactor WorkflowExecutor::handle_step_validation

## Problem Summary

**Location**: ./src/cook/workflow/executor/validation.rs:WorkflowExecutor::handle_step_validation:346
**Priority Score**: 30.04
**Debt Type**: TestingGap (cognitive: 35, coverage: 0.0%, cyclomatic: 15)
**Current Metrics**:
- Lines of Code: 113
- Cyclomatic Complexity: 15
- Cognitive Complexity: 35
- Coverage: 0.0%
- Uncovered Lines: 33 ranges (lines 346, 354, 356-357, 360-362, 366-368, 373, 376-380, 411, 414, 416-418, 422-423, 429, 433, 435-436, 438, 447-451)

**Issue**: Complex business logic with 100% coverage gap. Cyclomatic complexity of 15 requires at least 15 test cases for full path coverage. The function handles step validation with multiple branches for dry-run mode, different validation spec types (Single/Multiple/Detailed), timeout handling, and result formatting. Testing before refactoring ensures no regressions during complexity reduction.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.5 (from 15 to ~10.5)
- Coverage Improvement: 50.0% (from 0% to 50%)
- Risk Reduction: 12.61

**Success Criteria**:
- [ ] At least 15 test cases covering all branches (100% branch coverage)
- [ ] Extract 7 pure functions with complexity ≤3 each
- [ ] Coverage reaches 80%+ for the validation module
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Core Path Tests (Dry-Run and Basic Execution)

**Goal**: Cover the primary execution paths through handle_step_validation, focusing on dry-run mode and basic validation execution.

**Changes**:
- Add test for dry-run mode with Single validation spec
- Add test for dry-run mode with Multiple validation specs
- Add test for dry-run mode with Detailed validation config
- Add test for successful validation execution (non-dry-run, Single spec)
- Add test for successful validation execution (Multiple specs)

**Testing**:
```bash
cargo test --lib validation::tests::test_handle_step_validation_dry_run_single
cargo test --lib validation::tests::test_handle_step_validation_dry_run_multiple
cargo test --lib validation::tests::test_handle_step_validation_dry_run_detailed
cargo test --lib validation::tests::test_handle_step_validation_success_single
cargo test --lib validation::tests::test_handle_step_validation_success_multiple
```

**Success Criteria**:
- [ ] 5 new tests pass
- [ ] Coverage for lines 354-382 achieved
- [ ] Dry-run branches fully covered
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 2: Add Timeout and Failure Path Tests

**Goal**: Cover timeout handling and validation failure scenarios.

**Changes**:
- Add test for validation with timeout (successful before timeout)
- Add test for validation timeout expiration
- Add test for validation failure with Single spec
- Add test for validation failure with Multiple specs
- Add test for validation failure with Detailed config

**Testing**:
```bash
cargo test --lib validation::tests::test_handle_step_validation_timeout_success
cargo test --lib validation::tests::test_handle_step_validation_timeout_expired
cargo test --lib validation::tests::test_handle_step_validation_failure_single
cargo test --lib validation::tests::test_handle_step_validation_failure_multiple
cargo test --lib validation::tests::test_handle_step_validation_failure_detailed
```

**Success Criteria**:
- [ ] 5 new tests pass (total: 10)
- [ ] Coverage for lines 411-430 achieved
- [ ] Timeout logic fully covered
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 3: Add Result Formatting and Edge Case Tests

**Goal**: Cover result display formatting and edge cases like missing timeout, different step types.

**Changes**:
- Add test for validation result formatting (passed validation)
- Add test for validation result formatting (failed validation with details)
- Add test for validation without timeout specified
- Add test for step with explicit name vs. derived name
- Add test for validation with empty results list

**Testing**:
```bash
cargo test --lib validation::tests::test_handle_step_validation_format_passed
cargo test --lib validation::tests::test_handle_step_validation_format_failed_with_details
cargo test --lib validation::tests::test_handle_step_validation_no_timeout
cargo test --lib validation::tests::test_handle_step_validation_step_naming
cargo test --lib validation::tests::test_handle_step_validation_empty_results
```

**Success Criteria**:
- [ ] 5 new tests pass (total: 15)
- [ ] Coverage for lines 433-456 achieved
- [ ] All branches covered (100% branch coverage)
- [ ] All existing tests pass
- [ ] Ready to commit

### Phase 4: Extract Pure Functions for Validation Executor Creation

**Goal**: Extract the complex validation executor creation logic into pure, testable functions.

**Changes**:
- Extract `create_step_validation_executor()` - builds StepValidationExecutor
- Extract `create_validation_execution_context()` - builds ExecutionContext for validation
- Add 6 unit tests (3 per extracted function)
- Refactor `handle_step_validation` to use extracted functions

**Testing**:
```bash
cargo test --lib validation::tests::test_create_step_validation_executor_*
cargo test --lib validation::tests::test_create_validation_execution_context_*
cargo test --lib validation::tests::test_handle_step_validation_* # Regression
```

**Success Criteria**:
- [ ] 2 pure functions extracted with complexity ≤3
- [ ] 6 new unit tests pass
- [ ] All 15 integration tests still pass (regression check)
- [ ] Function length reduced by ~20 lines
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Extract Pure Functions for Validation Result Handling

**Goal**: Extract validation result processing and display logic into pure functions.

**Changes**:
- Extract `process_validation_result()` - determines success/failure and formats messages
- Extract `format_validation_failure_details()` - formats failure detail list
- Extract `determine_validation_display_message()` - creates appropriate success/warning message
- Add 9 unit tests (3 per extracted function)
- Refactor `handle_step_validation` to use extracted functions

**Testing**:
```bash
cargo test --lib validation::tests::test_process_validation_result_*
cargo test --lib validation::tests::test_format_validation_failure_details_*
cargo test --lib validation::tests::test_determine_validation_display_message_*
cargo test --lib validation::tests::test_handle_step_validation_* # Regression
```

**Success Criteria**:
- [ ] 3 pure functions extracted with complexity ≤3
- [ ] 9 new unit tests pass
- [ ] All previous tests still pass (regression check)
- [ ] Function length reduced by ~30 more lines
- [ ] Cyclomatic complexity reduced to ~10
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 6: Extract Pure Functions for Timeout Logic

**Goal**: Extract timeout handling logic into pure, testable functions.

**Changes**:
- Extract `should_apply_timeout()` - determines if timeout should be applied
- Extract `create_timeout_result()` - creates timeout failure result
- Add 6 unit tests (3 per extracted function)
- Refactor `handle_step_validation` timeout handling to use extracted functions

**Testing**:
```bash
cargo test --lib validation::tests::test_should_apply_timeout_*
cargo test --lib validation::tests::test_create_timeout_result_*
cargo test --lib validation::tests::test_handle_step_validation_* # Regression
```

**Success Criteria**:
- [ ] 2 pure functions extracted with complexity ≤3
- [ ] 6 new unit tests pass (total: ~36 new tests)
- [ ] All previous tests still pass
- [ ] Final function length: ~50-60 lines (down from 113)
- [ ] Final cyclomatic complexity: ≤10 (down from 15)
- [ ] Final coverage: 80%+ for handle_step_validation
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib validation::tests` to verify new tests pass
2. Run `cargo test --lib` to ensure no regressions in other modules
3. Run `cargo clippy` to check for warnings
4. Visually inspect coverage with `cargo tarpaulin --lib` (if available)

**Test organization**:
- Integration tests (Phase 1-3): Test full `handle_step_validation` function behavior
- Unit tests (Phase 4-6): Test extracted pure functions in isolation
- Use existing test infrastructure: `create_test_env()`, `MockUserInteraction`
- Follow existing test naming patterns in the module

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Proper formatting
4. `cargo tarpaulin --lib` - Verify coverage improvement
5. Review final metrics: complexity ≤10, coverage ≥80%

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or compilation error
3. Check if mock setup needs adjustment
4. Check if async/await handling is correct (this is an async function)
5. Verify ExecutionEnvironment and WorkflowContext are properly initialized
6. Adjust the approach and retry

Common gotchas:
- `handle_step_validation` is async, tests must use `#[tokio::test]`
- Uses `Box::pin()` for async execution - may need special handling in tests
- StepValidationCommandExecutor uses raw pointer - may need unsafe blocks or alternative approach
- MockUserInteraction must be Arc-wrapped
- ExecutionEnvironment requires Arc-wrapped paths

## Notes

### Key Observations:
1. Function already has some extracted pure functions at module level (format_validation_passed_message, etc.)
2. Main complexity comes from branching on StepValidationSpec enum variants (Single/Multiple/Detailed)
3. Timeout logic adds significant branching (Option<u64> handling)
4. Dry-run mode creates early return with simulated result
5. Validation executor creation involves complex Arc/pointer setup

### Related Code:
- Pure formatting functions already exist (lines 83-125) and are well-tested
- `StepValidationExecutor` is used but defined in another module
- `determine_step_name()` helper already exists and is tested
- Integration with `user_interaction` for display output

### Extraction Targets:
1. **Validation executor creation** (lines 385-391): Complex Arc/pointer setup
2. **Execution context creation** (lines 394-402): HashMap and struct building
3. **Timeout application logic** (lines 411-427): Nested conditional with tokio::timeout
4. **Result processing** (lines 433-453): Iterating failed results and formatting
5. **Display message determination** (lines 433-444): Choosing success vs. warning
6. **Dry-run result creation** (lines 376-381): Struct construction

### Coverage Strategy:
- **Phase 1-3**: Achieve 100% branch coverage through integration tests
- **Phase 4-6**: Maintain coverage while refactoring via unit tests on extracted functions
- Target: 15+ branches × 1 test/branch = 15+ integration tests minimum
- Additional unit tests for extracted functions = 21 unit tests
- Total: ~36 new tests
