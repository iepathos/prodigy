---
number: 81
title: Error Recovery Handler Execution
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 81: Error Recovery Handler Execution

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

Error recovery handlers are a critical feature for workflow resilience, allowing workflows to automatically recover from failures through custom recovery actions. Currently, the implementation in `src/cook/workflow/error_recovery.rs` contains TODO comments indicating that handlers only log commands without executing them (lines 481, 488). Additionally, condition evaluation (line 353) is not implemented, making conditional error handling non-functional.

This renders the entire error recovery system ineffective, forcing users to manually intervene when errors occur instead of having automated recovery mechanisms. The error recovery system is meant to:
- Execute compensating actions when errors occur
- Retry operations with different parameters
- Clean up resources after failures
- Send notifications or alerts
- Implement circuit breaker patterns

## Objective

Complete the implementation of error recovery handlers to enable automatic execution of recovery actions, including condition evaluation and proper state management.

## Requirements

### Functional Requirements

1. **Handler Command Execution**
   - Implement actual execution of shell commands (line 481)
   - Implement actual execution of Claude commands (line 488)
   - Support all command types (shell, claude, goal_seek, foreach)
   - Pass context and variables to executed commands
   - Capture and return command outputs

2. **Condition Evaluation**
   - Implement condition evaluation logic (line 353)
   - Support boolean expressions in conditions
   - Access to error context and workflow variables
   - Support complex conditions with AND/OR logic

3. **State Management**
   - Extract error recovery state from checkpoints (line 344)
   - Persist handler execution results
   - Track retry attempts and history
   - Maintain error context across retries

4. **Error Handler Chain**
   - Execute multiple handlers in sequence
   - Support handler priorities
   - Implement short-circuit on success
   - Allow handler composition

### Non-Functional Requirements

1. **Reliability**
   - Handlers must not introduce additional failures
   - Graceful degradation if handler fails
   - Timeout protection for long-running handlers

2. **Performance**
   - Minimal overhead when no errors occur
   - Efficient condition evaluation
   - Async execution where possible

3. **Debuggability**
   - Comprehensive logging of handler execution
   - Clear error messages for handler failures
   - Trace handler execution path

## Acceptance Criteria

- [ ] Shell commands in error handlers execute successfully
- [ ] Claude commands in error handlers execute successfully
- [ ] Conditions are properly evaluated before handler execution
- [ ] Error recovery state is correctly extracted from checkpoints
- [ ] Handler outputs are captured and available to subsequent steps
- [ ] Failed handlers don't crash the workflow
- [ ] All existing error recovery tests pass
- [ ] New integration tests validate end-to-end error recovery
- [ ] Handler execution is logged with appropriate detail
- [ ] Timeout protection prevents hung handlers

## Technical Details

### Implementation Approach

1. **Command Execution Implementation**
   ```rust
   // Replace TODO at line 481
   let output = match handler.action {
       ErrorAction::Shell { command } => {
           self.command_executor.execute_shell(
               &command,
               &context.with_error(error_info)
           ).await?
       },
       ErrorAction::Claude { command } => {
           self.command_executor.execute_claude(
               &command,
               &context.with_error(error_info)
           ).await?
       },
       // ... other action types
   };
   ```

2. **Condition Evaluation**
   ```rust
   // Replace TODO at line 353
   fn evaluate_condition(
       &self,
       condition: &str,
       context: &ErrorContext
   ) -> Result<bool> {
       let expr = parse_condition(condition)?;
       let result = self.expression_evaluator.evaluate(
           &expr,
           &context.variables
       )?;
       Ok(result.as_bool().unwrap_or(false))
   }
   ```

3. **State Extraction**
   ```rust
   // Replace placeholder at line 344
   fn extract_recovery_state(
       &self,
       checkpoint: &Checkpoint
   ) -> Result<Option<RecoveryState>> {
       checkpoint.metadata
           .get("error_recovery")
           .map(|v| serde_json::from_value(v.clone()))
           .transpose()
           .context("Failed to deserialize recovery state")
   }
   ```

### Architecture Changes

- Integrate with `CommandExecutor` for command execution
- Use `ExpressionEvaluator` for condition evaluation
- Extend checkpoint format to include recovery state
- Add timeout wrapper for handler execution

### Data Structures

```rust
pub struct ErrorContext {
    pub error: ErrorInfo,
    pub variables: Variables,
    pub attempt: usize,
    pub handler_outputs: Vec<HandlerOutput>,
}

pub struct HandlerOutput {
    pub handler_name: String,
    pub success: bool,
    pub output: Option<String>,
    pub duration: Duration,
}

pub struct RecoveryState {
    pub handlers_executed: Vec<String>,
    pub retry_count: usize,
    pub last_error: Option<String>,
    pub context: HashMap<String, Value>,
}
```

### APIs and Interfaces

```rust
#[async_trait]
pub trait ErrorRecoveryExecutor {
    async fn execute_handler(
        &self,
        handler: &ErrorHandler,
        context: &ErrorContext,
    ) -> Result<HandlerOutput>;

    fn evaluate_condition(
        &self,
        condition: &str,
        context: &ErrorContext,
    ) -> Result<bool>;

    async fn recover_from_error(
        &self,
        error: &Error,
        handlers: &[ErrorHandler],
        context: &ExecutionContext,
    ) -> Result<RecoveryResult>;
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `ErrorRecoveryManager`
  - `CommandExecutor`
  - `ExpressionEvaluator`
  - `CheckpointManager`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test condition evaluation with various expressions
  - Validate command execution for each handler type
  - Test state extraction from checkpoints
  - Verify timeout behavior

- **Integration Tests**:
  - End-to-end error recovery scenarios
  - Test handler chains with multiple handlers
  - Validate state persistence across retries
  - Test conditional handler execution

- **Failure Tests**:
  - Handler execution failures don't crash workflow
  - Timeout protection works correctly
  - Malformed conditions are handled gracefully

- **User Acceptance**:
  - Create workflow with error handlers
  - Trigger errors and verify recovery
  - Test various handler types and conditions

## Documentation Requirements

- **Code Documentation**:
  - Document all public methods in error recovery module
  - Add examples for common error recovery patterns
  - Include inline comments for complex logic

- **User Documentation**:
  - Guide for writing error handlers
  - Common error recovery patterns
  - Troubleshooting error handlers
  - Best practices for conditions

- **Architecture Updates**:
  - Update ARCHITECTURE.md with error recovery flow
  - Document state management approach
  - Include sequence diagrams for handler execution

## Implementation Notes

- Start with basic command execution before adding conditions
- Ensure backward compatibility with existing workflows
- Consider adding dry-run mode for testing handlers
- Implement comprehensive logging for production debugging
- Add metrics for handler execution success rates
- Consider implementing handler templates for common patterns

## Migration and Compatibility

- Existing workflows without handlers continue to work
- Old checkpoints without recovery state handled gracefully
- No breaking changes to workflow format
- Consider adding handler versioning for future extensions