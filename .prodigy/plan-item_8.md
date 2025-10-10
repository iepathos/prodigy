# Implementation Plan: Add Tests and Refactor clean_all_checkpoints

## Problem Summary

**Location**: ./src/cli/commands/checkpoints.rs:clean_all_checkpoints:307
**Priority Score**: 29.21
**Debt Type**: TestingGap (cognitive: 43, cyclomatic: 9, coverage: 0%)
**Current Metrics**:
- Lines of Code: 33
- Functions: 1
- Cyclomatic Complexity: 9
- Coverage: 0%
- Nesting Depth: 6

**Issue**: Add 7 tests for 100% coverage gap, then refactor complexity 9 into 8 functions

**Rationale**: Complex business logic with 100% gap. Cyclomatic complexity of 9 requires at least 9 test cases for full path coverage. After extracting 8 functions, each will need only 3-5 tests. Testing before refactoring ensures no regressions.

## Target State

**Expected Impact**:
- Complexity Reduction: 2.7
- Coverage Improvement: 50.0%
- Risk Reduction: 12.27

**Success Criteria**:
- [ ] 9+ tests covering all branches (100% coverage)
- [ ] Extract 8 pure functions (complexity ≤3 each)
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Comprehensive Test Coverage

**Goal**: Achieve 100% test coverage for `clean_all_checkpoints` before refactoring

**Changes**:
- Create test module for checkpoint cleaning functionality
- Add test fixtures and helper functions for checkpoint creation
- Write 9 test cases covering all branches:
  1. Test cleaning with empty checkpoint directory
  2. Test cleaning with no completed checkpoints
  3. Test cleaning with only in-progress checkpoints
  4. Test cleaning with mix of completed and in-progress checkpoints
  5. Test cleaning with force flag (no confirmation)
  6. Test cleaning with non-JSON files in directory
  7. Test cleaning with corrupted checkpoint files
  8. Test error handling when file removal fails
  9. Test concurrent checkpoint access scenarios

**Testing**:
- Run `cargo test --lib commands::checkpoints::tests` to verify all tests pass
- Run `cargo tarpaulin --out Html --output-dir coverage` to verify 100% coverage
- Verify all 9 branches are covered

**Success Criteria**:
- [ ] 9+ tests written covering all code paths
- [ ] All tests pass
- [ ] Coverage for `clean_all_checkpoints` reaches 100%
- [ ] Ready to commit

### Phase 2: Extract Pure Functions - Checkpoint Filtering Logic

**Goal**: Extract filtering and validation logic into testable pure functions

**Changes**:
- Extract `is_json_checkpoint_file(entry: &DirEntry) -> bool`
  - Pure predicate checking if entry is a JSON checkpoint file
  - Complexity: 2 (checks is_file and extension)

- Extract `extract_workflow_id(path: &Path) -> Option<String>`
  - Pure function extracting workflow ID from path
  - Complexity: 2 (file_stem and string conversion)

- Extract `is_completed_checkpoint(checkpoint: &WorkflowCheckpoint) -> bool`
  - Pure predicate checking if checkpoint is completed
  - Complexity: 1 (status comparison)

**Testing**:
- Write 3-5 unit tests per extracted function
- Test edge cases (no extension, invalid UTF-8, etc.)
- Run `cargo test --lib` to ensure all tests pass

**Success Criteria**:
- [ ] 3 pure functions extracted
- [ ] Each function has ≤3 complexity
- [ ] 9-15 new unit tests for extracted functions
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Pure Functions - Checkpoint Loading and Deletion

**Goal**: Extract checkpoint management operations into focused functions

**Changes**:
- Extract `collect_checkpoint_entries(checkpoint_dir: &PathBuf) -> Result<Vec<PathBuf>>`
  - Returns list of valid checkpoint file paths
  - Complexity: 3 (iteration, filtering, collection)

- Extract `load_checkpoint_safely(manager: &CheckpointManager, workflow_id: &str) -> Option<WorkflowCheckpoint>`
  - Safe wrapper around checkpoint loading with error handling
  - Complexity: 2 (load and error conversion)

