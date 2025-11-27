---
number: 188
title: MapReduce Phase-Level Resume
category: parallel
priority: high
status: draft
dependencies: [162, 184, 187]
created: 2025-11-26
---

# Specification 188: MapReduce Phase-Level Resume

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: Spec 162 (MapReduce Incremental Checkpoint System), Spec 184 (Unified Checkpoint System), Spec 187 (Unified Workflow Model)

## Context

MapReduce workflows have three distinct phases with different resume requirements:

```
[Setup Phase] ──────> [Map Phase] ──────> [Reduce Phase]
  Sequential           Parallel            Sequential
  Single worktree      N agent worktrees   Parent worktree
```

Each phase has unique resume characteristics:

| Phase | Execution | Checkpoint Granularity | Resume Semantics |
|-------|-----------|------------------------|------------------|
| Setup | Sequential | Per-step | Retry failed step, skip completed |
| Map | Parallel | Per-item + periodic | Skip completed items, retry failed |
| Reduce | Sequential | Per-step | Retry failed step, skip completed |

### Current Gaps

1. **Setup phase resume**: If setup fails, must restart from beginning
2. **Map phase resume**: Works via Spec 162, but inconsistent with sequential
3. **Reduce phase resume**: Limited checkpoint granularity
4. **Cross-phase resume**: Variable flow between phases not fully preserved
5. **Worktree state**: Agent worktrees may be orphaned on failure

## Objective

Enable reliable resume from any point in a MapReduce workflow:
1. **Setup**: Resume from failed step, preserve captured variables
2. **Map**: Resume with completed items skipped, failed items retried
3. **Reduce**: Resume from failed step, aggregate all map results
4. **Cross-phase**: Preserve variable flow across phase boundaries

## Requirements

### Functional Requirements

#### FR1: Setup Phase Resume
- **MUST** checkpoint after each setup step
- **MUST** preserve captured variables from completed steps
- **MUST** resume from failed step (not restart setup)
- **MUST** make setup variables available to map phase on resume

#### FR2: Map Phase Resume
- **MUST** checkpoint completed items incrementally (per N items)
- **MUST** checkpoint on signal (SIGINT/SIGTERM)
- **MUST** skip completed items on resume
- **MUST** retry failed items (from DLQ) on resume
- **MUST** preserve agent worktrees for incomplete items

#### FR3: Reduce Phase Resume
- **MUST** checkpoint after each reduce step
- **MUST** preserve map results in checkpoint
- **MUST** resume from failed step
- **MUST** aggregate results from pre-interrupt + post-resume

#### FR4: Cross-Phase Variable Preservation
- **MUST** store phase outputs in checkpoint
- **MUST** restore previous phase outputs on resume
- **MUST** make `${setup.*}` available after setup resume
- **MUST** make `${map.*}` available after map resume

#### FR5: Worktree Management on Resume
- **MUST** reuse existing parent worktree on resume
- **MUST** reuse existing agent worktrees for incomplete items
- **MUST** create new agent worktrees for pending items
- **MUST** clean up completed agent worktrees progressively

### Non-Functional Requirements

#### NFR1: Resume Latency
- Resume from setup failure: < 2 seconds
- Resume from map failure: < 5 seconds + time to recreate missing worktrees
- Resume from reduce failure: < 2 seconds

#### NFR2: Data Integrity
- No duplicate item processing
- No lost item results
- Variable state consistent across resume

## Acceptance Criteria

### Setup Phase Resume

- [ ] **AC1**: Resume from failed setup step
  - Setup has 3 steps, step 2 fails
  - Checkpoint shows: setup step 1 completed, step 2 failed
  - Resume: skips step 1, retries step 2
  - Step 3 executes after step 2 succeeds

- [ ] **AC2**: Setup variables preserved on resume
  - Setup step 1 captures `items` variable
  - Step 2 fails
  - Resume: `items` variable available from checkpoint
  - Map phase can use `${setup.items}`

### Map Phase Resume

- [ ] **AC3**: Resume skips completed items
  - Map phase: 20 items, 8 completed before failure
  - Resume: only processes items 9-20
  - Items 1-8 results loaded from checkpoint

- [ ] **AC4**: Resume retries failed items
  - Item 5 failed (in DLQ)
  - Resume with `--include-dlq`: retries item 5
  - Resume without `--include-dlq`: skips item 5

- [ ] **AC5**: Agent worktree reuse
  - Item 10 was in-progress when interrupted
  - Agent worktree for item 10 still exists
  - Resume: reuses worktree, doesn't recreate

