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
  - Setup phase captures `items` variable (JSON array)
  - Map phase accesses `${setup.items}` via `WorkItemSource::Variable`
  - **Success**: Map phase processes all items from setup.items
  - **Failure cases**:
    - Error if `${setup.items}` is undefined: `PhaseError::MissingInput`
    - Error if `${setup.items}` is not an array: `PhaseError::TypeMismatch`
    - Error if JSONPath fails to match: `PhaseError::JSONPathError`

- [ ] **AC8**: Parallel to sequential variable aggregation
  - Map phase agents each produce `result` variable
  - Reduce phase accesses `${map.results}` (aggregated via declared strategy)
  - **Success**: Reduce receives aggregated results (e.g., array if strategy=Collect)
  - **Failure cases**:
    - Error if agents produce heterogeneous types without declared aggregation
    - ALL type mismatches reported via `Validation::Failure` (not just first)
    - Example: Agent 1 returns number, Agent 2 returns string → both errors reported

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
    /// Aggregation configuration for parallel phases (optional)
    pub aggregations: Vec<VariableAggregation>,
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

/// Variable aggregation configuration for phase outputs
#[derive(Debug, Clone)]
pub struct VariableAggregation {
    /// Variable name to aggregate
    pub name: String,
    /// Aggregation strategy (from Spec 171)
    pub strategy: AggregationStrategy,
    /// Initial value for aggregation
    pub initial: Option<Value>,
}

/// Aggregation strategies for combining results across parallel agents
#[derive(Debug, Clone)]
pub enum AggregationStrategy {
    /// Collect all values into an array (default for undeclared variables)
    Collect,
    /// Count occurrences
    Count,
    /// Sum numeric values
    Sum,
    /// Find minimum value
    Min,
    /// Find maximum value
    Max,
    /// Calculate average
    Average,
    /// Calculate median
    Median,
    /// Calculate standard deviation
    StdDev,
    /// Calculate variance
    Variance,
    /// Collect unique values only
    Unique,
    /// Concatenate strings
    Concat { separator: Option<String> },
    /// Merge objects (first value wins for duplicate keys)
    Merge,
    /// Flatten nested arrays
    Flatten,
    /// Sort values
    Sort { descending: bool },
    /// Group values by key
    GroupBy { key_field: String },
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

/// Validation for workflow normalization
fn validate_workflow_config(config: &WorkflowConfig) -> Validation<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check for empty commands
    if config.commands.is_empty() {
        errors.push(ValidationError::EmptyWorkflow {
            reason: "Workflow must have at least one command".to_string(),
        });
    }

    // Check for reserved phase names
    let reserved_names = ["setup", "map", "reduce", "main"];
    if let Some(name) = &config.name {
        if reserved_names.contains(&name.as_str()) {
            errors.push(ValidationError::ReservedName {
                name: name.clone(),
                reserved: reserved_names.iter().map(|s| s.to_string()).collect(),
            });
        }
    }

    // Validate command structure
    for (idx, cmd) in config.commands.iter().enumerate() {
        if is_empty_command(cmd) {
            errors.push(ValidationError::EmptyCommand { step_index: idx });
        }
    }

    if errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(errors)
    }
}

/// Validation for MapReduce workflow normalization
fn validate_mapreduce_config(config: &MapReduceWorkflowConfig) -> Validation<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Map phase is required
    if config.map.agent_template.is_empty() {
        errors.push(ValidationError::MissingRequiredPhase {
            phase: "map".to_string(),
            reason: "MapReduce workflow must have map phase with agent template".to_string(),
        });
    }

    // Validate input source
    if config.map.input.is_empty() {
        errors.push(ValidationError::InvalidPhaseConfig {
            phase: "map".to_string(),
            reason: "Map phase must specify input file".to_string(),
        });
    }

    // Check for phase name conflicts
    let mut phase_names = HashSet::new();

    if config.setup.is_some() {
        phase_names.insert("setup");
    }
    phase_names.insert("map");
    if config.reduce.is_some() {
        phase_names.insert("reduce");
    }

    // Check aggregation variable names don't conflict
    if let Some(aggregations) = &config.map.aggregations {
        let mut agg_names = HashSet::new();
        for agg in aggregations {
            if !agg_names.insert(&agg.name) {
                errors.push(ValidationError::DuplicateAggregation {
                    variable: agg.name.clone(),
                });
            }
        }
    }

    // Validate max_parallel is reasonable
    if let Some(max) = config.map.max_parallel {
        if max == 0 {
            errors.push(ValidationError::InvalidPhaseConfig {
                phase: "map".to_string(),
                reason: "max_parallel must be at least 1".to_string(),
            });
        } else if max > 1000 {
            errors.push(ValidationError::InvalidPhaseConfig {
                phase: "map".to_string(),
                reason: "max_parallel should not exceed 1000 (too many concurrent agents)".to_string(),
            });
        }
    }

    if errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(errors)
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    EmptyWorkflow { reason: String },
    EmptyCommand { step_index: usize },
    ReservedName { name: String, reserved: Vec<String> },
    MissingRequiredPhase { phase: String, reason: String },
    InvalidPhaseConfig { phase: String, reason: String },
    DuplicateAggregation { variable: String },
}

/// Normalize with validation
pub fn normalize_and_validate_sequential(
    config: &WorkflowConfig,
) -> Result<Workflow, Vec<ValidationError>> {
    validate_workflow_config(config)
        .to_result()
        .map(|_| normalize_sequential(config))
}

pub fn normalize_and_validate_mapreduce(
    config: &MapReduceWorkflowConfig,
) -> Result<Workflow, Vec<ValidationError>> {
    validate_mapreduce_config(config)
        .to_result()
        .map(|_| normalize_mapreduce(config))
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
                    execute_step_with_checkpoint(&phase.name, idx, step, progress)
                })
            },
        )
        .and_then(|progress| save_phase_checkpoint(&phase.name, progress.clone()).map(|_| progress))
        .map(|progress| progress.into_result())
}

