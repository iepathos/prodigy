# Bug Report: Failed Workflows Cannot Be Resumed

## Summary

When a workflow fails, Prodigy creates a checkpoint and sets the session status to `Failed`. However, when attempting to resume the failed workflow, the resume operation is rejected with the error:

```
Session workflow-xxx is not resumable (status: Failed)
```

This prevents users from fixing issues and resuming failed workflows, forcing them to start over from scratch.

## Root Cause

The bug is in `src/cook/session/state.rs:248` in the `is_resumable()` function:

```rust
pub fn is_resumable(&self) -> bool {
    matches!(
        self.status,
        SessionStatus::InProgress | SessionStatus::Interrupted
    ) && self.workflow_state.is_some()
}
```

This function only allows sessions with `InProgress` or `Interrupted` status to be resumed. Sessions with `Failed` status are rejected, even if they have checkpoint data.

## Expected Behavior

Failed workflows **with checkpoint data** should be resumable. The logic should be:

- ✅ `InProgress` + checkpoint data = resumable
- ✅ `Interrupted` + checkpoint data = resumable
- ✅ `Failed` + checkpoint data = resumable (CURRENTLY BROKEN)
- ❌ `Completed` = not resumable (workflow finished)
- ❌ Any status without checkpoint data = not resumable

## Test Coverage

Comprehensive tests have been added in `tests/workflow_failed_resume_test.rs`:

### Tests that PASS (confirming the bug):
- ✅ `test_failed_session_is_not_resumable_bug` - Confirms Failed sessions are not resumable
- ✅ `test_interrupted_session_is_resumable` - Confirms Interrupted sessions work correctly
- ✅ `test_inprogress_session_is_resumable` - Confirms InProgress sessions work correctly
- ✅ `test_checkpoint_with_failed_status` - Confirms checkpoints are created for failures

### Tests that FAIL (will pass once bug is fixed):
- ❌ `test_failed_session_should_be_resumable` - Documents expected behavior
- ❌ `test_is_resumable_logic_after_fix` - Tests the correct logic for all scenarios

## Reproduction Steps

1. Run a workflow that fails partway through (e.g., spec 151 implementation that fails linting)
2. Observe that Prodigy saves a checkpoint and sets status to `Failed`
3. Try to resume: `prodigy resume workflow-xxx`
4. Observe error: "Session workflow-xxx is not resumable (status: Failed)"

## User Impact

This bug affects **every workflow that fails**, which is common during:
- Iterative development with test/lint failures
- Long-running workflows that encounter errors
- Goal-seeking workflows with validation failures

Users lose all progress when a workflow fails and must restart from the beginning.

## Proposed Fix

Change `is_resumable()` in `src/cook/session/state.rs` to:

```rust
pub fn is_resumable(&self) -> bool {
    // Failed sessions with checkpoint data should be resumable
    // Completed sessions should never be resumable
    matches!(
        self.status,
        SessionStatus::InProgress | SessionStatus::Interrupted | SessionStatus::Failed
    ) && self.workflow_state.is_some()
}
```

Or more explicitly:

```rust
pub fn is_resumable(&self) -> bool {
    // Only Completed sessions are not resumable
    // All other statuses with checkpoint data can be resumed
    if matches!(self.status, SessionStatus::Completed) {
        return false;
    }

    // Must have checkpoint data to resume
    self.workflow_state.is_some()
}
```

## Additional Considerations

### Session State Transition on Resume

When resuming a Failed session, the orchestrator should:
1. Validate the session has checkpoint data (already done)
2. Load the checkpoint (already done)
3. **Transition session status from Failed to InProgress** (may need to be added)
4. Clear any previous error messages
5. Continue execution from the failed step

### Unified Session Manager

The same fix should be applied to the unified session manager if it has similar logic.

## Testing

Run the test suite to verify:

```bash
# These should PASS (bug confirmation):
cargo test --test workflow_failed_resume_test test_failed_session_is_not_resumable_bug
cargo test --test workflow_failed_resume_test test_interrupted_session_is_resumable
cargo test --test workflow_failed_resume_test test_inprogress_session_is_resumable
cargo test --test workflow_failed_resume_test test_checkpoint_with_failed_status

# These should FAIL now, PASS after fix:
cargo test --test workflow_failed_resume_test test_failed_session_should_be_resumable --include-ignored
cargo test --test workflow_failed_resume_test test_is_resumable_logic_after_fix --include-ignored
```

After implementing the fix, all tests should pass.

## Related Files

- `src/cook/session/state.rs:248` - The `is_resumable()` function (needs fix)
- `src/cook/orchestrator/execution_pipeline.rs:282` - Calls `validate_session_resumable()`
- `tests/workflow_failed_resume_test.rs` - Comprehensive test coverage

## Priority

**HIGH** - This bug blocks users from resuming any failed workflow, causing significant productivity loss.
