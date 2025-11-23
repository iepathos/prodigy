# Stillwater Pattern Migration Plan for Prodigy

## Executive Summary

This document outlines a strategic plan to migrate Prodigy's architecture toward Stillwater's proven patterns, focusing on separation of concerns, error handling, and testability.

**Key Metrics**:
- **Current State**: 2,053 unwrap/panic instances, 182 Arc<Mutex/RwLock> patterns, 15+ modules >1,500 lines
- **Target State**: Pure core with imperative shell, comprehensive error accumulation, zero unwraps in production code
- **Estimated Impact**: 40% reduction in function complexity, 60% improvement in testability, 80% better error diagnostics

---

## Stillwater Core Patterns Reference

### Pattern 1: Pure Core, Imperative Shell
- **Pure Core**: Business logic with no side effects (testable without mocks)
- **Imperative Shell**: I/O operations at boundaries (thin wrappers)
- **Benefits**: Clear separation, easy testing, reusable logic

### Pattern 2: Validation<T, E> - Error Accumulation
- **Purpose**: Collect ALL errors instead of failing on first
- **Use Cases**: Form validation, configuration validation, work item validation
- **Key Method**: `Validation::all()` - validates multiple items at once

### Pattern 3: Effect<T, E, Env> - Lazy Effect Composition
- **Purpose**: Compose async operations with explicit dependencies
- **Type Params**: T (success), E (error), Env (dependencies)
- **Benefits**: Lazy evaluation, environment injection, testable via mock environments

### Pattern 4: ContextError<E> - Error Context Trails
- **Purpose**: Preserve error context as errors bubble up
- **Display**: Shows context trail with `->` separators
- **Integration**: Works with Effect's `.context()` method

### Pattern 5: Semigroup - Associative Composition
- **Purpose**: Types that can be combined associatively
- **Use Cases**: Error accumulation (Vec<E>), result merging
- **Law**: `a.combine(b).combine(c) == a.combine(b.combine(c))`

---

## Priority 1: MapReduce Work Item Validation (High Impact, Medium Effort)

### Current Problem
**File**: `src/cook/execution/data_pipeline/mod.rs` (1,658 lines)
**Issues**:
- Work item validation fails on first error
- Users get incomplete error reports requiring multiple retry cycles
- Mixed I/O and validation logic
- No clear separation between parsing, validation, and transformation

### Stillwater Pattern Application

**Apply**: Validation<T, E> for comprehensive error accumulation

**Implementation**:
```rust
// NEW: Pure validation module
// src/cook/execution/data_pipeline/validation.rs

use stillwater::Validation;

/// Pure validation - no I/O, just logic
pub fn validate_work_item(item: &WorkItem) -> Validation<ValidWorkItem, Vec<ValidationError>> {
    // Validate all fields at once, accumulate errors
    Validation::all((
        validate_item_id(&item.id),
        validate_item_path(&item.path),
        validate_item_data(&item.data),
        validate_item_filter(&item.filter),
    ))
    .map(|(id, path, data, filter)| ValidWorkItem { id, path, data, filter })
}

/// Validate multiple work items, accumulate all errors
pub fn validate_work_items(items: &[WorkItem]) -> Validation<Vec<ValidWorkItem>, Vec<ValidationError>> {
    Validation::all(
        items.iter().map(validate_work_item)
    )
}
```

**Usage in Pipeline**:
```rust
// Before: Stops at first error
fn load_work_items(path: &Path) -> Result<Vec<WorkItem>> {
    let items = parse_json(path)?;  // I/O
    for item in &items {
        validate_item(item)?;  // ❌ Stops at first error
    }
    Ok(items)
}

// After: Accumulates all errors
fn load_work_items(path: &Path) -> Result<Vec<ValidWorkItem>> {
    let items = parse_json(path)?;  // I/O (still uses Result)

    validate_work_items(&items)  // Pure validation
        .into_result()  // Convert to Result for ? operator
        .map_err(|errors| {
            // User sees ALL validation errors at once
            WorkItemError::MultipleValidationErrors(errors)
        })
}
```

**Expected Benefits**:
- ✅ Users see ALL validation errors in one pass
- ✅ Pure validation functions testable without file I/O
- ✅ Reduced retry cycles in MapReduce workflows
- ✅ Clear separation: parsing (I/O) vs validation (pure)