/// Execute a single step with checkpointing (extracted for clarity and testability)
fn execute_step_with_checkpoint(
    phase_name: &str,
    step_idx: usize,
    step: &WorkflowStep,
    mut progress: PhaseProgress,
) -> Effect<PhaseProgress, PhaseError, WorkflowEnv> {
    // Execute the step
    execute_step(step)
        .context(format!("Step {}: {}", step_idx, step.description()))
        .and_then(move |result| {
            // Update progress
            progress.add_step_result(step_idx, result.clone());

            // Checkpoint based on strategy
            should_checkpoint_at_step(phase_name, step_idx)
                .and_then(|should_checkpoint| {
                    if should_checkpoint {
                        save_step_checkpoint(phase_name, step_idx, &progress)
                            .map(|_| progress)
                    } else {
                        Effect::pure(progress)
                    }
                })
        })
}

/// Execute a single workflow step
fn execute_step(step: &WorkflowStep) -> Effect<StepResult, StepError, WorkflowEnv> {
    match step {
        WorkflowStep::Claude { prompt, .. } => execute_claude_command(prompt),
        WorkflowStep::Shell { command, .. } => execute_shell_command(command),
        WorkflowStep::Test { .. } => execute_test_command(step),
        WorkflowStep::Foreach { .. } => execute_foreach_command(step),
    }
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
                    execute_item_with_failure_policy(
                        &phase.steps,
                        idx,
                        item,
                        failure_policy,
                    )
                })
                .collect();

            // Execute with bounded parallelism based on failure policy
            execute_parallel_with_policy(item_effects, max_parallel, failure_policy)
        })
        .and_then(|results| {
            // Checkpoint partial results before aggregation
            checkpoint_parallel_results(&phase.name, &results)
                .map(|_| results)
        })
        .and_then(|results| aggregate_results(results))
        .and_then(|aggregated| {
            save_phase_checkpoint(&phase.name, aggregated.clone())
                .map(|_| aggregated)
        })
}

/// Execute item with failure policy handling
fn execute_item_with_failure_policy(
    steps: &[WorkflowStep],
    idx: usize,
    item: WorkItem,
    policy: &FailurePolicy,
) -> Effect<ItemResult, ItemError, WorkflowEnv> {
    match policy {
        FailurePolicy::FailFast => {
            // No retry, fail immediately
            execute_item_steps(steps, idx, item)
        }
        FailurePolicy::ContinueOnFailure { .. } => {
            // Convert error to partial result
            execute_item_steps(steps, idx, item)
                .or_else(move |err| {
                    Effect::pure(ItemResult::Failed {
                        item_idx: idx,
                        error: err,
                    })
                })
        }
        FailurePolicy::RetryThenFail { max_retries } => {
            // Retry with exponential backoff
            execute_item_with_retry(steps, idx, item, *max_retries)
        }
    }
}

/// Execute parallel effects with failure policy
fn execute_parallel_with_policy(
    effects: Vec<Effect<ItemResult, ItemError, WorkflowEnv>>,
    max_parallel: usize,
    policy: &FailurePolicy,
) -> Effect<Vec<ItemResult>, PhaseError, WorkflowEnv> {
    match policy {
        FailurePolicy::FailFast => {
            // par_all_limit short-circuits on first error
            Effect::par_all_limit(effects, max_parallel)
                .map_err(|errors| PhaseError::ParallelFailures {
                    errors,
                    completed: vec![], // No results preserved on fail-fast
                })
        }
        FailurePolicy::ContinueOnFailure { max_failures } => {
            // Collect all results (successes and failures)
            Effect::par_all_limit(effects, max_parallel)
                .map(move |results| {
                    let failures: Vec<_> = results.iter()
                        .filter_map(|r| match r {
                            ItemResult::Failed { item_idx, error } =>
                                Some((*item_idx, error.clone())),
                            _ => None,
                        })
                        .collect();

                    // Check if we exceeded max failures
                    if let Some(max) = max_failures {
                        if failures.len() > *max {
                            return Err(PhaseError::TooManyFailures {
                                failures,
                                max_allowed: *max,
                                completed: results.iter()
                                    .filter(|r| matches!(r, ItemResult::Success { .. }))
                                    .cloned()
                                    .collect(),
                            });
                        }
                    }

                    Ok(results)
                })
                .and_then(|res| res.into())
        }
        FailurePolicy::RetryThenFail { .. } => {
            // Retries handled per-item, so this is like FailFast after retries
            Effect::par_all_limit(effects, max_parallel)
                .map_err(|errors| PhaseError::ParallelFailures {
                    errors,
                    completed: vec![],
                })
        }
    }
}

/// Execute item with exponential backoff retry
fn execute_item_with_retry(
    steps: &[WorkflowStep],
    idx: usize,
    item: WorkItem,
    max_retries: u32,
) -> Effect<ItemResult, ItemError, WorkflowEnv> {
    let mut attempt = 0;

    Effect::retry(
        max_retries,
        move || {
            attempt += 1;
            execute_item_steps(steps, idx, item.clone())
                .context(format!("Attempt {}/{}", attempt, max_retries + 1))
        },
        |error| {
            // Exponential backoff: 1s, 2s, 4s, 8s, ...
            let delay = Duration::from_secs(2u64.pow(attempt - 1));
            Effect::sleep(delay).map(|_| ())
        },
    )
}

#[derive(Debug, Clone)]
pub enum ItemResult {
    Success {
        item_idx: usize,
        variables: HashMap<String, Value>,
    },
    Failed {
        item_idx: usize,
        error: ItemError,
    },
}

#[derive(Debug, Clone)]
pub enum PhaseError {
    /// Parallel execution failed (fail-fast mode)
    ParallelFailures {
        errors: Vec<ItemError>,
        completed: Vec<ItemResult>,
    },
    /// Too many failures in continue-on-failure mode
    TooManyFailures {
        failures: Vec<(usize, ItemError)>,
        max_allowed: usize,
        completed: Vec<ItemResult>,
    },
    /// Aggregation errors (from Spec 171 validation)
    MultipleAggregationErrors(Vec<PhaseError>),
    /// Single aggregation error
    AggregationError {
        variable: String,
        reason: String,
    },
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

/// Aggregate results from parallel phase using Stillwater Validation
fn aggregate_results(results: Vec<ItemResult>) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    use prodigy::cook::execution::variables::semigroup::{
        AggregateResult, aggregate_map_results, aggregate_with_initial
    };
    use stillwater::Validation;

