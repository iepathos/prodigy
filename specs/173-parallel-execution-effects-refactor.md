---
number: 173
title: Parallel Execution Effects Refactor
category: parallel
priority: high
status: draft
dependencies: [172]
created: 2025-11-24
---

# Specification 173: Parallel Execution Effects Refactor

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation Integration)

## Context

Prodigy's MapReduce implementation currently uses manual async coordination with tokio::spawn, manual chunking for concurrency limits, and complex error collection logic. This approach has several problems:

**Current Issues:**
- **Manual async coordination** - Complex tokio::spawn with error-prone handle management
- **Manual concurrency limiting** - Custom chunking logic for `max_parallel`
- **Error handling complexity** - Nested Result types and manual success/failure splitting
- **Testing difficulty** - Hard to test without actual I/O
- **Mixed concerns** - Pure planning logic mixed with I/O execution

**Example of current complexity:**
```rust
// 50+ lines of manual async coordination
let mut handles = Vec::new();
for item in items {
    let handle = tokio::spawn(async move {
        let worktree = create_worktree(...).await?;
        let result = execute_commands(...).await?;
        merge_to_parent(...).await?;
        Ok(result)
    });
    handles.push(handle);
}

// Manual chunking for concurrency
let mut results = Vec::new();
for chunk in handles.chunks(self.max_parallel) {
    let chunk_results = futures::future::join_all(chunk).await;
    results.extend(chunk_results);
}

// Manual error collection
let mut successes = Vec::new();
let mut failures = Vec::new();
for result in results {
    match result {
        Ok(Ok(r)) => successes.push(r),
        Ok(Err(e)) | Err(e) => failures.push(e),
    }
}
```

## Objective

Replace imperative async coordination with functional Effect composition by:
1. **Extracting pure planning logic** from I/O execution
2. **Implementing Effect-based I/O** for worktrees, commands, and merges
3. **Using Stillwater's parallel execution** (`par_all`, `par_all_limit`)
4. **Enabling dependency-based parallelization** for setup/reduce phases
5. **Improving testability** through Effect composition and mock environments

## Requirements

### Functional Requirements

#### FR1: Pure Work Planning
- **MUST** extract work assignment planning as pure function
- **MUST** separate filtering, limiting, and assignment logic
- **MUST** make planning testable without I/O
- **MUST** preserve existing filtering and offset semantics
- **MUST** generate work assignments with all required metadata

#### FR2: Effect-Based I/O Operations
- **MUST** implement `create_worktree_effect` for git worktree creation
- **MUST** implement `execute_commands_effect` for command execution
- **MUST** implement `merge_to_parent_effect` for merging results
- **MUST** support effect composition with `and_then`, `map`
- **MUST** preserve error context through effect chains

#### FR3: Parallel Agent Execution
- **MUST** use `Effect::par_all_limit` for bounded parallelism
- **MUST** respect `max_parallel` configuration
- **MUST** maintain execution order semantics where required
- **MUST** handle agent failures without stopping other agents
- **MUST** aggregate results from all agents

#### FR4: Setup/Reduce Phase Parallelization
- **MUST** analyze command dependencies to detect independence
- **MUST** execute independent commands in parallel batches
- **MUST** preserve execution order for dependent commands
- **MUST** use `Effect::par_all` for unlimited parallel execution
- **MUST** fall back to sequential execution when dependencies exist

#### FR5: Type-Safe Environment Access
- **MUST** define environment types for each phase (MapEnv, PhaseEnv)
- **MUST** use Effect environment parameter for dependency injection
- **MUST** eliminate manual environment threading
- **MUST** support mock environments for testing
- **MUST** maintain backward compatibility with existing config

### Non-Functional Requirements

#### NFR1: Performance
- **MUST** maintain or improve parallel execution performance
- **MUST** show speedup proportional to parallelism (> 0.7 * num_agents)
- **MUST** have minimal overhead from Effect abstraction
- **MUST** utilize all available parallelism opportunities

#### NFR2: Testability
- **MUST** enable pure function testing without I/O
- **MUST** support mock environments for Effect testing
- **MUST** make dependency analysis unit testable
- **MUST** verify timing properties in integration tests

#### NFR3: Maintainability
- **MUST** reduce map phase code size by > 30%
- **MUST** make effect composition clear and linear
- **MUST** eliminate nested error handling
- **MUST** follow existing Prodigy code conventions