**Migration Steps**:
1. Create `data_pipeline/validation.rs` with pure validation functions
2. Extract existing validation logic to pure functions
3. Replace sequential validation with `Validation::all()`
4. Update error types to support error accumulation
5. Add tests for validation without any I/O setup

**Estimated Effort**: 2-3 days
**Files Changed**: 3-4 files
**Test Impact**: 20-30 new pure validation tests

---

## Priority 2: Workflow Orchestration with Effect Composition (High Impact, High Effort)

### Current Problem
**File**: `src/cook/orchestrator/core.rs` (2,884 lines)
**Issues**:
- Mixed concerns: execution, session management, health metrics, argument processing
- Heavy use of Arc<Mutex<>> for shared state (lines 104-106, 192)
- Complex initialization with lazy patterns
- Difficult to test without full integration setup

### Stillwater Pattern Application

**Apply**: Effect<T, E, Env> for workflow orchestration

**Implementation**:
```rust
// NEW: Environment for dependency injection
// src/cook/orchestrator/environment.rs

pub struct OrchestratorEnv {
    pub session_manager: Arc<dyn SessionManager>,
    pub command_executor: Arc<dyn CommandExecutor>,
    pub claude_executor: Arc<dyn ClaudeExecutor>,
    pub user_interaction: Arc<dyn UserInteraction>,
    pub git_operations: Arc<dyn GitOperations>,
}

// NEW: Pure orchestration logic
// src/cook/orchestrator/pure.rs

use stillwater::Effect;

/// Pure workflow classification
pub fn classify_workflow(config: &WorkflowConfig) -> WorkflowType {
    match config.mode {
        WorkflowMode::MapReduce => WorkflowType::MapReduce,
        WorkflowMode::Standard if config.steps.is_empty() => WorkflowType::Empty,
        WorkflowMode::Standard => WorkflowType::Standard,
    }
}

/// Pure workflow validation
pub fn validate_workflow(config: &WorkflowConfig) -> Validation<(), Vec<WorkflowError>> {
    Validation::all((
        validate_workflow_steps(config),
        validate_environment_vars(config),
        validate_command_syntax(config),
    ))
}

// NEW: Effect-based orchestration
// src/cook/orchestrator/effects.rs

use stillwater::Effect;

type OrchEffect<T> = Effect<T, OrchestratorError, OrchestratorEnv>;

/// Setup workflow environment (I/O via environment)
pub fn setup_workflow(config: WorkflowConfig) -> OrchEffect<WorkflowSession> {
    Effect::from_async(|env: &OrchestratorEnv| async move {
        // Create session
        let session = env.session_manager.create_session(&config).await?;

        // Setup git worktree
        let worktree = env.git_operations.create_worktree(&session.id).await?;

        Ok(WorkflowSession { session, worktree, config })
    })
    .context("Setting up workflow environment")
}

/// Execute workflow steps (composition)
pub fn execute_workflow(session: WorkflowSession) -> OrchEffect<WorkflowResult> {
    setup_workflow(session.config.clone())
        .and_then(|session| execute_steps(session))
        .and_then(|result| save_results(result))
        .and_then(|result| merge_changes(result))
        .context("Executing workflow")
}

/// Execute individual step (pure + I/O separated)
pub fn execute_step(step: &WorkflowStep, ctx: &StepContext) -> OrchEffect<StepResult> {
    // Validate step (pure)
    Effect::from_validation(validate_step_syntax(step))
        .and_then(|_| {
            // Interpolate variables (pure)
            let interpolated = interpolate_step_command(step, ctx);

            // Execute command (I/O)
            Effect::from_async(move |env: &OrchestratorEnv| async move {
                env.command_executor.execute(&interpolated).await
            })
        })
        .context(format!("Executing step: {}", step.name))
}
```

