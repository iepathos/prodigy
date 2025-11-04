# Implementation Plan: Reduce Complexity in ResumeExecutor::execute_from_checkpoint

## Problem Summary

**Location**: ./src/cook/workflow/resume.rs:ResumeExecutor::execute_from_checkpoint:337
**Priority Score**: 32.5
**Debt Type**: ComplexityHotspot (Cyclomatic: 54, Cognitive: 227)
**Current Metrics**:
- Lines of Code: 546
- Cyclomatic Complexity: 54
- Cognitive Complexity: 227
- Nesting Depth: 6

**Issue**: Reduce complexity from 54 to ~10. High complexity 54/227 makes function hard to test and maintain. This massive function handles checkpoint loading, workflow parsing, step execution, error recovery, and progress tracking all in one place.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 27.0
- Coverage Improvement: 0.0
- Risk Reduction: 11.375

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 54 to ≤10
- [ ] Function length reduced from 546 lines to ≤50 lines
- [ ] Nesting depth reduced from 6 to ≤3
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting
- [ ] Each extracted function has single, clear responsibility

## Implementation Phases

### Phase 1: Extract Workflow Loading and Parsing Logic

**Goal**: Extract workflow file loading and parsing into dedicated functions to reduce initial complexity.

**Changes**:
- Extract workflow file loading (lines 421-435) into `load_workflow_file(workflow_path: &PathBuf) -> Result<WorkflowConfig>`
- Extract WorkflowCommand to WorkflowStep conversion (lines 438-525) into `convert_commands_to_steps(commands: Vec<WorkflowCommand>) -> Vec<WorkflowStep>`
- Extract ExtendedWorkflowConfig creation (lines 527-542) into `build_extended_workflow(checkpoint: &WorkflowCheckpoint, steps: Vec<WorkflowStep>) -> ExtendedWorkflowConfig`

**Expected Complexity Reduction**: ~8 points (removes nested conditionals in command parsing)

**Testing**:
- Run `cargo test resume::` to verify resume functionality
- Run `cargo test workflow::executor::` to verify workflow execution
- Verify workflow loading with both YAML and JSON files

**Success Criteria**:
- [ ] Three new pure functions created with clear responsibilities
- [ ] Main function delegates workflow loading/parsing to extracted functions
- [ ] All existing resume tests pass
- [ ] Complexity reduced by ~8 points
- [ ] Ready to commit

### Phase 2: Extract Progress Tracking Setup

**Goal**: Isolate progress tracking initialization and display logic.

**Changes**:
- Extract progress tracker creation (lines 394-404) into `create_progress_tracker(checkpoint: &WorkflowCheckpoint, workflow_id: &str) -> SequentialProgressTracker`
- Extract initial progress display setup (lines 406-419) into `initialize_progress_display(tracker: &mut SequentialProgressTracker, display: &mut ProgressDisplay, workflow_id: &str) -> Result<()>`
- Combine environment creation (lines 544-560) into `build_execution_environment(workflow_path: &PathBuf, workflow_id: &str) -> ExecutionEnvironment`

**Expected Complexity Reduction**: ~4 points (removes initialization branches)

**Testing**:
- Run `cargo test resume::`
- Verify progress tracking displays correctly during resume
- Test with checkpoints at different stages

**Success Criteria**:
- [ ] Progress tracking logic extracted to focused functions
- [ ] Main function uses extracted functions for setup
- [ ] All existing tests pass
- [ ] Complexity reduced by additional ~4 points
- [ ] Ready to commit

### Phase 3: Extract Step Execution Loop into Focused Function

**Goal**: Extract the massive step execution loop (lines 608-843) which handles execution, error recovery, and retries.

**Changes**:
- Extract step execution loop into `execute_remaining_steps(executor: &mut WorkflowExecutorImpl, workflow: &ExtendedWorkflowConfig, start_from: usize, env: &ExecutionEnvironment, workflow_context: &mut WorkflowContext, progress_tracker: &mut SequentialProgressTracker, progress_display: &mut ProgressDisplay, error_recovery: &ResumeErrorRecovery, checkpoint: &WorkflowCheckpoint) -> Result<usize>`
- This function returns the number of steps executed
- Maintains all error handling and recovery logic
- Delegates to error recovery handler (already extracted in codebase)

**Expected Complexity Reduction**: ~10 points (most complex nested logic)

**Testing**:
- Run `cargo test resume::` to verify step execution
- Test error recovery scenarios
- Test with workflows that have on_failure handlers
- Verify retry logic works correctly

**Success Criteria**:
- [ ] Step execution loop fully extracted
- [ ] Error recovery logic preserved and functional
- [ ] All existing tests pass
- [ ] Complexity reduced by additional ~10 points
- [ ] Ready to commit

### Phase 4: Extract Error Recovery Action Handling

**Goal**: Extract the complex match statement for recovery actions (lines 698-840) into a dedicated function.

**Changes**:
- Extract recovery action handling into `handle_recovery_action(recovery_action: RecoveryAction, step_index: usize, step: &WorkflowStep, executor: &mut WorkflowExecutorImpl, env: &ExecutionEnvironment, workflow_context: &mut WorkflowContext, progress_tracker: &mut SequentialProgressTracker, checkpoint_manager: &CheckpointManager, workflow_id: &str) -> Result<RecoveryOutcome>`
- Define `RecoveryOutcome` enum: `Retry`, `Continue`, `Abort`
- Simplifies the main execution loop error handling

