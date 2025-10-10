# Implementation Plan: Add Tests and Refactor execute_map_with_checkpoints

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/checkpoint_integration.rs:CheckpointedCoordinator::execute_map_with_checkpoints:215
**Priority Score**: 31.26
**Debt Type**: TestingGap (cognitive: 48, coverage: 0.0, cyclomatic: 11)
**Current Metrics**:
- Lines of Code: 51
- Functions: 1
- Cyclomatic Complexity: 11
- Coverage: 0%

**Issue**: Add 7 tests for 100% coverage gap, then refactor complexity 11 into 9 functions. Complex business logic with 100% gap. Cyclomatic complexity of 11 requires at least 11 test cases for full path coverage. After extracting 9 functions, each will need only 3-5 tests. Testing before refactoring ensures no regressions.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 3.3
- Coverage Improvement: 50.0%
- Risk Reduction: 13.13

**Success Criteria**:
- [ ] 100% test coverage for execute_map_with_checkpoints function
- [ ] Cyclomatic complexity reduced from 11 to ~3 per function
- [ ] Extract 9 pure functions from complex logic
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Integration Tests for Happy Path Coverage

**Goal**: Create comprehensive integration tests that cover the main execution path and key state transitions.

**Changes**:
- Add test for successful map phase execution with empty work items
- Add test for phase transition from Setup to Map
- Add test for work items loading and checkpoint state update
- Add test for batch processing loop with multiple items
- Add test for checkpoint saving at proper intervals

**Testing**:
- Run `cargo test test_execute_map_with_checkpoints` to verify new tests pass
- Verify coverage increases from 0% to ~40%

**Success Criteria**:
- [ ] 5 integration tests passing
- [ ] Coverage for lines 220, 223-224, 228-234, 238-239, 244-250, 260-261
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 2: Add Tests for Checkpoint Decision Logic

**Goal**: Test the checkpoint triggering logic and items counter management.

**Changes**:
- Add test for should_checkpoint logic with various item counts
- Add test for items_since_checkpoint counter reset after checkpoint
- Add test for checkpoint triggering at exact threshold
- Add test for no checkpoint when below threshold
- Add test for final checkpoint at phase end

**Testing**:
- Run `cargo test test_checkpoint` to verify checkpoint tests pass
- Verify coverage increases to ~60%

**Success Criteria**:
- [ ] 5 checkpoint-related tests passing
- [ ] Coverage for lines 253-255, 263
- [ ] Counter reset logic tested
- [ ] Ready to commit

### Phase 3: Extract Pure Functions for Testability

**Goal**: Extract complex logic into pure functions to reduce cyclomatic complexity.

**Changes**:
- Extract `validate_checkpoint_state` - validates checkpoint is in correct state
- Extract `calculate_batch_size` - determines optimal batch size
- Extract `should_save_checkpoint` - checkpoint decision logic
- Extract `prepare_work_items` - transforms raw items into WorkItems
- Extract `update_phase_metadata` - updates checkpoint phase information

**Testing**:
- Run `cargo test` to ensure no regressions
- Add unit tests for each extracted pure function (3-5 tests each)

**Success Criteria**:
- [ ] 5 pure functions extracted
- [ ] Each function has complexity ≤3
- [ ] 15-25 unit tests for pure functions
- [ ] Coverage increases to ~75%
- [ ] Ready to commit

### Phase 4: Extract Batch Processing Logic

**Goal**: Further reduce complexity by extracting batch processing and result handling.

**Changes**:
- Extract `process_work_batch` - handles single batch processing
- Extract `aggregate_batch_results` - combines results from batches
- Extract `update_checkpoint_progress` - updates checkpoint with batch progress
- Extract `handle_batch_completion` - manages post-batch checkpoint logic

**Testing**:
- Add unit tests for batch processing functions
- Test edge cases (empty batches, single item, large batches)
- Verify error handling paths

**Success Criteria**:
- [ ] 4 additional pure functions extracted
- [ ] Total of 9 pure functions as required
- [ ] Coverage increases to ~90%
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Add Edge Case and Error Condition Tests

**Goal**: Achieve 100% test coverage by testing edge cases and error paths.

**Changes**:
- Add test for None checkpoint state handling
- Add test for empty work items processing
- Add test for maximum batch size limits
- Add test for checkpoint save failure recovery
- Add property-based tests for batch processing logic

**Testing**:
- Run `cargo tarpaulin` to verify 100% coverage achieved
- Run `cargo test --lib` for full test suite
- Run `just ci` for complete validation

**Success Criteria**:
- [ ] 100% test coverage for execute_map_with_checkpoints
- [ ] All edge cases covered
- [ ] Property-based tests passing
- [ ] CI checks pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run specific test pattern for phase: `cargo test test_<phase_focus>`
4. Check coverage with `cargo tarpaulin --lib`

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage and verify 100%
3. `debtmap analyze` - Verify improvement in debt score

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - check test output and error messages
3. Adjust the plan - may need different extraction boundaries
4. Retry with adjusted approach

## Notes

- The function has high complexity due to multiple checkpoint state updates and batch processing logic
- Focus on extracting pure functions that can be easily unit tested
- The existing tests in the file provide good patterns to follow (test_create_work_items, test_update_checkpoint_to_map_phase, etc.)
- Some helper functions already exist (create_work_items, update_checkpoint_to_map_phase) showing the refactoring pattern
- Ensure extracted functions maintain the same async boundaries where needed
- Property-based testing will help ensure batch processing logic is robust