## Acceptance Criteria

- [ ] Pure `plan_work_assignments` function extracts filtering/assignment logic
- [ ] Effect modules created: `worktree.rs`, `commands.rs`, `merge.rs`
- [ ] `execute_agent` composes effects with `and_then` chains
- [ ] `distribute_work` uses `Effect::par_all_limit` for parallel execution
- [ ] Setup/reduce phases use `analyze_dependencies` for parallelization
- [ ] Integration tests verify parallel execution timing
- [ ] Property tests verify order independence where expected
- [ ] Unit tests for pure planning require no I/O mocks
- [ ] Effect tests use mock environments
- [ ] Performance benchmarks show parallelism benefits
- [ ] Map phase code reduced from ~250 LOC to ~100 LOC

## Technical Details

### Implementation Approach

#### 1. Pure Work Planning

```rust
// src/cook/execution/mapreduce/pure/work_planning.rs

/// Pure: Plan work assignments from input items
pub fn plan_work_assignments(
    items: Vec<Value>,
    config: &MapConfig,
) -> Vec<WorkAssignment> {
    // Pure transformations: filter → limit → assign
    let filtered = apply_filters(items, &config.filter);
    let limited = apply_limits(filtered, config.offset, config.max_items);

    limited.into_iter()
        .enumerate()
        .map(|(idx, item)| WorkAssignment {
            id: idx,
            item,
            worktree_name: format!("agent-{}", idx),
        })
        .collect()
}

/// Pure: Apply filter predicates
fn apply_filters(items: Vec<Value>, filter: &Option<Filter>) -> Vec<Value> {
    match filter {
        Some(f) => items.into_iter().filter(|item| f.matches(item)).collect(),
        None => items,
    }
}

/// Pure: Apply offset and limit
fn apply_limits(
    items: Vec<Value>,
    offset: usize,
    max_items: Option<usize>,
) -> Vec<Value> {
    let skipped = items.into_iter().skip(offset);
    match max_items {
        Some(limit) => skipped.take(limit).collect(),
        None => skipped.collect(),
    }
}
```

#### 2. Effect-Based I/O Operations

```rust
// src/cook/execution/mapreduce/effects/worktree.rs

use stillwater::Effect;

/// Effect: Create git worktree for agent
pub fn create_worktree_effect(
    name: &str,
) -> Effect<Worktree, WorktreeError, MapEnv> {
    let name = name.to_string();
    Effect::from_async_fn(move |env| async move {
        env.worktree_manager
            .create_worktree(&name)
            .await
            .with_context(|| format!("Creating worktree {}", name))
    })
}

// src/cook/execution/mapreduce/effects/commands.rs

/// Effect: Execute commands in worktree
pub fn execute_commands_effect(
    item: &Value,
    worktree: &Worktree,
) -> Effect<CommandResult, CommandError, MapEnv> {
    let item = item.clone();
    let worktree = worktree.clone();

    Effect::from_async_fn(move |env| async move {
        let variables = prepare_variables(&item, &env.config);

        for command in &env.agent_template {
            env.executor
                .execute_in_worktree(command, &variables, &worktree)
                .await?;
        }

        Ok(CommandResult {
            worktree: worktree.clone(),
            variables,
        })
    })
}

// src/cook/execution/mapreduce/effects/merge.rs

/// Effect: Merge worktree results to parent
pub fn merge_to_parent_effect(
    worktree: &Worktree,
) -> Effect<(), MergeError, MapEnv> {
    let worktree = worktree.clone();

    Effect::from_async_fn(move |env| async move {
        env.worktree_manager
            .merge_to_parent(&worktree)
            .await
            .with_context(|| format!("Merging worktree {}", worktree.name))
    })
}
```

#### 3. Effect Composition for Agent Execution