    // Get aggregation configuration from environment
    Effect::asks(|env: &WorkflowEnv| env.phase_aggregations())
        .and_then(move |aggregations| {
            // Group results by variable name
            let mut variable_groups: HashMap<String, Vec<Value>> = HashMap::new();

            for result in &results {
                for (var_name, value) in &result.variables {
                    variable_groups.entry(var_name.clone())
                        .or_insert_with(Vec::new)
                        .push(value.clone());
                }
            }

            // Aggregate each variable according to its strategy
            let mut aggregated = HashMap::new();
            let mut all_errors = Vec::new();

            for (var_name, values) in variable_groups {
                // Find aggregation config or use default (Collect)
                let strategy = aggregations.iter()
                    .find(|a| a.name == var_name)
                    .map(|a| &a.strategy)
                    .unwrap_or(&AggregationStrategy::Collect);

                // Convert values to AggregateResult based on strategy
                let aggregate_results: Vec<AggregateResult> = values.into_iter()
                    .map(|v| value_to_aggregate_result(v, strategy))
                    .collect();

                // Use homogeneous validation to ensure all types match
                match aggregate_map_results(aggregate_results) {
                    Validation::Success(combined) => {
                        aggregated.insert(var_name, combined.finalize());
                    }
                    Validation::Failure(errors) => {
                        // Type mismatch detected - accumulate ALL errors
                        all_errors.extend(errors.into_iter().map(|e|
                            PhaseError::AggregationError {
                                variable: var_name.clone(),
                                reason: e,
                            }
                        ));
                    }
                }
            }

            // Return success or ALL accumulated errors
            if all_errors.is_empty() {
                Effect::pure(PhaseResult {
                    output_variables: aggregated,
                    status: PhaseStatus::Completed,
                })
            } else {
                Effect::fail(PhaseError::MultipleAggregationErrors(all_errors))
            }
        })
}

/// Convert JSON value to AggregateResult based on strategy
fn value_to_aggregate_result(value: Value, strategy: &AggregationStrategy) -> AggregateResult {
    use AggregateResult::*;

    match strategy {
        AggregationStrategy::Count => Count(1),
        AggregationStrategy::Sum => {
            if let Some(n) = value.as_f64() {
                Sum(n)
            } else {
                Sum(0.0) // Type error will be caught by validation
            }
        }
        AggregationStrategy::Min => Min(value),
        AggregationStrategy::Max => Max(value),
        AggregationStrategy::Average => {
            if let Some(n) = value.as_f64() {
                Average(n, 1)
            } else {
                Average(0.0, 0) // Type error will be caught by validation
            }
        }
        AggregationStrategy::Median => Median(vec![value]),
        AggregationStrategy::StdDev => StdDev(vec![value]),
        AggregationStrategy::Variance => Variance(vec![value]),
        AggregationStrategy::Unique => Unique(HashSet::from([value])),
        AggregationStrategy::Concat { separator } => {
            if let Some(s) = value.as_str() {
                Concat(s.to_string(), separator.clone())
            } else {
                Concat(String::new(), separator.clone())
            }
        }
        AggregationStrategy::Merge => {
            if let Some(obj) = value.as_object() {
                Merge(obj.clone())
            } else {
                Merge(serde_json::Map::new())
            }
        }
        AggregationStrategy::Flatten => Flatten(vec![value]),
        AggregationStrategy::Sort { descending } => Sort(vec![value], *descending),
        AggregationStrategy::GroupBy { key_field } => {
            GroupBy(vec![value], key_field.clone())
        }
        AggregationStrategy::Collect => Collect(vec![value]),
    }
}
```

**Default Aggregation Behavior:**

1. **Declared variables**: Use specified `AggregationStrategy` from phase config
2. **Undeclared variables**: Default to `AggregationStrategy::Collect` (all values in array)
3. **Type mismatches**: Stillwater's `Validation` accumulates ALL errors before failing
4. **Empty results**: Return `None` or empty collection based on strategy

**Example YAML configuration:**

```yaml
map:
  input: "items.json"
  json_path: "$.items[*]"

  # Declare aggregation strategies
  aggregations:
    - name: total_count
      strategy: count
      initial: 0

    - name: total_size
      strategy: sum
      initial: 0

    - name: all_tags
      strategy: unique
      initial: []

    - name: results
      strategy: collect  # Explicit, but this is the default

  agent_template:
    - claude: "/process ${item}"
      capture:
        total_count: 1
        total_size: "${item.size}"
        all_tags: "${item.tags}"
        results: "${output}"
```

**Error handling:**

```rust
// If agents produce different types for the same variable:
// Agent 1: {"score": 42}        (Number)
// Agent 2: {"score": "high"}     (String)

// Validation accumulates ALL type mismatches:
Validation::Failure([
    AggregationError {
        variable: "score",
        reason: "Expected Number, got String at index 1"
    }
])
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

#### 7. Resume Plan Execution

