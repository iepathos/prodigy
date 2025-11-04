# Implementation Plan: Reduce Complexity in ResumeExecutor::execute_remaining_steps

## Problem Summary

**Location**: src/cook/workflow/resume.rs:ResumeExecutor::execute_remaining_steps:575
**Priority Score**: 28.21
**Debt Type**: ComplexityHotspot (Cyclomatic: 28, Cognitive: 142)
**Current Metrics**:
- Function Length: 281 lines
- Cyclomatic Complexity: 28
- Cognitive Complexity: 142
- Nesting Depth: 6 levels

**Issue**: High complexity (28 cyclomatic, 142 cognitive) makes function hard to test and maintain. The function mixes multiple concerns: step execution, progress tracking, error handling, retry logic, and recovery action processing.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 14.0 (target cyclomatic: ~10-14)
- Coverage Improvement: 0.0
- Risk Reduction: 9.8735

**Success Criteria**:
- [ ] Reduce cyclomatic complexity from 28 to ≤14
- [ ] Reduce cognitive complexity from 142 to ≤70
- [ ] Extract at least 4 focused helper functions
- [ ] Reduce nesting depth from 6 to ≤3
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

### Phase 1: Extract Cleanup Step Construction

**Goal**: Extract the inline cleanup step construction (lines 744-776) into a pure function

**Changes**:
- Create `build_cleanup_step(action: &HandlerCommand) -> WorkflowStep` helper function
- This function is pure - takes a HandlerCommand, returns a WorkflowStep
- Move the 30+ line struct construction into this helper
- Replace inline construction with function call

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- No new tests needed yet (pure extraction)

**Success Criteria**:
- [ ] New helper function created and used
- [ ] Reduces main function by ~30 lines
- [ ] Complexity reduction: -1 to -2 points
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

**Rationale**: This is the easiest extraction with zero risk. The cleanup step construction is completely independent logic that can be tested in isolation.

### Phase 2: Extract Error Handler Execution Logic

**Goal**: Extract error handler execution and processing (lines 668-700) into a focused function

**Changes**:
- Create `async fn execute_step_error_handler(...) -> Result<ErrorHandlerOutcome>`
- Enum `ErrorHandlerOutcome { Recovered, Failed, NoHandler }`
- Move the on_failure handler execution logic into this function
- Simplify the main function's error handling branch

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Consider adding a unit test for the new function if error handler coverage is low

**Success Criteria**:
- [ ] New async helper function created
- [ ] ErrorHandlerOutcome enum defined
- [ ] Main function error handling simplified
- [ ] Complexity reduction: -3 to -5 points
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

**Rationale**: Error handler execution is a cohesive responsibility that can be cleanly separated. This reduces nesting depth significantly.

### Phase 3: Extract Recovery Action Processing

**Goal**: Extract the large RecoveryAction match statement (lines 708-847) into a dedicated function

**Changes**:
- Create `async fn process_recovery_action(...) -> Result<RecoveryOutcome>`
- Enum `RecoveryOutcome { Retry(WorkflowStep), Continue, Abort, RequiresIntervention(String) }`
- Move the entire match statement and its branches into this function
- Main function becomes cleaner: just handle the outcome

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- This is the highest-risk extraction - test thoroughly

**Success Criteria**:
- [ ] New async function for recovery action processing
- [ ] RecoveryOutcome enum defined
- [ ] Main function simplified to outcome handling
- [ ] Complexity reduction: -6 to -8 points
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

**Rationale**: The RecoveryAction match is the largest complexity hotspot (6 branches, deep nesting). Extracting it provides the most significant complexity reduction.

### Phase 4: Extract Step Execution with Progress Tracking

**Goal**: Extract the step execution logic (lines 632-660) into a reusable function

**Changes**:
- Create `async fn execute_single_step(...) -> Result<StepExecutionResult>`
- Struct `StepExecutionResult { success: bool, duration: Duration }`
- Consolidate progress tracking and step execution logic
- Main loop becomes cleaner and more readable

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] New async helper function created
- [ ] StepExecutionResult struct defined
- [ ] Progress tracking logic encapsulated
- [ ] Complexity reduction: -2 to -3 points
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

**Rationale**: This consolidates the successful step execution path, making the main loop focus on flow control rather than implementation details.

### Phase 5: Final Cleanup and Verification

**Goal**: Review the refactored code, optimize flow, and verify all metrics

**Changes**:
- Review all extracted functions for potential further simplification
- Ensure consistent error handling patterns
- Add inline documentation to new helper functions
- Verify cyclomatic complexity target achieved

**Testing**:
- Run `just ci` - Full CI checks
- Run `cargo clippy -- -W clippy::cognitive_complexity` to check cognitive complexity
- Manual review of code readability

**Success Criteria**:
- [ ] Cyclomatic complexity ≤14 (target: ~10)
- [ ] Cognitive complexity ≤70 (target: ~60)
- [ ] All helper functions have doc comments
- [ ] Nesting depth ≤3
- [ ] All CI checks pass
- [ ] Code review by human (if available)
- [ ] Ready to commit

**Rationale**: Final verification ensures we've met all targets and the refactored code is production-ready.

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Manually verify the function still behaves correctly
4. Check git diff to ensure changes are minimal and focused

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo clippy -- -W clippy::cognitive_complexity -W clippy::too_many_arguments` - Check complexity metrics
3. Manual code review for readability and maintainability

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure - check test output and error messages
3. Adjust the extraction approach:
   - Smaller extraction if the change was too large
   - Different function signature if types don't align
   - Keep more context if dependencies are complex
4. Retry with adjusted approach

**Critical safeguard**: Each phase must pass all tests before proceeding. Never proceed with failing tests.

## Expected Complexity Reduction Timeline

- **After Phase 1**: Cyclomatic ~26-27, Cognitive ~130-135 (-1-2 cyclomatic, -7-12 cognitive)
- **After Phase 2**: Cyclomatic ~21-24, Cognitive ~110-120 (-3-5 cyclomatic, -15-25 cognitive)
- **After Phase 3**: Cyclomatic ~13-18, Cognitive ~70-90 (-6-8 cyclomatic, -30-50 cognitive)
- **After Phase 4**: Cyclomatic ~10-15, Cognitive ~55-75 (-2-3 cyclomatic, -10-20 cognitive)
- **After Phase 5**: Target achieved (≤14 cyclomatic, ≤70 cognitive)

## Notes

**Why This Order?**:
1. **Phase 1** (cleanup step) - Easiest, zero risk, pure function
2. **Phase 2** (error handler) - Medium risk, reduces nesting
3. **Phase 3** (recovery action) - Highest complexity reduction, most impactful
4. **Phase 4** (step execution) - Consolidates success path
5. **Phase 5** - Verification and polish

**Functional Programming Principles Applied**:
- Extracting pure functions (Phase 1: build_cleanup_step)
- Separating I/O from logic (Phase 2-4: async functions with clear boundaries)
- Using enum-based outcomes instead of boolean flags
- Single responsibility per helper function

**Risk Mitigation**:
- Each phase is independently committable
- Tests run after every phase
- Small, focused changes reduce merge conflict risk
- Rollback plan for each phase
