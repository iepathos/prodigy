---
number: 123
title: Checkpoint-on-Error Recovery System
category: foundation
priority: critical
status: draft
dependencies: [122]
created: 2025-10-06
---

# Specification 123: Checkpoint-on-Error Recovery System

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: Spec 122 (Checkpoint Path Resolution System)

## Context

Currently, when a Prodigy workflow fails (e.g., due to `commit_required=true` with no commits), the orchestrator exits immediately **without saving a checkpoint**. This makes resume completely impossible because there is no checkpoint to resume from.

**Example failure scenario**:
```
❌ Session failed: Step 'claude: /prodigy-implement-spec $ARG' has commit_required=true
   but no commits were created
💡 To resume from last checkpoint, run: prodigy resume session-5f2f491e-136a-4e58-873a-57a2695fc60b
```

When the user attempts to resume:
```
Error: No checkpoint files found in /Users/glen/.prodigy/state/session-5f2f491e-136a-4e58-873a-57a2695fc60b/checkpoints
```

The checkpoint directory exists but is **empty** because the error occurred before any checkpoint save logic executed. This violates the principle of graceful degradation: the system should save recovery state before terminating.

## Objective

Implement checkpoint-on-error logic that ensures workflow state is persisted **before** the orchestrator exits, regardless of whether the workflow succeeded or failed. This enables users to inspect workflow state, debug failures, and potentially resume from the point of failure.

Follow functional programming principles by:
- Separating pure checkpoint creation logic from I/O
- Using `Result` chaining to compose operations
- Ensuring checkpoint saves happen in both success and error paths

## Requirements

### Functional Requirements

1. **Error Path Checkpointing**: Save checkpoint immediately when workflow execution fails
   - Capture current workflow state (completed steps, variables, iteration)
   - Include error context (error message, failed step index, stack trace)
   - Set checkpoint status to `Failed` or `Interrupted` as appropriate

2. **Success Path Checkpointing**: Continue saving checkpoints on successful completion
   - Mark checkpoint status as `Completed`
   - Include final workflow context and results
   - Clean up checkpoint if workflow completes successfully (optional config)

3. **Checkpoint Before Exit**: Ensure checkpoint save happens before process terminates
   - Handle `commit_required` failures
   - Handle command execution errors
   - Handle validation failures
   - Handle panic/interrupt signals (already implemented via signal handler)

4. **Pure Checkpoint Creation Functions**: Separate checkpoint data construction from I/O
   - `create_completion_checkpoint(...)` - pure function for success case
   - `create_error_checkpoint(...)` - pure function for error case
   - Both return `Result<WorkflowCheckpoint>` without side effects

### Non-Functional Requirements

1. **Reliability**: Checkpoint save must not fail silently
   - Log warnings if checkpoint save fails, but don't abort workflow
   - Use atomic writes to prevent partial checkpoint corruption

2. **Performance**: Checkpoint save should complete within 1 second for typical workflows
   - Async I/O to prevent blocking
   - Minimal serialization overhead

3. **Functional Purity**: Separate pure logic from I/O operations
   - Checkpoint data structure creation is pure
   - I/O operations are clearly marked and separated

4. **Error Context**: Include actionable debugging information in error checkpoints
   - Failed step command
   - Error message and type
   - Workflow variables at time of failure

## Acceptance Criteria

- [ ] Workflow failures trigger checkpoint save before exit
- [ ] Saved checkpoints include error context (message, step, variables)
- [ ] Checkpoint status is set to `Failed` for errors, `Completed` for success
- [ ] Pure functions `create_completion_checkpoint` and `create_error_checkpoint` exist
- [ ] Both functions have no side effects and return `Result<WorkflowCheckpoint>`
- [ ] Unit tests verify checkpoint creation for error scenarios
- [ ] Integration test verifies checkpoint exists after `commit_required` failure
- [ ] Resume command can load and display error checkpoints
- [ ] Checkpoint save failures are logged but don't crash workflow
- [ ] No panics in checkpoint creation or save logic (Spec 101 compliance)

## Technical Details

### Implementation Approach

#### 1. Pure Checkpoint Creation Functions

Create pure functions in `src/cook/workflow/checkpoint.rs`:

