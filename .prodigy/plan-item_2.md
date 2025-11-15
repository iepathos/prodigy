# Implementation Plan: Extract Pure Functions from execute_mapreduce

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:WorkflowExecutor::execute_mapreduce:1265
**Priority Score**: 29.73
**Debt Type**: ComplexityHotspot (Cyclomatic: 28, Cognitive: 72)

**Current Metrics**:
- Lines of Code: 271
- Cyclomatic Complexity: 28
- Cognitive Complexity: 72
- Function Length: 271 lines
- Nesting Depth: 3

**Issue**: Reduce complexity from 28 to ~10. High complexity 28/72 makes function hard to test and maintain.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 14.0
- Coverage Improvement: 0.0
- Lines Reduction: 0
- Risk Reduction: 7.64

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 28 to ≤14 (50% reduction)
- [ ] Each extracted function has complexity ≤5
- [ ] Main orchestration function is ≤100 lines
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt

## Implementation Phases

### Phase 1: Extract Dry-Run Validation Logic

**Goal**: Extract the dry-run validation block (lines 1276-1334) into a pure function, reducing complexity by ~8 points.

**Changes**:
- Create new pure function `validate_mapreduce_dry_run()` in the `orchestration` module
- Extract dry-run config creation, validator creation, and result handling
- Function signature: `async fn validate_mapreduce_dry_run(workflow: &ExtendedWorkflowConfig) -> Result<()>`
- Replace original block with single function call
- Move all dry-run-specific logic to the new function

**Testing**:
- Run `cargo test executor::` to verify existing tests pass
- Run `cargo clippy -- -D warnings` to check for new warnings
- Verify dry-run mode still works: `cargo run -- run examples/mapreduce.yml --dry-run`

**Success Criteria**:
- [ ] Dry-run validation extracted to pure function
- [ ] Original function reduced by ~50 lines
- [ ] Complexity reduced by ~8 points
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Environment and Context Setup

**Goal**: Extract environment setup and workflow context initialization (lines 1338-1386) into a focused function, reducing complexity by ~3 points.

**Changes**:
- Create new function `prepare_mapreduce_environment()` in the `orchestration` module
- Extract immutable environment context creation
- Extract workflow context initialization with environment variables
- Function signature: `fn prepare_mapreduce_environment(env: &ExecutionEnvironment, global_env_config: Option<&EnvironmentConfig>) -> Result<(ExecutionEnvironment, WorkflowContext)>`
- Return both the worktree environment and initialized workflow context
- Replace original block with single function call

**Testing**:
- Run `cargo test executor::` to verify tests pass
- Run `cargo test context::` to verify context initialization
- Verify environment variables are properly populated

**Success Criteria**:
- [ ] Environment setup extracted to pure function
- [ ] Workflow context initialization isolated
- [ ] Original function reduced by ~40 lines
- [ ] Complexity reduced by ~3 points
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: Extract Setup Phase Execution

**Goal**: Extract setup phase execution logic (lines 1388-1446) into a focused async function, reducing complexity by ~5 points.

**Changes**:
- Create new async function `execute_mapreduce_setup()` in the `orchestration` module
- Extract setup phase configuration logic
- Extract setup executor creation and execution
- Extract file detection and variable capture
- Function signature: `async fn execute_mapreduce_setup<'a>(workflow: &ExtendedWorkflowConfig, executor: &mut WorkflowExecutor, env: &ExecutionEnvironment, context: &mut WorkflowContext, user_interaction: &impl UserInteraction) -> Result<Option<String>>`
- Return the generated input file path (if any)
- Replace original block with single function call

**Testing**:
- Run `cargo test setup::` to verify setup phase tests
- Run `cargo test executor::` to verify integration
- Test setup phase execution with sample workflow

**Success Criteria**:
- [ ] Setup phase execution extracted to async function
- [ ] Variable capture and file detection preserved
- [ ] Original function reduced by ~50 lines
- [ ] Complexity reduced by ~5 points
- [ ] All tests pass
- [ ] Ready to commit

### Phase 4: Extract Map Phase Configuration

**Goal**: Extract map phase configuration and input interpolation (lines 1448-1468) into a pure function, reducing complexity by ~2 points.

