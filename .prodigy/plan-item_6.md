# Implementation Plan: Add Test Coverage and Refactor MapReduceCoordinator::handle_on_failure

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/coordination/executor.rs:MapReduceCoordinator::handle_on_failure:1091
**Priority Score**: 32.22
**Debt Type**: TestingGap (41 cognitive complexity, 0% coverage, 16 cyclomatic complexity)
**Current Metrics**:
- Lines of Code: 98
- Cyclomatic Complexity: 16
- Cognitive Complexity: 41
- Coverage: 0% (32 uncovered lines)

**Issue**: Complex business logic with 100% test coverage gap. Cyclomatic complexity of 16 requires at least 16 test cases for full path coverage. The function handles on_failure configuration with three different config variants (Advanced, SingleCommand, and default), executing either Claude commands or shell commands with variable interpolation, error handling, and exit code processing.

## Target State

**Expected Impact**:
- Complexity Reduction: 4.8 (from 16 to ~11)
- Coverage Improvement: 50% (from 0% to 50%+)
- Risk Reduction: 13.53

**Success Criteria**:
- [ ] Test coverage ≥ 80% for handle_on_failure function
- [ ] Extract at least 4 pure functions with complexity ≤ 3 each
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Add Comprehensive Test Coverage

**Goal**: Achieve 80%+ test coverage for the current implementation before refactoring

**Changes**:
- Create new test module `handle_on_failure_tests` in executor.rs tests
- Write 10 tests covering all three OnFailureConfig variants:
  - Test 1-2: Advanced config with Claude command (success/failure)
  - Test 3-4: Advanced config with shell command (success/failure)
  - Test 5-6: SingleCommand with Claude command (starts with "/")
  - Test 7-8: SingleCommand with shell command (no "/")
  - Test 9: Default/empty config (returns Ok(true))
  - Test 10: Variable interpolation errors
- Use mock ClaudeExecutor and SubprocessManager for isolation

**Testing**:
- Run `cargo test handle_on_failure` to verify all new tests pass
- Run `cargo tarpaulin --lib` to verify coverage improvement
- Confirm coverage reaches 80%+ for lines 1091-1187

**Success Criteria**:
- [ ] 10 tests added covering all code paths
- [ ] Coverage ≥ 80% for handle_on_failure
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Command Parsing Logic

**Goal**: Extract pure function for parsing OnFailureConfig into command types

**Changes**:
- Extract `parse_failure_commands` pure function:
  ```rust
  fn parse_failure_commands(config: &OnFailureConfig)
      -> (Option<&str>, Option<&str>)
  ```
  - Returns (claude_cmd, shell_cmd)
  - Handles all three config variants
  - Pure logic with no side effects
  - Complexity ≤ 3
- Update `handle_on_failure` to use the extracted function
- Add 4 unit tests for `parse_failure_commands`:
  - Test parsing Advanced config with both commands
  - Test parsing SingleCommand with "/" prefix
  - Test parsing SingleCommand without "/" prefix
  - Test parsing default config

**Testing**:
- Run `cargo test parse_failure_commands` for new tests
- Run `cargo test handle_on_failure` to ensure integration works
- Verify existing behavior unchanged

**Success Criteria**:
- [ ] `parse_failure_commands` extracted with complexity ≤ 3
- [ ] 4 unit tests added and passing
- [ ] All existing tests still pass
- [ ] Ready to commit

### Phase 3: Extract Variable Interpolation Logic

**Goal**: Extract variable interpolation into a pure, testable function

**Changes**:
- Extract `interpolate_command` pure function:
  ```rust
  fn interpolate_command(
      cmd: &str,
      variables: &HashMap<String, String>
  ) -> MapReduceResult<String>
  ```
  - Creates InterpolationEngine and context
  - Performs interpolation with proper error handling
  - Pure logic with no I/O
  - Complexity ≤ 2
- Update both Claude and shell command blocks to use extracted function
- Add 3 unit tests:
  - Test successful interpolation with variables
  - Test interpolation with missing variables
  - Test interpolation error handling

**Testing**:
- Run `cargo test interpolate_command` for new tests
- Run `cargo test handle_on_failure` for integration
- Verify no duplicate interpolation code remains

**Success Criteria**:
- [ ] `interpolate_command` extracted with complexity ≤ 2
- [ ] 3 unit tests added and passing
- [ ] Code duplication eliminated
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Exit Code Processing Logic

**Goal**: Extract exit code conversion into a pure function

**Changes**:
- Extract `process_exit_status` pure function:
  ```rust
  fn process_exit_status(status: &ExitStatus) -> i32
  ```
  - Converts ExitStatus enum to integer code
  - Pure logic with no side effects
  - Complexity ≤ 2
- Update shell command block to use extracted function
- Add 4 unit tests:
  - Test Success status → 0
  - Test Error(code) status → code
  - Test Timeout status → -1
  - Test Signal(sig) status → -sig

**Testing**:
- Run `cargo test process_exit_status` for new tests
- Run `cargo test handle_on_failure` for integration
- Verify exit code logic works correctly

**Success Criteria**:
- [ ] `process_exit_status` extracted with complexity ≤ 2
- [ ] 4 unit tests added and passing
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Extract Command Execution Helpers

**Goal**: Simplify main function by extracting command execution logic

**Changes**:
- Extract `execute_claude_failure_handler` async function:
  ```rust
  async fn execute_claude_failure_handler(
      cmd: &str,
      worktree_path: &Path,
      variables: &HashMap<String, String>,
      claude_executor: &Arc<dyn ClaudeExecutor>
  ) -> MapReduceResult<bool>
  ```
  - Handles Claude command interpolation and execution
  - Complexity ≤ 3
- Extract `execute_shell_failure_handler` async function:
  ```rust
  async fn execute_shell_failure_handler(
      cmd: &str,
      worktree_path: &Path,
      variables: &HashMap<String, String>,
      subprocess: &Arc<SubprocessManager>
  ) -> MapReduceResult<bool>
  ```
  - Handles shell command interpolation and execution
  - Complexity ≤ 3
- Update `handle_on_failure` to orchestrate via extracted functions
- Add 6 integration tests (3 per executor):
  - Test successful execution
  - Test execution failure
  - Test interpolation errors

**Testing**:
- Run `cargo test execute_claude_failure_handler`
- Run `cargo test execute_shell_failure_handler`
- Run `cargo test handle_on_failure`
- Verify main function is now simple orchestration

**Success Criteria**:
- [ ] Two execution functions extracted with complexity ≤ 3 each
- [ ] 6 integration tests added and passing
- [ ] Main function reduced to simple orchestration
- [ ] All tests pass
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo test <specific_test>` for new tests
3. Run `cargo clippy` to check for warnings
4. Run `cargo fmt` to ensure formatting

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin --lib` - Verify coverage ≥ 80%
3. Verify cyclomatic complexity reduced from 16 to ~11

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review test failures or compilation errors
3. Adjust the implementation approach
4. Retry the phase

## Notes

- **Testing Priority**: Phase 1 establishes the safety net before any refactoring
- **Pure Functions**: Phases 2-4 extract pure, easily testable logic
- **I/O Separation**: Phase 5 separates orchestration from business logic
- **Incremental**: Each phase is independently valuable and committable
- **No Behavior Changes**: All refactoring maintains existing behavior exactly
- **Mock Usage**: Tests use mocks for ClaudeExecutor and SubprocessManager to avoid real I/O
- **Error Handling**: All extracted functions maintain proper error propagation using `?` operator
- **Coverage Target**: Aiming for 80%+ coverage, exceeding the 50% expected improvement
