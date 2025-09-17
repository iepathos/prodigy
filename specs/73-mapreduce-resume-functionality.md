---
number: 73
title: MapReduce Resume Functionality
category: execution
priority: critical
status: draft
dependencies: [72]
created: 2025-01-16
---

# Specification 73: MapReduce Resume Functionality

**Category**: execution
**Priority**: critical
**Status**: draft
**Dependencies**: [72 - Resume with Error Recovery]

## Context

MapReduce workflow resume functionality is currently broken and fails with exit code 1. The resume mechanism fails to properly restore MapReduce job state, including work item distribution, agent coordination, and cross-worktree synchronization. This is critical because MapReduce workflows can run for hours or days, and users must be able to resume interrupted jobs without losing progress.

Key issues with current MapReduce resume:
1. Job state restoration from global storage fails
2. Work item redistribution doesn't account for completed items
3. Agent coordination state is not properly restored
4. Cross-worktree event synchronization breaks during resume
5. DLQ state integration with resume is incomplete

## Objective

Implement robust MapReduce workflow resume functionality that can restore job execution from any interruption point, properly distribute remaining work items across agents, coordinate cross-worktree operations, and maintain full observability during resumed execution.

## Requirements

### Functional Requirements

1. **Job State Restoration**
   - Restore MapReduce job state from global storage checkpoints
   - Rebuild work item queue with remaining items
   - Restore agent state and worktree assignments
   - Recover event log continuity
   - Maintain job correlation IDs

2. **Work Item Management**
   - Calculate remaining work items from checkpoint
   - Redistribute work items to available agents
   - Handle partially completed items
   - Integrate with DLQ for failed item recovery
   - Preserve work item order and priorities

3. **Agent Coordination**
   - Restore agent pool configuration
   - Reassign worktrees to new agent instances
   - Synchronize agent state across resume
   - Handle agent failures during resume
   - Maintain agent-to-worktree mapping

4. **Cross-Worktree Synchronization**
   - Restore event log synchronization
   - Coordinate checkpoint saves across agents
   - Handle worktree cleanup and recreation
   - Maintain shared state consistency
   - Synchronize progress tracking

5. **Phase Resume Support**
   - Resume from any MapReduce phase (setup, map, reduce)
   - Handle partial phase completion
   - Restore phase-specific state
   - Maintain phase transition logic
   - Support phase-level error recovery

### Non-Functional Requirements

1. **Reliability**
   - Resume succeeds consistently across different interruption points
   - No work item loss during resume operations
   - Consistent state across all agents and worktrees
   - Robust handling of environment changes

2. **Performance**
   - Fast job state restoration (< 30 seconds for large jobs)
   - Efficient work item redistribution
   - Minimal overhead for resume preparation
   - Scalable to thousands of work items

3. **Observability**
   - Clear logging of resume operations
   - Progress tracking during resume
   - Resume performance metrics
   - Cross-worktree resume coordination visibility

## Acceptance Criteria

- [ ] MapReduce workflows resume successfully from any interruption point
- [ ] Work item distribution correctly accounts for completed items
- [ ] Agent coordination state is properly restored
- [ ] Cross-worktree event synchronization works during resume
- [ ] DLQ items can be reprocessed as part of resume
- [ ] Resume from setup, map, and reduce phases works correctly
- [ ] Large jobs (1000+ items) resume within 30 seconds
- [ ] Resume handles agent failures gracefully
- [ ] Progress tracking continues seamlessly after resume
- [ ] Event log integrity is maintained across resume operations

## Technical Details

### Implementation Approach

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceResumeState {
    pub job_id: String,
    pub current_phase: MapReducePhase,
    pub completed_items: HashSet<String>,
    pub failed_items: Vec<WorkItem>,
    pub agent_assignments: HashMap<String, String>, // agent_id -> worktree_path
    pub phase_results: HashMap<MapReducePhase, PhaseResult>,
    pub resume_metadata: ResumeMetadata,
}

#[derive(Debug, Clone)]
pub struct MapReduceResumeManager {
    storage: Arc<GlobalStorage>,
    event_manager: Arc<EventManager>,
    agent_coordinator: Arc<AgentCoordinator>,
    checkpoint_manager: Arc<CheckpointManager>,
}

impl MapReduceResumeManager {
    pub async fn resume_job(
        &self,
        job_id: &str,
        resume_options: ResumeOptions,
    ) -> MapReduceResult<ResumeResult> {
        // Load job state from global storage
        let job_state = self.load_job_state(job_id).await?;

        // Validate resume feasibility
        self.validate_resume_conditions(&job_state, &resume_options).await?;

        // Restore work item queue
        let remaining_items = self.calculate_remaining_items(&job_state).await?;

        // Rebuild agent coordination
        let agent_pool = self.restore_agent_pool(&job_state, &resume_options).await?;

        // Resume from appropriate phase
        match job_state.current_phase {
            MapReducePhase::Setup => self.resume_from_setup(&job_state, remaining_items).await,
            MapReducePhase::Map => self.resume_from_map(&job_state, remaining_items, agent_pool).await,
            MapReducePhase::Reduce => self.resume_from_reduce(&job_state).await,
        }
    }

    async fn calculate_remaining_items(
        &self,
        job_state: &MapReduceResumeState,
    ) -> MapReduceResult<Vec<WorkItem>> {
        // Load original work items
        let original_items = self.load_original_work_items(&job_state.job_id).await?;

        // Filter out completed items
        let remaining: Vec<WorkItem> = original_items
            .into_iter()
            .filter(|item| !job_state.completed_items.contains(&item.id))
            .collect();

        // Add failed items from DLQ for retry
        let dlq_items = self.load_dlq_items(&job_state.job_id).await?;
        let mut all_remaining = remaining;
        all_remaining.extend(dlq_items);

        // Apply resume-specific filters and ordering
        self.apply_resume_filters(all_remaining, job_state).await
    }