```rust
use stillwater::Effect;

/// Execute a resume plan (Effect-based, not pure)
pub fn execute_resume_plan(
    plan: &WorkflowResumePlan,
    workflow: &Workflow,
) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
    workflow.phases.iter()
        .zip(plan.phase_plans.iter())
        .enumerate()
        .fold(
            Effect::pure(WorkflowProgress::new(&workflow.name)),
            |acc, (phase_idx, (phase, phase_plan))| {
                acc.and_then(move |progress| {
                    execute_phase_with_plan(phase, phase_plan, phase_idx)
                        .map(|result| progress.with_phase_result(phase_idx, result))
                })
            },
        )
        .map(|progress| progress.into_result())
}

/// Execute a single phase according to its plan
fn execute_phase_with_plan(
    phase: &Phase,
    plan: &PhasePlan,
    phase_idx: usize,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    match plan {
        PhasePlan::Skip { restore_outputs } => {
            // Pure restoration from checkpoint
            Effect::pure(PhaseResult {
                phase_name: phase.name.clone(),
                output_variables: restore_outputs.clone(),
                status: PhaseStatus::Completed,
            })
        }
        PhasePlan::Execute => {
            // Full execution from beginning
            execute_phase(phase)
        }
        PhasePlan::ResumeSequential { start_step, skip_steps } => {
            // Resume sequential phase with step skipping
            execute_sequential_phase_resume(phase, *start_step, skip_steps)
        }
        PhasePlan::ResumeParallel { skip_items, retry_items } => {
            // Resume parallel phase with item filtering
            execute_parallel_phase_resume(phase, skip_items, retry_items)
        }
    }
    .context(format!("Phase {}: {}", phase_idx, phase.name))
}

/// Resume sequential phase from specific step
fn execute_sequential_phase_resume(
    phase: &Phase,
    start_step: usize,
    skip_steps: &HashSet<usize>,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    // Load previous progress from checkpoint
    load_phase_checkpoint(&phase.name)
        .and_then(move |checkpoint| {
            // Execute only steps that need re-execution
            phase.steps.iter()
                .enumerate()
                .filter(|(idx, _)| *idx >= start_step && !skip_steps.contains(idx))
                .fold(
                    Effect::pure(checkpoint),
                    |acc, (idx, step)| {
                        acc.and_then(move |progress| {
                            execute_step_with_checkpoint(&phase.name, idx, step, progress)
                        })
                    },
                )
        })
        .map(|progress| progress.into_result())
}

/// Resume parallel phase with specific items
fn execute_parallel_phase_resume(
    phase: &Phase,
    skip_items: &HashSet<usize>,
    retry_items: &HashSet<usize>,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    // Load work items and filter based on resume plan
    load_work_items_for_phase(phase)
        .and_then(move |items| {
            // Create effects only for items that need processing
            let item_effects: Vec<_> = items.into_iter()
                .enumerate()
                .filter(|(idx, _)| !skip_items.contains(idx))
                .map(|(idx, item)| {
                    let is_retry = retry_items.contains(&idx);
                    execute_item_steps(&phase.steps, idx, item)
                        .context(format!("Item {} ({})", idx, if is_retry { "retry" } else { "new" }))
                })
                .collect();

            // Load previous results for skipped items
            load_phase_checkpoint(&phase.name)
                .and_then(move |checkpoint| {
                    // Execute pending items in parallel
                    let max_parallel = extract_max_parallel(phase);
                    Effect::par_all_limit(item_effects, max_parallel)
                        .map(move |new_results| {
                            // Merge new results with checkpointed results
                            merge_partial_results(checkpoint, new_results)
                        })
                })
        })
        .and_then(|merged| {
            save_phase_checkpoint(&phase.name, merged.clone())
                .map(|_| merged.into_result())
        })
}
```

#### 8. Checkpoint Migration Strategy

