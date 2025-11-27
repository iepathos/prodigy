---
number: 187
title: Unified Workflow Model
category: foundation
priority: high
status: draft
dependencies: [183, 184, 186]
created: 2025-11-26
---

# Specification 187: Unified Workflow Model

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 183 (Effect-Based Workflow Execution), Spec 184 (Unified Checkpoint System), Spec 186 (Non-MapReduce Workflow Resume)

## Context

Prodigy currently has two separate workflow execution paths:

1. **Sequential workflows**: Linear step execution via `WorkflowExecutor`
2. **MapReduce workflows**: Three-phase execution via `MapReduceCoordinator`

This separation leads to:
- Duplicated logic for command execution, variables, error handling
- Inconsistent checkpoint/resume behavior
- Different code paths to maintain
- Difficulty adding new workflow patterns

### Architectural Insight

Both workflow types share a common structure when viewed as **phases**:

```
Sequential Workflow:
  Phase 1 (sequential): [Step1, Step2, Step3, Step4]

MapReduce Workflow:
  Phase 1 (sequential): Setup [Step1, Step2]
  Phase 2 (parallel):   Map [AgentTemplate] × N work items
  Phase 3 (sequential): Reduce [Step1, Step2]
```

The key difference is that MapReduce's map phase executes the same template **in parallel across work items**, while sequential workflows execute steps **one at a time**.

## Objective

Create a **unified workflow model** where:
1. All workflows are compositions of **phases**
2. Each phase is either **sequential** or **parallel** (over work items)
3. Checkpoint/resume works consistently across all phases
4. The Effect-based infrastructure (Spec 183) handles both execution modes
5. New workflow patterns can be added by composing phases

## Requirements

### Functional Requirements

#### FR1: Phase-Based Workflow Model
- **MUST** model all workflows as a sequence of phases
- **MUST** support sequential phases (steps run one at a time)
- **MUST** support parallel phases (steps run across work items)
- **MUST** support mixed workflows (sequential + parallel phases)

#### FR2: Unified Step Execution
- **MUST** use same step execution logic for all phase types
- **MUST** support same command types (claude, shell, test, foreach)
- **MUST** support same error handlers (on_failure, on_success)
- **MUST** support same variable interpolation

#### FR3: Phase-Level Checkpointing
- **MUST** checkpoint at phase boundaries
- **MUST** checkpoint progress within phases (per-step or per-item)
- **MUST** support resume from any phase
- **MUST** support resume from within a phase

#### FR4: Variable Flow Between Phases
- **MUST** pass variables from phase N to phase N+1
- **MUST** aggregate variables from parallel phases (semigroup pattern)
- **MUST** make previous phase results available via `${phase_name.var}`

### Non-Functional Requirements

#### NFR1: Backward Compatibility
- Existing sequential workflow YAMLs MUST work unchanged
- Existing MapReduce workflow YAMLs MUST work unchanged

#### NFR2: Extensibility
- New phase types should be easy to add
- Custom aggregation strategies should be pluggable

## Acceptance Criteria

### Unified Model

- [ ] **AC1**: Sequential workflow modeled as single phase
  - Workflow with 5 steps
  - Internally represented as: `[Phase { mode: Sequential, steps: [1,2,3,4,5] }]`

- [ ] **AC2**: MapReduce workflow modeled as three phases
  - MapReduce workflow with setup, map, reduce
  - Internally represented as:
    ```
    [
      Phase { name: "setup", mode: Sequential, steps: [...] },
      Phase { name: "map", mode: Parallel(work_items), steps: agent_template },
      Phase { name: "reduce", mode: Sequential, steps: [...] },
    ]
    ```

- [ ] **AC3**: Hybrid workflow supported
  - Workflow with: setup → parallel processing → intermediate → more parallel → finalize
  - Multiple parallel phases with sequential phases between them

### Resume Consistency

- [ ] **AC4**: Resume works identically for sequential phase
  - Sequential workflow fails at step 3
  - Resume retries step 3, skips 1-2
  - Same behavior whether standalone or within MapReduce setup phase

- [ ] **AC5**: Resume works for parallel phase
  - Map phase fails on item 7/20
  - Resume processes items 7-20 (retries 7, skips 1-6)
  - Completed items preserved

- [ ] **AC6**: Resume from specific phase
  - MapReduce workflow fails in reduce phase
  - Resume skips setup, skips map, retries reduce
  - Setup and map results loaded from checkpoint

### Variable Flow

- [ ] **AC7**: Sequential to parallel variable flow
  - Setup phase captures `items` variable
  - Map phase accesses `${setup.items}` for work items