```rust
// src/cook/execution/mapreduce/phases/map.rs

use stillwater::Effect;

/// Effect: Execute single agent (composed I/O chain)
fn execute_agent(
    assignment: WorkAssignment,
) -> Effect<AgentResult, AgentError, MapEnv> {
    // Compose: create worktree → execute → merge → return result
    create_worktree_effect(&assignment.worktree_name)
        .and_then(|worktree| {
            execute_commands_effect(&assignment.item, &worktree)
                .map(move |result| (worktree, result))
        })
        .and_then(|(worktree, result)| {
            merge_to_parent_effect(&worktree)
                .map(move |_| AgentResult {
                    id: assignment.id,
                    result: result.variables,
                })
        })
        .context(format!("Executing agent {}", assignment.id))
}

/// Parallel work distribution with bounded concurrency
async fn distribute_work(
    &self,
    items: Vec<Value>,
    env: &MapEnv,
) -> Result<Vec<AgentResult>, PhaseError> {
    // Pure planning (testable without I/O)
    let assignments = plan_work_assignments(items, &self.config);

    // Functional parallel execution
    let effects: Vec<_> = assignments
        .into_iter()
        .map(|assignment| execute_agent(assignment))
        .collect();

    // Execute with concurrency limit
    let results = Effect::par_all_limit(effects, self.max_parallel)
        .run_async(env)
        .await?;

    Ok(results)
}
```

#### 4. Dependency-Based Parallelization

```rust
// src/cook/execution/mapreduce/pure/dependency_analysis.rs

/// Pure: Analyze command dependencies
pub fn analyze_dependencies(commands: &[Command]) -> CommandGraph {
    let mut graph = CommandGraph::new();

    for (idx, cmd) in commands.iter().enumerate() {
        let reads = extract_variable_reads(cmd);
        let writes = extract_variable_writes(cmd);

        // Add dependencies: command depends on prior writers
        for prior_idx in 0..idx {
            let prior_writes = extract_variable_writes(&commands[prior_idx]);
            if reads.iter().any(|r| prior_writes.contains(r)) {
                graph.add_edge(prior_idx, idx);
            }
        }
    }

    graph
}

/// Pure: Extract parallel execution batches
impl CommandGraph {
    pub fn parallel_batches(&self) -> Vec<Vec<usize>> {
        let mut batches = Vec::new();
        let mut remaining: HashSet<_> = (0..self.node_count()).collect();

        while !remaining.is_empty() {
            // Find nodes with no unsatisfied dependencies
            let ready: Vec<_> = remaining
                .iter()
                .filter(|&&idx| {
                    self.dependencies(idx)
                        .all(|dep| !remaining.contains(&dep))
                })
                .copied()
                .collect();

            batches.push(ready.clone());

            for idx in ready {
                remaining.remove(&idx);
            }
        }

        batches
    }
}

// src/cook/execution/mapreduce/phases/setup.rs

/// Effect: Parallel setup execution
async fn execute_setup_commands(
    commands: Vec<Command>,
    env: &PhaseEnv,
) -> Result<PhaseResult, PhaseError> {
    // Pure dependency analysis
    let graph = analyze_dependencies(&commands);

    // Execute batches in parallel
    for batch in graph.parallel_batches() {
        let effects: Vec<_> = batch
            .iter()
            .map(|&idx| execute_command_effect(&commands[idx]))
            .collect();

        // All commands in batch run in parallel
        Effect::par_all(effects).run_async(env).await?;
    }

    Ok(PhaseResult::success())
}
```

### Architecture Changes

**New Modules:**
```
src/cook/execution/mapreduce/
├── pure/
│   ├── work_planning.rs       # Pure work assignment planning
│   └── dependency_analysis.rs # Pure command dependency graph
├── effects/
│   ├── worktree.rs            # Worktree I/O effects
│   ├── commands.rs            # Command execution effects
│   └── merge.rs               # Merge I/O effects
└── phases/
    ├── map.rs                 # Effect-based map phase (refactored)
    ├── setup.rs               # Parallel setup (refactored)
    └── reduce.rs              # Parallel reduce (refactored)
```

**Environment Types:**
```rust
// src/cook/execution/mapreduce/environment.rs

#[derive(Clone)]
pub struct MapEnv {
    pub config: MapConfig,
    pub worktree_manager: WorktreeManager,
    pub executor: CommandExecutor,
    pub agent_template: Vec<Command>,
    pub storage: Arc<Storage>,
}

#[derive(Clone)]
pub struct PhaseEnv {
    pub config: PhaseConfig,
    pub executor: CommandExecutor,
    pub storage: Arc<Storage>,
    pub variables: Arc<Variables>,
}
```

### Data Structures