    async fn restore_agent_pool(
        &self,
        job_state: &MapReduceResumeState,
        options: &ResumeOptions,
    ) -> MapReduceResult<AgentPool> {
        let config = AgentPoolConfig {
            max_parallel: options.max_parallel.unwrap_or(job_state.original_max_parallel),
            worktree_management: WorktreeManagement::Reuse,
            existing_assignments: job_state.agent_assignments.clone(),
        };

        // Create new agent pool with existing worktree assignments
        let mut pool = AgentPool::new(config)?;

        // Restore agent state from checkpoints
        for (agent_id, worktree_path) in &job_state.agent_assignments {
            if self.validate_worktree_state(worktree_path).await? {
                pool.restore_agent(agent_id, worktree_path).await?;
            } else {
                // Recreate worktree if corrupted
                pool.recreate_agent_worktree(agent_id).await?;
            }
        }

        Ok(pool)
    }

    async fn resume_from_map(
        &self,
        job_state: &MapReduceResumeState,
        remaining_items: Vec<WorkItem>,
        agent_pool: AgentPool,
    ) -> MapReduceResult<ResumeResult> {
        // Create MapReduce executor with resume state
        let executor = MapReduceExecutor::from_resume_state(
            job_state,
            agent_pool,
            self.event_manager.clone(),
        )?;

        // Setup event log continuation
        self.setup_event_log_continuation(&job_state.job_id).await?;

        // Execute remaining map phase
        let map_result = executor.execute_map_phase(remaining_items).await?;

        // Continue to reduce phase or complete
        if let Some(reduce_config) = &job_state.workflow.reduce {
            executor.execute_reduce_phase(reduce_config, map_result).await
        } else {
            Ok(ResumeResult::MapOnlyCompleted(map_result))
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResumeOptions {
    pub max_parallel: Option<usize>,
    pub force_recreation: bool,
    pub include_dlq_items: bool,
    pub validate_environment: bool,
    pub reset_failed_agents: bool,
}
```

### Architecture Changes

1. **Resume State Management**
   - New `MapReduceResumeState` structure for comprehensive state capture
   - Resume state persistence in global storage
   - Cross-worktree state synchronization

2. **Agent Pool Restoration**
   - Enhanced `AgentPool` with resume capabilities
   - Worktree validation and recreation logic
   - Agent state restoration from checkpoints

3. **Event Log Continuation**
   - Event log resumption without gaps
   - Cross-worktree event synchronization
   - Correlation ID preservation across resume

4. **Work Item Management**
   - Remaining item calculation with DLQ integration
   - Work item redistribution algorithms
   - Progress tracking continuation

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapReducePhase {
    Setup,
    Map,
    Reduce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: MapReducePhase,
    pub completed_at: DateTime<Utc>,
    pub success: bool,
    pub items_processed: usize,
    pub output: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeMetadata {
    pub original_start_time: DateTime<Utc>,
    pub last_checkpoint_time: DateTime<Utc>,
    pub resume_attempts: u32,
    pub interruption_reason: Option<String>,
    pub environment_snapshot: EnvironmentSnapshot,
}

#[derive(Debug, Clone)]
pub enum ResumeResult {
    MapOnlyCompleted(MapResult),
    FullWorkflowCompleted(MapReduceResult),
    PartialResume { phase: MapReducePhase, progress: f64 },
}
```

### Integration Points

1. **Global Storage Integration**
   - Enhanced checkpoint save/load for MapReduce state
   - Cross-worktree state aggregation
   - Resume state validation

2. **DLQ Integration**
   - Failed item recovery during resume
   - DLQ state synchronization
   - Automatic retry of failed items

3. **Event System Integration**
   - Event log continuation across resume
   - Cross-worktree event aggregation
   - Resume event correlation

## Dependencies

- **Prerequisites**: [72 - Resume with Error Recovery]
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/coordinators/agent_pool.rs`
  - `src/storage/global.rs`
  - `src/cook/execution/events/`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Job state restoration logic
  - Work item redistribution algorithms
  - Agent pool restoration
  - Event log continuation

- **Integration Tests**:
  - End-to-end MapReduce resume scenarios
  - Cross-worktree resume coordination
  - Large-scale resume performance
  - DLQ integration during resume

- **Edge Cases**:
  - Resume with corrupted worktrees
  - Agent failures during resume
  - Environment changes between runs
  - Network partitions during resume

- **Performance Tests**:
  - Resume time for large jobs (1000+ items)
  - Memory usage during resume
  - Scalability of resume operations

## Documentation Requirements

- **Code Documentation**:
  - MapReduce resume architecture
  - Agent coordination during resume
  - Event synchronization mechanisms

- **User Documentation**:
  - MapReduce resume troubleshooting
  - Performance tuning for large job resume
  - Environment requirements for resume

- **Architecture Updates**:
  - Resume flow diagrams
  - Cross-worktree coordination
  - State management architecture

## Implementation Notes

1. **State Validation**: Comprehensive validation of resume state before attempting resume
2. **Graceful Degradation**: Fallback options when full resume isn't possible
3. **Progress Preservation**: Maintain all progress indicators across resume
4. **Performance**: Optimize for fast resume of large jobs
5. **Observability**: Detailed logging and metrics for resume operations

## Migration and Compatibility

- Backward compatible with existing MapReduce checkpoints
- Automatic migration of legacy job state
- Graceful handling of unsupported resume scenarios
- Progressive feature rollout with compatibility checks