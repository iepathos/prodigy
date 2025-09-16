---
number: 69
title: Workflow Resume and Recovery System
category: foundation
priority: critical
status: draft
dependencies: [30, 65]
created: 2025-09-16
---

# Specification 69: Workflow Resume and Recovery System

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [30 (checkpoint system), 65 (workflow execution)]

## Context

Currently, when a Prodigy workflow is interrupted (whether by user cancellation, system crash, or error), users must manually navigate to the worktree directory and execute individual workflow steps to continue from where they left off. This is particularly problematic during long-running workflows with multiple retry attempts, as demonstrated by the issue where a workflow interrupted during test retries (attempt 3 of 5) lost all retry progress because checkpoints were only saved after successful step completion.

The checkpoint infrastructure exists but has critical gaps:
1. Checkpoints are not saved during retry attempts within steps
2. The resume command exists but lacks proper integration with the checkpoint system
3. Type mismatches between `ExtendedWorkflowConfig` and `NormalizedWorkflow` prevent proper checkpoint creation
4. Session management is disconnected from the checkpoint recovery process

## Objective

Implement a robust, user-friendly workflow resume system that allows users to continue interrupted workflows from their exact point of failure with a single command, preserving all state including retry attempts, variables, and partial progress.

## Requirements

### Functional Requirements

1. **Checkpoint Granularity**
   - Save checkpoints after each retry attempt within a step
   - Capture retry state including attempt number, failure history, and debug outputs
   - Preserve all workflow variables and captured outputs at each checkpoint
   - Support checkpointing for all step types (claude, shell, test, validation, goal-seek)

2. **Simple Resume Command**
   - Single command to resume any interrupted workflow: `prodigy resume [session-id]`
   - Auto-detect last interrupted session if no session-id provided
   - Support resuming from specific checkpoint with `--from-checkpoint` flag
   - List available checkpoints with `prodigy checkpoints list`

3. **State Recovery**
   - Restore complete workflow context including variables, outputs, and environment
   - Resume from exact retry attempt (e.g., attempt 3 of 5 for test retries)
   - Preserve git worktree state and branch position
   - Maintain session timing and progress metrics

4. **Checkpoint Management**
   - Automatic checkpoint cleanup after successful workflow completion
   - Configurable checkpoint retention policy
   - Checkpoint validation to detect corruption or incompatibility
   - Support for checkpoint migration across Prodigy versions

### Non-Functional Requirements

1. **Performance**
   - Checkpoint save operations must complete within 500ms
   - Resume operations must restore state within 2 seconds
   - Minimal overhead during normal workflow execution (<5% performance impact)

2. **Reliability**
   - Atomic checkpoint writes to prevent corruption
   - Graceful handling of concurrent checkpoint access
   - Recovery from partially written checkpoints
   - Backward compatibility with existing checkpoint format

3. **Usability**
   - Clear progress indicators showing resume point
   - Helpful error messages for checkpoint issues
   - Automatic detection of resumable workflows
   - Integration with existing workflow commands

## Acceptance Criteria

- [ ] Checkpoints are saved after every retry attempt in test, validation, and goal-seek steps
- [ ] `prodigy resume` command successfully restores workflow from any interruption point
- [ ] Retry state (attempt number, failure history) is preserved and restored correctly
- [ ] All workflow variables and captured outputs are restored on resume
- [ ] Git worktree state is properly maintained across resume operations
- [ ] Resume works for both regular workflows and MapReduce jobs
- [ ] Checkpoint list command shows all available checkpoints with status
- [ ] Automatic cleanup removes checkpoints for completed workflows
- [ ] Resume command provides clear feedback about what's being restored
- [ ] Integration tests verify resume functionality for all workflow types
- [ ] Performance benchmarks show <5% overhead for checkpoint operations
- [ ] Documentation includes comprehensive resume workflow examples

## Technical Details

### Implementation Approach

1. **Enhanced Checkpoint System**
   ```rust
   pub struct EnhancedCheckpoint {
       workflow_id: String,
       session_id: String,
       workflow_type: WorkflowType,
       execution_state: ExecutionState,
       retry_states: HashMap<usize, RetryState>,
       variable_context: VariableContext,
       git_state: GitState,
       timestamp: DateTime<Utc>,
       version: u32,
   }

   pub struct RetryState {
       step_index: usize,
       current_attempt: usize,
       max_attempts: usize,
       failure_history: Vec<FailureRecord>,
       last_output: Option<String>,
       debug_context: HashMap<String, String>,
   }
   ```

2. **Unified Resume Command**
   ```rust
   pub async fn resume_workflow(
       session_id: Option<String>,
       options: ResumeOptions,
   ) -> Result<ResumeResult> {
       let session = resolve_session(session_id)?;
       let checkpoint = load_latest_checkpoint(&session)?;
       let workflow = load_workflow_from_checkpoint(&checkpoint)?;
       let executor = create_executor_from_checkpoint(&checkpoint)?;

       executor.resume_from_checkpoint(workflow, checkpoint).await
   }
   ```