**Expected Complexity Reduction**: ~6 points (nested match with multiple branches)

**Testing**:
- Run `cargo test resume::`
- Test each recovery action type: Retry, Continue, SafeAbort, Fallback, PartialResume, RequestIntervention
- Verify cleanup actions execute on SafeAbort
- Verify checkpoint saved on RequestIntervention

**Success Criteria**:
- [ ] Recovery action handling extracted to dedicated function
- [ ] RecoveryOutcome enum clearly defines outcomes
- [ ] All recovery scenarios tested
- [ ] Complexity reduced by additional ~6 points
- [ ] Ready to commit

### Phase 5: Final Refactoring and Complexity Verification

**Goal**: Achieve target complexity ≤10 through final refinements and verification.

**Changes**:
- Extract checkpoint validation and early return logic (lines 367-386) into `check_already_completed(checkpoint: &WorkflowCheckpoint, options: &ResumeOptions, workflow_id: &str) -> Result<Option<ResumeResult>>`
- Extract final summary display (lines 852-862) into `display_completion_summary(total_steps: usize, skipped_steps: usize, steps_executed: usize, start_time: std::time::Instant)`
- Refactor main function to be a clear orchestration of extracted functions
- Run debtmap analysis to verify complexity reduction

**Expected Complexity Reduction**: ~3 points (removes remaining conditionals)

**Testing**:
- Run full test suite: `cargo test --lib`
- Run clippy: `cargo clippy`
- Verify formatting: `cargo fmt --check`
- Run `debtmap analyze` to verify complexity metrics

**Success Criteria**:
- [ ] Main function is <50 lines of orchestration code
- [ ] Cyclomatic complexity ≤10
- [ ] Cognitive complexity significantly reduced
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Debtmap shows improvement
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test resume::` to verify resume-specific tests pass
2. Run `cargo test workflow::executor::` to verify workflow execution
3. Run `cargo clippy` to check for warnings
4. Manually test with sample workflows to verify behavior

**Final verification**:
1. `cargo test --lib` - All library tests pass
2. `cargo clippy` - No warnings
3. `cargo fmt --check` - Proper formatting
4. `debtmap analyze` - Verify complexity reduced to target

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure and test output
3. Adjust the extraction strategy
4. Ensure extracted functions have proper error handling
5. Retry with refined approach

## Notes

### Key Architectural Patterns

**Separation Strategy**:
- **Pure Logic**: Extract workflow parsing, config building (can be unit tested easily)
- **I/O Operations**: Keep async operations in extracted functions but with clear boundaries
- **Error Handling**: Preserve all error recovery logic, just organize it better

**Function Design Principles**:
- Each extracted function should have ≤3 parameters (use structs for more)
- Each function should do ONE thing well
- Prefer returning Results to panicking
- Keep nesting depth ≤2 in extracted functions

**Gotchas**:
- The error recovery logic is complex but well-designed - preserve its semantics
- Progress tracking is stateful - pass by mutable reference where needed
- WorkflowContext is mutable state that flows through execution
- Don't break the async execution model

### Expected Final Structure

After all phases, `execute_from_checkpoint` should look approximately like:

```rust
pub async fn execute_from_checkpoint(
    &mut self,
    workflow_id: &str,
    workflow_path: &PathBuf,
    options: ResumeOptions,
) -> Result<ResumeResult> {
    // Validate executors configured
    self.ensure_executors_configured()?;

    // Load and validate checkpoint
    let checkpoint = self.load_and_validate_checkpoint(workflow_id, &options).await?;

    // Check if already completed
    if let Some(result) = check_already_completed(&checkpoint, &options, workflow_id)? {
        return Ok(result);
    }

    // Setup execution environment
    let workflow = load_workflow_file(workflow_path).await?;
    let steps = convert_commands_to_steps(workflow.commands);
    let extended_workflow = build_extended_workflow(&checkpoint, steps);
    let env = build_execution_environment(workflow_path, workflow_id);

    // Setup progress tracking
    let mut progress_tracker = create_progress_tracker(&checkpoint, workflow_id);
    let mut progress_display = ProgressDisplay::new();
    initialize_progress_display(&mut progress_tracker, &mut progress_display, workflow_id).await;

    // Restore context and create executor
    let mut workflow_context = self.restore_workflow_context(&checkpoint)?;
    let mut executor = self.create_workflow_executor(workflow_path, workflow_id);

    // Execute remaining steps
    let steps_executed = execute_remaining_steps(
        &mut executor,
        &extended_workflow,
        checkpoint.execution_state.current_step_index,
        &env,
        &mut workflow_context,
        &mut progress_tracker,
        &mut progress_display,
        &self.error_recovery,
        &checkpoint,
        &self.checkpoint_manager,
        workflow_id,
    ).await?;

    // Complete and cleanup
    display_completion_summary(
        extended_workflow.steps.len(),
        checkpoint.execution_state.current_step_index,
        steps_executed,
        progress_tracker.start_time,
    );

    self.checkpoint_manager.delete_checkpoint(workflow_id).await?;

    Ok(ResumeResult {
        success: true,
        total_steps_executed: extended_workflow.steps.len(),
        skipped_steps: checkpoint.execution_state.current_step_index,
        new_steps_executed: steps_executed,
        final_context: workflow_context,
    })
}
```

This transformation reduces the function from 546 lines to ~50 lines of clear orchestration, with each responsibility delegated to a focused, testable function.
