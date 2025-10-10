# Implementation Plan: Test and Refactor PhaseCoordinator::execute_workflow

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/phases/coordinator.rs:PhaseCoordinator::execute_workflow:156
**Priority Score**: 28.025
**Debt Type**: TestingGap (0% coverage, complexity 11)

**Current Metrics**:
- Lines of Code: 105
- Cyclomatic Complexity: 11
- Cognitive Complexity: 37
- Coverage: 0.0% (0% direct coverage, 25% transitive coverage)
- Function Role: PureLogic

**Issue**: Add 7 tests for 100% coverage gap, then refactor complexity 11 into 7 functions

**Rationale**: Complex business logic with 100% coverage gap. Cyclomatic complexity of 11 requires at least 11 test cases for full path coverage. After extracting 7 functions, each will need only 3-5 tests. Testing before refactoring ensures no regressions.

**Uncovered Lines**: 156, 161, 165-170, 172-178, 185-187, 189-191, 194-196, 198-208, 214-221, 223-229, 234, 238-240

## Target State

**Expected Impact**:
- Complexity Reduction: 3.3 (from 11 to ~8)
- Coverage Improvement: 50.0% (from 0% to 50%+)
- Risk Reduction: 11.77

**Success Criteria**:
- [ ] Coverage increases from 0% to at least 50% (target: 80%+)
- [ ] Cyclomatic complexity reduced from 11 to ≤8
- [ ] At least 7 extracted pure functions with complexity ≤3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Core Integration Tests

**Goal**: Cover the main execution paths and achieve baseline test coverage for the workflow state machine.

**Changes**:
- Add test for successful workflow execution (setup → map → reduce)
- Add test for workflow without setup phase (map → reduce)
- Add test for workflow without reduce phase (setup → map)
- Add test for minimal workflow (map only)

**Testing**:
```bash
cargo test --lib coordinator::coordinator_test::test_execute_workflow
cargo tarpaulin --skip-clean --out Stdout -- coordinator::execute_workflow
```

**Success Criteria**:
- [ ] 4 new tests covering happy paths
- [ ] Coverage increases to ~30-40%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Add Error Handling Tests

**Goal**: Cover error scenarios and phase failure transitions.

**Changes**:
- Add test for setup phase failure (should propagate error)
- Add test for map phase failure (should stop workflow)
- Add test for reduce phase failure (should propagate error)
- Add test for custom transition handler overriding default error behavior

**Testing**:
```bash
cargo test --lib coordinator::coordinator_test::test_execute_workflow_with_setup_failure
cargo test --lib coordinator::coordinator_test::test_execute_workflow_with_map_failure
cargo test --lib coordinator::coordinator_test::test_execute_workflow_with_reduce_failure
cargo tarpaulin --skip-clean --out Stdout -- coordinator::execute_workflow
```

**Success Criteria**:
- [ ] 4 new tests covering error paths
- [ ] Coverage increases to ~50-60%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Phase Execution Decision Logic

**Goal**: Extract pure functions for phase execution decisions to reduce complexity and improve testability.

**Changes**:
- Extract `should_skip_phase` function (checks transition handler + executor.can_skip)
- Extract `should_execute_reduce` function (checks reduce executor exists + map results available)
- Extract `create_skipped_result` function (creates PhaseResult for skipped phases)
- Add unit tests for each extracted function

**Testing**:
```bash
cargo test --lib coordinator::coordinator_test::test_should_skip_phase
cargo test --lib coordinator::coordinator_test::test_should_execute_reduce
cargo test --lib coordinator::coordinator_test::test_create_skipped_result
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 3 pure functions extracted
- [ ] Each function has complexity ≤3
- [ ] 3 new unit tests (one per function)
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Error Handling Logic

**Goal**: Extract pure functions for error handling and transition decision logic.

**Changes**:
- Extract `handle_phase_error` function (consolidates error logging + transition handling)
- Extract `convert_transition_to_error` function (converts PhaseTransition to MapReduceError)
- Add unit tests for extracted functions

**Testing**:
```bash
cargo test --lib coordinator::coordinator_test::test_handle_phase_error
cargo test --lib coordinator::coordinator_test::test_convert_transition_to_error
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 2 pure functions extracted
- [ ] Each function has complexity ≤3
- [ ] 2 new unit tests
- [ ] Cyclomatic complexity of execute_workflow reduced to ~8
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Extract Phase Result Processing Logic

