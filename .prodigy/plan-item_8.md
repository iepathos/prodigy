# Implementation Plan: Add Test Coverage and Refactor `execute_map_with_checkpoints`

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/checkpoint_integration.rs:CheckpointedCoordinator::execute_map_with_checkpoints:215
**Priority Score**: 31.56
**Debt Type**: TestingGap (cognitive: 53, cyclomatic: 11, coverage: 0.0%)

**Current Metrics**:
- Lines of Code: 59
- Cyclomatic Complexity: 11
- Cognitive Complexity: 53
- Coverage: 0.0%
- Nesting Depth: 2
- Uncovered Lines: 215, 220, 223-225, 229-230, 233-240, 242, 246-247, 250, 252-253, 256, 258, 261-263, 268-269, 271

**Issue**: Complex business logic with 100% testing gap and high cognitive complexity (53). Cyclomatic complexity of 11 requires at least 11 test cases for full path coverage. The function orchestrates checkpoint-based map phase execution but mixes concerns: checkpoint state management, work item loading, batch processing coordination, and periodic checkpoint saving.

## Target State

**Expected Impact**:
- Complexity Reduction: 3.3 (from 11 to ~7-8)
- Coverage Improvement: 50.0% (from 0% to 50%+)
- Risk Reduction: 13.26

**Success Criteria**:
- [ ] At least 7 comprehensive tests covering critical branches (minimum for 50% coverage)
- [ ] Extract 3-5 pure functions for validation, checkpoint decision logic, and state updates
- [ ] Reduce cyclomatic complexity to ≤8 per function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting (rustfmt)

## Implementation Phases

### Phase 1: Add Foundation Tests for Happy Path

**Goal**: Establish baseline test coverage for the main execution flow, proving the function works correctly in the common case.

**Changes**:
1. Create new test module `#[cfg(test)]` in `checkpoint_integration.rs` (or separate test file)
2. Add helper functions for test setup:
   - `create_test_coordinator()` - Build CheckpointedCoordinator with test dependencies
   - `create_test_map_phase()` - Create simple MapPhase configuration
   - `create_test_env()` - Create minimal ExecutionEnvironment
3. Write 3 core tests:
   - `test_execute_map_with_checkpoints_empty_items` - Verify behavior with no work items
   - `test_execute_map_with_checkpoints_single_batch` - Process one batch successfully
   - `test_execute_map_with_checkpoints_multiple_batches` - Process multiple batches with checkpointing

**Testing**:
```bash
cargo test --lib checkpoint_integration::tests
cargo test --lib  # Verify no regressions
```

**Success Criteria**:
- [ ] 3 passing tests for happy path scenarios
- [ ] Test coverage increases from 0% to ~20-30%
- [ ] All existing tests still pass
- [ ] No clippy warnings

### Phase 2: Add Error Path and Edge Case Tests

**Goal**: Cover error conditions, checkpoint triggers, and boundary cases to improve branch coverage.

**Changes**:
1. Add tests for error scenarios:
   - `test_execute_map_checkpoint_on_interval` - Verify checkpointing triggers based on item count
   - `test_execute_map_checkpoint_on_time` - Verify time-based checkpoint triggers (if applicable)
   - `test_execute_map_with_failed_batch` - Handle batch processing failures
   - `test_execute_map_resumes_correctly` - Verify state persistence across checkpoint saves
2. Mock or stub dependencies to control behavior:
   - Checkpoint save operations
   - Batch processing results
   - Work item loading

**Testing**:
```bash
cargo test --lib checkpoint_integration::tests::test_execute_map
cargo tarpaulin --lib --packages prodigy --exclude-files "tests/*"
```

**Success Criteria**:
- [ ] 7+ total tests covering critical branches
- [ ] Test coverage reaches 50%+ on `execute_map_with_checkpoints`
- [ ] All tests pass
- [ ] Error paths properly validated

### Phase 3: Extract Pure Function - Checkpoint Decision Logic

**Goal**: Reduce complexity by extracting the checkpoint decision logic into a pure, testable function.

**Changes**:
1. Create new pure function:
   ```rust
   fn should_save_checkpoint_for_batch(
       items_processed_since_last: usize,
       last_checkpoint_time: DateTime<Utc>,
       checkpoint_config: &CheckpointConfig,
   ) -> bool
   ```
2. Replace inline checkpoint decision in `execute_map_with_checkpoints` (lines 261-263)
3. Add 3-5 unit tests specifically for this function:
   - `test_should_checkpoint_by_item_count`
   - `test_should_checkpoint_by_time_interval`
   - `test_should_not_checkpoint_too_soon`

