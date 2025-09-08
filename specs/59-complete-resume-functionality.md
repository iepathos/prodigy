---
number: 59
title: Complete Resume Functionality Implementation
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-09-08
---

# Specification 59: Complete Resume Functionality Implementation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current resume functionality in Prodigy has a solid foundation but contains critical gaps that prevent it from being production-ready. While basic session state persistence and restoration work, several key features are incomplete or missing entirely. This includes improper step tracking, missing output preservation, lack of validation, and separate resume mechanisms for different workflow types that could confuse users.

## Objective

Complete the implementation of resume functionality to provide robust, reliable workflow resumption after interruption, ensuring that no work is lost or duplicated, and that complex workflows with dependencies can be properly restored.

## Requirements

### Functional Requirements

1. **Complete Step Tracking**
   - Properly populate the `completed_steps` vector after each successful step execution
   - Store step outputs, duration, and success status
   - Skip already-completed steps on resume
   - Provide accurate progress reporting

2. **Output Preservation and Context Restoration**
   - Capture and store step outputs in workflow state
   - Restore execution context variables on resume
   - Enable variable interpolation from previous steps
   - Maintain output format consistency

3. **Workflow Validation**
   - Calculate and store workflow hash on initial execution
   - Verify workflow hasn't changed before resume
   - Detect incompatible workflow modifications
   - Provide clear error messages for validation failures

4. **Enhanced Error Recovery**
   - Detect partial step completion
   - Implement rollback mechanism for failed steps
   - Enable retry of individual failed steps
   - Track failure reasons and recovery attempts

5. **Unified Resume Interface**
   - Integrate MapReduce resume with main cook --resume flag
   - Auto-detect workflow type from saved state
   - Route to appropriate resume handler
   - Provide consistent user experience

6. **Advanced Resume Features**
   - Progress percentage tracking
   - Estimated time remaining calculation
   - Dry-run mode for resume preview
   - Ability to modify arguments on resume
   - Resume from specific step

### Non-Functional Requirements

1. **Performance**
   - Minimal overhead for checkpoint creation
   - Fast session state loading
   - Efficient step output storage
   - Quick validation checks

2. **Reliability**
   - Atomic checkpoint updates
   - Corruption detection and recovery
   - Concurrent access protection
   - Graceful degradation

3. **Usability**
   - Clear progress indicators
   - Informative error messages
   - Intuitive resume commands
   - Helpful documentation

## Acceptance Criteria

- [ ] All workflow steps properly track completion with outputs and metadata
- [ ] Variable interpolation works correctly after resume with previous step outputs
- [ ] Workflow validation prevents resuming modified workflows
- [ ] Failed steps can be individually retried without re-running successful steps
- [ ] MapReduce workflows can be resumed using the standard --resume flag
- [ ] Dry-run mode accurately shows what would be executed on resume
- [ ] Progress percentage and time estimates are displayed during execution
- [ ] Resume works correctly for all workflow types (Standard, Iterative, Structured, MapReduce)
- [ ] Concurrent modification detection prevents corruption
- [ ] Stale checkpoints are automatically cleaned up
- [ ] Comprehensive test coverage for all resume scenarios
- [ ] Documentation clearly explains resume functionality and limitations

## Technical Details

### Implementation Approach

1. **Step Tracking Enhancement**
```rust
// In execute_step method, after successful execution
let step_result = StepResult {
    step_index: index,
    command: format_step_command(step),
    success: result.is_ok(),
    output: capture_output(result),
    duration: elapsed,
    error: result.err().map(|e| e.to_string()),
};

workflow_state.completed_steps.push(step_result);
session_manager.update_session(
    SessionUpdate::UpdateWorkflowState(workflow_state)
).await?;
```

2. **Output Preservation**
```rust
pub struct ExecutionContext {
    variables: HashMap<String, String>,
    step_outputs: HashMap<usize, String>,
    environment: HashMap<String, String>,
}

impl ExecutionContext {
    pub fn restore_from_state(workflow_state: &WorkflowState) -> Self {
        let mut context = Self::new();
        for step in &workflow_state.completed_steps {
            context.step_outputs.insert(
                step.step_index, 
                step.output.clone().unwrap_or_default()
            );
            context.variables.insert(
                format!("step_{}_output", step.step_index),
                step.output.clone().unwrap_or_default()
            );
        }
        context
    }
}
```

3. **Workflow Validation**
```rust
use sha2::{Sha256, Digest};

pub fn calculate_workflow_hash(workflow: &WorkflowConfig) -> String {
    let mut hasher = Sha256::new();
    let serialized = serde_json::to_string(workflow).unwrap();
    hasher.update(serialized);
    format!("{:x}", hasher.finalize())
}

// Before resume
if let Some(stored_hash) = state.workflow_hash {
    let current_hash = calculate_workflow_hash(&config.workflow);
    if current_hash != stored_hash {
        return Err(anyhow!(
            "Workflow has been modified since interruption. \
             Use --force to override or start a new session."
        ));
    }
}
```