- Extract `delete_checkpoint_file(path: &Path) -> Result<()>`
  - Simple wrapper for file deletion with context
  - Complexity: 1 (single operation)

**Testing**:
- Write 3-5 unit tests per extracted function
- Test error conditions (missing files, permission errors)
- Run `cargo test --lib` to ensure all tests pass

**Success Criteria**:
- [ ] 3 pure functions extracted
- [ ] Each function has ≤3 complexity
- [ ] 9-15 new unit tests for extracted functions
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Pure Functions - Orchestration and Confirmation

**Goal**: Separate orchestration logic and user interaction

**Changes**:
- Extract `filter_completed_checkpoints(entries: Vec<PathBuf>, manager: &CheckpointManager) -> Result<Vec<(PathBuf, String)>>`
  - Filters paths to only completed checkpoints with workflow IDs
  - Complexity: 3 (iteration, filtering, mapping)

- Extract `confirm_deletion(workflow_id: &str, force: bool) -> Result<bool>`
  - Handles user confirmation logic (separated for testability)
  - Complexity: 3 (force check, I/O, input validation)

**Testing**:
- Write 3-5 unit tests per extracted function
- Mock user input for confirmation tests
- Run `cargo test --lib` to ensure all tests pass

**Success Criteria**:
- [ ] 2 pure functions extracted
- [ ] Each function has ≤3 complexity
- [ ] 6-10 new unit tests for extracted functions
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Refactor Main Function - Compose Pure Functions

**Goal**: Refactor `clean_all_checkpoints` to compose the extracted pure functions

**Changes**:
- Rewrite `clean_all_checkpoints` as a simple composition:
  1. Call `collect_checkpoint_entries` to get checkpoint paths
  2. Call `filter_completed_checkpoints` to identify completed ones
  3. For each completed checkpoint:
     - Call `confirm_deletion` (if not force)
     - Call `delete_checkpoint_file`
  4. Print summary
- Target complexity: ≤4 (down from 9)
- Reduce nesting depth to ≤3 (down from 6)

**Testing**:
- All existing tests should continue to pass
- Run integration tests to verify end-to-end behavior
- Run `cargo clippy` to check for warnings
- Run `cargo tarpaulin` to verify final coverage ≥50%

**Success Criteria**:
- [ ] `clean_all_checkpoints` complexity ≤4
- [ ] Nesting depth ≤3
- [ ] All original tests pass
- [ ] All new unit tests pass
- [ ] Coverage ≥50%
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib commands::checkpoints` to verify tests pass
2. Run `cargo clippy -- -D warnings` to check for issues
3. Run `cargo fmt --check` to verify formatting

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --out Html --output-dir coverage` - Verify coverage improvement
3. Verify metrics improvement:
   - Cyclomatic complexity: 9 → ≤6 (target: 6.3)
   - Coverage: 0% → ≥50%
   - Number of functions: 1 → 9 (main + 8 extracted)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure logs and test output
3. Adjust the plan or implementation approach
4. Retry with fixes

If tests are failing:
- Do NOT disable tests
- Investigate root cause
- Fix implementation or test expectations
- Ensure all tests pass before proceeding

## Notes

**Key Insights from Code Analysis**:
- Function has 6 levels of nesting (while loop → if file → if json → if extract → if load → if completed)
- The function mixes I/O (file operations, user prompts) with business logic (filtering completed checkpoints)
- No error handling for user input confirmation
- Force flag bypasses confirmation but still prints individual messages

**Extraction Strategy**:
- Phase 1 adds tests to lock in current behavior before refactoring
- Phases 2-4 extract pure functions progressively (filtering → loading → orchestration)
- Phase 5 composes the pure functions into a simpler main function
- Each extracted function should be independently testable with minimal mocking

**Testing Priorities**:
1. Cover all 9 branches in Phase 1 to establish baseline
2. Test edge cases for each extracted function (Phases 2-4)
3. Integration tests to ensure composition works correctly (Phase 5)

**Potential Challenges**:
- User input testing may require stdin mocking
- Async testing with tokio runtime
- File system operations may need temp directories
- CheckpointManager is a complex dependency (consider mock/trait)