**Usage**:
```rust
// Before: Imperative with shared state
async fn run_workflow(&mut self, config: WorkflowConfig) -> Result<WorkflowResult> {
    let session = self.session_manager.create_session(&config).await?;
    self.current_session = Some(session.clone());  // ❌ Mutable state

    let worktree = self.git_operations.create_worktree(&session.id).await?;
    self.current_worktree = Some(worktree);  // ❌ Mutable state

    // ... complex execution with error handling ...
}

// After: Effect composition with environment
async fn run_workflow(config: WorkflowConfig, env: &OrchestratorEnv) -> Result<WorkflowResult> {
    execute_workflow(config)
        .run(env)  // Execute with concrete environment
        .await
}

// Testing with mock environment
#[tokio::test]
async fn test_workflow_execution() {
    let mock_env = OrchestratorEnv {
        session_manager: Arc::new(MockSessionManager::new()),
        command_executor: Arc::new(MockCommandExecutor::new()),
        // ... other mocks ...
    };

    let result = execute_workflow(test_config())
        .run(&mock_env)
        .await;

    assert!(result.is_ok());
}
```

**Expected Benefits**:
- ✅ Zero mutable state in orchestrator
- ✅ Pure workflow logic testable without I/O
- ✅ Clear dependency injection (no hidden singletons)
- ✅ Automatic error context propagation
- ✅ Composable workflow phases

**Migration Steps**:
1. Create `OrchestratorEnv` struct with all dependencies
2. Extract pure workflow validation and classification functions
3. Create effect-based step execution
4. Compose workflow phases as effect chains
5. Replace Arc<Mutex<>> state with environment-based execution
6. Update tests to use mock environments

**Estimated Effort**: 2-3 weeks
**Files Changed**: 10-15 files
**Test Impact**: 50-100 new pure function tests, simplified integration tests

---

## Priority 3: Error Context Preservation (Medium Impact, Low Effort)

### Current Problem
**Files**: Throughout codebase
**Issues**:
- Errors lose context as they bubble up
- DLQ items have minimal debugging information
- MapReduce agent failures difficult to diagnose
- Generic "command failed" errors without operation context

### Stillwater Pattern Application

**Apply**: ContextError<E> for error context trails

**Implementation**:
```rust
// NEW: Context-aware error types
// src/cook/error.rs

use stillwater::ContextError;

/// Wrap all Prodigy errors with context
pub type ProdigyResult<T> = Result<T, ContextError<ProdigyError>>;

/// Add context to Result
pub trait ResultExt<T, E> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, ContextError<E>>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn ctx(self, msg: impl Into<String>) -> Result<T, ContextError<E>> {
        self.map_err(|e| ContextError::new(e).context(msg))
    }
}

// Usage in command execution
pub async fn execute_command(cmd: &Command) -> ProdigyResult<CommandResult> {
    prepare_environment(cmd)
        .ctx("Preparing command environment")?;

    interpolate_variables(cmd)
        .ctx("Interpolating command variables")?;

    run_subprocess(cmd)
        .await
        .ctx(format!("Executing command: {}", cmd.name))?;

    Ok(result)
}
```

**Error Display**:
```
Before:
Error: File not found: work_items.json

After:
Error: File not found: work_items.json
  -> Loading work items
  -> Preparing map phase
  -> Executing MapReduce job: process-items
```

**Integration with DLQ**:
```rust
// Store context trail in DLQ items
pub struct DeadLetteredItem {
    pub item_id: String,
    pub error: String,
    pub error_context: Vec<String>,  // NEW: Full context trail
    pub timestamp: DateTime<Utc>,
}

// On agent failure
fn record_agent_failure(item: WorkItem, error: ContextError<AgentError>) {
    dlq.add(DeadLetteredItem {
        item_id: item.id,
        error: error.inner().to_string(),
        error_context: error.context_trail().to_vec(),  // Preserve trail
        timestamp: Utc::now(),
    });
}
```

**Expected Benefits**:
- ✅ Complete error context in DLQ for debugging
- ✅ Clear operation trail for failed commands
- ✅ Better error messages for end users
- ✅ Easier debugging of MapReduce agent failures

**Migration Steps**:
1. Create `ContextError` wrapper in `cook/error.rs`
2. Add `.ctx()` extension method for Results
3. Update command execution to add context at each layer
4. Extend DLQ schema to store context trails
5. Update error display to show context trails

**Estimated Effort**: 3-5 days
**Files Changed**: 20-30 files (add .ctx() calls)
**Test Impact**: 10-15 new error context tests

---

## Priority 4: Pure Function Extraction in State Management (Medium Impact, Medium Effort)