```rust
/// Checkpoint version history
pub const CHECKPOINT_VERSION_LEGACY_SEQUENTIAL: u32 = 1;
pub const CHECKPOINT_VERSION_LEGACY_MAPREDUCE: u32 = 2;
pub const CHECKPOINT_VERSION_UNIFIED: u32 = 3;

/// Load checkpoint with automatic migration
pub fn load_checkpoint_with_migration(
    session_id: &SessionId,
) -> Effect<WorkflowCheckpoint, CheckpointError, StorageEnv> {
    load_raw_checkpoint(session_id)
        .and_then(|raw| {
            // Detect version and migrate if needed
            migrate_checkpoint(raw)
        })
        .and_then(|migrated| {
            // Validate migrated checkpoint
            validate_checkpoint(&migrated)
                .map_err(|errors| CheckpointError::InvalidCheckpoint(errors))
                .to_result()
                .map(|_| migrated)
        })
}

/// Migrate checkpoint to latest version
fn migrate_checkpoint(raw: RawCheckpoint) -> Effect<WorkflowCheckpoint, CheckpointError, StorageEnv> {
    match raw.version {
        CHECKPOINT_VERSION_UNIFIED => {
            // Already latest version
            Effect::pure(raw.into_unified())
        }
        CHECKPOINT_VERSION_LEGACY_SEQUENTIAL => {
            // Migrate v1 (sequential) → v3 (unified)
            migrate_sequential_checkpoint(raw)
                .and_then(|unified| {
                    // Save migrated checkpoint
                    save_checkpoint(&unified)
                        .map(|_| unified)
                })
        }
        CHECKPOINT_VERSION_LEGACY_MAPREDUCE => {
            // Migrate v2 (mapreduce) → v3 (unified)
            migrate_mapreduce_checkpoint(raw)
                .and_then(|unified| {
                    save_checkpoint(&unified)
                        .map(|_| unified)
                })
        }
        unknown => Effect::fail(CheckpointError::UnsupportedVersion(unknown)),
    }
}

/// Migrate legacy sequential checkpoint
fn migrate_sequential_checkpoint(raw: RawCheckpoint) -> Effect<WorkflowCheckpoint, CheckpointError, StorageEnv> {
    Effect::pure(WorkflowCheckpoint {
        version: CHECKPOINT_VERSION_UNIFIED,
        workflow_name: raw.workflow_name,
        session_id: raw.session_id,
        current_phase: 0,
        phase_progress: vec![PhaseCheckpoint {
            phase_name: "main".to_string(),
            status: migrate_status(&raw.status),
            current_step: raw.current_step,
            completed_steps: raw.completed_steps
                .into_iter()
                .map(|step_idx| CompletedStepRecord {
                    step_index: step_idx,
                    output: None,
                })
                .collect(),
            completed_items: vec![],
            failed_items: vec![],
            pending_items: vec![],
            output_variables: raw.variables,
        }],
        global_variables: raw.global_env,
        workflow_path: raw.workflow_path,
        worktree_path: raw.worktree_path,
        created_at: raw.created_at,
    })
}

/// Migrate legacy MapReduce checkpoint
fn migrate_mapreduce_checkpoint(raw: RawCheckpoint) -> Effect<WorkflowCheckpoint, CheckpointError, StorageEnv> {
    let mut phase_progress = Vec::new();
    let mut current_phase = 0;

    // Setup phase (if exists)
    if let Some(setup) = raw.setup {
        phase_progress.push(PhaseCheckpoint {
            phase_name: "setup".to_string(),
            status: PhaseStatus::Completed,
            current_step: None,
            completed_steps: setup.completed_steps
                .into_iter()
                .map(|idx| CompletedStepRecord { step_index: idx, output: None })
                .collect(),
            completed_items: vec![],
            failed_items: vec![],
            pending_items: vec![],
            output_variables: setup.captured_outputs,
        });
        current_phase = 1;
    }

    // Map phase
    let map_status = if raw.map.all_completed {
        PhaseStatus::Completed
    } else if raw.map.has_failures {
        PhaseStatus::Failed { error: "Some items failed".to_string() }
    } else {
        PhaseStatus::InProgress
    };

    phase_progress.push(PhaseCheckpoint {
        phase_name: "map".to_string(),
        status: map_status,
        current_step: None,
        completed_steps: vec![],
        completed_items: raw.map.completed_items
            .into_iter()
            .map(|(idx, result)| CompletedItemRecord {
                item_idx: idx,
                result,
                worktree: None,
            })
            .collect(),
        failed_items: raw.map.failed_items
            .into_iter()
            .map(|(idx, error)| FailedItemRecord {
                item_idx: idx,
                error,
                retry_count: 0,
            })
            .collect(),
        pending_items: raw.map.pending_items,
        output_variables: HashMap::from([
            ("results".to_string(), aggregate_map_results(&raw.map.completed_items)),
            ("successful".to_string(), json!(raw.map.completed_items.len())),
            ("total".to_string(), json!(raw.map.total_items)),
        ]),
    });

    if !matches!(map_status, PhaseStatus::Completed) {
        current_phase = 1;
    }

    // Reduce phase (if exists)
    if let Some(reduce) = raw.reduce {
        phase_progress.push(PhaseCheckpoint {
            phase_name: "reduce".to_string(),
            status: migrate_status(&reduce.status),
            current_step: reduce.current_step,
            completed_steps: reduce.completed_steps
                .into_iter()
                .map(|idx| CompletedStepRecord { step_index: idx, output: None })
                .collect(),
            completed_items: vec![],
            failed_items: vec![],
            pending_items: vec![],
            output_variables: HashMap::new(),
        });
        if !matches!(reduce.status, "completed") {
            current_phase = 2;
        }
    }

    Effect::pure(WorkflowCheckpoint {
        version: CHECKPOINT_VERSION_UNIFIED,
        workflow_name: raw.workflow_name,
        session_id: raw.session_id,
        current_phase,
        phase_progress,
        global_variables: raw.global_env,
        workflow_path: raw.workflow_path,
        worktree_path: raw.worktree_path,
        created_at: raw.created_at,
    })
}

/// Validate migrated checkpoint structure
fn validate_checkpoint(checkpoint: &WorkflowCheckpoint) -> Validation<(), Vec<CheckpointValidationError>> {
    let mut errors = Vec::new();

    // Check version
    if checkpoint.version != CHECKPOINT_VERSION_UNIFIED {
        errors.push(CheckpointValidationError::InvalidVersion(checkpoint.version));
    }

    // Check phase count
    if checkpoint.phase_progress.is_empty() {
        errors.push(CheckpointValidationError::NoPhases);
    }

    // Check current phase index
    if checkpoint.current_phase >= checkpoint.phase_progress.len() {
        errors.push(CheckpointValidationError::InvalidCurrentPhase {
            current: checkpoint.current_phase,
            total: checkpoint.phase_progress.len(),
        });
    }

    // Validate each phase
    for (idx, phase) in checkpoint.phase_progress.iter().enumerate() {
        // Check phase name
        if phase.phase_name.is_empty() {
            errors.push(CheckpointValidationError::EmptyPhaseName { phase_idx: idx });
        }

        // Check consistency between status and progress
        match &phase.status {
            PhaseStatus::Completed => {
                if !phase.pending_items.is_empty() {
                    errors.push(CheckpointValidationError::InconsistentPhaseState {
                        phase_idx: idx,
                        reason: "Completed phase has pending items".to_string(),
                    });
                }
            }
            PhaseStatus::InProgress => {
                // Must have either current_step or pending/completed items
                if phase.current_step.is_none()
                    && phase.pending_items.is_empty()
                    && phase.completed_items.is_empty() {
                    errors.push(CheckpointValidationError::InconsistentPhaseState {
                        phase_idx: idx,
                        reason: "InProgress phase has no progress markers".to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    if errors.is_empty() {
        Validation::Success(())
    } else {
        Validation::Failure(errors)
    }
}

#[derive(Debug, Clone)]
pub enum CheckpointValidationError {
    InvalidVersion(u32),
    NoPhases,
    InvalidCurrentPhase { current: usize, total: usize },
    EmptyPhaseName { phase_idx: usize },
    InconsistentPhaseState { phase_idx: usize, reason: String },
}

/// Rollback strategy: keep original checkpoint as backup
pub fn save_checkpoint_with_backup(
    checkpoint: &WorkflowCheckpoint,
) -> Effect<(), CheckpointError, StorageEnv> {
    let checkpoint_path = get_checkpoint_path(&checkpoint.session_id);
    let backup_path = checkpoint_path.with_extension("backup");

    // If checkpoint exists, back it up first
    file_exists(&checkpoint_path)
        .and_then(move |exists| {
            if exists {
                copy_file(&checkpoint_path, &backup_path)
            } else {
                Effect::pure(())
            }
        })
        .and_then(move |_| {
            // Write new checkpoint
            write_checkpoint(&checkpoint_path, checkpoint)
                .map_err(|err| {
                    // On failure, restore from backup if it exists
                    CheckpointError::WriteFailed {
                        path: checkpoint_path.clone(),
                        error: err,
                        backup_available: backup_path.exists(),
                    }
                })
        })
}
```

#### 9. Stillwater Integration Patterns

This spec builds heavily on Stillwater patterns from Specs 168, 175, and 176. Here's how they integrate:

**Context Error Preservation (Spec 168):**

```rust
use stillwater::ContextError;

/// Phase execution preserves full error context
pub fn execute_phase(phase: &Phase) -> Effect<PhaseResult, ContextError<PhaseError>, WorkflowEnv> {
    match &phase.mode {
        PhaseMode::Sequential => execute_sequential_phase(phase),
        PhaseMode::Parallel { .. } => execute_parallel_phase(phase, ...),
    }
    .with_context(|| format!("Phase '{}' ({})", phase.name, phase.mode.description()))
}

/// Step execution adds context at each level
fn execute_step_with_checkpoint(
    phase_name: &str,
    step_idx: usize,
    step: &WorkflowStep,
    progress: PhaseProgress,
) -> Effect<PhaseProgress, ContextError<StepError>, WorkflowEnv> {
    execute_step(step)
        .with_context(|| format!("Step {} in phase '{}'", step_idx, phase_name))
        .with_context(|| format!("Command: {}", step.command_summary()))
        .and_then(|result| {
            progress.add_step_result(step_idx, result);
            save_step_checkpoint(phase_name, step_idx, &progress)
                .with_context(|| "Saving step checkpoint")
                .map(|_| progress)
        })
}

/// Full error trace example:
/// Phase 'map' (Parallel, 10 workers)
///   └─ Item 7
///      └─ Step 2 in phase 'map'
///         └─ Command: claude "/process ${item}"
///            └─ Claude execution failed
///               └─ Saving step checkpoint
///                  └─ I/O error: disk full
```