4. **Unified Resume Interface**
```rust
// In orchestrator.rs
async fn resume_workflow(&self, session_id: &str, config: CookConfig) -> Result<()> {
    let state = self.session_manager.load_session(session_id).await?;
    
    // Auto-detect workflow type
    match state.workflow_type {
        Some(WorkflowType::MapReduce) => {
            self.resume_mapreduce_job(&state, config).await
        }
        _ => {
            self.resume_standard_workflow(&state, config).await
        }
    }
}
```

### Architecture Changes

1. **SessionState Extension**
```rust
pub struct SessionState {
    // Existing fields...
    pub workflow_hash: Option<String>,
    pub workflow_type: Option<WorkflowType>,
    pub execution_context: Option<ExecutionContext>,
    pub checkpoint_version: u32,
    pub last_validated_at: Option<DateTime<Utc>>,
}
```

2. **StepResult Enhancement**
```rust
pub struct StepResult {
    pub step_index: usize,
    pub command: String,
    pub success: bool,
    pub output: Option<String>,
    pub duration: Duration,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub exit_code: Option<i32>,
}
```

### Data Structures

1. **ResumeOptions**
```rust
pub struct ResumeOptions {
    pub dry_run: bool,
    pub force: bool,
    pub from_step: Option<usize>,
    pub override_args: Option<Vec<String>>,
    pub skip_validation: bool,
}
```

2. **ProgressTracker**
```rust
pub struct ProgressTracker {
    total_steps: usize,
    completed_steps: usize,
    current_step: Option<usize>,
    step_durations: Vec<Duration>,
    
    pub fn estimate_remaining(&self) -> Duration;
    pub fn progress_percentage(&self) -> f32;
    pub fn format_progress(&self) -> String;
}
```

### APIs and Interfaces

1. **Enhanced Session Manager**
```rust
#[async_trait]
pub trait SessionManager {
    // Existing methods...
    
    async fn validate_checkpoint(&self, session_id: &str) -> Result<bool>;
    async fn get_resume_preview(&self, session_id: &str) -> Result<ResumePreview>;
    async fn cleanup_stale_checkpoints(&self, ttl: Duration) -> Result<usize>;
    async fn lock_session(&self, session_id: &str) -> Result<SessionLock>;
}
```

2. **Resume Preview**
```rust
pub struct ResumePreview {
    pub session_id: String,
    pub workflow_path: PathBuf,
    pub completed_steps: Vec<String>,
    pub remaining_steps: Vec<String>,
    pub estimated_time: Duration,
    pub captured_outputs: HashMap<usize, String>,
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `cook::orchestrator`
  - `cook::session`
  - `cook::execution`
  - `cook::workflow`
- **External Dependencies**: 
  - `sha2` crate for workflow hashing
  - `chrono` for timestamp management

## Testing Strategy

- **Unit Tests**: 
  - Test step tracking with various success/failure scenarios
  - Verify output preservation and restoration
  - Test workflow hash calculation and validation
  - Validate progress tracking calculations

- **Integration Tests**:
  - Test full workflow execution with interruption and resume
  - Verify variable interpolation across resume
  - Test concurrent access protection
  - Validate cleanup of stale checkpoints

- **Performance Tests**:
  - Measure checkpoint creation overhead
  - Test resume speed with large state files
  - Validate memory usage with many completed steps

- **User Acceptance**:
  - Test resume with real-world workflows
  - Verify user experience and error messages
  - Validate dry-run accuracy
  - Test edge cases and failure scenarios

## Documentation Requirements

- **Code Documentation**:
  - Document all new public APIs
  - Add examples for resume functionality
  - Document error conditions and recovery

- **User Documentation**:
  - Add resume section to README
  - Create resume troubleshooting guide
  - Document best practices for resumable workflows

- **Architecture Updates**:
  - Update ARCHITECTURE.md with resume flow
  - Document checkpoint storage format
  - Explain workflow validation process

## Implementation Notes

1. **Backward Compatibility**: Ensure existing sessions without new fields can still be loaded
2. **Atomic Operations**: Use atomic file operations for checkpoint updates
3. **Cleanup Strategy**: Implement configurable TTL for checkpoint cleanup
4. **Error Messages**: Provide actionable error messages with recovery suggestions
5. **Performance**: Consider using compression for large step outputs
6. **Security**: Ensure checkpoint files have appropriate permissions

## Migration and Compatibility

1. **Existing Sessions**: 
   - Provide migration for sessions without workflow hash
   - Handle missing step outputs gracefully
   - Default missing fields to reasonable values

2. **Configuration**:
   - Add resume-related configuration options
   - Support environment variables for defaults
   - Maintain backward compatibility with existing configs

3. **Breaking Changes**:
   - None expected for existing functionality
   - New features are additive only
   - Maintain existing CLI interface