---
number: 72
title: Resume with Error Recovery
category: foundation
priority: critical
status: draft
dependencies: [62]
created: 2025-01-16
---

# Specification 72: Resume with Error Recovery

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [62 - Workflow-Level Error Handling Directives]

## Context

Resume functionality in Prodigy currently fails when workflows contain error handlers (`on_failure` blocks). The resume process exits with code 1 instead of properly restoring the execution state and handling errors during resumed execution. This is a critical gap because error handlers are essential for robust workflows, and users expect to be able to resume workflows that have error handling logic.

The core issue is that the resume mechanism doesn't properly:
1. Restore error handling state from checkpoints
2. Apply error handlers during resumed execution
3. Handle failures that occur during the resume process itself
4. Maintain error context across resume boundaries

## Objective

Implement robust error recovery during workflow resume operations, ensuring that error handlers are properly restored and executed during resumed workflow execution, and that the resume process itself can handle and recover from errors.

## Requirements

### Functional Requirements

1. **Error Handler Restoration**
   - Restore `on_failure` handlers from checkpoint state
   - Preserve error handler context and variables
   - Maintain error handler execution order
   - Support nested error handlers
   - Handle conditional error handlers

2. **Resume Error Recovery**
   - Handle errors that occur during resume initialization
   - Recover from corrupted checkpoint files
   - Handle missing dependencies during resume
   - Gracefully handle environment changes
   - Provide clear error messages with recovery suggestions

3. **Error State Persistence**
   - Save error handler state to checkpoints
   - Persist error context across resume operations
   - Track error handler execution history
   - Maintain error correlation IDs
   - Store error recovery metadata

4. **Error Handler Execution During Resume**
   - Execute error handlers for failures during resumed operations
   - Maintain error handler scope and context
   - Support error handler retries after resume
   - Handle cascading failures properly
   - Preserve original error information

5. **Recovery Strategies**
   - Automatic retry with exponential backoff
   - Fallback to alternative execution paths
   - Partial resume from known good state
   - Manual intervention prompts
   - Safe abort with cleanup

### Non-Functional Requirements

1. **Reliability**
   - Resume must succeed even with complex error handlers
   - Error handlers must execute correctly after resume
   - No data loss during error recovery
   - Consistent error handling behavior

2. **Observability**
   - Clear logging of resume and error recovery operations
   - Error handler execution tracing
   - Recovery operation metrics
   - Detailed error diagnostics

3. **Performance**
   - Minimal overhead for error state persistence
   - Fast error handler restoration
   - Efficient error context reconstruction

## Acceptance Criteria

- [ ] Workflows with `on_failure` handlers resume successfully (exit code 0)
- [ ] Error handlers execute correctly during resumed workflow execution
- [ ] Resume process handles its own errors gracefully
- [ ] Error context is properly restored from checkpoints
- [ ] Nested error handlers work correctly after resume
- [ ] Conditional error handlers are evaluated properly
- [ ] Error correlation IDs are maintained across resume
- [ ] Resume provides helpful error messages for recovery
- [ ] Error handler retries work correctly after resume
- [ ] Performance overhead is less than 3% for error state persistence

## Technical Details

### Implementation Approach

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryState {
    pub active_handlers: Vec<ErrorHandler>,
    pub error_context: HashMap<String, Value>,
    pub handler_execution_history: Vec<HandlerExecution>,
    pub retry_state: Option<RetryState>,
    pub correlation_id: String,
}

#[derive(Debug, Clone)]
pub struct ResumeErrorRecovery {
    recovery_state: ErrorRecoveryState,
    checkpoint_manager: Arc<CheckpointManager>,
    error_handler_executor: Arc<ErrorHandlerExecutor>,
}