**Reader Pattern Environment Access (Spec 175):**

```rust
use prodigy::cook::execution::mapreduce::environment_helpers::*;

/// Access phase configuration from environment
fn execute_phase_with_env(phase: &Phase) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    // Get max_parallel from environment
    get_max_parallel()
        .and_then(|max| {
            // Use local override for memory-intensive phase
            let adjusted_max = if phase.is_memory_intensive() {
                max / 2
            } else {
                max
            };

            with_max_parallel(adjusted_max, execute_parallel_phase(phase, ...))
        })
}

/// Access phase inputs and outputs
fn execute_phase_isolated(
    phase: &Phase,
    inputs: HashMap<String, Value>,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    // Create isolated environment with phase inputs
    Effect::local(
        |env| env.with_phase_inputs(inputs.clone()),
        // Execute phase with scoped inputs
        execute_phase(phase)
            .and_then(|result| {
                // Make outputs available to next phase
                set_phase_outputs(&phase.name, result.output_variables.clone())
                    .map(|_| result)
            })
    )
}

/// Compose environment access with other effects
fn aggregate_results_with_config(
    results: Vec<ItemResult>,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    // Read aggregation config from environment
    get_aggregations()
        .and_then(move |aggregations| {
            // Perform aggregation with config
            aggregate_with_strategies(results, aggregations)
        })
}
```

**Validation for Error Accumulation (Spec 176):**

```rust
use stillwater::Validation;

/// Validate phase inputs before execution
fn validate_phase_inputs(
    phase: &Phase,
    inputs: &HashMap<String, Value>,
) -> Validation<ValidatedInputs, Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check required inputs
    for required in &phase.input_variables {
        if !inputs.contains_key(required) {
            errors.push(ValidationError::MissingInput {
                phase: phase.name.clone(),
                variable: required.clone(),
            });
        }
    }

    // Validate input types
    for (name, value) in inputs {
        if let Some(expected_type) = phase.get_input_type(name) {
            if !value_matches_type(value, expected_type) {
                errors.push(ValidationError::TypeMismatch {
                    variable: name.clone(),
                    expected: expected_type.to_string(),
                    got: value.type_name(),
                });
            }
        }
    }

    // Return success or ALL errors
    if errors.is_empty() {
        Validation::Success(ValidatedInputs { inputs: inputs.clone() })
    } else {
        Validation::Failure(errors)
    }
}

/// Validate work items before parallel execution
fn validate_and_execute_parallel(
    phase: &Phase,
    work_items: Vec<WorkItem>,
) -> Effect<PhaseResult, PhaseError, WorkflowEnv> {
    use prodigy::cook::execution::mapreduce::validation::validate_work_items;

    // Validate ALL work items, accumulate errors
    match validate_work_items(&work_items, phase.item_schema()) {
        Validation::Success(validated) => {
            // All valid, proceed with execution
            execute_parallel_phase_with_items(phase, validated)
        }
        Validation::Failure(errors) => {
            // Report ALL validation errors at once
            Effect::fail(PhaseError::ValidationFailed {
                phase: phase.name.clone(),
                errors,
            })
        }
    }
}

/// Combine validation with Effect execution
fn execute_workflow_with_validation(
    workflow: &Workflow,
) -> Effect<WorkflowResult, WorkflowError, WorkflowEnv> {
    // Validate workflow structure (pure, uses Validation)
    match validate_workflow_structure(workflow) {
        Validation::Success(_) => {
            // Structure valid, execute with effects
            execute_workflow(workflow)
        }
        Validation::Failure(errors) => {
            // Structure invalid, return ALL errors
            Effect::fail(WorkflowError::InvalidStructure(errors))
        }
    }
}
```

**Key Integration Points:**

1. **Error Context**: Every phase/step adds context via `.with_context()`
2. **Environment Access**: Use Reader helpers for clean env access
3. **Validation**: Accumulate ALL errors before failing
4. **Composition**: Chain validation → execution → aggregation

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

**Feature Flag Strategy**: Each phase uses feature flags to allow gradual rollout and easy rollback.

#### Phase 1: Create Unified Model Structures (Week 1)
**Feature Flag**: `UNIFIED_MODEL_ENABLED=false` (disabled by default)

**Deliverables:**
- Define `Workflow`, `Phase`, `PhaseMode` types
- Define `VariableAggregation`, `AggregationStrategy` types
- Define `WorkflowCheckpoint`, `PhaseCheckpoint` types
- No execution logic yet, just data structures

**Tests:**
- Unit tests for type constructors
- Serialization/deserialization tests
- Property tests for invariants

**Success Criteria:**
- All types compile
- 100% test coverage of data structures
- Documentation for all public types

**Rollback**: Delete new module, no impact on existing code

---

#### Phase 2: Implement Normalization with Validation (Week 2)
**Feature Flag**: `UNIFIED_NORMALIZATION_ENABLED=false`

**Deliverables:**
- `normalize_sequential()` function
- `normalize_mapreduce()` function
- `validate_workflow_config()` and `validate_mapreduce_config()`
- Normalization tests

**Tests:**
- Normalization preserves all workflow semantics
- Validation catches all edge cases (empty workflows, reserved names, etc.)
- Round-trip tests (YAML → Workflow → execution semantics match)

**Success Criteria:**
- Both workflow types normalize correctly
- All validation errors accumulated (Spec 176 pattern)
- Existing workflows pass normalization

**Rollback**: Disable flag, continue using old parsing

---

#### Phase 3: Implement Unified Sequential Execution (Weeks 3-4)
**Feature Flag**: `UNIFIED_SEQUENTIAL_ENABLED=false`

**Deliverables:**
- `execute_sequential_phase()` implementation
- `execute_step_with_checkpoint()` helper
- Step-level checkpointing
- Stillwater context integration

