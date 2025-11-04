# Implementation Plan: Reduce Complexity in handle_commit_verification

## Problem Summary

**Location**: ./src/cook/workflow/executor.rs:WorkflowExecutor::handle_commit_verification:446
**Priority Score**: 22.5225
**Debt Type**: ComplexityHotspot (Cognitive: 81, Cyclomatic: 21)
**Current Metrics**:
- Function Length: 70 lines
- Cyclomatic Complexity: 21
- Cognitive Complexity: 81
- Nesting Depth: 6

**Issue**: Reduce complexity from 21 to ~10. High complexity 21/81 makes function hard to test and maintain.

The function has deeply nested conditionals (6 levels) handling multiple scenarios:
1. No commits created + auto_commit enabled + has changes
2. No commits created + auto_commit enabled + no changes + commit_required
3. No commits created + auto_commit disabled + commit_required
4. Commits created → verify and track metadata

This creates 21 different execution paths, making the code difficult to understand, test, and modify.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 10.5 (from 21 to ~10)
- Coverage Improvement: 0.0
- Risk Reduction: 7.88

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 21 to ≤10
- [ ] Nesting depth reduced from 6 to ≤3
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting with `cargo fmt`

## Implementation Phases

### Phase 1: Extract Pure Decision Logic

**Goal**: Extract the decision tree for "no commits created" scenario into a pure function that returns an action enum.

**Changes**:
- Create an enum `CommitVerificationAction` representing the possible actions:
  - `CreateAutoCommit(String)` - with commit message
  - `RequireCommitError` - should fail with error
  - `NoAction` - nothing to do
- Extract function `determine_no_commit_action()` that takes:
  - `step: &WorkflowStep`
  - `has_changes: Result<bool>`
  - Returns `CommitVerificationAction`
- This function contains the pure decision logic (no I/O, no side effects)

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] New pure function with cyclomatic complexity ≤5
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 2: Extract Auto-Commit Execution Logic

**Goal**: Move the auto-commit execution logic into a separate helper function.

**Changes**:
- Create function `execute_auto_commit()` that takes:
  - `commit_handler: &CommitHandler`
  - `working_dir: &Path`
  - `message: &str`
  - `step_display: &str`
  - `commit_required: bool`
  - Returns `Result<bool>` (true if commit was created)
- This function handles the auto-commit attempt and error handling
- Call this function from `handle_commit_verification` when action is `CreateAutoCommit`

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] New helper function with cyclomatic complexity ≤3
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 3: Simplify Main Function Control Flow

**Goal**: Refactor `handle_commit_verification` to use the extracted functions, reducing nesting.

**Changes**:
- Replace the deeply nested `if head_after == head_before` block with:
  1. Call `determine_no_commit_action()`
  2. Match on the action enum
  3. Execute the appropriate action
- Early return pattern to avoid else branches
- Use `?` operator consistently for error propagation

**Expected structure**:
```rust
if head_after == head_before {
    match determine_no_commit_action(step, has_uncommitted_changes) {
        CommitVerificationAction::CreateAutoCommit(message) => {
            execute_auto_commit(...)?
        }
        CommitVerificationAction::RequireCommitError => {
            self.handle_no_commits_error(step)?
        }
        CommitVerificationAction::NoAction => {}
    }
    return Ok(false);
}

// Commits were created - verify and track
let (_, commits) = commit_handler
    .verify_and_handle_commits(...)
    .await?;

workflow_context.variables.insert(...);
Ok(true)
```

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings
- Verify nesting depth is reduced

**Success Criteria**:
- [ ] Main function has cyclomatic complexity ≤10
- [ ] Nesting depth ≤3
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 4: Add Unit Tests for Pure Logic

**Goal**: Add unit tests for the new pure function `determine_no_commit_action()`.

**Changes**:
- Add test module for the new pure function
- Test all decision paths:
  - auto_commit=true, has_changes=Ok(true) → CreateAutoCommit
  - auto_commit=true, has_changes=Ok(false), commit_required=true → RequireCommitError
  - auto_commit=true, has_changes=Ok(false), commit_required=false → NoAction
  - auto_commit=true, has_changes=Err(_), commit_required=true → RequireCommitError
  - auto_commit=true, has_changes=Err(_), commit_required=false → NoAction
  - auto_commit=false, commit_required=true → RequireCommitError
  - auto_commit=false, commit_required=false → NoAction

**Testing**:
- Run `cargo test --lib` to verify all tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [ ] Test coverage for all decision paths
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Ready to commit

### Phase 5: Final Verification and Documentation

**Goal**: Verify the refactoring achieved the target complexity reduction and add documentation.

**Changes**:
- Add doc comments to the new functions explaining their purpose
- Add doc comment to `handle_commit_verification` explaining the refactored flow
- Run final verification checks

**Testing**:
- Run `just ci` - Full CI checks
- Run `cargo clippy` - Verify no warnings
- Run `cargo fmt --check` - Verify formatting
- Manually verify cyclomatic complexity using `cargo-geiger` or similar tool

**Success Criteria**:
- [ ] Cyclomatic complexity reduced from 21 to ≤10
- [ ] All CI checks pass
- [ ] Documentation added
- [ ] Ready to commit

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure proper formatting

**Final verification**:
1. `just ci` - Full CI checks
2. Manual review of cyclomatic complexity reduction
3. Verify nesting depth is ≤3 (can inspect code manually)

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure logs and error messages
3. Adjust the implementation approach
4. Retry the phase with the updated approach

## Notes

- **Focus on decision logic extraction**: The main complexity comes from nested conditionals. Extracting the decision logic into a pure function with an enum return type will significantly reduce cognitive load.
- **Early returns**: Use early return patterns to avoid else branches and reduce nesting.
- **Pure functions first**: Extract the pure decision logic before extracting I/O operations. This makes testing easier.
- **Preserve behavior**: The refactoring must preserve exact behavior. All existing tests must pass after each phase.
- **Enum over booleans**: Using an enum for the action type is more expressive than multiple boolean returns and easier to extend in the future.