**Goal**: Extract functions for processing phase results and updating workflow state.

**Changes**:
- Extract `process_phase_success` function (logs success + calls transition handler)
- Extract `update_workflow_result` function (updates workflow_result based on phase)
- Add unit tests for extracted functions

**Testing**:
```bash
cargo test --lib coordinator::coordinator_test::test_process_phase_success
cargo test --lib coordinator::coordinator_test::test_update_workflow_result
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] 2 pure functions extracted
- [ ] Each function has complexity ≤3
- [ ] 2 new unit tests
- [ ] All tests pass
- [ ] Ready to commit

### Phase 6: Final Verification and Edge Cases

**Goal**: Ensure complete coverage and test edge cases.

**Changes**:
- Add test for workflow with no successful phases (should return error)
- Add test for reduce phase skipped due to no map results
- Add test for transition handler returning non-error transition on failure
- Verify coverage metrics

**Testing**:
```bash
cargo test --lib coordinator
cargo tarpaulin --skip-clean --out Stdout -- coordinator::execute_workflow
cargo clippy -- -D warnings
just ci
```

**Success Criteria**:
- [ ] 3 new edge case tests
- [ ] Coverage ≥80% for execute_workflow
- [ ] Cyclomatic complexity ≤8
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib coordinator` to verify existing tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Run `cargo tarpaulin --skip-clean --out Stdout -- coordinator` to check coverage progress
4. Review test output for any failures or unexpected behavior

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --out Stdout` - Full coverage report
3. `debtmap analyze` - Verify improvement in debt score

**Coverage Targets by Phase**:
- Phase 1: ~30-40% (happy paths)
- Phase 2: ~50-60% (error handling)
- Phase 3-5: ~60-75% (refactored logic with tests)
- Phase 6: ≥80% (edge cases)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure output
3. Identify the root cause
4. Adjust the plan if needed
5. Retry the phase

For test failures:
- Check if existing tests are affected
- Verify mock setup is correct
- Review assertion expectations

For refactoring issues:
- Ensure extracted functions maintain original behavior
- Check that function signatures are correct
- Verify all call sites are updated

## Notes

### Key Insights from Code Analysis

1. **Workflow State Machine**: The function implements a clear state machine (Setup → Map → Reduce) with conditional phase execution
2. **Error Handling Patterns**: Currently duplicates error handling logic for each phase (warn + transition handler + return)
3. **Result Tracking**: Uses mutable `workflow_result` variable that gets updated after each successful phase
4. **Skip Conditions**: Reduce phase has complex skip logic (check executor exists + map results available)

### Testing Approach

- **Use existing test utilities**: `create_test_environment()`, `create_test_*_phase()` functions
- **Mock SubprocessManager**: Already using `SubprocessManager::production()` in tests
- **Focus on behavior**: Test workflow transitions, not implementation details
- **Property-based testing**: Could add property tests for phase ordering invariants (future enhancement)

### Extraction Patterns

**Phase execution decisions** → Pure predicates:
- `should_skip_phase(handler, executor, context) -> bool`
- `should_execute_reduce(reduce_executor, map_results) -> bool`

**Error handling** → Pure transformations:
- `handle_phase_error(handler, phase_type, error) -> Result<(), MapReduceError>`
- `convert_transition_to_error(transition, fallback_error) -> MapReduceError`

**Success handling** → Side-effect free operations:
- `process_phase_success(handler, phase_type, result)`
- `update_workflow_result(current_result, new_result) -> Option<PhaseResult>`

### Complexity Reduction Strategy

Current complexity sources:
1. Setup phase presence check (if-let) → Keep as-is (single branch)
2. Setup phase error handling (match) → Extract to `handle_phase_error`
3. Map phase error handling (match) → Extract to `handle_phase_error`
4. Reduce phase presence check (if-let) → Extract to `should_execute_reduce`
5. Map results availability check (if) → Extract to `should_execute_reduce`
6. Reduce phase error handling (match) → Extract to `handle_phase_error`

By extracting these patterns, we reduce cyclomatic complexity from 11 to ~7-8, with each extracted function having complexity ≤3.

### Future Enhancements (Out of Scope)

- Add property-based tests for phase ordering invariants
- Consider builder pattern for PhaseCoordinator to simplify test setup
- Add tracing instrumentation for better observability
- Consider refactoring to use a proper state machine library