- [ ] **AC8**: Parallel to sequential variable aggregation
  - Map phase agents produce `result` variable each
  - Reduce phase accesses `${map.results}` (aggregated)

## Technical Details

### Implementation Approach

#### 1. Core Data Structures

```rust
/// Unified workflow representation
#[derive(Debug, Clone)]
pub struct Workflow {
    pub name: String,
    pub phases: Vec<Phase>,
    pub global_env: HashMap<String, String>,
}

/// A phase is a unit of execution with a mode
#[derive(Debug, Clone)]
pub struct Phase {
    /// Phase identifier (e.g., "setup", "map", "reduce", or auto-generated)
    pub name: String,
    /// Steps to execute in this phase
    pub steps: Vec<WorkflowStep>,
    /// How this phase executes
    pub mode: PhaseMode,
    /// Checkpoint strategy for this phase
    pub checkpoint_strategy: CheckpointStrategy,
    /// Variables from previous phases available here
    pub input_variables: Vec<String>,
    /// Variables this phase produces
    pub output_variables: Vec<String>,
    /// Phase-level timeout
    pub timeout: Option<Duration>,
}

/// How a phase executes its steps
#[derive(Debug, Clone)]
pub enum PhaseMode {
    /// Execute steps one at a time
    Sequential,
    /// Execute steps in parallel across work items
    Parallel {
        /// Source of work items (file path, variable, or inline)
        work_items: WorkItemSource,
        /// Maximum concurrent executions
        max_parallel: usize,
        /// How to handle item failures
        failure_policy: FailurePolicy,
    },
}

/// Source of work items for parallel phases
#[derive(Debug, Clone)]
pub enum WorkItemSource {
    /// Read from JSON file with JSONPath
    JsonFile { path: String, json_path: String },
    /// Read from variable (from previous phase)
    Variable { name: String, json_path: Option<String> },
    /// Inline list
    Inline(Vec<serde_json::Value>),
}

/// When to create checkpoints within a phase
#[derive(Debug, Clone)]
pub enum CheckpointStrategy {
    /// Checkpoint after every step (sequential phases)
    PerStep,
    /// Checkpoint after every N items (parallel phases)
    PerNItems(usize),
    /// Checkpoint on time interval
    TimeInterval(Duration),
    /// Only checkpoint at phase boundaries
    PhaseOnly,
}

/// How to handle failures in parallel phases
#[derive(Debug, Clone)]
pub enum FailurePolicy {
    /// Stop on first failure
    FailFast,
    /// Continue processing, collect failures
    ContinueOnFailure { max_failures: Option<usize> },
    /// Retry failed items up to N times before failing
    RetryThenFail { max_retries: u32 },
}
```

#### 2. Workflow Normalization

Convert existing YAML formats to unified model:

```rust
/// Convert sequential workflow to unified model
pub fn normalize_sequential(config: &WorkflowConfig) -> Workflow {
    Workflow {
        name: config.name.clone().unwrap_or_default(),
        phases: vec![Phase {
            name: "main".to_string(),
            steps: normalize_steps(&config.commands),
            mode: PhaseMode::Sequential,
            checkpoint_strategy: CheckpointStrategy::PerStep,
            input_variables: vec![],
            output_variables: vec![],
            timeout: None,
        }],
        global_env: config.env.clone().unwrap_or_default(),
    }
}

/// Convert MapReduce workflow to unified model
pub fn normalize_mapreduce(config: &MapReduceWorkflowConfig) -> Workflow {
    let mut phases = Vec::new();

    // Setup phase (optional)
    if let Some(setup) = &config.setup {
        phases.push(Phase {
            name: "setup".to_string(),
            steps: normalize_steps(&setup.commands),
            mode: PhaseMode::Sequential,
            checkpoint_strategy: CheckpointStrategy::PerStep,
            input_variables: vec![],
            output_variables: setup.capture_outputs.keys().cloned().collect(),
            timeout: setup.timeout.map(Duration::from_secs),
        });
    }

    // Map phase (required)
    phases.push(Phase {
        name: "map".to_string(),
        steps: normalize_steps(&config.map.agent_template),
        mode: PhaseMode::Parallel {
            work_items: WorkItemSource::JsonFile {
                path: config.map.input.clone(),
                json_path: config.map.json_path.clone(),
            },
            max_parallel: config.map.max_parallel.unwrap_or(10),
            failure_policy: normalize_failure_policy(&config.map),
        },
        checkpoint_strategy: CheckpointStrategy::PerNItems(5),
        input_variables: vec!["setup.*".to_string()],
        output_variables: vec!["results".to_string()],
        timeout: config.map.agent_timeout.map(Duration::from_secs),
    });

    // Reduce phase (optional)
    if let Some(reduce) = &config.reduce {
        phases.push(Phase {
            name: "reduce".to_string(),
            steps: normalize_steps(&reduce.commands),
            mode: PhaseMode::Sequential,
            checkpoint_strategy: CheckpointStrategy::PerStep,
            input_variables: vec!["map.*".to_string()],
            output_variables: vec![],
            timeout: reduce.timeout.map(Duration::from_secs),
        });
    }

    Workflow {
        name: config.name.clone(),
        phases,
        global_env: config.env.clone().unwrap_or_default(),
    }
}
```