**Changes**:
- Create new function `configure_map_phase()` in the `orchestration` module
- Extract map phase input resolution
- Extract environment variable interpolation logic
- Function signature: `fn configure_map_phase(workflow: &ExtendedWorkflowConfig, generated_input: Option<String>, context: &WorkflowContext) -> Result<MapPhase>`
- Return configured MapPhase ready for execution
- Replace original block with single function call

**Testing**:
- Run `cargo test executor::` to verify tests pass
- Verify input path interpolation works correctly
- Test with and without generated input files

**Success Criteria**:
- [ ] Map phase configuration extracted to pure function
- [ ] Environment variable interpolation isolated
- [ ] Original function reduced by ~20 lines
- [ ] Complexity reduced by ~2 points
- [ ] All tests pass
- [ ] Ready to commit

### Phase 5: Final Cleanup and Verification

**Goal**: Clean up the main function, verify all complexity targets met, and ensure code quality standards.

**Changes**:
- Review main `execute_mapreduce()` function - should now be ~100 lines
- Add inline comments explaining the high-level workflow steps
- Ensure proper error context at each step
- Verify all extracted functions are properly documented
- Run full test suite and linters

**Testing**:
- Run `cargo test --lib` to verify all unit tests pass
- Run `cargo test --test '*'` to verify integration tests
- Run `cargo clippy -- -D warnings` to ensure no warnings
- Run `cargo fmt -- --check` to verify formatting
- Run `just ci` if available for full CI checks

**Success Criteria**:
- [ ] Main function is ≤100 lines
- [ ] Cyclomatic complexity ≤14 (target met)
- [ ] All extracted functions have complexity ≤5
- [ ] All tests pass (100% existing coverage maintained)
- [ ] No clippy warnings
- [ ] Code properly formatted
- [ ] Documentation complete
- [ ] Ready for final commit

## Testing Strategy

**For each phase**:
1. Run `cargo test executor::` to verify executor tests pass
2. Run `cargo clippy -- -D warnings` to check for new warnings
3. Run `cargo fmt` to ensure proper formatting
4. Manually test the specific functionality extracted

**Final verification**:
1. `cargo test --lib` - All unit tests must pass
2. `cargo test --test '*'` - All integration tests must pass
3. `cargo clippy -- -D warnings` - Zero warnings
4. `cargo fmt -- --check` - Properly formatted
5. `just ci` - Full CI checks (if available)
6. Manual smoke test: Run a MapReduce workflow end-to-end

**Complexity verification**:
- Run debtmap analysis after each phase to track complexity reduction
- Final debtmap should show complexity ≤14 for `execute_mapreduce()`

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the compilation errors or test failures
3. Identify the issue (usually missing parameters or incorrect error handling)
4. Adjust the extraction strategy
5. Retry the phase with corrections

If multiple phases fail:
1. Consider whether the extracted function boundaries are correct
2. May need to combine or split phases differently
3. Ensure async/await is properly handled in extracted functions
4. Verify all lifetimes and ownership patterns are correct

## Notes

**Key Considerations**:
- This function is the main orchestration point for MapReduce workflows
- Each extracted function should be self-contained and testable
- Preserve all existing behavior - this is a pure refactoring
- The function currently handles: dry-run, environment setup, setup phase, map config, execution
- Extract in order of independence: dry-run first (can return early), then sequential steps

**Async/Await Handling**:
- Some extracted functions will need to be async (setup execution, validation)
- Others can be pure synchronous functions (config, environment setup)
- Ensure proper `.await` handling after extraction

**Error Context**:
- Each extracted function should add appropriate error context
- Use `.context()` or `.with_context()` for clear error messages
- Preserve the error chain from original implementation

**Module Organization**:
- Place extracted functions in `src/cook/workflow/executor/orchestration.rs`
- This module already exists and handles high-level workflow orchestration
- Keep functions private to the module unless they need to be reused elsewhere

**Testing Approach**:
- Focus on preserving existing test coverage
- No new tests required (this is refactoring, not new functionality)
- If tests break, it means behavior changed - fix the extraction
- Consider adding unit tests for extracted pure functions in future work