```rust
/// Pure function: create checkpoint for successful completion
pub fn create_completion_checkpoint(
    workflow_id: String,
    workflow: &NormalizedWorkflow,
    context: &WorkflowContext,
    completed_steps: Vec<CompletedStep>,
    workflow_hash: String,
) -> Result<WorkflowCheckpoint> {
    let mut checkpoint = create_checkpoint(
        workflow_id,
        workflow,
        context,
        completed_steps,
        context.current_step_index,
        workflow_hash,
    );

    checkpoint.execution_state.status = WorkflowStatus::Completed;
    Ok(checkpoint)
}

/// Pure function: create checkpoint with error context for failure recovery
pub fn create_error_checkpoint(
    workflow_id: String,
    workflow: &NormalizedWorkflow,
    context: &WorkflowContext,
    completed_steps: Vec<CompletedStep>,
    workflow_hash: String,
    error: &anyhow::Error,
    failed_step_index: usize,
) -> Result<WorkflowCheckpoint> {
    let mut checkpoint = create_checkpoint(
        workflow_id,
        workflow,
        context,
        completed_steps,
        failed_step_index,
        workflow_hash,
    );

    // Set status to Failed
    checkpoint.execution_state.status = WorkflowStatus::Failed;

    // Store error context in variable_state for debugging
    checkpoint.variable_state.insert(
        "__error_message".to_string(),
        Value::String(error.to_string()),
    );
    checkpoint.variable_state.insert(
        "__failed_step_index".to_string(),
        Value::Number(failed_step_index.into()),
    );
    checkpoint.variable_state.insert(
        "__error_timestamp".to_string(),
        Value::String(Utc::now().to_rfc3339()),
    );

    Ok(checkpoint)
}
```

#### 2. Workflow Executor Error Handling

Modify `src/cook/workflow/executor.rs` to save checkpoint on error:

```rust
pub async fn execute_workflow_with_checkpointing(
    &self,
    workflow: &NormalizedWorkflow,
    env: &ExecutionEnvironment,
    context: &mut WorkflowContext,
) -> Result<()> {
    let checkpoint_manager = self.checkpoint_manager
        .as_ref()
        .context("Checkpoint manager not configured")?;

    let workflow_id = self.workflow_id
        .as_ref()
        .context("Workflow ID not set")?;

    // Execute workflow steps
    let execution_result = self.execute_steps(workflow, env, context).await;

    // Save checkpoint based on result
    let checkpoint_result = match &execution_result {
        Ok(_) => {
            // Success: save completion checkpoint
            create_completion_checkpoint(
                workflow_id.clone(),
                workflow,
                context,
                self.completed_steps.clone(),
                self.calculate_workflow_hash(workflow),
            )
            .and_then(|cp| {
                // I/O operation: save to disk
                checkpoint_manager.save_checkpoint(&cp).await
            })
        }
        Err(error) => {
            // Failure: save error recovery checkpoint
            create_error_checkpoint(
                workflow_id.clone(),
                workflow,
                context,
                self.completed_steps.clone(),
                self.calculate_workflow_hash(workflow),
                error,
                context.current_step_index,
            )
            .and_then(|cp| {
                // I/O operation: save to disk
                checkpoint_manager.save_checkpoint(&cp).await
            })
        }
    };

    // Log checkpoint errors but don't fail the workflow
    if let Err(checkpoint_err) = checkpoint_result {
        tracing::error!(
            "Failed to save checkpoint for workflow {}: {}",
            workflow_id,
            checkpoint_err
        );
        // Continue - don't abort workflow due to checkpoint failure
    }

    // Return original execution result
    execution_result
}
```

#### 3. Orchestrator Integration

Update `src/cook/orchestrator.rs` to use checkpoint-on-error:

```rust
// In run_workflow or execute_workflow
let result = workflow_executor
    .execute_workflow_with_checkpointing(&workflow, &env, &mut context)
    .await;

match result {
    Ok(_) => {
        println!("✅ Workflow completed successfully");
        // Optionally delete checkpoint if configured
    }
    Err(e) => {
        eprintln!("❌ Workflow failed: {}", e);
        eprintln!("💡 To resume from last checkpoint, run: prodigy resume {}", session_id);
        // Checkpoint already saved by execute_workflow_with_checkpointing
        return Err(e);
    }
}
```

### Architecture Changes

- **Modified**: `WorkflowExecutor` in `src/cook/workflow/executor.rs`
  - Add `execute_workflow_with_checkpointing` method
  - Wrap existing execution logic with checkpoint save on both paths

- **Modified**: `checkpoint.rs`
  - Add `create_completion_checkpoint` pure function
  - Add `create_error_checkpoint` pure function
  - Extend `WorkflowCheckpoint.variable_state` to store error context

- **Modified**: Orchestrator
  - Use new checkpointing methods
  - Display resume instructions on failure

### Data Structures

**Extended WorkflowCheckpoint**:
```rust
pub struct WorkflowCheckpoint {
    // ... existing fields ...

    // Error context stored in variable_state:
    // - "__error_message": String - error message
    // - "__failed_step_index": Number - index of failed step
    // - "__error_timestamp": String - ISO 8601 timestamp
}
```

### Functional Programming Principles