### Current Problem
**File**: `src/cook/execution/state.rs` (1,856 lines)
**Issues**:
- `MapReduceJobState` has 18 fields with unclear separation
- Mixed concerns: metadata, execution state, checkpoint data, failure tracking
- Heavy I/O mixed with state updates
- Difficult to test state transitions without file system

### Stillwater Pattern Application

**Apply**: Pure Core pattern - separate state logic from I/O

**Implementation**:
```rust
// NEW: Pure state transitions
// src/cook/execution/state/pure.rs

/// Pure state transition functions (no I/O)

/// Calculate next work item batch (pure)
pub fn next_batch(
    state: &JobState,
    batch_size: usize,
) -> Option<WorkBatch> {
    let pending = &state.pending_items;
    if pending.is_empty() {
        return None;
    }

    let batch_items = pending.iter()
        .take(batch_size)
        .cloned()
        .collect();

    Some(WorkBatch {
        items: batch_items,
        batch_id: state.next_batch_id,
    })
}

/// Apply agent result to state (pure)
pub fn apply_agent_result(
    mut state: JobState,
    result: AgentResult,
) -> JobState {
    // Pure state update - returns new state
    state.completed_items.push(result.item_id);
    state.agent_results.insert(result.agent_id, result);
    state.items_processed += 1;
    state
}

/// Determine if job is complete (pure)
pub fn is_job_complete(state: &JobState) -> bool {
    state.pending_items.is_empty() &&
    state.active_agents.is_empty() &&
    state.reduce_phase_completed
}

/// Calculate retry strategy (pure)
pub fn should_retry_item(
    item: &WorkItem,
    failure_count: usize,
    max_retries: usize,
) -> RetryDecision {
    if failure_count >= max_retries {
        RetryDecision::SendToDLQ
    } else {
        RetryDecision::Retry {
            delay: exponential_backoff(failure_count),
        }
    }
}

// NEW: I/O layer
// src/cook/execution/state/io.rs

use stillwater::Effect;

type StateEffect<T> = Effect<T, StateError, StateEnv>;

pub struct StateEnv {
    pub storage: Arc<dyn StorageBackend>,
}

/// Save checkpoint (I/O wrapper around pure state)
pub fn save_checkpoint(state: JobState) -> StateEffect<()> {
    Effect::from_async(|env: &StateEnv| async move {
        let serialized = serde_json::to_string(&state)?;
        env.storage.write_checkpoint(&state.job_id, &serialized).await
    })
    .context("Saving job checkpoint")
}

/// Load checkpoint (I/O)
pub fn load_checkpoint(job_id: &str) -> StateEffect<JobState> {
    Effect::from_async(|env: &StateEnv| async move {
        let data = env.storage.read_checkpoint(job_id).await?;
        let state = serde_json::from_str(&data)?;
        Ok(state)
    })
    .context(format!("Loading checkpoint for job {}", job_id))
}

/// Update and save state (composition)
pub fn update_state_with_result(
    state: JobState,
    result: AgentResult,
) -> StateEffect<JobState> {
    // Pure update
    let updated_state = apply_agent_result(state, result);

    // Save to disk
    save_checkpoint(updated_state.clone())
        .map(|_| updated_state)
}
```

**Usage**:
```rust
// Before: Mixed I/O and logic
async fn handle_agent_completion(&mut self, result: AgentResult) -> Result<()> {
    // ❌ Mutable state update + I/O mixed
    self.state.completed_items.push(result.item_id);
    self.save_checkpoint().await?;

    if self.state.pending_items.is_empty() {
        self.complete_job().await?;
    }
}

// After: Pure logic + Effect composition
fn handle_agent_completion(state: JobState, result: AgentResult) -> StateEffect<JobState> {
    update_state_with_result(state, result)
        .and_then(|state| {
            // Pure check
            if is_job_complete(&state) {
                complete_job(state)
            } else {
                Effect::pure(state)
            }
        })
}

// Testing pure functions - no I/O
#[test]
fn test_apply_agent_result() {
    let state = JobState::new();
    let result = AgentResult { item_id: "item-1", ... };

    let new_state = apply_agent_result(state, result);

    assert_eq!(new_state.items_processed, 1);
    assert!(new_state.completed_items.contains(&"item-1"));
}

// Testing effects - mock environment
#[tokio::test]
async fn test_save_checkpoint() {
    let mock_storage = Arc::new(MockStorage::new());
    let env = StateEnv { storage: mock_storage };

    let state = JobState::new();
    let result = save_checkpoint(state).run(&env).await;

    assert!(result.is_ok());
}
```