- [ ] **AC6**: Incremental checkpoint during map
  - Checkpoint every 5 items (configurable)
  - Interrupt after 17 items
  - Checkpoint has 15 items (3 checkpoints × 5)
  - Items 16-17 treated as pending on resume

### Reduce Phase Resume

- [ ] **AC7**: Resume from failed reduce step
  - Reduce has 3 steps, step 2 fails
  - Resume: skips step 1, retries step 2
  - All map results available via `${map.results}`

- [ ] **AC8**: Map results preserved for reduce resume
  - Map completes (20 items successful)
  - Reduce step 1 succeeds, step 2 fails
  - Resume: 20 map results still available
  - `${map.successful}` = 20

### Cross-Phase Resume

- [ ] **AC9**: Resume into map phase with setup outputs
  - Setup completes, map fails at item 5
  - Resume: setup.captured_vars available
  - Map agents can interpolate setup variables

- [ ] **AC10**: Resume into reduce with full map results
  - Map completes (partial failures)
  - Reduce fails
  - Resume: `${map.successful}`, `${map.failed}`, `${map.results}` correct

### Worktree Lifecycle

- [ ] **AC11**: Parent worktree preserved on failure
  - Failure at any phase
  - Parent worktree still exists
  - Resume uses same parent worktree

- [ ] **AC12**: Agent worktrees cleaned progressively
  - Agent completes successfully
  - Agent worktree merged to parent and cleaned
  - Only incomplete agent worktrees remain

## Technical Details

### Implementation Approach

