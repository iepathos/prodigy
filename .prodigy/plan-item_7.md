# Implementation Plan: Reduce Complexity in ExecutionPipeline::finalize_session

## Problem Summary

**Location**: ./src/cook/orchestrator/execution_pipeline.rs:ExecutionPipeline::finalize_session:165
**Priority Score**: 24.18
**Debt Type**: ComplexityHotspot (Cyclomatic: 19, Cognitive: 73)
**Current Metrics**:
- Lines of Code: 114
- Cyclomatic Complexity: 19
- Cognitive Complexity: 73
- Function Length: 114 lines
- Nesting Depth: 3

**Issue**: High complexity 19/73 makes function hard to test and maintain. The function handles multiple responsibilities including execution result processing, session status updates, interruption handling, worktree state management, cleanup orchestration, and user messaging.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 9.5 (from 19 to ~10)
- Coverage Improvement: 0.0
- Risk Reduction: 6.34

**Success Criteria**:
- [ ] Cyclomatic complexity reduced to 10 or below
- [ ] Each extracted function has single responsibility
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with cargo fmt

## Implementation Phases

### Phase 1: Extract Pure Result Classification Logic

**Goal**: Extract pure decision logic that classifies execution results and determines appropriate actions.

**Changes**:
- Create pure function `classify_execution_result(result: &Result<()>, session_status: SessionStatus) -> ExecutionOutcome` enum
- Extract pure function `should_save_checkpoint(outcome: &ExecutionOutcome) -> bool`
- Extract pure function `determine_resume_message(session_id: &str, playbook_path: &str, outcome: &ExecutionOutcome) -> Option<String>`

**Testing**:
```bash
# Add unit tests for new pure functions
cargo test execution_pipeline::classify_execution_result
cargo test execution_pipeline::should_save_checkpoint
cargo test execution_pipeline::determine_resume_message
cargo test --lib
```

**Success Criteria**:
- [ ] New pure functions have 100% test coverage
- [ ] Functions are independently testable without mocks
- [ ] All tests pass
- [ ] Complexity reduced by ~3 points
- [ ] Ready to commit

### Phase 2: Extract Worktree State Management

**Goal**: Separate worktree state management logic into focused helper methods.

**Changes**:
- Create `create_worktree_manager(&self, config: &CookConfig) -> Result<WorktreeManager>` to encapsulate WorktreeManager creation logic
- Extract `update_worktree_interrupted_state(worktree_manager: &WorktreeManager, worktree_name: &str) -> Result<()>`
- This consolidates duplicated logic from `finalize_session` and `setup_signal_handlers`

**Testing**:
```bash
# Test worktree state management
cargo test execution_pipeline::create_worktree_manager
cargo test execution_pipeline::update_worktree_interrupted_state
cargo test --lib
cargo clippy
```

**Success Criteria**:
- [ ] Worktree management logic consolidated
- [ ] Duplication with signal handler setup eliminated
- [ ] Tests verify worktree state transitions
- [ ] Complexity reduced by ~2 points
- [ ] Ready to commit

### Phase 3: Extract Session Finalization Actions

**Goal**: Create focused functions for each type of session finalization (success, interruption, failure).

**Changes**:
- Extract `async fn handle_session_success(&self) -> Result<()>` - updates status, displays success message
- Extract `async fn handle_session_interruption(&self, session_id: &str, config: &CookConfig, env: &ExecutionEnvironment) -> Result<()>` - saves checkpoint, updates worktree, displays resume message
- Extract `async fn handle_session_failure(&self, error: anyhow::Error, session_id: &str) -> Result<()>` - updates status with error, displays failure message and resume info

**Testing**:
```bash
# Test each finalization path
cargo test execution_pipeline::handle_session_success
cargo test execution_pipeline::handle_session_interruption
cargo test execution_pipeline::handle_session_failure
cargo test --lib
```

**Success Criteria**:
- [ ] Each handler function has single responsibility
- [ ] Clear separation of concerns (success/interrupt/failure)
- [ ] Easier to test each path independently
- [ ] Complexity reduced by ~3 points
- [ ] Ready to commit

### Phase 4: Extract Post-Execution Operations

**Goal**: Separate cleanup, session completion, and health display into focused orchestration.

**Changes**:
- Extract `async fn execute_cleanup_and_completion(&self, cleanup_fn: impl Future<Output = Result<()>>, config: &CookConfig) -> Result<SessionSummary>`
- Extract `async fn display_completion_summary(&self, summary: &SessionSummary, config: &CookConfig, display_health_fn: impl Future<Output = Result<()>>) -> Result<()>`
- This groups the cleanup, complete_session, and conditional display logic

**Testing**:
```bash
# Test cleanup orchestration
cargo test execution_pipeline::execute_cleanup_and_completion
cargo test execution_pipeline::display_completion_summary
cargo test --lib
cargo clippy
```

**Success Criteria**:
- [ ] Post-execution flow is clear and linear
- [ ] Easy to add new post-execution steps
- [ ] Tests verify cleanup ordering
- [ ] Complexity reduced by ~1.5 points
- [ ] Ready to commit

### Phase 5: Refactor Main Function to Use Extracted Helpers

**Goal**: Simplify `finalize_session` to orchestrate extracted functions with minimal conditional logic.

**Changes**:
- Rewrite `finalize_session` to use extracted functions
- Should follow clear linear flow:
  1. Classify execution result
  2. Route to appropriate handler (success/interrupt/failure)
  3. Execute cleanup and completion (only on success)
  4. Display summary
- Target cyclomatic complexity: 8-10

**Testing**:
```bash
# Full integration test
cargo test execution_pipeline::finalize_session
cargo test --all
cargo clippy
cargo fmt --check
```

**Success Criteria**:
- [ ] Cyclomatic complexity ≤ 10
- [ ] Cognitive complexity significantly reduced
- [ ] Function reads like a clear workflow
- [ ] All integration tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Write tests for new pure functions FIRST (TDD approach)
2. Run `cargo test --lib` to verify existing tests pass
3. Run `cargo clippy` to check for warnings
4. Run `cargo fmt` to ensure formatting

**Phase-specific testing**:
- **Phase 1**: Focus on testing pure logic with various input combinations
- **Phase 2**: Test worktree state transitions and error handling
- **Phase 3**: Test each finalization path with mocked dependencies
- **Phase 4**: Test cleanup orchestration and ordering
- **Phase 5**: Integration tests verifying end-to-end flow

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo test --all` - All tests pass
3. `cargo clippy -- -D warnings` - No warnings
4. `debtmap analyze` - Verify complexity reduction

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure:
   - Test failures: Check if assumptions about behavior were incorrect
   - Clippy warnings: Review linting suggestions
   - Build failures: Check for missing imports or type mismatches
3. Adjust the plan:
   - Break phase into smaller steps if needed
   - Reconsider extraction boundaries
   - Add intermediate commits
4. Retry with adjustments

## Notes

**Key Insights**:
- The function mixes I/O (user interaction, session updates) with decision logic - these should be separated
- Worktree state management logic is duplicated with `setup_signal_handlers` - extracting it reduces duplication
- Each execution outcome (success, interrupt, failure) has distinct handling logic - separate functions make this explicit
- The cleanup and completion logic is independent and can be extracted

**Potential Gotchas**:
- Ensure error propagation works correctly after extraction (use `?` operator consistently)
- Preserve exact error messages for user-facing output
- Maintain async/await flow through extracted functions
- Keep session state updates in correct order

**Dependencies**:
- No changes to public API
- No changes to existing tests required
- All changes are internal refactoring
- Maintains backward compatibility