**Tests:**
- All existing sequential workflow tests pass
- New unified sequential tests
- Checkpoint compatibility tests
- Error context preservation tests

**Success Criteria:**
- Existing sequential workflows work with unified path
- Performance within 5% of old path
- Error messages improved (full context)

**Rollback**: Disable flag → old sequential executor

---

#### Phase 4: Migrate Parallel Execution (Weeks 5-6)
**Feature Flag**: `UNIFIED_PARALLEL_ENABLED=false`

**Deliverables:**
- `execute_parallel_phase()` implementation
- Failure policy enforcement
- Variable aggregation with validation
- Item-level checkpointing

**Tests:**
- All existing MapReduce tests pass
- Failure policy tests (FailFast, ContinueOnFailure, RetryThenFail)
- Aggregation tests (all strategies)
- Partial failure checkpointing

**Success Criteria:**
- Existing MapReduce workflows work with unified path
- Failure modes correctly preserved in checkpoints
- Aggregation errors accumulated (all type mismatches reported)

**Rollback**: Disable flag → old MapReduce coordinator

---

#### Phase 5: Unify Checkpoint Format with Migration (Week 7)
**Feature Flag**: `UNIFIED_CHECKPOINT_ENABLED=false`

**Deliverables:**
- `load_checkpoint_with_migration()` implementation
- `migrate_sequential_checkpoint()` and `migrate_mapreduce_checkpoint()`
- Checkpoint validation
- Backup/rollback on migration failure

**Tests:**
- Old sequential checkpoints migrate correctly
- Old MapReduce checkpoints migrate correctly
- Migrated checkpoints resume successfully
- Migration failure triggers rollback
- All checkpoint validation errors accumulated

**Success Criteria:**
- 100% of old checkpoints migrate successfully
- Resume works from migrated checkpoints
- No data loss during migration
- Backup created before migration

**Rollback**: Disable flag → old checkpoint format (backups preserved)

---

#### Phase 6: Unify Resume Logic (Week 8)
**Feature Flag**: `UNIFIED_RESUME_ENABLED=true` (enable by default after this phase)

**Deliverables:**
- `plan_workflow_resume()` pure planning
- `execute_resume_plan()` effect execution
- Phase-aware resume (skip completed, resume in-progress)
- Resume testing across all phase types

**Tests:**
- Resume from each phase type
- Resume with skip/retry logic
- Hybrid workflow resume
- Resume failure handling

**Success Criteria:**
- Single resume command works for all workflow types
- Resume plan is pure and testable
- Skip logic prevents re-execution of completed work
- Partial results preserved

**Rollback**: Disable flag → separate resume commands (but migrated checkpoints still work)

---

#### Phase 7: Deprecate Separate Code Paths (Week 9+)
**Feature Flag**: All flags removed (unified path only)

**Deliverables:**
- Remove old sequential executor
- Remove old MapReduce coordinator
- Remove feature flags
- Update all documentation
- Performance benchmarking

**Tests:**
- Full regression test suite
- Performance benchmarks (should be within 5% of old)
- Memory usage tests
- Load tests with 1000+ parallel agents

**Success Criteria:**
- Old code paths deleted
- All tests pass
- Performance acceptable
- Documentation updated

**Rollback**: NOT POSSIBLE (old code deleted)

---

**Monitoring per Phase:**
- Unit test pass rate
- Integration test pass rate
- Performance metrics (latency, memory)
- Error rate in production
- Checkpoint migration success rate

**Canary Deployment:**
- Phase 3-6: Deploy with flag disabled, enable for 10% of workflows
- Monitor for 48 hours before increasing to 50%, then 100%
- Any regression → disable flag immediately

## Dependencies

### Prerequisites

**IMPORTANT**: Verify implementation status before starting:

- **Spec 183**: Effect-Based Workflow Execution
  - Status: ⚠️ **MUST BE VERIFIED**
  - Required: `Effect` type, `and_then`, `map`, `par_all_limit`
  - Blocker: Cannot implement phase execution without Effect infrastructure

- **Spec 184**: Unified Checkpoint System
  - Status: ⚠️ **MUST BE VERIFIED**
  - Required: Checkpoint storage, versioning, migration support
  - Blocker: Cannot implement unified checkpoints without migration framework

- **Spec 186**: Non-MapReduce Workflow Resume
  - Status: ⚠️ **MUST BE VERIFIED**
  - Required: Resume planning, step skipping logic
  - Blocker: Resume plan logic builds on existing resume infrastructure

**Action Required**: Before implementation, create a dependency status document confirming:
1. Which specs are complete
2. Which specs need implementation first
3. Any API changes needed for compatibility

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

#[test]
fn test_variable_flow_between_phases() {
    // Setup phase produces items variable
    let workflow = Workflow {
        phases: vec![
            Phase {
                name: "setup".to_string(),
                mode: PhaseMode::Sequential,
                output_variables: vec!["items".to_string()],
                ..
            },
            Phase {
                name: "map".to_string(),
                mode: PhaseMode::Parallel {
                    work_items: WorkItemSource::Variable {
                        name: "setup.items".to_string(),
                        json_path: None,
                    },
                    ..
                },
                input_variables: vec!["setup.items".to_string()],
                aggregations: vec![
                    VariableAggregation {
                        name: "results".to_string(),
                        strategy: AggregationStrategy::Collect,
                        initial: None,
                    }
                ],
                output_variables: vec!["results".to_string()],
                ..
            },
            Phase {
                name: "reduce".to_string(),
                mode: PhaseMode::Sequential,
                input_variables: vec!["map.results".to_string()],
                ..
            },
        ],
        ..
    };

    // Execute and verify variable flow
    let result = execute_workflow(&workflow).run_async(&env).await.unwrap();

    // Map phase should have received setup.items
    assert!(result.phase_results[1].accessed_variables.contains("setup.items"));

    // Reduce phase should have received map.results
    assert!(result.phase_results[2].accessed_variables.contains("map.results"));
}