#### 1. Phase-Aware Checkpoint Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceCheckpoint {
    pub version: u32,
    pub job_id: String,
    pub session_id: SessionId,
    pub workflow_path: PathBuf,

    /// Current phase
    pub current_phase: MapReducePhase,

    /// Setup phase state
    pub setup_state: Option<SetupPhaseState>,

    /// Map phase state
    pub map_state: Option<MapPhaseState>,

    /// Reduce phase state
    pub reduce_state: Option<ReducePhaseState>,

    /// Worktree information
    pub worktree_info: WorktreeInfo,

    /// Checkpoint metadata
    pub created_at: DateTime<Utc>,
    pub checkpoint_reason: CheckpointReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapReducePhase {
    Setup,
    Map,
    Reduce,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhaseState {
    pub status: PhaseStatus,
    pub current_step: Option<usize>,
    pub completed_steps: Vec<CompletedStepRecord>,
    pub captured_variables: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhaseState {
    pub status: PhaseStatus,
    pub total_items: usize,
    pub completed_items: Vec<CompletedItemRecord>,
    pub failed_items: Vec<FailedItemRecord>,
    pub in_progress_items: Vec<InProgressItemRecord>,
    /// Aggregated results from completed items
    pub aggregated_results: AggregatedMapResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhaseState {
    pub status: PhaseStatus,
    pub current_step: Option<usize>,
    pub completed_steps: Vec<CompletedStepRecord>,
    /// Map results snapshot for reduce phase
    pub map_results_snapshot: AggregatedMapResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub parent_worktree: PathBuf,
    pub parent_branch: String,
    pub agent_worktrees: HashMap<usize, AgentWorktreeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentWorktreeInfo {
    pub item_index: usize,
    pub path: PathBuf,
    pub branch: String,
    pub status: AgentStatus,
}
```

#### 2. Phase-Specific Resume Effects

```rust
use stillwater::Effect;

/// Resume MapReduce workflow from checkpoint
pub fn resume_mapreduce(
    checkpoint: MapReduceCheckpoint,
    workflow: MapReduceWorkflow,
) -> Effect<MapReduceResult, MapReduceError, MapReduceEnv> {
    match checkpoint.current_phase {
        MapReducePhase::Setup => resume_from_setup(checkpoint, workflow),
        MapReducePhase::Map => resume_from_map(checkpoint, workflow),
        MapReducePhase::Reduce => resume_from_reduce(checkpoint, workflow),
        MapReducePhase::Complete => Effect::pure(MapReduceResult::AlreadyComplete),
    }
}

/// Resume from setup phase
fn resume_from_setup(
    checkpoint: MapReduceCheckpoint,
    workflow: MapReduceWorkflow,
) -> Effect<MapReduceResult, MapReduceError, MapReduceEnv> {
    let setup_state = checkpoint.setup_state.unwrap_or_default();

    // Restore captured variables
    Effect::local(
        |env| env.with_variables(setup_state.captured_variables.clone()),
        // Resume setup from failed step
        resume_sequential_phase(
            &workflow.setup.steps,
            setup_state.current_step.unwrap_or(0),
            &setup_state.completed_steps,
        )
    )
    // Then continue with map and reduce
    .and_then(|setup_result| {
        execute_map_phase(&workflow.map, &setup_result)
    })
    .and_then(|map_result| {
        execute_reduce_phase(&workflow.reduce, &map_result)
    })
}

/// Resume from map phase
fn resume_from_map(
    checkpoint: MapReduceCheckpoint,
    workflow: MapReduceWorkflow,
) -> Effect<MapReduceResult, MapReduceError, MapReduceEnv> {
    let setup_state = checkpoint.setup_state.unwrap_or_default();
    let map_state = checkpoint.map_state.unwrap_or_default();

    // Restore setup variables
    Effect::local(
        |env| env.with_variables(setup_state.captured_variables.clone()),
        // Resume map phase
        resume_map_phase(
            &workflow.map,
            &map_state.completed_items,
            &map_state.failed_items,
            map_state.aggregated_results,
        )
    )
    .and_then(|map_result| {
        execute_reduce_phase(&workflow.reduce, &map_result)
    })
}

/// Resume from reduce phase
fn resume_from_reduce(
    checkpoint: MapReduceCheckpoint,
    workflow: MapReduceWorkflow,
) -> Effect<MapReduceResult, MapReduceError, MapReduceEnv> {
    let reduce_state = checkpoint.reduce_state.unwrap_or_default();

    // Restore map results for reduce
    Effect::local(
        |env| env.with_map_results(reduce_state.map_results_snapshot.clone()),
        // Resume reduce from failed step
        resume_sequential_phase(
            &workflow.reduce.steps,
            reduce_state.current_step.unwrap_or(0),
            &reduce_state.completed_steps,
        )
    )
    .map(|_| MapReduceResult::Complete)
}
```

#### 3. Resume Map Phase with Item Tracking

```rust
/// Resume map phase: skip completed, retry failed, process pending
fn resume_map_phase(
    config: &MapConfig,
    completed: &[CompletedItemRecord],
    failed: &[FailedItemRecord],
    aggregated: AggregatedMapResults,
) -> Effect<MapPhaseResult, MapPhaseError, MapReduceEnv> {
    // Load all work items
    load_work_items(&config.input, &config.json_path)
        .and_then(move |all_items| {
            let completed_indices: HashSet<_> = completed.iter()
                .map(|c| c.item_index)
                .collect();

            let failed_indices: HashSet<_> = failed.iter()
                .map(|f| f.item_index)
                .collect();

            // Determine which items to process
            get_resume_options()
                .and_then(move |options| {
                    let items_to_process: Vec<_> = all_items.into_iter()
                        .enumerate()
                        .filter(|(idx, _)| {
                            // Skip completed items
                            if completed_indices.contains(idx) {
                                return false;
                            }
                            // Include failed items only if requested
                            if failed_indices.contains(idx) {
                                return options.include_dlq_items;
                            }
                            // Process pending items
                            true
                        })
                        .collect();

                    info!(
                        "Resuming map phase: {} completed, {} failed, {} to process",
                        completed.len(),
                        failed.len(),
                        items_to_process.len()
                    );

                    // Execute remaining items
                    execute_map_items(items_to_process, config.max_parallel)
                })
        })
        // Merge with previously completed results
        .map(move |new_results| {
            AggregatedMapResults::merge(aggregated, new_results)
        })
}
```

#### 4. Worktree Management on Resume

```rust
/// Manage worktrees during resume
fn prepare_worktrees_for_resume(
    checkpoint: &MapReduceCheckpoint,
    items_to_process: &[(usize, WorkItem)],
) -> Effect<WorktreeAllocation, WorktreeError, MapReduceEnv> {
    let existing_worktrees = &checkpoint.worktree_info.agent_worktrees;

    // Categorize items by worktree availability
    let mut reuse = Vec::new();
    let mut create = Vec::new();

    for (idx, item) in items_to_process {
        if let Some(wt_info) = existing_worktrees.get(idx) {
            if wt_info.path.exists() {
                reuse.push((*idx, wt_info.clone()));
            } else {
                create.push((*idx, item.clone()));
            }
        } else {
            create.push((*idx, item.clone()));
        }
    }

    info!(
        "Worktree allocation: {} to reuse, {} to create",
        reuse.len(),
        create.len()
    );

    // Create missing worktrees
    create_worktrees_for_items(create)
        .map(move |new_worktrees| {
            WorktreeAllocation {
                reused: reuse,
                created: new_worktrees,
            }
        })
}

/// Clean up completed agent worktrees progressively
fn cleanup_completed_worktree(
    item_idx: usize,
    worktree: &AgentWorktreeInfo,
) -> Effect<(), WorktreeError, MapReduceEnv> {
    // Merge agent branch to parent
    merge_agent_to_parent(&worktree.branch)
        .and_then(|_| {
            // Delete worktree
            delete_worktree(&worktree.path)
        })
        .tap(|_| {
            info!("Cleaned up worktree for item {}", item_idx);
            // Update checkpoint to reflect cleanup
            update_checkpoint_worktree_removed(item_idx)
        })
}
```

#### 5. Checkpoint Timing

```rust
/// Checkpoint triggers for MapReduce phases
pub struct MapReduceCheckpointTriggers {
    /// Setup: checkpoint after each step
    pub setup_per_step: bool,

    /// Map: checkpoint every N completed items
    pub map_item_interval: usize,

    /// Map: checkpoint every N seconds
    pub map_time_interval: Option<Duration>,

    /// Reduce: checkpoint after each step
    pub reduce_per_step: bool,

    /// Always checkpoint on signal
    pub on_signal: bool,

    /// Always checkpoint at phase transitions
    pub on_phase_transition: bool,
}

impl Default for MapReduceCheckpointTriggers {
    fn default() -> Self {
        Self {
            setup_per_step: true,
            map_item_interval: 5,
            map_time_interval: Some(Duration::from_secs(30)),
            reduce_per_step: true,
            on_signal: true,
            on_phase_transition: true,
        }
    }
}
```

### Resume Flow Diagram

```
                    ┌──────────────────────────────────┐
                    │      Load Checkpoint             │
                    └──────────────┬───────────────────┘
                                   │
                    ┌──────────────┴───────────────────┐
                    │      Determine Current Phase     │
                    └──────────────┬───────────────────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        │                          │                          │
        ▼                          ▼                          ▼
┌───────────────┐       ┌──────────────────┐       ┌───────────────┐
│ Setup Phase   │       │   Map Phase      │       │ Reduce Phase  │
├───────────────┤       ├──────────────────┤       ├───────────────┤
│ Restore vars  │       │ Restore setup    │       │ Restore map   │
│ from checkpoint│       │ vars from ckpt   │       │ results       │
│               │       │                  │       │               │
│ Resume from   │       │ Skip completed   │       │ Resume from   │
│ failed step   │       │ items            │       │ failed step   │
│               │       │                  │       │               │
│ Continue to   │       │ Retry failed     │       │ Complete      │
│ Map phase     │       │ items (optional) │       │ workflow      │
│               │       │                  │       │               │
│               │       │ Process pending  │       │               │
│               │       │ items            │       │               │
│               │       │                  │       │               │
│               │       │ Continue to      │       │               │
│               │       │ Reduce phase     │       │               │
└───────────────┘       └──────────────────┘       └───────────────┘
```

### Variable Flow Across Phases

```
Setup Phase                   Map Phase                    Reduce Phase
────────────                   ─────────                    ────────────
step1 captures               For each item:                Has access to:
  - items                    Has access to:                  - setup.items
  - config                     - setup.items                 - setup.config
                               - setup.config                - map.successful
step2 uses ${items}            - item (current)              - map.failed
                               - item.field                  - map.results
Outputs:                                                     - map.total
  setup.items               Produces:
  setup.config                item_result

                            Aggregates to:
                              map.successful
                              map.failed
                              map.results (array)
                              map.total
```

## Dependencies

### Prerequisites
- **Spec 162**: MapReduce Incremental Checkpoint System
- **Spec 184**: Unified Checkpoint System
- **Spec 187**: Unified Workflow Model

### Affected Components
- MapReduce coordinator
- Phase executors
- Checkpoint manager
- Resume command
- Worktree manager

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_resume_from_setup_failure() {
    let checkpoint = MapReduceCheckpoint {
        current_phase: MapReducePhase::Setup,
        setup_state: Some(SetupPhaseState {
            status: PhaseStatus::Failed,
            current_step: Some(1),
            completed_steps: vec![step_record(0)],
            captured_variables: hashmap! { "items" => json!([1, 2, 3]) },
        }),
        ..
    };

    let plan = plan_mapreduce_resume(&checkpoint, &workflow);

    assert!(matches!(plan.setup_plan, Some(PhasePlan::ResumeSequential { start_step: 1, .. })));
    assert!(matches!(plan.map_plan, PhasePlan::Execute));
    assert!(matches!(plan.reduce_plan, PhasePlan::Execute));
}

#[test]
fn test_resume_from_map_with_completed_items() {
    let checkpoint = MapReduceCheckpoint {
        current_phase: MapReducePhase::Map,
        setup_state: Some(SetupPhaseState {
            status: PhaseStatus::Completed,
            captured_variables: hashmap! { "items" => json!([1,2,3,4,5]) },
            ..
        }),
        map_state: Some(MapPhaseState {
            completed_items: vec![item_record(0), item_record(1), item_record(2)],
            failed_items: vec![],
            ..
        }),
        ..
    };

    let plan = plan_mapreduce_resume(&checkpoint, &workflow);

    assert!(matches!(plan.setup_plan, Some(PhasePlan::Skip { .. })));
    assert!(matches!(plan.map_plan, PhasePlan::ResumeParallel { skip_items, .. } if skip_items.len() == 3));
}

#[test]
fn test_variable_restoration_on_reduce_resume() {
    let checkpoint = MapReduceCheckpoint {
        current_phase: MapReducePhase::Reduce,
        setup_state: Some(SetupPhaseState {
            captured_variables: hashmap! { "config" => json!({"key": "value"}) },
            ..
        }),
        map_state: Some(MapPhaseState {
            aggregated_results: AggregatedMapResults {
                successful: 10,
                failed: 2,
                results: vec![...],
            },
            ..
        }),
        reduce_state: Some(ReducePhaseState {
            current_step: Some(1),
            map_results_snapshot: ..., // Should match map_state
            ..
        }),
        ..
    };

    // Verify map results are available in reduce
    let env = create_env_from_checkpoint(&checkpoint);
    assert_eq!(env.get_variable("map.successful"), Some(json!(10)));
    assert_eq!(env.get_variable("map.failed"), Some(json!(2)));
    assert_eq!(env.get_variable("setup.config.key"), Some(json!("value")));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_mapreduce_resume_cycle() {
    // Create workflow: setup → map(10 items) → reduce
    let workflow = create_test_mapreduce_workflow(10);

    // First run: fail at item 5
    let env = create_env_failing_at_item(5);
    let result = execute_mapreduce(workflow.clone()).run(&env).await;
    assert!(result.is_err());

    // Verify checkpoint state
    let checkpoint = load_checkpoint(&env.job_id).await.unwrap();
    assert_eq!(checkpoint.current_phase, MapReducePhase::Map);
    assert_eq!(checkpoint.map_state.unwrap().completed_items.len(), 5);

    // Resume - should complete successfully
    let fixed_env = create_env_succeeding();
    let result = resume_mapreduce(checkpoint, workflow)
        .run(&fixed_env)
        .await;

    assert!(result.is_ok());

    // Verify only items 6-10 were processed
    let log = fixed_env.execution_log();
    assert!(!log.contains("item-1")); // Skipped
    assert!(log.contains("item-6"));  // Processed
    assert!(log.contains("item-10")); // Processed
}

#[tokio::test]
async fn test_resume_preserves_worktrees() {
    let workflow = create_test_mapreduce_workflow(5);

    // First run: fail at item 3
    let env = create_env_failing_at_item(3);
    execute_mapreduce(workflow.clone()).run(&env).await;

    // Check worktrees exist
    let checkpoint = load_checkpoint(&env.job_id).await.unwrap();
    assert!(checkpoint.worktree_info.parent_worktree.exists());

    // Resume
    let fixed_env = create_env_succeeding();
    resume_mapreduce(checkpoint, workflow).run(&fixed_env).await;

    // Verify same parent worktree used
    assert_eq!(fixed_env.worktree_path, checkpoint.worktree_info.parent_worktree);
}
```

## Documentation Requirements

### Code Documentation
- Document phase-specific checkpoint structures
- Document variable flow between phases
- Document worktree lifecycle during resume

### User Documentation
- Explain MapReduce resume behavior
- Document `--include-dlq-items` option
- Troubleshoot common resume issues

## Migration and Compatibility

### Breaking Changes
None - existing MapReduce workflows continue to work.

### Migration
- Old checkpoint format auto-migrated on load
- Version field enables format evolution

## Success Metrics

### Quantitative
- 100% of MapReduce failures create valid checkpoint
- Resume success rate > 95%
- Zero duplicate item processing
- Worktree reuse rate > 90% on resume

### Qualitative
- Consistent resume behavior across all phases
- Clear progress indication during resume
- Variables preserved correctly across phases