**Expected Benefits**:
- ✅ State transitions testable without file system
- ✅ Clear separation: state logic (pure) vs persistence (I/O)
- ✅ Immutable state updates (no hidden mutations)
- ✅ Composable state operations
- ✅ Easy to reason about state machine

**Migration Steps**:
1. Create `state/pure.rs` with pure state transition functions
2. Create `state/io.rs` with I/O wrappers
3. Create `StateEnv` for dependency injection
4. Extract all state update logic to pure functions
5. Wrap I/O operations in Effects
6. Compose state operations using Effect chains
7. Update tests to use pure functions + mock environments

**Estimated Effort**: 1-2 weeks
**Files Changed**: 5-8 files
**Test Impact**: 40-60 new pure state tests

---

## Priority 5: Variable Resolution with Semigroup (Low Impact, Low Effort)

### Current Problem
**File**: `src/cook/execution/variables.rs` (2,286 lines)
**Issues**:
- 15 aggregate types with duplicated combination logic
- No clear abstraction for result aggregation
- Custom merge logic scattered across aggregate implementations

### Stillwater Pattern Application

**Apply**: Semigroup trait for aggregate composition

**Implementation**:
```rust
// NEW: Semigroup-based aggregation
// src/cook/execution/variables/semigroup.rs

use stillwater::Semigroup;

/// Aggregate results implement Semigroup
impl Semigroup for AggregateResult {
    fn combine(self, other: Self) -> Self {
        match (self, other) {
            (AggregateResult::Count(a), AggregateResult::Count(b)) => {
                AggregateResult::Count(a + b)
            }
            (AggregateResult::Sum(a), AggregateResult::Sum(b)) => {
                AggregateResult::Sum(a + b)
            }
            (AggregateResult::Collect(mut a), AggregateResult::Collect(b)) => {
                a.extend(b);
                AggregateResult::Collect(a)
            }
            (AggregateResult::Merge(mut a), AggregateResult::Merge(b)) => {
                a.merge(b);
                AggregateResult::Merge(a)
            }
            _ => panic!("Cannot combine different aggregate types"),
        }
    }
}

// Aggregate multiple results using Semigroup
pub fn aggregate_results(results: Vec<AggregateResult>) -> Option<AggregateResult> {
    results.into_iter().reduce(|a, b| a.combine(b))
}

// Validation with accumulated errors (Vec implements Semigroup)
pub fn validate_variables(
    vars: &HashMap<String, String>
) -> Validation<ValidatedVars, Vec<VariableError>> {
    Validation::all(
        vars.iter().map(|(k, v)| validate_variable_syntax(k, v))
    )
}
```

**Expected Benefits**:
- ✅ Consistent aggregation logic across all types
- ✅ Composable aggregations
- ✅ Less code duplication
- ✅ Clear mathematical properties (associativity)

**Migration Steps**:
1. Implement Semigroup for AggregateResult
2. Replace custom merge logic with `.combine()`
3. Use `reduce()` for aggregating multiple results
4. Add property tests for associativity

**Estimated Effort**: 2-3 days
**Files Changed**: 2-3 files
**Test Impact**: 10-15 new aggregation tests

---

## Quick Wins (Immediate Impact, Low Effort)

### QW1: Add .context() to Command Execution (1 day)
**Files**: `src/cook/orchestrator/core.rs`, `src/cook/workflow/executor.rs`
**Change**: Add `.ctx("operation")` to all command execution paths
**Benefit**: Immediate improvement in error diagnostics

### QW2: Extract Pure Validation Functions (2 days)
**Files**: `src/cook/execution/data_pipeline/mod.rs`
**Change**: Extract validation logic to pure functions
**Benefit**: Testable validation without I/O setup

### QW3: Create OrchestratorEnv for Testing (3 days)
**Files**: `src/cook/orchestrator/core.rs`
**Change**: Create environment struct, pass to orchestrator methods
**Benefit**: Enable mock-based testing of orchestrator