#[tokio::test]
async fn test_failure_policy_continue_on_failure() {
    // 10 items, 3 fail, policy = ContinueOnFailure
    let workflow = create_workflow_with_failure_policy(
        10,
        vec![2, 5, 7], // Fail items 2, 5, 7
        FailurePolicy::ContinueOnFailure { max_failures: Some(5) },
    );

    let result = execute_workflow(&workflow).run_async(&env).await.unwrap();

    // Should complete 7 items
    let map_phase = &result.phase_results[0];
    assert_eq!(map_phase.completed_items.len(), 7);
    assert_eq!(map_phase.failed_items.len(), 3);

    // Checkpoint should preserve both successes and failures
    let checkpoint = load_checkpoint(&result.session_id).await.unwrap();
    assert_eq!(checkpoint.phase_progress[0].completed_items.len(), 7);
    assert_eq!(checkpoint.phase_progress[0].failed_items.len(), 3);
}

#[tokio::test]
async fn test_checkpoint_migration_v1_to_v3() {
    // Create old v1 sequential checkpoint
    let v1_checkpoint = create_v1_sequential_checkpoint();
    save_raw_checkpoint(&v1_checkpoint).await.unwrap();

    // Load with migration
    let migrated = load_checkpoint_with_migration(&v1_checkpoint.session_id)
        .run_async(&storage_env)
        .await
        .unwrap();

    // Verify migration
    assert_eq!(migrated.version, CHECKPOINT_VERSION_UNIFIED);
    assert_eq!(migrated.phase_progress.len(), 1);
    assert_eq!(migrated.phase_progress[0].phase_name, "main");

    // Resume should work
    let workflow = normalize_sequential(&v1_checkpoint.workflow_config);
    let plan = plan_workflow_resume(&migrated, &workflow);
    let result = execute_resume_plan(&plan, &workflow)
        .run_async(&env)
        .await
        .unwrap();

    assert_eq!(result.status, WorkflowStatus::Completed);
}

#[tokio::test]
async fn test_checkpoint_migration_v2_to_v3() {
    // Create old v2 MapReduce checkpoint
    let v2_checkpoint = create_v2_mapreduce_checkpoint();
    save_raw_checkpoint(&v2_checkpoint).await.unwrap();

    // Load with migration
    let migrated = load_checkpoint_with_migration(&v2_checkpoint.session_id)
        .run_async(&storage_env)
        .await
        .unwrap();

    // Verify migration
    assert_eq!(migrated.version, CHECKPOINT_VERSION_UNIFIED);
    assert_eq!(migrated.phase_progress.len(), 3); // setup, map, reduce

    // Check setup phase migrated
    assert_eq!(migrated.phase_progress[0].phase_name, "setup");
    assert_eq!(migrated.phase_progress[0].status, PhaseStatus::Completed);

    // Check map phase migrated with items
    assert_eq!(migrated.phase_progress[1].phase_name, "map");
    assert!(!migrated.phase_progress[1].completed_items.is_empty());

    // Resume should work
    let workflow = normalize_mapreduce(&v2_checkpoint.workflow_config);
    let result = execute_resume_plan(
        &plan_workflow_resume(&migrated, &workflow),
        &workflow,
    )
    .run_async(&env)
    .await
    .unwrap();

    assert_eq!(result.status, WorkflowStatus::Completed);
}

#[test]
fn test_hybrid_workflow_phases() {
    // AC3: setup → parallel → intermediate → parallel → finalize
    let workflow = Workflow {
        phases: vec![
            Phase {
                name: "setup".to_string(),
                mode: PhaseMode::Sequential,
                ..
            },
            Phase {
                name: "first_map".to_string(),
                mode: PhaseMode::Parallel { .. },
                ..
            },
            Phase {
                name: "intermediate".to_string(),
                mode: PhaseMode::Sequential,
                ..
            },
            Phase {
                name: "second_map".to_string(),
                mode: PhaseMode::Parallel { .. },
                ..
            },
            Phase {
                name: "finalize".to_string(),
                mode: PhaseMode::Sequential,
                ..
            },
        ],
        ..
    };

    assert_eq!(workflow.phases.len(), 5);
    assert!(matches!(workflow.phases[0].mode, PhaseMode::Sequential));
    assert!(matches!(workflow.phases[1].mode, PhaseMode::Parallel { .. }));
    assert!(matches!(workflow.phases[2].mode, PhaseMode::Sequential));
    assert!(matches!(workflow.phases[3].mode, PhaseMode::Parallel { .. }));
    assert!(matches!(workflow.phases[4].mode, PhaseMode::Sequential));
}

#[tokio::test]
async fn test_aggregation_type_mismatch_all_errors() {
    // Agents produce different types for same variable
    let items = vec![
        json!({"id": "a", "score": 42}),        // Number
        json!({"id": "b", "score": "high"}),    // String (type error)
        json!({"id": "c", "score": [1, 2]}),    // Array (type error)
        json!({"id": "d", "score": 85}),        // Number
    ];

    let workflow = create_workflow_with_aggregation(
        items,
        VariableAggregation {
            name: "score".to_string(),
            strategy: AggregationStrategy::Sum,
            initial: Some(json!(0)),
        },
    );

    let result = execute_workflow(&workflow).run_async(&env).await;

    // Should fail with ALL type mismatches reported
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 2); // Two type mismatches

    // Check both errors are reported
    assert!(errors.iter().any(|e| matches!(e,
        PhaseError::AggregationError { variable, .. } if variable == "score"
    )));
}

#[test]
fn test_normalization_validation_accumulates_errors() {
    // Invalid workflow with multiple errors
    let config = WorkflowConfig {
        name: Some("setup".to_string()), // Reserved name
        commands: vec![],                 // Empty commands
        ..
    };

    let result = normalize_and_validate_sequential(&config);

    // Should report ALL errors at once
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.len() >= 2); // At least reserved name + empty commands

    assert!(errors.iter().any(|e| matches!(e, ValidationError::ReservedName { .. })));
    assert!(errors.iter().any(|e| matches!(e, ValidationError::EmptyWorkflow { .. })));
}

#[tokio::test]
async fn test_error_context_preservation() {
    // Workflow that fails deep in execution
    let workflow = create_workflow_failing_at("map", 7, 2); // Map phase, item 7, step 2

    let result = execute_workflow(&workflow).run_async(&env).await;

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Error should have full context trace
    let context_trace = error.context_trace();
    assert!(context_trace.contains("Phase 'map'"));
    assert!(context_trace.contains("Item 7"));
    assert!(context_trace.contains("Step 2"));
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
