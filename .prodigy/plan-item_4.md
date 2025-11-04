# Implementation Plan: Reduce Complexity of execute_agent_for_item

## Problem Summary

**Location**: ./src/cook/execution/mapreduce/coordination/executor.rs:MapReduceCoordinator::execute_agent_for_item:828
**Priority Score**: 28.405
**Debt Type**: ComplexityHotspot (Cognitive: 106, Cyclomatic: 29)
**Current Metrics**:
- Lines of Code: 278
- Cyclomatic Complexity: 29
- Cognitive Complexity: 106
- Nesting Depth: 4

**Issue**: High complexity (29 cyclomatic, 106 cognitive) makes function hard to test and maintain. The function handles too many responsibilities: agent creation, timeout management, command execution with variable interpolation, error handling, merge coordination, and cleanup orchestration.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 14.5 (target cyclomatic: ~10-15)
- Coverage Improvement: 0.0 (function is orchestration code)
- Risk Reduction: 9.94175

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 29 to ≤15
- [ ] Cognitive complexity reduced from 106 to ≤50
- [ ] Extract 4-5 focused pure functions for testable logic
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Variable Preparation Logic

**Goal**: Extract the complex variable building logic (lines 910-933) into a pure, testable function.

**Changes**:
- Create new pure function `build_item_variables(item: &Value, item_id: &str) -> HashMap<String, String>`
- Move the HashMap construction and item field flattening logic
- Replace inline logic with function call
- Add unit tests for variable building edge cases

**Testing**:
- Unit test with Object containing strings, numbers, bools, nulls
- Unit test with nested objects (should serialize to JSON)
- Unit test with empty object
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] Variable building logic extracted to pure function
- [ ] Cyclomatic complexity reduced by ~3
- [ ] Unit tests added for variable building
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: Extract Timeout Management Logic

**Goal**: Extract timeout registration and unregistration into helper methods.

**Changes**:
- Create `register_agent_timeout(enforcer: Option<&Arc<TimeoutEnforcer>>, agent_id: &str, item_id: &str, commands: &[WorkflowStep]) -> Option<TimeoutHandle>`
- Create `register_command_lifecycle(enforcer: Option<&Arc<TimeoutEnforcer>>, agent_id: &str, index: usize, elapsed: Option<Duration>) -> Result<()>`
- Replace timeout management blocks (lines 868-881, 901-905, 947-955, 1027-1031) with function calls
- Simplify error handling for timeout operations

**Testing**:
- Unit test timeout registration with Some enforcer
- Unit test timeout registration with None enforcer
- Unit test command lifecycle tracking
- Run `cargo test --lib` to verify existing tests pass

**Success Criteria**:
- [ ] Timeout management logic extracted to 2 helper functions
- [ ] Cyclomatic complexity reduced by ~4
- [ ] Nesting depth reduced by 1
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Command Execution Loop

**Goal**: Extract the command execution loop (lines 892-1004) into a separate method to reduce nesting and complexity.

**Changes**:
- Create `execute_agent_commands(handle: &AgentHandle, commands: &[WorkflowStep], item: &Value, item_id: &str, agent_id: &str, env: &ExecutionEnvironment, claude_executor: &Arc<dyn ClaudeExecutor>, subprocess: &Arc<SubprocessManager>, timeout_enforcer: Option<&Arc<TimeoutEnforcer>>, user_interaction: &Arc<dyn UserInteraction>) -> MapReduceResult<(String, Vec<String>, Vec<PathBuf>)>`
- Return tuple of (output, commits, files_modified)
- Move variable building call into this function
- Move step execution loop into this function
- Keep the main function focused on orchestration

**Testing**:
- Integration test with mock commands
- Test error handling in command loop
- Test on_failure handler execution
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] Command execution loop extracted to separate method
- [ ] Cyclomatic complexity reduced by ~8
- [ ] Nesting depth reduced by 2
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Merge and Cleanup Logic

**Goal**: Extract merge coordination and cleanup logic (lines 1033-1087) into a focused method.

**Changes**:
- Create `merge_and_cleanup_agent(agent_manager: &Arc<dyn AgentLifecycleManager>, merge_queue: &Arc<MergeQueue>, handle: AgentHandle, config: &AgentConfig, result: &AgentResult, env: &ExecutionEnvironment, agent_id: &str, item_id: &str) -> MapReduceResult<bool>`
- Return bool indicating merge success
- Consolidate error logging and cleanup handling
- Simplify the conditional logic for commits vs no-commits

**Testing**:
- Test merge with commits (successful merge)
- Test merge with commits (failed merge)
- Test cleanup without commits
- Test cleanup failure handling
- Run `cargo test --lib` to verify all tests pass

**Success Criteria**:
- [ ] Merge and cleanup logic extracted to separate method
- [ ] Cyclomatic complexity reduced by ~6
- [ ] Error handling simplified
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Cleanup and Verification

**Goal**: Clean up the main function, verify complexity targets met, and ensure code quality.

**Changes**:
- Ensure main function is now a clear orchestration flow:
  1. Create agent config
  2. Create agent with worktree
  3. Register timeout
  4. Execute commands
  5. Unregister timeout
  6. Merge and cleanup
  7. Return result
- Add docstring comments to all new helper functions
- Run full CI suite

**Testing**:
- Run `cargo test --all` - all tests pass
- Run `cargo clippy -- -D warnings` - no warnings
- Run `cargo fmt -- --check` - properly formatted
- Run `just ci` - full CI checks pass
- Manually verify cyclomatic complexity with `cargo-geiger` or similar tool

**Success Criteria**:
- [ ] Main function is clear orchestration with ≤15 cyclomatic complexity
- [ ] All helper functions documented
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Complexity target achieved (≤15 cyclomatic, ≤50 cognitive)

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Add unit tests for extracted functions where applicable
4. Commit working changes

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test --all` - All tests pass
3. Manual review of function complexity
4. Verify all 4-5 extracted functions are focused and testable

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure (test failures, clippy warnings, logic errors)
3. Adjust the approach (e.g., different function signature, different extraction boundaries)
4. Retry the phase

## Notes

**Key Complexity Sources**:
- Multiple nested conditionals (timeout checks, error handling, on_failure)
- Long command execution loop with variable interpolation
- Merge and cleanup coordination with error handling
- Timeout registration/unregistration scattered throughout

**Refactoring Approach**:
- Focus on extracting **orchestration concerns** (timeout, merge) into helper methods
- Extract **pure logic** (variable building) into testable functions
- Keep the main function as a **clear sequential flow** of high-level steps
- Don't try to force test coverage on orchestration code - focus on making it readable

**Risk Mitigation**:
- Each phase is independently testable
- All changes preserve existing behavior
- Commit after each phase for easy rollback
- No changes to public API or function signature