#### 3. Unified Phase Execution

```rust
use stillwater::Effect;

/// Execute a single phase (either sequential or parallel)
pub fn execute_phase(phase: &Phase) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    match &phase.mode {
        PhaseMode::Sequential => execute_sequential_phase(phase),
        PhaseMode::Parallel { work_items, max_parallel, failure_policy } => {
            execute_parallel_phase(phase, work_items, *max_parallel, failure_policy)
        }
    }
    .context(format!("Phase: {}", phase.name))
}

/// Execute sequential phase (same as current sequential workflow)
fn execute_sequential_phase(phase: &Phase) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    phase.steps.iter()
        .enumerate()
        .fold(
            Effect::pure(PhaseProgress::new(&phase.name)),
            |acc, (idx, step)| {
                acc.and_then(move |progress| {
                    with_step_checkpoint(&phase.name, idx, step)
                        .map(|result| progress.with_step_result(idx, result))
                })
            },
        )
        .and_then(|progress| save_phase_checkpoint(&phase.name, progress.clone()).map(|_| progress))
        .map(|progress| progress.into_result())
}

/// Execute parallel phase (same as current MapReduce map phase)
fn execute_parallel_phase(
    phase: &Phase,
    work_items: &WorkItemSource,
    max_parallel: usize,
    failure_policy: &FailurePolicy,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    // Load work items
    load_work_items(work_items)
        .and_then(move |items| {
            // Create an effect for each work item
            let item_effects: Vec<_> = items.into_iter()
                .enumerate()
                .map(|(idx, item)| {
                    execute_item_steps(&phase.steps, idx, item)
                        .or_else(move |err| handle_item_failure(idx, err, failure_policy))
                })
                .collect();

            // Execute with bounded parallelism
            Effect::par_all_limit(item_effects, max_parallel)
                .map_err(|errors| PhaseError::ParallelFailures(errors))
        })
        .and_then(|results| aggregate_results(results))
        .and_then(|aggregated| {
            save_phase_checkpoint(&phase.name, aggregated.clone())
                .map(|_| aggregated)
        })
}

/// Execute steps for a single work item (agent)
fn execute_item_steps(
    steps: &[WorkflowStep],
    item_idx: usize,
    item: WorkItem,
) -> Effect<ItemResult, ItemError, WorkflowEnv> {
    // Create isolated environment for this item
    Effect::local(
        move |env| env.with_item_context(item_idx, &item),
        steps.iter()
            .enumerate()
            .fold(
                Effect::pure(ItemProgress::new(item_idx)),
                |acc, (step_idx, step)| {
                    acc.and_then(move |progress| {
                        with_item_checkpoint(item_idx, step_idx, step)
                            .map(|result| progress.with_step_result(step_idx, result))
                    })
                },
            )
            .map(|progress| progress.into_result()),
    )
}
```

#### 4. Unified Workflow Execution

```rust
/// Execute entire workflow as sequence of phases
pub fn execute_workflow(workflow: &Workflow) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
    workflow.phases.iter()
        .enumerate()
        .fold(
            Effect::pure(WorkflowProgress::new(&workflow.name)),
            |acc, (phase_idx, phase)| {
                acc.and_then(move |progress| {
                    // Check if phase should be skipped (from checkpoint)
                    should_skip_phase(phase_idx)
                        .and_then(move |skip| {
                            if skip {
                                load_phase_result_from_checkpoint(phase_idx)
                                    .map(|result| progress.with_phase_result(phase_idx, result))
                            } else {
                                // Pass previous phase outputs to this phase
                                let prev_outputs = progress.latest_outputs();
                                Effect::local(
                                    move |env| env.with_phase_inputs(prev_outputs),
                                    execute_phase(phase)
                                )
                                .map(|result| progress.with_phase_result(phase_idx, result))
                            }
                        })
                })
            },
        )
        .map(|progress| progress.into_result())
}
```

#### 5. Unified Checkpoint Structure