**Work Assignment:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct WorkAssignment {
    pub id: usize,
    pub item: Value,
    pub worktree_name: String,
}
```

**Command Graph:**
```rust
pub struct CommandGraph {
    nodes: Vec<CommandNode>,
    edges: HashMap<usize, HashSet<usize>>,
}

struct CommandNode {
    reads: HashSet<String>,
    writes: HashSet<String>,
}
```

### APIs and Interfaces

**Public API:**
```rust
// Pure functions (no I/O)
pub fn plan_work_assignments(
    items: Vec<Value>,
    config: &MapConfig,
) -> Vec<WorkAssignment>;

pub fn analyze_dependencies(commands: &[Command]) -> CommandGraph;

// Effects (I/O)
pub fn create_worktree_effect(name: &str) -> Effect<Worktree, WorktreeError, MapEnv>;
pub fn execute_commands_effect(item: &Value, worktree: &Worktree)
    -> Effect<CommandResult, CommandError, MapEnv>;
pub fn merge_to_parent_effect(worktree: &Worktree)
    -> Effect<(), MergeError, MapEnv>;
```

## Dependencies

### Prerequisites
- **Spec 172** completed (Stillwater foundation integration)
- Stillwater `Effect::par_all` and `Effect::par_all_limit` available
- Existing MapReduce functionality working

### Affected Components
- `src/cook/execution/mapreduce/phases/map.rs` - Complete refactor
- `src/cook/execution/mapreduce/phases/setup.rs` - Add parallelization
- `src/cook/execution/mapreduce/phases/reduce.rs` - Add parallelization
- All MapReduce tests - Update for new structure

### External Dependencies
- `stillwater = "0.2.0"` (Effect, par_all, par_all_limit)

## Testing Strategy

### Unit Tests

**Pure Work Planning:**
```rust
#[test]
fn test_plan_work_assignments_applies_filters() {
    let items = vec![
        json!({"type": "a", "value": 1}),
        json!({"type": "b", "value": 2}),
        json!({"type": "a", "value": 3}),
    ];

    let config = MapConfig {
        filter: Some(Filter::Eq("type", "a")),
        offset: 0,
        max_items: None,
    };

    let assignments = plan_work_assignments(items, &config);

    assert_eq!(assignments.len(), 2);
    assert_eq!(assignments[0].item["value"], 1);
    assert_eq!(assignments[1].item["value"], 3);
}

#[test]
fn test_plan_work_assignments_applies_offset_and_limit() {
    let items = vec![json!(1), json!(2), json!(3), json!(4), json!(5)];

    let config = MapConfig {
        filter: None,
        offset: 1,
        max_items: Some(2),
    };

    let assignments = plan_work_assignments(items, &config);

    assert_eq!(assignments.len(), 2);
    assert_eq!(assignments[0].item, json!(2));
    assert_eq!(assignments[1].item, json!(3));
}
```

**Dependency Analysis:**
```rust
#[test]
fn test_analyze_dependencies_detects_independence() {
    let commands = vec![
        Command::Shell { cmd: "echo $A".into() },  // Reads A
        Command::Shell { cmd: "B=1".into() },      // Writes B
        Command::Shell { cmd: "C=2".into() },      // Writes C
    ];

    let graph = analyze_dependencies(&commands);
    let batches = graph.parallel_batches();

    // Command 0 must be first (reads A)
    // Commands 1 and 2 can run in parallel
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0], vec![0]);
    assert_eq!(batches[1].len(), 2);
    assert!(batches[1].contains(&1) && batches[1].contains(&2));
}
```

### Effect Tests with Mock Environment

```rust
#[tokio::test]
async fn test_execute_agent_composition() {
    let mock_env = MockMapEnv {
        worktree_manager: MockWorktreeManager::default(),
        executor: MockExecutor::default(),
        // ...
    };

    let assignment = WorkAssignment {
        id: 0,
        item: json!({"test": true}),
        worktree_name: "agent-0".into(),
    };

    let effect = execute_agent(assignment);
    let result = effect.run_async(&mock_env).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().id, 0);
}
```

### Integration Tests

**Parallel Execution Timing:**
```rust
#[tokio::test]
async fn test_parallel_execution_shows_speedup() {
    // 10 work items, each takes 1 second, max_parallel = 5
    let items = create_slow_work_items(10, Duration::from_secs(1));

    let start = Instant::now();
    let result = distribute_work(items, max_parallel = 5).await;
    let elapsed = start.elapsed();

    assert!(result.is_ok());

    // Should complete in ~2 seconds (10 items / 5 parallel)
    // Allow 30% overhead for scheduling
    assert!(elapsed < Duration::from_secs(3));
    assert!(elapsed > Duration::from_secs(2));
}
```

### Property Tests

**Order Independence:**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_parallel_execution_order_independent(
        items in prop::collection::vec(any::<Value>(), 1..20)
    ) {
        let env = create_deterministic_env();

        let result1 = distribute_work(items.clone(), &env).await.unwrap();
        let result2 = distribute_work(items, &env).await.unwrap();

        // Results should be same regardless of execution order
        prop_assert_eq!(sort_by_id(result1), sort_by_id(result2));
    }
}
```