1. **Pure Functions**: Checkpoint creation has no side effects
2. **Result Chaining**: Use `and_then()` to compose checkpoint creation + save
3. **Explicit I/O**: Separate pure logic from I/O operations
4. **Error as Values**: Use `Result<T>` instead of panicking
5. **Immutability**: Checkpoints are immutable once created

## Dependencies

- **Prerequisites**:
  - Spec 122 (Checkpoint Path Resolution) - ensures checkpoints saved to correct location
  - Spec 101 (Error Handling Guidelines) - no panics in production code

- **Affected Components**:
  - `src/cook/workflow/executor.rs` - workflow execution with checkpointing
  - `src/cook/workflow/checkpoint.rs` - pure checkpoint creation functions
  - `src/cook/orchestrator.rs` - error handling and user feedback

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_create_error_checkpoint_includes_context() {
    let error = anyhow!("Test error: commit required but no commits");
    let checkpoint = create_error_checkpoint(
        "test-workflow".to_string(),
        &workflow,
        &context,
        vec![],
        "hash123".to_string(),
        &error,
        5, // failed at step 5
    ).unwrap();

    assert_eq!(checkpoint.execution_state.status, WorkflowStatus::Failed);

    let error_msg = checkpoint.variable_state.get("__error_message").unwrap();
    assert!(error_msg.as_str().unwrap().contains("commit required"));

    let failed_step = checkpoint.variable_state.get("__failed_step_index").unwrap();
    assert_eq!(failed_step.as_u64().unwrap(), 5);
}

#[test]
fn test_create_completion_checkpoint_status() {
    let checkpoint = create_completion_checkpoint(
        "test-workflow".to_string(),
        &workflow,
        &context,
        vec![],
        "hash123".to_string(),
    ).unwrap();

    assert_eq!(checkpoint.execution_state.status, WorkflowStatus::Completed);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_checkpoint_saved_on_commit_required_failure() {
    // Setup workflow with commit_required=true step
    let workflow = create_test_workflow_with_commit_required();
    let temp_dir = TempDir::new().unwrap();
    let session_id = "test-session-123";

    // Create checkpoint manager with Session storage
    let storage = CheckpointStorage::Session {
        session_id: session_id.to_string(),
    };
    let checkpoint_manager = Arc::new(CheckpointManager::with_storage(storage));

    // Execute workflow (will fail due to no commits)
    let executor = WorkflowExecutor::new(/* ... */)
        .with_checkpoint_manager(checkpoint_manager.clone(), session_id.to_string());

    let result = executor.execute_workflow_with_checkpointing(&workflow, &env, &mut context).await;

    // Verify workflow failed
    assert!(result.is_err());

    // Verify checkpoint was saved
    let checkpoint = checkpoint_manager.load_checkpoint(session_id).await;
    assert!(checkpoint.is_ok(), "Checkpoint should exist after failure");

    let checkpoint = checkpoint.unwrap();
    assert_eq!(checkpoint.execution_state.status, WorkflowStatus::Failed);
    assert!(checkpoint.variable_state.contains_key("__error_message"));
}
```

### Error Scenario Tests

Test checkpoint creation for all error types:
- Commit required failures
- Command execution failures
- Validation failures
- Timeout errors
- Signal interruptions

## Documentation Requirements

### Code Documentation

- Document `create_error_checkpoint` and `create_completion_checkpoint` as pure functions
- Explain error context storage in checkpoint `variable_state`
- Document checkpoint-on-error pattern in executor

### User Documentation

Update user guide:
- Explain that checkpoints are saved even on failures
- Document how to inspect error checkpoints
- Show resume workflow for debugging failed workflows

### Architecture Updates

Add to `ARCHITECTURE.md`:
- Checkpoint-on-error pattern
- Error recovery workflow
- Pure function approach to checkpoint creation

## Implementation Notes

### Error Handling Best Practices

1. **Don't fail checkpoint saves loudly**: Log warnings but continue
2. **Include actionable error context**: Step index, command, error message
3. **Use atomic writes**: Prevent partial checkpoint corruption
4. **Test error paths explicitly**: Don't just test happy path

### Common Pitfalls

- **Don't** abort workflow if checkpoint save fails (just log warning)
- **Don't** create checkpoint in error path without I/O boundary separation
- **Don't** lose original error when checkpoint fails
- **Do** save checkpoint before returning error
- **Do** include enough context to debug the failure

## Migration and Compatibility

### Breaking Changes

None - this is additive functionality.

### Migration Path

1. Add pure checkpoint creation functions
2. Update workflow executor to use new functions
3. Test checkpoint-on-error with existing workflows
4. Roll out gradually with feature flag if needed

## Success Metrics

- Zero "No checkpoint files found" errors after workflow failures
- 100% of workflow errors result in saved checkpoints
- Error checkpoints include sufficient debug information
- Resume command can successfully load error checkpoints
- No performance degradation from checkpoint saves (< 1s overhead)
