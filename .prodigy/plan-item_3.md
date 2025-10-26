# Implementation Plan: Extract Pure Functions from run_checkpoints_command

## Problem Summary

**Location**: ./src/cli/commands/checkpoints.rs:run_checkpoints_command:47
**Priority Score**: 20.475
**Debt Type**: TestingGap (0% coverage with high complexity)
**Current Metrics**:
- Lines of Code: 136
- Cyclomatic Complexity: 35
- Cognitive Complexity: 113
- Coverage: 0%

**Issue**: Complex entry point with 100% coverage gap. The function has cyclomatic complexity of 35, requiring at least 35 test cases for full path coverage. This CLI command handler contains orchestration logic mixed with validation, execution, and output formatting. The function has 3 levels of nesting and handles 5 distinct subcommands, each with their own branching logic.

## Target State

**Expected Impact**:
- Complexity Reduction: 10.5 (reduce cyclomatic complexity from 35 to ~24)
- Coverage Improvement: 50% (from 0% to 50%+)
- Risk Reduction: 8.5995

**Success Criteria**:
- [ ] Extract 22 pure functions with complexity ≤3 each
- [ ] Achieve 50%+ test coverage on the entry point
- [ ] 80%+ coverage on extracted pure functions
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Working Directory Resolution Logic

**Goal**: Extract the repeated "get working directory" pattern into a pure function that can be easily tested.

**Changes**:
- Create pure function `resolve_working_directory(path: Option<PathBuf>) -> Result<PathBuf>`
- Replace 5 instances of the `match path { Some(p) => p, None => std::env::current_dir()? }` pattern
- Add unit tests for the extracted function

**Testing**:
- Test with `Some(path)` - should return the path
- Test with `None` - should return current directory
- Test error handling when current directory is unavailable

**Success Criteria**:
- [ ] Function extracted and used in all 5 subcommands
- [ ] 100% coverage on `resolve_working_directory`
- [ ] Cyclomatic complexity reduced from 35 to ~30
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Storage Initialization Logic

**Goal**: Extract the repeated storage initialization pattern into a reusable function.

**Changes**:
- Create function `initialize_checkpoint_storage(working_dir: &Path) -> Result<(GlobalStorage, String, PathBuf)>`
- Returns tuple of (storage, repo_name, checkpoint_dir)
- Replace 4 instances of the storage initialization pattern
- Add unit tests for the extracted function

**Testing**:
- Test with valid repository directory
- Test with invalid/non-git directory
- Test error propagation from GlobalStorage
- Mock filesystem for deterministic testing

**Success Criteria**:
- [ ] Function extracted and used in 4 subcommands
- [ ] 80%+ coverage on `initialize_checkpoint_storage`
- [ ] Cyclomatic complexity reduced from ~30 to ~26
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Checkpoint Manager Creation Logic

**Goal**: Simplify the repeated CheckpointManager initialization pattern.

**Changes**:
- Create function `create_checkpoint_manager(checkpoint_dir: PathBuf) -> CheckpointManager`
- Encapsulates the `#[allow(deprecated)]` pattern and CheckpointStorage::Local usage
- Replace 3 instances in List, Show, and Clean commands
- Add unit tests for the extracted function

**Testing**:
- Test manager creation with valid directory
- Test manager creation with non-existent directory
- Verify correct CheckpointStorage type used

**Success Criteria**:
- [ ] Function extracted and used in 3 subcommands
- [ ] 100% coverage on `create_checkpoint_manager`
- [ ] Cyclomatic complexity reduced from ~26 to ~23
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Subcommand Dispatch Logic

**Goal**: Separate command validation from execution for testability.

**Changes**:
- Create enum `CheckpointOperation` to represent validated operations
- Create function `validate_clean_operation(workflow_id: Option<String>, all: bool) -> Result<CleanOperation>`
  - Returns enum: `CleanSpecific(String)`, `CleanAll`, or `InvalidRequest`
- Extract validation logic from Clean subcommand
- Add comprehensive unit tests for validation logic

**Testing**:
- Test with workflow_id present -> CleanSpecific
- Test with all=true -> CleanAll
- Test with neither -> InvalidRequest
- Test with both (edge case)

**Success Criteria**:
- [ ] Validation function extracted with 100% coverage
- [ ] Command execution separated from validation
- [ ] Cyclomatic complexity reduced from ~23 to ~20
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Add Integration Tests for Entry Point

**Goal**: Achieve 50%+ coverage on the main entry point function.

**Changes**:
- Add integration test for `run_checkpoints_command` with List subcommand
- Add integration test for Clean subcommand (force mode)
- Add integration test for Show subcommand
- Add integration test for error paths (missing checkpoint)
- Use tempdir for isolated test environments

**Testing**:
- Test each subcommand path through the entry point
- Test error handling and propagation
- Test verbose flag behavior
- Test force flag behavior

**Success Criteria**:
- [ ] 50%+ coverage on `run_checkpoints_command`
- [ ] At least 17 test cases covering critical branches
- [ ] All tests pass independently and together
- [ ] No test flakiness
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests first (TDD approach) for extracted functions
2. Extract the function and verify tests pass
3. Run `cargo test --lib` to verify existing tests pass
4. Run `cargo clippy` to check for warnings
5. Run `cargo fmt` to ensure proper formatting

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage improvements
3. Verify cyclomatic complexity reduction via `debtmap analyze`

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure reason
3. Adjust the approach:
   - If tests are flaky, add better isolation
   - If extraction breaks existing code, refine the function signature
   - If complexity doesn't reduce, reconsider the extraction point
4. Retry with adjusted approach

## Notes

### Context from Debtmap Analysis

The debtmap analysis identified this as a CLI Handler with these specific patterns:
- **Command pattern**: Use trait-based dispatch for each subcommand
- **Strategy pattern**: Different output formats could use strategy pattern
- **Separation of concerns**: Extract validation, execution, and output into separate functions

### Complexity Sources

Main contributors to cyclomatic complexity:
1. 5 match arms for subcommands (5 branches)
2. Repeated Option<PathBuf> handling (5 × 2 branches = 10)
3. Conditional logic in Clean (workflow_id vs all) (2 branches)
4. Verbose flag checks (2 branches)
5. Error handling paths (multiple branches)

### Future Refactoring Opportunities

After this initial cleanup, consider:
- Trait-based command dispatch (Spec pattern recommendation)
- Separate output formatting functions
- Command validation layer
- However, these are out of scope for this focused debt fix

### Testing Approach

Following the recommendation to "test before refactoring":
1. Phase 1-4: Extract and test pure functions immediately
2. Phase 5: Add integration tests for the orchestration layer
3. This ensures no regressions and provides safety net for future refactoring