3. **Checkpoint Trigger Points**
   - After each step completion
   - After each retry attempt
   - After validation attempts
   - Before long-running operations
   - On graceful interruption signals

### Architecture Changes

1. **WorkflowExecutor Modifications**
   - Add checkpoint tracking to all retry loops
   - Implement `Resumable` trait for stateful recovery
   - Support partial step execution from checkpoints

2. **Session Manager Integration**
   - Link checkpoints with session lifecycle
   - Track resumable sessions in global state
   - Provide session discovery mechanisms

3. **Type System Alignment**
   - Create bidirectional conversion between `ExtendedWorkflowConfig` and `NormalizedWorkflow`
   - Implement checkpoint-compatible workflow representation
   - Ensure all workflow types support serialization

### Data Structures

```rust
// Checkpoint storage structure
pub struct CheckpointStore {
    base_path: PathBuf,
    active_checkpoints: HashMap<String, CheckpointMetadata>,
    retention_policy: RetentionPolicy,
}

// Resume context for workflow restoration
pub struct ResumeContext {
    checkpoint: EnhancedCheckpoint,
    workflow_path: PathBuf,
    working_directory: PathBuf,
    environment: ExecutionEnvironment,
    session_manager: Arc<dyn SessionManager>,
}

// Resume options for user control
pub struct ResumeOptions {
    force: bool,
    from_checkpoint: Option<String>,
    reset_failures: bool,
    dry_run: bool,
    verbose: bool,
}
```

### APIs and Interfaces

```rust
// Checkpoint management trait
pub trait CheckpointManager {
    async fn save_checkpoint(&self, checkpoint: &EnhancedCheckpoint) -> Result<()>;
    async fn load_checkpoint(&self, id: &str) -> Result<EnhancedCheckpoint>;
    async fn list_checkpoints(&self) -> Result<Vec<CheckpointSummary>>;
    async fn cleanup_completed(&self) -> Result<usize>;
}

// Resumable workflow executor
pub trait ResumableExecutor {
    async fn resume_from_checkpoint(
        &mut self,
        workflow: &WorkflowConfig,
        checkpoint: EnhancedCheckpoint,
    ) -> Result<ExecutionResult>;

    async fn get_resume_point(&self) -> Option<ResumePoint>;
}
```

## Dependencies

- **Prerequisites**:
  - Spec 30: Basic checkpoint infrastructure
  - Spec 65: Workflow execution system
- **Affected Components**:
  - `WorkflowExecutor` - needs checkpoint integration
  - `SessionManager` - requires resume capability
  - `CheckpointManager` - needs enhancement for retry states
  - CLI commands - new resume command implementation
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Checkpoint creation at various interruption points
  - State serialization and deserialization
  - Resume context building
  - Checkpoint cleanup logic

- **Integration Tests**:
  - Resume workflow after interruption at each step type
  - Resume from middle of retry sequence
  - Resume with complex variable state
  - Concurrent resume operations

- **Performance Tests**:
  - Checkpoint overhead during normal execution
  - Resume operation timing with large state
  - Checkpoint storage growth over time

- **User Acceptance**:
  - Resume workflow interrupted during test retries
  - Resume MapReduce job with partial completion
  - Resume after system restart
  - Resume with modified workflow file

## Documentation Requirements

- **Code Documentation**:
  - Document all checkpoint trigger points
  - Explain retry state tracking logic
  - Describe resume algorithm and decision points

- **User Documentation**:
  - Complete guide to workflow resume functionality
  - Troubleshooting checkpoint issues
  - Best practices for long-running workflows
  - Examples of various resume scenarios

- **Architecture Updates**:
  - Update checkpoint system design in ARCHITECTURE.md
  - Document session-checkpoint relationship
  - Add resume flow diagrams

## Implementation Notes

1. **Checkpoint Compatibility**
   - Maintain version field in checkpoints for future migrations
   - Support reading old checkpoint formats during transition
   - Provide checkpoint upgrade utilities if schema changes

2. **Error Recovery**
   - Handle corrupted checkpoints gracefully
   - Provide checkpoint repair tools
   - Support manual checkpoint editing for debugging

3. **Performance Optimization**
   - Use async I/O for checkpoint operations
   - Implement checkpoint compression for large states
   - Consider incremental checkpoints for minor changes

4. **User Experience**
   - Show clear progress when resuming
   - Display what was skipped vs what will be re-executed
   - Provide dry-run mode to preview resume actions

## Migration and Compatibility

1. **Existing Checkpoints**
   - Automatically migrate v1 checkpoints to enhanced format
   - Preserve backward compatibility for 2 major versions
   - Provide migration tools for manual checkpoint updates

2. **Workflow Format Changes**
   - Support resuming workflows even if definition changed
   - Warn about incompatible changes that prevent resume
   - Allow forcing resume with compatibility override flag

3. **Breaking Changes**
   - New checkpoint format requires migration
   - Session management integration may affect existing sessions
   - Retry state tracking changes checkpoint size

4. **Rollback Plan**
   - Keep backup of original checkpoints during migration
   - Support downgrade by preserving v1 checkpoint reader
   - Document manual recovery procedures