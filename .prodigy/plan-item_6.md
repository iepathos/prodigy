# Implementation Plan: Add Tests for run_checkpoints_command Entry Point

## Problem Summary

**Location**: ./src/cli/commands/checkpoints.rs:run_checkpoints_command:194
**Priority Score**: 25.59
**Debt Type**: TestingGap (cognitive: 69, cyclomatic: 21, coverage: 0%)
**Current Metrics**:
- Lines of Code: 89
- Cyclomatic Complexity: 21
- Cognitive Complexity: 69
- Coverage: 0%

**Issue**: Function has 0% coverage with complexity 21/69. This is a CLI command handler that orchestrates checkpoint operations across 6 different subcommands (List, Clean, Show, Validate, MapReduce, Delete). While orchestration code naturally has higher complexity due to branching logic, the complete lack of test coverage creates significant risk. The function has already been well-refactored with extracted pure functions, but the entry point itself remains untested.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 6.3 (achieved through extracting complex branches)
- Coverage Improvement: 50% (minimum target from debtmap analysis)
- Risk Reduction: 5.76

**Success Criteria**:
- [ ] Add comprehensive test coverage for all 6 checkpoint subcommands
- [ ] Achieve at least 50% coverage of the entry point function
- [ ] All existing tests continue to pass
- [ ] No clippy warnings introduced
- [ ] Proper formatting maintained

## Implementation Phases

The strategy is to add focused integration tests for each command path through the entry point, building coverage incrementally. Each phase adds tests for specific subcommands, ensuring all branches are exercised.

### Phase 1: Test Infrastructure and List Command

**Goal**: Set up test infrastructure and add tests for the List subcommand (both all checkpoints and specific workflow)

**Changes**:
- Add test module for `run_checkpoints_command` entry point tests
- Create helper functions for test checkpoint creation and storage setup
- Add test for `List` command with workflow_id (specific checkpoint)
- Add test for `List` command without workflow_id (all checkpoints)
- Add test for `List` command with non-existent checkpoint directory

**Testing**:
- `cargo test run_checkpoints_command` - Run the new tests
- `cargo test --lib` - Verify all existing tests still pass
- Verify tests cover both verbose and non-verbose modes

**Success Criteria**:
- [ ] Test infrastructure helpers created and working
- [ ] List command with workflow_id covered
- [ ] List command without workflow_id covered
- [ ] List command with non-existent directory covered
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Clean Command Tests

**Goal**: Add comprehensive tests for the Clean subcommand covering all validation paths

**Changes**:
- Add test for `Clean` with workflow_id (specific checkpoint)
- Add test for `Clean` with --all flag (all completed)
- Add test for `Clean` with neither workflow_id nor --all (invalid request)
- Add test for `Clean` with force flag (no confirmation)
- Add test for `Clean` on non-existent checkpoint directory

**Testing**:
- `cargo test run_checkpoints_command::clean` - Run clean command tests
- Verify clean_specific_checkpoint and clean_all_checkpoints are invoked
- Check that force flag bypasses confirmation

**Success Criteria**:
- [ ] Clean with workflow_id covered
- [ ] Clean with --all flag covered
- [ ] Clean invalid request path covered
- [ ] Clean with force flag covered
- [ ] Clean on non-existent directory covered
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Show and Validate Command Tests

**Goal**: Add tests for Show and Validate subcommands

**Changes**:
- Add test for `Show` command with valid workflow_id
- Add test for `Show` command with non-existent workflow_id
- Add test for `Validate` command with valid checkpoint
- Add test for `Validate` command with repair flag
- Add test for `Validate` command with invalid checkpoint

**Testing**:
- `cargo test run_checkpoints_command::show` - Run show tests
- `cargo test run_checkpoints_command::validate` - Run validate tests
- Verify show_checkpoint_details is invoked correctly
- Verify validate_checkpoint handles repair flag

**Success Criteria**:
- [ ] Show command with valid workflow_id covered
- [ ] Show command with invalid workflow_id covered
- [ ] Validate command basic path covered
- [ ] Validate command with repair flag covered
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: MapReduce and Delete Command Tests

**Goal**: Complete coverage by adding tests for MapReduce and Delete subcommands

**Changes**:
- Add test for `MapReduce` command with job_id
- Add test for `MapReduce` command with detailed flag
- Add test for `MapReduce` command with non-existent job
- Add test for `Delete` command with checkpoint_id
- Add test for `Delete` command with force flag
- Add test for `Delete` command with non-existent checkpoint

**Testing**:
- `cargo test run_checkpoints_command::mapreduce` - Run mapreduce tests
- `cargo test run_checkpoints_command::delete` - Run delete tests
- Verify list_mapreduce_checkpoints is invoked
- Verify delete_checkpoint handles force flag

**Success Criteria**:
- [ ] MapReduce command basic path covered
- [ ] MapReduce command with detailed flag covered
- [ ] Delete command basic path covered
- [ ] Delete command with force flag covered
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Verification and Coverage Analysis

**Goal**: Verify coverage targets met and all edge cases handled

**Changes**:
- Run full test suite with coverage analysis
- Add any missing edge case tests identified by coverage report
- Verify coverage improvement meets 50% target
- Document test coverage in module documentation
- Run clippy and fix any warnings

**Testing**:
- `just ci` - Full CI checks
- `cargo tarpaulin --lib` - Generate coverage report
- `cargo clippy -- -W clippy::cognitive_complexity` - Check for warnings
- Verify coverage report shows improvement

**Success Criteria**:
- [ ] Coverage of run_checkpoints_command >= 50%
- [ ] All clippy warnings resolved
- [ ] All tests pass
- [ ] No formatting issues
- [ ] Documentation updated
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo test run_checkpoints_command` to test new coverage
3. Use `--test-threads=1` if tests have I/O conflicts
4. Verify no test pollution between tests using temp directories

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib --exclude-files 'tests/*'` - Coverage report
3. `debtmap analyze` - Verify debt reduction (if available)

**Test Organization**:
- Create `test_run_checkpoints_command` module in existing tests section
- Use existing helper functions (create_test_checkpoint, save_checkpoint_to_file)
- Create new helpers for CheckpointCommands construction
- Use TempDir for all file I/O to ensure test isolation

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure details
3. Check if existing helper functions can be reused
4. Adjust test expectations or implementation
5. Retry with corrected approach

## Notes

**Why Entry Point Tests Matter**:
- Entry points are critical paths - failures affect all users
- Orchestration logic needs integration testing, not just unit tests
- Tests document expected behavior for each subcommand
- Refactoring is safer with comprehensive entry point coverage

**Testing Strategy Rationale**:
- Focus on integration tests rather than mocking everything
- Use real file system operations with TempDir for isolation
- Test both success and error paths for each command
- Verify command routing logic works correctly

**Complexity vs Coverage**:
- High complexity is acceptable for CLI orchestration code
- The complexity is well-managed through extracted pure functions
- Coverage is the primary concern - need to verify all paths work
- Tests will document the expected behavior of each branch

**Existing Test Infrastructure**:
- File already has excellent pure function tests (lines 777-1324)
- Helper functions like create_test_checkpoint and save_checkpoint_to_file exist
- Can reuse test patterns from clean_all_checkpoints tests
- TempDir pattern already established and working well
