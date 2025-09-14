---
number: 62
title: Workflow Resume and Checkpoint Recovery
category: foundation
priority: critical
status: draft
dependencies: [61]
created: 2025-01-14
---

# Specification 62: Workflow Resume and Checkpoint Recovery

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [61 - DLQ Reprocessing]

## Context

The whitepaper emphasizes resumable workflows as a core feature:
- "Resume from failure point"
- "Progress Tracking: Resume from failure point"
- `prodigy resume workflow.yml` command

Currently, while `prodigy resume-job` exists, it only displays job status without actually resuming execution. This forces users to restart failed workflows from the beginning, losing all progress and potentially reprocessing already-completed items.

## Objective

Implement complete workflow resume functionality with checkpoint-based recovery, enabling workflows to continue from their exact point of failure with full state restoration.

## Requirements

### Functional Requirements
- Resume interrupted workflows from last checkpoint
- Automatic checkpoint creation at configurable intervals
- Preserve completed work and skip re-execution
- Restore variable state and context
- Support both MapReduce and sequential workflows
- Handle partial item completion in parallel execution
- Merge resumed results with existing progress
- Detect and prevent duplicate execution

### Non-Functional Requirements
- Checkpoint overhead < 5% of execution time
- Support workflows with 10,000+ items
- Atomic checkpoint writes to prevent corruption
- Clear indication of resumed vs fresh execution

## Acceptance Criteria

- [ ] `prodigy resume workflow-123` continues from last checkpoint
- [ ] Completed steps are skipped during resume
- [ ] MapReduce jobs resume with remaining items only
- [ ] Variable state restored correctly on resume
- [ ] Progress bar shows combined original + resumed progress
- [ ] Checkpoint frequency configurable
- [ ] Manual checkpoint command available
- [ ] Corrupted checkpoints handled gracefully
- [ ] Resume works across different machines
- [ ] Clear logs indicate resumed execution

## Technical Details

### Implementation Approach

1. **Enhanced Checkpoint Structure**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowCheckpoint {
       pub workflow_id: String,
       pub workflow_config: WorkflowConfig,
       pub execution_state: ExecutionState,
       pub completed_steps: Vec<CompletedStep>,
       pub variable_state: HashMap<String, Value>,
       pub mapreduce_state: Option<MapReduceCheckpoint>,
       pub timestamp: DateTime<Utc>,
       pub version: String,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ExecutionState {
       pub current_step_index: usize,
       pub total_steps: usize,
       pub status: WorkflowStatus,
       pub start_time: DateTime<Utc>,
       pub last_checkpoint: DateTime<Utc>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MapReduceCheckpoint {
       pub completed_items: HashSet<String>,
       pub failed_items: Vec<String>,
       pub in_progress_items: HashMap<String, AgentState>,
       pub reduce_completed: bool,
   }
   ```

2. **Resume Executor**:
   ```rust
   pub struct ResumeExecutor {
       checkpoint_manager: CheckpointManager,
       workflow_executor: WorkflowExecutor,
   }

   impl ResumeExecutor {
       pub async fn resume(
           &self,
           workflow_id: &str,
           options: ResumeOptions,
       ) -> Result<ExecutionResult> {
           // Load checkpoint
           let checkpoint = self.checkpoint_manager
               .load_checkpoint(workflow_id)
               .await?;

           // Validate checkpoint integrity
           self.validate_checkpoint(&checkpoint)?;

           // Create resume context
           let resume_context = ResumeContext {
               skip_steps: checkpoint.completed_steps.clone(),
               variable_state: checkpoint.variable_state.clone(),
               mapreduce_state: checkpoint.mapreduce_state.clone(),
           };

           // Execute with resume context
           let result = self.workflow_executor
               .execute_with_resume(
                   &checkpoint.workflow_config,
                   resume_context,
                   options,
               )
               .await?;

           // Merge results
           self.merge_results(&checkpoint, &result).await?;

           Ok(result)
       }

       async fn validate_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
           // Verify workflow hasn't changed incompatibly
           // Check file integrity
           // Validate state consistency
           Ok(())
       }
   }
   ```

3. **Checkpoint Manager**:
   ```rust
   pub struct CheckpointManager {
       storage_path: PathBuf,
       checkpoint_interval: Duration,
   }

   impl CheckpointManager {
       pub async fn save_checkpoint(
           &self,
           state: &WorkflowState,
       ) -> Result<()> {
           let checkpoint = self.create_checkpoint(state)?;

           // Atomic write with temp file
           let temp_path = self.get_temp_path(&checkpoint.workflow_id);
           let final_path = self.get_checkpoint_path(&checkpoint.workflow_id);

           // Write to temp
           let json = serde_json::to_string_pretty(&checkpoint)?;
           tokio::fs::write(&temp_path, json).await?;

           // Atomic rename
           tokio::fs::rename(temp_path, final_path).await?;

           Ok(())
       }

       pub async fn auto_checkpoint(&self, state: &WorkflowState) -> Result<()> {
           if state.last_checkpoint.elapsed() > self.checkpoint_interval {
               self.save_checkpoint(state).await?;
           }
           Ok(())
       }
   }
   ```

### Architecture Changes
- Add `CheckpointManager` to execution pipeline
- Enhance `WorkflowExecutor` with resume support
- Modify `MapReduceExecutor` for partial completion
- Add checkpoint commands to CLI
- Implement state restoration logic

### Data Structures
```yaml
# Checkpoint configuration
checkpoint:
  enabled: true
  interval: 60s  # Checkpoint every 60 seconds
  on_step_completion: true  # Also checkpoint after each step
  storage: "~/.prodigy/checkpoints"
  retention: 7d  # Keep checkpoints for 7 days

# Resume options
resume:
  force: false  # Resume even if workflow appears complete
  from_step: null  # Resume from specific step
  reset_failures: false  # Retry previously failed items
```

## Dependencies

- **Prerequisites**: [61 - DLQ Reprocessing] for failure handling
- **Affected Components**:
  - `src/cook/execution/state.rs` - Checkpoint management
  - `src/cook/workflow/executor.rs` - Resume logic
  - `src/cli/resume.rs` - Resume commands
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Checkpoint serialization/deserialization
  - State restoration accuracy
  - Duplicate detection logic
- **Integration Tests**:
  - Full workflow resume cycle
  - MapReduce partial completion
  - Variable state preservation
  - Cross-machine resume
- **Failure Tests**:
  - Corrupted checkpoint handling
  - Incompatible workflow changes
  - Concurrent resume attempts
- **Performance Tests**:
  - Checkpoint overhead measurement
  - Large state serialization
  - Resume startup time

## Documentation Requirements

- **Code Documentation**: Document checkpoint format and resume algorithm
- **User Documentation**:
  - Resume guide with common scenarios
  - Checkpoint configuration reference
  - Troubleshooting failed resumes
- **Architecture Updates**: Add checkpoint/resume flow diagrams

## Implementation Notes

- Use filesystem atomic operations for checkpoint integrity
- Consider checkpoint compression for large states
- Implement checkpoint garbage collection
- Support checkpoint export/import for debugging
- Future: Distributed checkpoint storage for team workflows

## Migration and Compatibility

- No breaking changes to existing workflows
- Workflows without checkpoints can't be resumed
- Old checkpoint formats auto-migrated on load
- Clear error messages for incompatible resumes