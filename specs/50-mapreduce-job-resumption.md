---
number: 50
title: MapReduce Job Resumption Capability
category: foundation
priority: critical
status: draft
dependencies: [49]
created: 2025-01-29
---

# Specification 50: MapReduce Job Resumption Capability

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: [49 - MapReduce Persistent State and Checkpointing]

## Context

With persistent state management in place (Spec 49), we need the ability to resume MapReduce jobs from their last checkpoint. This enables recovery from failures, allows pausing/resuming long-running jobs, and provides resilience for distributed execution. Users currently lose all progress when a job fails, requiring complete re-execution.

## Objective

Implement comprehensive job resumption capabilities that allow MapReduce jobs to continue from their last successful checkpoint, intelligently handling partial completion and maintaining execution consistency.

## Requirements

### Functional Requirements
- Resume job from latest valid checkpoint
- Identify and skip already-completed work items
- Re-queue failed items for retry
- Restore execution context and environment
- Maintain job ID continuity across resumption
- Support both automatic and manual resume
- Handle checkpoint corruption gracefully
- Merge previously completed results

### Non-Functional Requirements
- Resume operation completes within 5 seconds for 1000 agents
- Zero data loss for completed agents
- Idempotent resumption (safe to call multiple times)
- Support resuming jobs up to 7 days old
- Handle concurrent resume attempts safely

## Acceptance Criteria

- [ ] `mmm resume-job <job-id>` command implemented
- [ ] Automatic resume on crash recovery implemented
- [ ] Completed agents skipped on resume
- [ ] Failed agents retried with updated retry count
- [ ] Execution continues from exact interruption point
- [ ] Parent worktree state restored correctly
- [ ] Resume works across different MMM versions
- [ ] Progress bar shows resumed state accurately
- [ ] Event log tracks resume operations
- [ ] Resume validates checkpoint integrity

## Technical Details

### Implementation Approach

1. **Resume Command**
```rust
#[derive(Parser)]
pub struct ResumeCommand {
    /// Job ID to resume
    job_id: String,
    
    /// Force resume even if job appears complete
    #[arg(long)]
    force: bool,
    
    /// Maximum additional retries for failed items
    #[arg(long, default_value = "2")]
    max_retries: u32,
}
```

2. **Resume Logic**
```rust
impl MapReduceExecutor {
    pub async fn resume_job(&self, job_id: &str, options: ResumeOptions) -> Result<Vec<AgentResult>> {
        // Load checkpoint
        let state = self.checkpoint_manager.load_latest(job_id).await?;
        
        // Validate checkpoint integrity
        self.validate_checkpoint(&state)?;
        
        // Restore execution environment
        let env = self.restore_environment(&state).await?;
        
        // Identify remaining work
        let pending = self.calculate_pending_items(&state)?;
        
        // Resume execution
        self.execute_with_state(state, pending, env).await
    }
    
    async fn calculate_pending_items(&self, state: &MapReduceJobState) -> Result<Vec<WorkItem>> {
        let mut pending = Vec::new();
        
        // Add never-attempted items
        for item in &state.work_items {
            let id = Self::extract_item_identifier(item);
            if !state.completed_agents.contains(&id) && 
               !state.failed_agents.contains_key(&id) {
                pending.push(WorkItem::new(item.clone()));
            }
        }
        
        // Add retriable failed items
        for (id, failure) in &state.failed_agents {
            if failure.attempts < self.max_retries {
                if let Some(item) = state.find_work_item(id) {
                    pending.push(WorkItem::retry(item, failure.attempts));
                }
            }
        }
        
        pending
    }
}
```

3. **State Restoration**
```rust
struct RestoredState {
    pub job_state: MapReduceJobState,
    pub execution_env: ExecutionEnvironment,
    pub parent_worktree: Option<WorktreeSession>,
    pub existing_results: Vec<AgentResult>,
}

impl StateRestorer {
    async fn restore(&self, checkpoint: &MapReduceJobState) -> Result<RestoredState> {
        // Restore parent worktree if exists
        let parent_worktree = if let Some(wt_name) = &checkpoint.parent_worktree {
            Some(self.restore_worktree(wt_name).await?)
        } else {
            None
        };
        
        // Rebuild execution environment
        let env = self.rebuild_environment(checkpoint)?;
        
        // Load existing results
        let results = self.load_agent_results(checkpoint).await?;
        
        Ok(RestoredState {
            job_state: checkpoint.clone(),
            execution_env: env,
            parent_worktree,
            existing_results: results,
        })
    }
}
```

### Architecture Changes
- Add `ResumeCommand` to CLI structure
- Extend `MapReduceExecutor` with resume methods
- Add `StateRestorer` component
- Integrate with crash recovery system

### Data Structures
```rust
pub struct ResumeOptions {
    pub force: bool,
    pub max_additional_retries: u32,
    pub skip_validation: bool,
}

pub struct ResumeResult {
    pub job_id: String,
    pub resumed_from_version: u32,
    pub total_items: usize,
    pub already_completed: usize,
    pub remaining_items: usize,
    pub final_results: Vec<AgentResult>,
}
```

### APIs and Interfaces
```rust
pub trait Resumable {
    async fn can_resume(&self, job_id: &str) -> Result<bool>;
    async fn resume(&self, job_id: &str, options: ResumeOptions) -> Result<ResumeResult>;
    async fn list_resumable_jobs(&self) -> Result<Vec<ResumableJob>>;
}
```

## Dependencies

- **Prerequisites**: [49 - MapReduce Persistent State and Checkpointing]
- **Affected Components**: 
  - `src/cook/execution/mapreduce.rs`
  - `src/main.rs` (CLI commands)
  - `src/cook/command.rs`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Test pending item calculation
  - Verify state restoration logic
  - Test retry count management
  - Validate checkpoint integrity checks
  
- **Integration Tests**: 
  - Resume job after simulated crash
  - Resume with mix of completed/failed agents
  - Test resume with corrupted checkpoint
  - Verify idempotent resume
  
- **Performance Tests**: 
  - Resume job with 1000+ agents
  - Measure resume initialization time
  - Test memory usage during resume
  
- **User Acceptance**: 
  - Resume interrupted workflow
  - View resume progress in UI
  - Handle resume failures gracefully

## Documentation Requirements

- **Code Documentation**: 
  - Document resume algorithm
  - Explain state restoration process
  - Document retry logic for failed items
  
- **User Documentation**: 
  - Add resume command to CLI docs
  - Create troubleshooting guide
  - Document resume limitations
  
- **Architecture Updates**: 
  - Add resume flow diagram
  - Document state machine for resumption

## Implementation Notes

- Use optimistic locking for concurrent resume prevention
- Implement health checks before resume
- Consider checkpoint migration for version compatibility
- Add dry-run mode for resume validation
- Log all resume decisions for debugging
- Implement circuit breaker for repeatedly failing items

## Migration and Compatibility

- Support resuming jobs from previous MMM versions
- Graceful handling of missing checkpoint data
- Automatic checkpoint format migration
- Backward compatible CLI interface
- Clear error messages for incompatible checkpoints