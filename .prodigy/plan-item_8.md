# Implementation Plan: Extract Pure Functions from run_with_context

## Problem Summary

**Location**: ./src/cook/execution/runner.rs:RealCommandRunner::run_with_context:137
**Priority Score**: 27.925
**Debt Type**: ComplexityHotspot (cognitive: 19, cyclomatic: 9)
**Current Metrics**:
- Lines of Code: 69
- Cyclomatic Complexity: 9
- Cognitive Complexity: 19
- Coverage: Not specified
- Upstream Callers: 5

**Issue**: The function has manageable cyclomatic complexity (9) but high cognitive complexity (19). The function mixes command building logic, configuration logic, execution path selection, and result transformation. This makes it harder to test individual pieces and understand the control flow.

The function handles:
1. Building a ProcessCommand from ExecutionContext
2. Applying environment variables, timeout, and stdin
3. Deciding between streaming and batch execution modes
4. Creating processors for streaming mode
5. Executing the command via different runners
6. Transforming output into ExecutionResult

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 4.5
- Coverage Improvement: 0.0
- Risk Reduction: 9.77

**Success Criteria**:
- [ ] Cognitive complexity reduced to ≤14 (target: 19 - 4.5 ≈ 14-15)
- [ ] Pure functions extracted for command building and result transformation
- [ ] Clear separation between decision logic and execution logic
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Command Building Logic

**Goal**: Extract the ProcessCommand building logic into a pure function that can be independently tested.

**Changes**:
- Create a new pure function `build_command_from_context` that takes `cmd`, `args`, and `ExecutionContext` and returns a `ProcessCommand`
- This function should handle:
  - Base command and args
  - Working directory
  - Environment variables
  - Timeout
  - Stdin
- Move lines 143-162 into this new function
- Update `run_with_context` to call this new function

**Testing**:
- Add unit tests for `build_command_from_context` covering:
  - Basic command with args
  - Command with environment variables
  - Command with timeout
  - Command with stdin
  - Command with all options combined
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] `build_command_from_context` is a pure, testable function
- [ ] All new tests pass
- [ ] Existing tests continue to pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Result Transformation Logic

**Goal**: Extract the ExecutionResult transformation logic into pure functions.

**Changes**:
- Create two pure functions:
  - `streaming_output_to_result(output: StreamingOutput) -> ExecutionResult` for lines 180-186
  - `batch_output_to_result(output: ProcessOutput) -> ExecutionResult` for lines 198-204
- These functions should be pure transformations with no side effects
- Update both execution paths to use these new functions

**Testing**:
- Add unit tests for both transformation functions:
  - Test with successful output
  - Test with failed output (non-zero exit code)
  - Test with stdout/stderr content
- Run `cargo test --lib` to verify all tests pass
- Run `cargo clippy`

**Success Criteria**:
- [ ] Result transformation logic is extracted into pure functions
- [ ] All new tests pass
- [ ] Existing tests continue to pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Extract Execution Path Selection Logic

**Goal**: Clarify the streaming vs batch mode decision logic.

**Changes**:
- Create a pure function `should_use_streaming(context: &ExecutionContext) -> bool` that encapsulates the logic from lines 165-167
- This makes the decision logic explicit and testable
- Update `run_with_context` to use this function
- Consider extracting the streaming execution path into a separate method `execute_streaming` if it simplifies the main function

**Testing**:
- Add unit tests for `should_use_streaming`:
  - No streaming config -> false
  - Streaming config with enabled=false -> false
  - Streaming config with enabled=true -> true
- Run `cargo test --lib`
- Run `cargo clippy`

**Success Criteria**:
- [ ] Decision logic is extracted and clear
- [ ] All new tests pass
- [ ] Existing tests continue to pass
- [ ] Cognitive complexity measurably reduced
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Add Test Coverage for Edge Cases

**Goal**: Improve test coverage for the refactored function, especially error paths and edge cases.

**Changes**:
- Add integration tests for `run_with_context` that cover:
  - Streaming mode with valid config
  - Batch mode fallback
  - Environment variable propagation
  - Timeout handling
  - Stdin handling
  - Error cases (command not found, etc.)
- Ensure the refactored code is thoroughly exercised

**Testing**:
- Run `cargo test` to verify all tests pass
- Run `cargo tarpaulin` to check coverage improvement
- Target: >80% coverage on the refactored functions

**Success Criteria**:
- [ ] Comprehensive test coverage added
- [ ] All tests pass
- [ ] Coverage improved for the module
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Validation and Documentation

**Goal**: Verify the complexity reduction and ensure the code is well-documented.

**Changes**:
- Run `debtmap analyze` to verify complexity reduction
- Add doc comments to the new pure functions explaining their purpose
- Update any relevant module-level documentation
- Ensure code follows project conventions

**Testing**:
- Run `just ci` for full CI checks
- Verify debtmap shows improvement in complexity scores
- Check that cognitive complexity is reduced

**Success Criteria**:
- [ ] Debtmap shows complexity reduction of ~4.5 points
- [ ] All documentation is clear and accurate
- [ ] Full CI passes
- [ ] Code follows project conventions
- [ ] Ready for final commit and merge

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure consistent formatting
4. Commit with a clear message describing the phase

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage report
3. `debtmap analyze` - Verify complexity improvement (target: -4.5 complexity points)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure in detail
3. Adjust the implementation approach
4. Retry with smaller increments if needed

If tests fail after refactoring:
1. Check if the behavior changed unintentionally
2. Fix the implementation to preserve existing behavior
3. Add tests to prevent regression

## Notes

**Key Insights**:
- The function is already reasonably structured, but extracting pure functions will make it more testable
- The complexity comes from conditional logic (streaming vs batch) and sequential configuration steps
- Extracting pure functions allows unit testing without async/subprocess complexity
- The main function will become a thin orchestrator after refactoring

**Gotchas**:
- Preserve exact behavior during refactoring - the function is used by 5 upstream callers
- Ensure error context is preserved when extracting functions
- The streaming path creates processors, which should remain in the main function (side effects)
- Don't over-abstract - keep the code readable and maintainable

**Dependencies**:
- No external dependencies needed
- All refactoring can be done within the existing module structure
- Tests can use existing test utilities (MockCommandRunner, etc.)
