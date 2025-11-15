# Implementation Plan: Reduce Complexity in WorkflowExecutor::execute_command_by_type

## Problem Summary

**Location**: ./src/cook/workflow/executor/commands.rs:WorkflowExecutor::execute_command_by_type:406
**Priority Score**: 49.51
**Debt Type**: ComplexityHotspot (Cognitive: 87, Cyclomatic: 31)
**Current Metrics**:
- Lines of Code: 152
- Cyclomatic Complexity: 31
- Cognitive Complexity: 87
- Nesting Depth: 5

**Issue**: High complexity (31 cyclomatic, 87 cognitive) makes function hard to test and maintain. This is a CLI command handler with natural branching for different command types, but the dry-run mode handling adds significant additional complexity.

## Target State

**Expected Impact**:
- Complexity Reduction: 15.5 (from 31 to ~15-16)
- Coverage Improvement: 0.0 (already well-tested)
- Risk Reduction: 9.04

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 31 to ≤16
- [ ] Cognitive complexity reduced from 87 to ≤50
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Dry-Run Mode Logic

**Goal**: Separate dry-run mode handling from the main command dispatch logic to reduce nesting and branching.

**Changes**:
- Create a new function `handle_dry_run_mode(&mut self, command_type: &CommandType, step: &WorkflowStep, env: &ExecutionEnvironment, ctx: &mut WorkflowContext) -> Result<StepResult>`
- Extract lines 414-489 (dry-run handling) into this function
- This includes:
  - Command description building
  - Dry-run output tracking
  - On-failure handler tracking
  - Commit requirement tracking
  - Validation simulation
- Function returns early with dry-run StepResult

**Testing**:
- Run existing tests: `cargo test execute_command_by_type`
- Run dry-run mode tests: `cargo test dry_run`
- Verify dry-run behavior is unchanged

**Success Criteria**:
- [ ] Dry-run logic extracted to separate function
- [ ] All tests pass
- [ ] Cyclomatic complexity reduced by ~5-7 points
- [ ] Ready to commit

### Phase 2: Extract Command Description Formatting

**Goal**: Simplify the dry-run mode handler by extracting the command description logic that's duplicated between dry-run and format_command_description.

**Changes**:
- Replace the inline match in dry-run mode (lines 416-428) with a call to the existing `format_command_description` function
- Remove `#[allow(dead_code)]` annotation from `format_command_description` (line 383)
- Ensure the format matches exactly what dry-run mode expects

**Testing**:
- Run dry-run tests: `cargo test dry_run`
- Verify command descriptions are identical to before
- Run `cargo clippy` to ensure no dead code warnings

**Success Criteria**:
- [ ] Duplicate command description logic eliminated
- [ ] format_command_description is actively used
- [ ] All tests pass
- [ ] No dead code warnings
- [ ] Ready to commit

### Phase 3: Extract Command Execution Dispatch

**Goal**: Simplify the main function by extracting the command type dispatch logic (lines 499-555) into a separate function.

**Changes**:
- Create function `dispatch_command_execution(&mut self, command_type: CommandType, step: &WorkflowStep, env: &ExecutionEnvironment, ctx: &mut WorkflowContext, env_vars: HashMap<String, String>) -> Result<StepResult>`
- Move the match statement (lines 499-555) into this function
- This handles all the actual command execution dispatch
- Keep variable interpolation and logging in the dispatch function

**Testing**:
- Run all command execution tests: `cargo test execute_command_by_type`
- Run tests for each command type (claude, shell, test, handler, goal_seek, foreach, write_file)
- Verify all command types execute correctly

**Success Criteria**:
- [ ] Command dispatch logic extracted to separate function
- [ ] All command types execute correctly
- [ ] All tests pass
- [ ] Cyclomatic complexity reduced by ~8-10 points
- [ ] Ready to commit

### Phase 4: Simplify Timeout Handling

**Goal**: Extract the timeout environment variable setup (lines 491-497) into a pure helper function to reduce complexity in the main flow.

**Changes**:
- Create pure function `fn add_timeout_to_env_vars(env_vars: &mut HashMap<String, String>, timeout: Option<u64>)`
- Move timeout logic into this helper
- Call the helper from the main function

**Testing**:
- Run tests with timeout configuration
- Verify timeout environment variable is set correctly
- Run `cargo test execute_command_by_type`

**Success Criteria**:
- [ ] Timeout handling extracted to helper function
- [ ] Environment variables set correctly
- [ ] All tests pass
- [ ] Minor complexity reduction
- [ ] Ready to commit

### Phase 5: Final Verification and Documentation

**Goal**: Verify the refactoring achieved the target complexity reduction and update documentation.

**Changes**:
- Run `cargo clippy` to verify no new warnings
- Run `cargo fmt` to ensure formatting
- Run full test suite: `cargo test --lib`
- Update module-level documentation if needed to reflect new structure
- Verify the function now has clear, single-level branching:
  1. Check dry-run → call handler
  2. Add timeout if needed
  3. Dispatch to command handler

**Testing**:
- Full CI: `just ci` (if available)
- Coverage check: `cargo tarpaulin` (if needed)
- Complexity verification: Use debtmap or similar tool to verify complexity reduced

**Success Criteria**:
- [ ] Cyclomatic complexity ≤16
- [ ] Cognitive complexity ≤50
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Code is formatted
- [ ] Documentation updated

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run phase-specific tests (dry-run, command execution, etc.)

**Final verification**:
1. `just ci` - Full CI checks (if available)
2. `cargo test --all` - All tests including integration tests
3. Verify complexity reduction using static analysis tools

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the test failure or error
3. Adjust the implementation approach
4. Retry the phase

If multiple phases need to be rolled back:
1. Use `git log` to find the commit before the phase sequence
2. Use `git reset --hard <commit-hash>` to return to that state
3. Re-plan the approach based on what was learned

## Notes

**Context**: This function is a CLI command handler, so some branching is natural and expected. The goal is not to eliminate all branches, but to reduce unnecessary nesting and complexity.

**Preservation of Behavior**: All refactoring must preserve exact behavior. This is critical for:
- Dry-run mode output and tracking
- Variable interpolation and resolution logging
- Command execution semantics
- Error handling and propagation

**Why This Approach**:
- Phase 1 provides the biggest complexity win by removing the deep nesting from dry-run mode
- Phase 2 eliminates duplication and improves maintainability
- Phase 3 separates command dispatch from setup, making the flow clearer
- Phase 4 is a minor cleanup for completeness
- Phase 5 ensures quality and verifies the goal was achieved

**Functional Programming Alignment**: The refactoring follows functional principles:
- Extract pure functions (timeout handling)
- Separate I/O (command dispatch) from setup logic
- Reduce nesting and side effects in the main flow
- Each extracted function has a single, clear responsibility