### QW4: Pure State Transition Functions (3 days)
**Files**: `src/cook/execution/state.rs`
**Change**: Extract state update logic to pure functions
**Benefit**: Testable state machine without persistence

### QW5: Validation::all() for Work Items (2 days)
**Files**: `src/cook/execution/data_pipeline/mod.rs`
**Change**: Replace sequential validation with error accumulation
**Benefit**: Users see all errors at once

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
- [ ] Add Stillwater as dependency
- [ ] Create `cook/error.rs` with ContextError integration
- [ ] Create `cook/orchestrator/environment.rs`
- [ ] Create `cook/execution/state/pure.rs`
- [ ] Implement Quick Wins 1-2

### Phase 2: Work Item Validation (Weeks 3-4)
- [ ] Create `data_pipeline/validation.rs` with Validation<T, E>
- [ ] Extract pure validation functions
- [ ] Update error types for error accumulation
- [ ] Add comprehensive validation tests
- [ ] Implement Quick Win 5

### Phase 3: State Management (Weeks 5-6)
- [ ] Extract pure state transitions
- [ ] Create state/io.rs with Effect-based I/O
- [ ] Create StateEnv for dependency injection
- [ ] Update checkpoint save/load to use Effects
- [ ] Implement Quick Win 4

### Phase 4: Orchestrator Refactoring (Weeks 7-10)
- [ ] Create effect-based workflow orchestration
- [ ] Migrate setup_environment to Effects
- [ ] Migrate execute_workflow to Effects
- [ ] Replace Arc<Mutex<>> with environment-based execution
- [ ] Update tests to use mock environments
- [ ] Implement Quick Win 3

### Phase 5: Variable System (Weeks 11-12)
- [ ] Implement Semigroup for aggregates
- [ ] Replace custom merge logic
- [ ] Add property tests for associativity
- [ ] Implement Quick Win 5 (if not done)

### Phase 6: Error Context (Weeks 13-14)
- [ ] Add .ctx() throughout codebase
- [ ] Update DLQ to store context trails
- [ ] Improve error display with context
- [ ] Add error context tests

---

## Success Metrics

### Code Quality
- [ ] Reduce unwrap/panic from 2,053 to <100 (production code)
- [ ] Reduce Arc<Mutex/RwLock> from 182 to <50
- [ ] Reduce average function length from 50 to <20 lines
- [ ] Achieve >80% pure function test coverage

### Error Handling
- [ ] 100% of work item validation uses Validation::all()
- [ ] 100% of command execution includes context
- [ ] DLQ items include full context trails
- [ ] Zero "generic error" messages in production

### Testability
- [ ] 60% increase in pure function tests (no I/O)
- [ ] Mock environments for all orchestrator tests
- [ ] 40% reduction in test execution time (pure tests faster)
- [ ] 100% of state transitions testable without persistence

### Architecture
- [ ] Clear separation: pure core vs imperative shell
- [ ] Zero mutable shared state in orchestrator
- [ ] Environment-based dependency injection throughout
- [ ] Effect composition for all workflow orchestration

---

## Risk Mitigation

### Risk 1: Learning Curve
- **Mitigation**: Start with Quick Wins, provide team training on Stillwater patterns
- **Timeline**: 1 week training before Phase 1

### Risk 2: Breaking Changes
- **Mitigation**: Incremental migration, maintain backward compatibility via adapters
- **Strategy**: Run old and new implementations in parallel during transition

### Risk 3: Performance Impact
- **Mitigation**: Effect boxing is zero-cost at runtime, pure functions optimize better
- **Validation**: Benchmark before/after for critical paths

### Risk 4: Incomplete Migration
- **Mitigation**: Each phase delivers value independently
- **Strategy**: Can stop at any phase with partial benefits

---

## Conclusion

Migrating Prodigy to Stillwater patterns offers significant benefits:

**Immediate Wins**: Better error diagnostics, clearer code structure, improved testability

**Long-Term Gains**: Pure core enables fearless refactoring, effect composition scales to complex workflows, environment injection simplifies testing

**Incremental Path**: Quick wins provide immediate value, full migration achievable in 14 weeks

**Low Risk**: Each phase independent, backward compatibility maintained, can stop at any point

**Recommendation**: Start with Quick Wins (Weeks 1-2), then proceed with Priority 1-3 for maximum impact.