### Performance Tests

**Benchmark parallel speedup:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_parallel_execution(c: &mut Criterion) {
    let items = create_work_items(100);

    let mut group = c.benchmark_group("parallel_execution");

    for &parallelism in &[1, 2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::from_parameter(parallelism),
            &parallelism,
            |b, &parallelism| {
                b.to_async(Runtime::new().unwrap()).iter(|| {
                    distribute_work(black_box(items.clone()), parallelism)
                })
            },
        );
    }

    group.finish();
}
```

## Documentation Requirements

### Code Documentation

**Effect composition examples:**
```rust
/// Executes a single agent with composed effects.
///
/// This function demonstrates the "pure core, imperative shell" pattern:
/// 1. Create worktree (I/O effect)
/// 2. Execute commands (I/O effect)
/// 3. Merge results (I/O effect)
///
/// Effects are composed using `and_then` and `map`, creating a linear
/// chain of operations that executes asynchronously when `run_async` is called.
///
/// # Example
///
/// ```rust
/// let assignment = WorkAssignment { id: 0, item: json!({"test": true}), worktree_name: "agent-0".into() };
/// let env = MapEnv { /* ... */ };
///
/// let effect = execute_agent(assignment);
/// let result = effect.run_async(&env).await?;
/// ```
fn execute_agent(assignment: WorkAssignment) -> Effect<AgentResult, AgentError, MapEnv>
```

### User Documentation

**Update CLAUDE.md with Effect patterns:**
- Document Effect-based I/O approach
- Explain pure planning vs imperative execution
- Provide examples of effect composition
- Show how to test effects with mock environments

### Architecture Updates

**Update ARCHITECTURE.md:**
- Add section on Effect-based parallelism
- Document pure/effects module separation
- Show data flow through effect chains
- Explain dependency-based parallelization

## Implementation Notes

### Critical Success Factors
1. **Pure planning is testable** - No I/O in planning functions
2. **Effect composition is linear** - Clear chains, not nested callbacks
3. **Parallelism is effective** - Actual speedup from parallel execution
4. **Testing is easy** - Mock environments work seamlessly

### Gotchas and Pitfalls
- **Environment cloning**: Ensure MapEnv is cheaply cloneable (Arc)
- **Effect capturing**: Be careful with move semantics in closures
- **Error propagation**: Use `context` for debugging effect chains
- **Dependency analysis**: Conservative (false dependencies okay, missing deps not okay)

### Best Practices
- Keep pure functions in `pure/` module
- Keep effects in `effects/` module
- Use `and_then` for sequencing, `map` for transforming
- Add context to effects for debugging
- Test pure functions without I/O
- Test effects with mock environments

## Migration and Compatibility

### Breaking Changes
- **None** - Internal refactoring only
- Public APIs unchanged
- Workflow files unchanged

### Backward Compatibility
- All existing MapReduce workflows work without modification
- Checkpoint format unchanged
- Resume functionality preserved
- Performance characteristics maintained or improved

### Migration Steps
1. Create pure planning and dependency analysis modules
2. Create effect modules for worktree, commands, merge
3. Refactor map phase to use effects
4. Add parallelization to setup/reduce phases
5. Update tests for new structure
6. Run performance benchmarks
7. Update documentation

### Rollback Strategy
If issues arise:
1. Revert to manual async coordination
2. Remove effect-based implementation
3. Restore original map/setup/reduce phases
4. Redeploy previous version

**Rollback impact:** Lose testability improvements, return to complex async code.