impl ResumeErrorRecovery {
    pub async fn restore_error_handlers(
        &self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<Vec<ErrorHandler>> {
        // Restore error handlers from checkpoint
        let handlers = checkpoint.error_recovery_state
            .as_ref()
            .map(|state| state.active_handlers.clone())
            .unwrap_or_default();

        // Validate handlers are still applicable
        self.validate_error_handlers(&handlers).await?;

        // Initialize error context
        self.restore_error_context(checkpoint).await?;

        Ok(handlers)
    }

    pub async fn handle_resume_error(
        &self,
        error: &ProdigyError,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<RecoveryAction> {
        match error {
            ProdigyError::CorruptedCheckpoint => {
                self.attempt_checkpoint_repair(checkpoint).await
            }
            ProdigyError::MissingDependency(_) => {
                self.resolve_missing_dependencies().await
            }
            ProdigyError::EnvironmentMismatch => {
                self.adapt_to_environment_changes().await
            }
            _ => {
                self.default_error_recovery(error).await
            }
        }
    }

    async fn execute_error_handler_with_resume_context(
        &self,
        handler: &ErrorHandler,
        error: &ExecutionError,
        resume_context: &ResumeContext,
    ) -> Result<()> {
        // Execute error handler with full context restoration
        let execution_context = self.build_execution_context(resume_context)?;

        // Maintain error correlation across resume
        let correlated_error = error.with_correlation_id(
            &resume_context.correlation_id
        );

        self.error_handler_executor
            .execute_with_context(handler, &correlated_error, &execution_context)
            .await
    }
}
```

### Architecture Changes

1. **Error Recovery Module**
   - New `error_recovery` module in `cook::workflow`
   - Error handler state management
   - Resume error handling logic
   - Recovery strategy implementations

2. **Checkpoint Extensions**
   - Add `error_recovery_state` field to `WorkflowCheckpoint`
   - Store error handler configurations
   - Persist error context and correlation IDs
   - Track error handler execution history

3. **Resume Process Integration**
   - Enhanced resume logic with error recovery
   - Error handler restoration pipeline
   - Recovery strategy selection
   - Error state validation

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandler {
    pub id: String,
    pub condition: Option<String>,
    pub commands: Vec<Command>,
    pub retry_config: Option<RetryConfig>,
    pub timeout: Option<Duration>,
    pub scope: ErrorHandlerScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorHandlerScope {
    Command,
    Step,
    Phase,
    Workflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerExecution {
    pub handler_id: String,
    pub executed_at: DateTime<Utc>,
    pub success: bool,
    pub error: Option<String>,
    pub retry_attempt: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    Retry { delay: Duration, max_attempts: u32 },
    Fallback { alternative_path: String },
    PartialResume { from_step: usize },
    RequestIntervention { message: String },
    SafeAbort { cleanup_actions: Vec<Command> },
}
```

### Integration Points

1. **Workflow Executor Integration**
   - Hook error recovery into resume process
   - Error handler execution during resumed operations
   - Error state persistence on checkpoints

2. **Command Execution Integration**
   - Error handler restoration before command execution
   - Error context propagation through execution pipeline
   - Recovery action execution

3. **Checkpoint Manager Integration**
   - Save/load error recovery state
   - Validate error state consistency
   - Handle checkpoint corruption gracefully

## Dependencies

- **Prerequisites**: [62 - Workflow-Level Error Handling Directives]
- **Affected Components**:
  - `src/cook/workflow/resume.rs`
  - `src/cook/workflow/executor.rs`
  - `src/cook/workflow/checkpoint.rs`
  - `src/cook/execution/error_handling.rs`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Error handler restoration from checkpoints
  - Resume error recovery mechanisms
  - Error context preservation
  - Recovery action selection

- **Integration Tests**:
  - End-to-end resume with error handlers
  - Complex error handler scenarios
  - Resume error recovery flows
  - Error correlation across resume boundaries

- **Edge Cases**:
  - Corrupted checkpoints with error handlers
  - Missing error handler dependencies
  - Nested error handler failures
  - Environment changes affecting error handlers

- **User Acceptance**:
  - Real workflows with complex error handling
  - Resume reliability measurement
  - Error message clarity validation

## Documentation Requirements

- **Code Documentation**:
  - Error recovery architecture overview
  - Recovery strategy guide
  - Error handler restoration process

- **User Documentation**:
  - Resume troubleshooting guide
  - Error handler best practices for resume
  - Recovery scenario cookbook

- **Architecture Updates**:
  - Error recovery flow diagrams
  - Resume process with error handling
  - Recovery strategy decision tree

## Implementation Notes

1. **Error State Isolation**: Keep error recovery state separate from main execution state
2. **Graceful Degradation**: Provide fallback recovery options when primary recovery fails
3. **User Feedback**: Clear error messages with actionable recovery suggestions
4. **Testing**: Comprehensive testing of error scenarios and recovery paths
5. **Monitoring**: Detailed observability for error recovery operations

## Migration and Compatibility

- Backward compatible with existing checkpoints (error recovery state optional)
- Automatic migration of legacy error handlers
- Graceful handling of unsupported error recovery features
- Progressive rollout with feature flags