```rust
/// Checkpoint for unified workflow model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    pub version: u32,
    pub workflow_name: String,
    pub session_id: SessionId,

    /// Phase-level progress
    pub phase_progress: Vec<PhaseCheckpoint>,

    /// Current phase index
    pub current_phase: usize,

    /// Global variables accumulated across phases
    pub global_variables: HashMap<String, Value>,

    /// Workflow metadata
    pub workflow_path: PathBuf,
    pub worktree_path: PathBuf,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseCheckpoint {
    pub phase_name: String,
    pub status: PhaseStatus,

    /// For sequential phases: which step we're on
    pub current_step: Option<usize>,
    pub completed_steps: Vec<CompletedStepRecord>,

    /// For parallel phases: which items completed
    pub completed_items: Vec<CompletedItemRecord>,
    pub failed_items: Vec<FailedItemRecord>,
    pub pending_items: Vec<usize>,

    /// Variables produced by this phase
    pub output_variables: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseStatus {
    NotStarted,
    InProgress,
    Completed,
    Failed { error: String },
}
```

#### 6. Resume Planning

```rust
/// Pure function: plan resume from checkpoint
pub fn plan_workflow_resume(
    checkpoint: &WorkflowCheckpoint,
    workflow: &Workflow,
) -> WorkflowResumePlan {
    let mut phase_plans = Vec::new();

    for (idx, phase) in workflow.phases.iter().enumerate() {
        let phase_checkpoint = checkpoint.phase_progress.get(idx);

        let phase_plan = match phase_checkpoint {
            None => PhasePlan::Execute, // Phase not started
            Some(pc) => match &pc.status {
                PhaseStatus::Completed => PhasePlan::Skip {
                    restore_outputs: pc.output_variables.clone(),
                },
                PhaseStatus::NotStarted => PhasePlan::Execute,
                PhaseStatus::InProgress => {
                    match &phase.mode {
                        PhaseMode::Sequential => PhasePlan::ResumeSequential {
                            start_step: pc.current_step.unwrap_or(0),
                            skip_steps: pc.completed_steps.iter().map(|s| s.step_index).collect(),
                        },
                        PhaseMode::Parallel { .. } => PhasePlan::ResumeParallel {
                            skip_items: pc.completed_items.iter().map(|i| i.item_idx).collect(),
                            retry_items: pc.failed_items.iter().map(|i| i.item_idx).collect(),
                        },
                    }
                }
                PhaseStatus::Failed { .. } => {
                    // Retry the failed phase from where it failed
                    match &phase.mode {
                        PhaseMode::Sequential => PhasePlan::ResumeSequential {
                            start_step: pc.current_step.unwrap_or(0),
                            skip_steps: pc.completed_steps.iter().map(|s| s.step_index).collect(),
                        },
                        PhaseMode::Parallel { .. } => PhasePlan::ResumeParallel {
                            skip_items: pc.completed_items.iter().map(|i| i.item_idx).collect(),
                            retry_items: pc.failed_items.iter().map(|i| i.item_idx).collect(),
                        },
                    }
                }
            },
        };

        phase_plans.push(phase_plan);
    }

    WorkflowResumePlan { phase_plans }
}

#[derive(Debug, Clone)]
pub enum PhasePlan {
    /// Skip this phase entirely, restore outputs from checkpoint
    Skip { restore_outputs: HashMap<String, Value> },
    /// Execute this phase from the beginning
    Execute,
    /// Resume sequential phase from specific step
    ResumeSequential {
        start_step: usize,
        skip_steps: HashSet<usize>,
    },
    /// Resume parallel phase with specific items
    ResumeParallel {
        skip_items: HashSet<usize>,
        retry_items: HashSet<usize>,
    },
}
```

### Architecture Changes

#### Directory Structure

```
src/cook/
├── workflow/
│   ├── model/                    # Unified workflow model
│   │   ├── mod.rs
│   │   ├── workflow.rs           # Workflow, Phase, PhaseMode
│   │   ├── normalization.rs      # YAML → unified model
│   │   └── checkpoint.rs         # WorkflowCheckpoint
│   ├── execution/                # Phase execution
│   │   ├── mod.rs
│   │   ├── sequential.rs         # Sequential phase execution
│   │   ├── parallel.rs           # Parallel phase execution
│   │   └── workflow.rs           # Full workflow orchestration
│   ├── resume/                   # Resume planning and execution
│   │   ├── mod.rs
│   │   ├── planning.rs           # Pure resume planning
│   │   └── execution.rs          # Resume orchestration
│   └── effects/                  # Effect-based operations
│       ├── step.rs
│       ├── checkpoint.rs
│       └── variables.rs
└── execution/
    └── mapreduce/                # Gradually migrate to use workflow/
```