**Testing**:
```bash
cargo test --lib should_save_checkpoint
cargo clippy -- -D warnings
```

**Success Criteria**:
- [ ] Pure function extracted with clear inputs/outputs
- [ ] Function has 3-5 dedicated unit tests
- [ ] Complexity of `execute_map_with_checkpoints` reduced by 1-2 points
- [ ] All tests pass
- [ ] No clippy warnings

### Phase 4: Extract Pure Function - Checkpoint State Initialization

**Goal**: Extract the checkpoint state mutation logic (lines 223-243) into a testable function.

**Changes**:
1. Create new function:
   ```rust
   fn initialize_map_phase_checkpoint(
       checkpoint: &mut Checkpoint,
       work_items: Vec<Value>,
       phase: PhaseType,
   ) -> Vec<WorkItem>
   ```
2. Replace inline state setup in `execute_map_with_checkpoints`
3. Add 3-4 unit tests:
   - `test_initialize_map_checkpoint_empty`
   - `test_initialize_map_checkpoint_with_items`
   - `test_initialize_map_checkpoint_preserves_metadata`

**Testing**:
```bash
cargo test --lib initialize_map_phase_checkpoint
cargo test --lib checkpoint_integration
```

**Success Criteria**:
- [ ] State initialization logic extracted
- [ ] Function has 3-4 dedicated tests
- [ ] Complexity further reduced
- [ ] All tests pass
- [ ] Main function reads more clearly

### Phase 5: Extract Pure Function - Results Aggregation Logic

**Goal**: Separate the results collection and checkpoint update coordination (lines 250-265).

**Changes**:
1. Create new function:
   ```rust
   async fn process_batches_with_checkpointing(
       coordinator: &CheckpointedCoordinator,
       map_phase: &MapPhase,
       env: &ExecutionEnvironment,
       max_parallel: usize,
   ) -> Result<Vec<AgentResult>>
   ```
2. Replace the while loop and result aggregation in `execute_map_with_checkpoints`
3. Add 3-4 tests:
   - `test_process_batches_single_batch`
   - `test_process_batches_triggers_checkpoint`
   - `test_process_batches_aggregates_results`

**Testing**:
```bash
cargo test --lib process_batches
cargo tarpaulin --lib --packages prodigy
```

**Success Criteria**:
- [ ] Batch processing loop extracted
- [ ] 3-4 tests for batch coordination
- [ ] `execute_map_with_checkpoints` now has complexity ≤8
- [ ] Test coverage ≥60% for the module
- [ ] All tests pass

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy -- -D warnings` to check for warnings
3. Run `cargo fmt` to ensure formatting
4. For phases with extraction, run `cargo tarpaulin` to measure coverage improvement

**Final verification**:
1. `cargo test --lib` - All tests pass
2. `cargo clippy -- -D warnings` - No warnings
3. `cargo fmt --check` - Properly formatted
4. `cargo tarpaulin --lib --packages prodigy` - Verify coverage ≥50%
5. Review `execute_map_with_checkpoints` - Confirm complexity ≤8, function length ≤30 lines

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failures or errors
3. If test design issue: Adjust test approach and retry
4. If implementation issue: Simplify extraction and retry
5. Document any blockers or assumptions that were incorrect

## Notes

**Key Testing Challenges**:
- Mocking async checkpoint operations may require `tokio::test` and careful setup
- `ExecutionEnvironment` may have complex dependencies - consider builder pattern for tests
- Work items are `serde_json::Value` - use simple JSON objects in tests
- Checkpoint state is wrapped in `Arc<RwLock<>>` - tests need to handle async locking

**Refactoring Principles**:
- Extract functions that take inputs and return results (minimize state mutations)
- Keep checkpoint state updates explicit and visible in main function
- Don't over-extract - keep related logic together if it reduces readability
- Prefer small, focused functions over large orchestrators

**Coverage Target Justification**:
- 50% coverage is realistic for first pass (7-8 tests covering main branches)
- 100% coverage would require extensive mocking of coordinator internals
- Focus on business logic paths, not every error branch in dependencies
- Priority: test checkpoint triggers, state updates, and batch coordination

**Complexity Sources**:
- Multiple mutable checkpoint state accesses (lines 223-243)
- While loop with multiple conditional branches (lines 252-265)
- Async operations and error handling throughout
- Integration with multiple dependencies (checkpoint manager, coordinator)