### Migration Path

1. **Phase 1**: Create unified model structures
2. **Phase 2**: Implement normalization for both workflow types
3. **Phase 3**: Implement unified phase execution (sequential first)
4. **Phase 4**: Migrate parallel execution to use unified model
5. **Phase 5**: Unify checkpoint format
6. **Phase 6**: Unify resume logic
7. **Phase 7**: Deprecate separate code paths

## Dependencies

### Prerequisites
- **Spec 183**: Effect-Based Workflow Execution
- **Spec 184**: Unified Checkpoint System
- **Spec 186**: Non-MapReduce Workflow Resume

### Affected Components
- Workflow configuration parsing
- Sequential workflow execution
- MapReduce workflow execution
- Checkpoint management
- Resume command

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_normalize_sequential_workflow() {
    let config = parse_yaml(r#"
        commands:
          - shell: "step1"
          - claude: "/step2"
    "#);

    let workflow = normalize_sequential(&config);

    assert_eq!(workflow.phases.len(), 1);
    assert_eq!(workflow.phases[0].name, "main");
    assert!(matches!(workflow.phases[0].mode, PhaseMode::Sequential));
    assert_eq!(workflow.phases[0].steps.len(), 2);
}

#[test]
fn test_normalize_mapreduce_workflow() {
    let config = parse_yaml(r#"
        mode: mapreduce
        setup:
          commands: [{ shell: "prepare" }]
        map:
          input: "items.json"
          json_path: "$.items[*]"
          agent_template: [{ shell: "process ${item}" }]
        reduce:
          commands: [{ shell: "aggregate" }]
    "#);

    let workflow = normalize_mapreduce(&config);

    assert_eq!(workflow.phases.len(), 3);
    assert_eq!(workflow.phases[0].name, "setup");
    assert_eq!(workflow.phases[1].name, "map");
    assert_eq!(workflow.phases[2].name, "reduce");
    assert!(matches!(workflow.phases[1].mode, PhaseMode::Parallel { .. }));
}

#[test]
fn test_resume_plan_skips_completed_phases() {
    let checkpoint = WorkflowCheckpoint {
        phase_progress: vec![
            PhaseCheckpoint { status: PhaseStatus::Completed, .. },
            PhaseCheckpoint { status: PhaseStatus::InProgress, .. },
            PhaseCheckpoint { status: PhaseStatus::NotStarted, .. },
        ],
        ..
    };

    let plan = plan_workflow_resume(&checkpoint, &workflow);

    assert!(matches!(plan.phase_plans[0], PhasePlan::Skip { .. }));
    assert!(matches!(plan.phase_plans[1], PhasePlan::ResumeSequential { .. } | PhasePlan::ResumeParallel { .. }));
    assert!(matches!(plan.phase_plans[2], PhasePlan::Execute));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_mapreduce_resume_from_map_phase() {
    // Create workflow that fails mid-map
    let workflow = create_mapreduce_workflow_failing_at_item(10, 5);

    // First run - fails at item 5
    let result = execute_workflow(&workflow).run(&env).await;
    assert!(result.is_err());

    // Load checkpoint
    let checkpoint = load_checkpoint(&session_id).await.unwrap();
    assert_eq!(checkpoint.current_phase, 1); // Map phase
    assert_eq!(checkpoint.phase_progress[0].status, PhaseStatus::Completed); // Setup done
    assert_eq!(checkpoint.phase_progress[1].completed_items.len(), 5);

    // Resume - should skip setup, resume map
    let plan = plan_workflow_resume(&checkpoint, &workflow);
    assert!(matches!(plan.phase_plans[0], PhasePlan::Skip { .. }));
    assert!(matches!(plan.phase_plans[1], PhasePlan::ResumeParallel { skip_items, .. } if skip_items.len() == 5));
}
```

## Documentation Requirements

### Code Documentation
- Document unified workflow model
- Document phase modes and their semantics
- Document checkpoint structure
- Document resume planning algorithm

### User Documentation
- Explain phase concept for advanced users
- Document hybrid workflow patterns
- Explain checkpoint behavior per phase type

## Migration and Compatibility

### Breaking Changes
None - existing YAML formats continue to work via normalization layer.

### Compatibility
- Old workflows automatically normalized to new model
- Old checkpoints migrated on load (version field)
- CLI unchanged

## Success Metrics

### Quantitative
- Single code path for step execution (eliminate duplication)
- Resume works for 100% of phase transitions
- < 5% performance overhead from normalization

### Qualitative
- Cleaner architecture
- Easier to add new workflow patterns
- Consistent resume behavior
