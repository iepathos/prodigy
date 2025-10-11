---
number: 131
title: MapReduce Executor - Split Phase Modules
category: foundation
priority: high
status: draft
dependencies: [129, 130]
created: 2025-10-11
---

# Specification 131: MapReduce Executor - Split Phase Modules

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: 129 (StepResult fix), 130 (Pure functions extracted)

## Context

With pure functions extracted (Spec 130), the MapReduce executor still contains ~1,600 lines mixing phase-specific logic (setup, map, reduce) with coordination logic. Each phase has distinct responsibilities that should be separated into focused modules.

### Current Phase Mixing

The executor currently interleaves:
- Setup phase execution (lines 260-392)
- Map phase execution (lines 522-689)
- Reduce phase execution (lines 1285-1371)
- Phase coordination and state management

This mixing makes it difficult to:
- Understand individual phase logic
- Test phases in isolation
- Modify one phase without affecting others
- Reuse phase logic in different contexts

## Objective

Extract setup, map, and reduce phase logic into separate focused modules, each with clear interfaces and single responsibility. Reduce executor.rs to ~1,000 lines focused solely on coordination.

## Requirements

### Functional Requirements

**Module 1: `phases/setup.rs`**
- Execute setup phase commands sequentially
- Handle commit validation
- Capture setup outputs for map phase
- Return setup results with file detection

**Module 2: `phases/map.rs`**
- Distribute work items across agents
- Execute agents in parallel with semaphore control
- Collect agent results
- Handle agent failures and DLQ processing

**Module 3: `phases/reduce.rs`**
- Aggregate map phase results
- Execute reduce commands with aggregated data
- Build final output from reduce phase
- Handle reduce failures gracefully

**Module 4: `phases/orchestrator.rs`** (pure logic only)
- Determine phase execution order
- Validate phase dependencies
- Plan resource allocation
- Calculate execution strategy

### Non-Functional Requirements

- Each phase module under 200 lines
- Clear separation: pure orchestration vs. I/O execution
- Thin coordinator delegates to phases
- All existing tests pass unchanged
- No performance regression
- Each module independently testable

## Acceptance Criteria

- [ ] Created `src/cook/execution/mapreduce/phases/` directory
- [ ] Created `setup.rs` (~150 lines) with setup execution
- [ ] Created `map.rs` (~200 lines) with map execution
- [ ] Created `reduce.rs` (~100 lines) with reduce execution
- [ ] Created `orchestrator.rs` (~100 lines, pure logic only)
- [ ] `executor.rs` reduced from ~1,600 to ~1,000 lines
- [ ] Each phase module has integration tests
- [ ] All existing tests pass unchanged
- [ ] Performance benchmarks show < 2% regression
- [ ] Each module has clear API documentation

## Technical Details

### Module Structure

```
src/cook/execution/mapreduce/
├── coordination/
│   ├── mod.rs
│   └── executor.rs         # Slim coordinator (~1,000 lines)
├── phases/
│   ├── mod.rs              # Public API exports
│   ├── orchestrator.rs     # Pure phase planning (~100 lines)
│   ├── setup.rs            # Setup execution (~150 lines)
│   ├── map.rs              # Map execution (~200 lines)
│   └── reduce.rs           # Reduce execution (~100 lines)
└── pure/
    └── ...                 # From Spec 130
```

### Phase Interfaces

**orchestrator.rs** (Pure Logic):

```rust
use super::types::{ExecutionPlan, PhaseSpec};

/// Determine execution order of phases
pub fn plan_phases(
    has_setup: bool,
    has_reduce: bool,
) -> ExecutionPlan {
    ExecutionPlan {
        phases: build_phase_sequence(has_setup, has_reduce),
        parallelism: calculate_parallelism_needs(),
        resource_requirements: estimate_resources(),
    }
}

fn build_phase_sequence(has_setup: bool, has_reduce: bool) -> Vec<PhaseSpec> {
    let mut phases = Vec::new();

    if has_setup {
        phases.push(PhaseSpec::Setup);
    }
    phases.push(PhaseSpec::Map);
    if has_reduce {
        phases.push(PhaseSpec::Reduce);
    }

    phases
}

/// Validate phase configuration
pub fn validate_phase_config(config: &MapReduceConfig) -> Result<()> {
    // Check required fields
    if config.map.agent_template.is_empty() {
        return Err(anyhow!("Map phase requires at least one command"));
    }

    // Validate setup if present
    if let Some(setup) = &config.setup {
        if setup.commands.is_empty() {
            return Err(anyhow!("Setup phase requires at least one command"));
        }
    }

    Ok(())
}
```

**setup.rs** (I/O Operations):

```rust
use crate::cook::execution::mapreduce::types::{SetupPhase, SetupResult};
use crate::cook::orchestrator::ExecutionEnvironment;
use anyhow::Result;

/// Execute setup phase commands
pub async fn execute_setup<E>(
    setup_phase: &SetupPhase,
    executor: &E,
    env: &ExecutionEnvironment,
) -> Result<SetupResult>
where
    E: CommandExecutor,
{
    info!("🔄 Running setup phase...");
    let start = Instant::now();

    let mut captured_outputs = HashMap::new();

    for (index, step) in setup_phase.commands.iter().enumerate() {
        info!("Executing setup step {}/{}", index + 1, setup_phase.commands.len());

        // Track HEAD before for commit validation
        let head_before = if step.commit_required {
            get_current_head(&env.working_dir).await?
        } else {
            String::new()
        };

        // Execute step
        let result = executor.execute_step(step, env).await?;

        // Validate commit if required
        if step.commit_required {
            let head_after = get_current_head(&env.working_dir).await?;
            if head_before == head_after {
                return Err(format_commit_requirement_error(
                    &get_step_display_name(step),
                    result.json_log_location.as_deref(),
                ));
            }
        }

        // Check for failure
        if !result.success {
            return Err(format_setup_error(index, &result, step.claude.is_some()));
        }

        // Capture outputs if configured
        if let Some(var_name) = &step.capture_output {
            captured_outputs.insert(var_name.clone(), result.stdout.clone());
        }
    }

    let duration = start.elapsed();
    info!("✅ Setup phase completed in {:?}", duration);

    Ok(SetupResult {
        captured_outputs,
        duration,
        generated_input_file: detect_generated_input(&env.working_dir).await?,
    })
}

/// Detect if setup generated a work-items.json file
async fn detect_generated_input(working_dir: &Path) -> Result<Option<PathBuf>> {
    let work_items_path = working_dir.join("work-items.json");
    if work_items_path.exists() {
        Ok(Some(work_items_path))
    } else {
        Ok(None)
    }
}

/// Get current git HEAD commit
async fn get_current_head(working_dir: &Path) -> Result<String> {
    use tokio::process::Command;

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(working_dir)
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("git rev-parse HEAD failed");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

**map.rs** (I/O Operations):

```rust
use crate::cook::execution::mapreduce::{
    agent::{AgentConfig, AgentLifecycleManager, AgentResult},
    types::MapPhase,
};
use tokio::sync::Semaphore;
use futures::stream::{self, StreamExt};

/// Execute map phase with parallel agents
pub async fn execute_map<A>(
    map_phase: &MapPhase,
    work_items: Vec<Value>,
    agent_manager: &A,
    parallelism: usize,
) -> Result<Vec<AgentResult>>
where
    A: AgentLifecycleManager,
{
    info!("🔄 Running map phase with {} items", work_items.len());
    let start = Instant::now();

    // Create semaphore for parallelism control
    let semaphore = Arc::new(Semaphore::new(parallelism));

    // Execute agents in parallel
    let results: Vec<AgentResult> = stream::iter(work_items.into_iter().enumerate())
        .map(|(index, item)| {
            let sem = semaphore.clone();
            let manager = agent_manager.clone();
            let commands = map_phase.agent_template.clone();

            async move {
                // Acquire semaphore permit
                let _permit = sem.acquire().await.unwrap();

                // Create agent
                let agent_id = format!("agent-{}", index);
                let config = AgentConfig {
                    agent_id: agent_id.clone(),
                    work_item: item.clone(),
                    commands,
                };

                // Execute agent
                info!("Starting {}", agent_id);
                let result = manager.execute_agent(config).await;

                // Log completion
                if result.is_ok() {
                    info!("✅ {} completed", agent_id);
                } else {
                    warn!("❌ {} failed", agent_id);
                }

                result
            }
        })
        .buffer_unordered(parallelism)
        .collect()
        .await;

    let duration = start.elapsed();
    let successful = results.iter().filter(|r| r.success).count();
    let failed = results.len() - successful;

    info!(
        "✅ Map phase completed: {} successful, {} failed in {:?}",
        successful, failed, duration
    );

    Ok(results)
}

/// Filter successful results
pub fn filter_successful_results(results: &[AgentResult]) -> Vec<&AgentResult> {
    results.iter().filter(|r| r.success).collect()
}

/// Collect failed items for DLQ
pub fn collect_failed_items(results: &[AgentResult]) -> Vec<DeadLetteredItem> {
    results
        .iter()
        .filter(|r| !r.success)
        .map(|r| DeadLetteredItem {
            item: r.work_item.clone(),
            agent_id: r.agent_id.clone(),
            error: r.error.clone(),
            timestamp: Utc::now(),
            attempts: 1,
        })
        .collect()
}
```

**reduce.rs** (I/O Operations):

```rust
use crate::cook::execution::mapreduce::{
    agent::AgentResult,
    types::ReducePhase,
};

/// Execute reduce phase with aggregated results
pub async fn execute_reduce<E>(
    reduce_phase: &ReducePhase,
    map_results: &[AgentResult],
    executor: &E,
    env: &ExecutionEnvironment,
) -> Result<ReduceResult>
where
    E: CommandExecutor,
{
    info!("🔄 Running reduce phase...");
    let start = Instant::now();

    // Build aggregated context
    let context = build_reduce_context(map_results);

    // Execute reduce commands
    let mut outputs = Vec::new();
    for (index, step) in reduce_phase.commands.iter().enumerate() {
        info!("Executing reduce step {}/{}", index + 1, reduce_phase.commands.len());

        // Interpolate with aggregated data
        let interpolated_step = interpolate_step(step, &context)?;

        // Execute step
        let result = executor.execute_step(&interpolated_step, env).await?;

        if !result.success {
            return Err(format_reduce_error(index, &result));
        }

        outputs.push(result.stdout);
    }

    let duration = start.elapsed();
    info!("✅ Reduce phase completed in {:?}", duration);

    Ok(ReduceResult {
        outputs,
        duration,
    })
}

/// Build interpolation context from map results
fn build_reduce_context(results: &[AgentResult]) -> HashMap<String, String> {
    let mut context = HashMap::new();

    // Add statistics
    context.insert("map.total".to_string(), results.len().to_string());
    context.insert(
        "map.successful".to_string(),
        results.iter().filter(|r| r.success).count().to_string(),
    );
    context.insert(
        "map.failed".to_string(),
        results.iter().filter(|r| !r.success).count().to_string(),
    );

    // Collect outputs
    let outputs: Vec<String> = results
        .iter()
        .filter(|r| r.success)
        .filter_map(|r| r.output.clone())
        .collect();

    context.insert("map.outputs".to_string(), outputs.join("\n"));

    context
}

fn format_reduce_error(index: usize, result: &StepResult) -> anyhow::Error {
    anyhow!("Reduce command {} failed: {}", index + 1, result.stderr)
}
```

### Updated Coordinator

**executor.rs** (Coordination Only):

```rust
impl MapReduceCoordinator {
    /// Execute complete MapReduce job
    pub async fn execute(&mut self, config: MapReduceConfig) -> Result<MapReduceResult> {
        // Validate configuration (pure)
        orchestrator::validate_phase_config(&config)?;

        // Plan execution (pure)
        let plan = orchestrator::plan_phases(
            config.setup.is_some(),
            config.reduce.is_some(),
        );

        // Execute phases
        let mut setup_result = None;
        if let Some(setup_phase) = &config.setup {
            setup_result = Some(
                phases::setup::execute_setup(setup_phase, &self.command_executor, &self.env)
                    .await?
            );
        }

        let work_items = self.load_work_items(&config, setup_result.as_ref()).await?;

        let map_results = phases::map::execute_map(
            &config.map,
            work_items,
            &self.agent_manager,
            plan.parallelism,
        )
        .await?;

        let mut reduce_result = None;
        if let Some(reduce_phase) = &config.reduce {
            reduce_result = Some(
                phases::reduce::execute_reduce(reduce_phase, &map_results, &self.command_executor, &self.env)
                    .await?
            );
        }

        Ok(MapReduceResult {
            setup: setup_result,
            map_results,
            reduce: reduce_result,
        })
    }
}
```

## Implementation Steps

1. **Create phases directory and module structure**
2. **Implement orchestrator.rs with pure planning logic**
3. **Extract setup phase to setup.rs**
   - Move setup execution code
   - Add integration tests
   - Update coordinator to use setup module
4. **Extract map phase to map.rs**
   - Move map execution code
   - Add integration tests
   - Update coordinator to use map module
5. **Extract reduce phase to reduce.rs**
   - Move reduce execution code
   - Add integration tests
   - Update coordinator to use reduce module
6. **Simplify coordinator**
   - Remove moved code
   - Update to delegate to phase modules
   - Verify all tests pass

## Testing Strategy

### Unit Tests (New)

Each phase module needs:
- Tests for happy path execution
- Tests for failure scenarios
- Tests for commit validation (setup)
- Tests for parallelism control (map)
- Tests for result aggregation (reduce)

### Integration Tests (Existing)

All existing MapReduce integration tests must pass:
- Full workflow execution
- Error handling
- Checkpoint recovery
- DLQ processing

### Performance Tests

Benchmark phase execution to ensure no regression:
```bash
cargo bench --bench mapreduce_phases
```

## Dependencies

**Prerequisites**:
- Spec 129 (StepResult fix)
- Spec 130 (Pure functions extracted)

**Affected Components**:
- MapReduce coordinator
- Setup phase executor (already exists but will be moved)
- Agent execution
- Event logging

## Implementation Notes

### Why This Split is Important

1. **Single Responsibility**: Each phase module has one clear purpose
2. **Testability**: Phases can be tested independently
3. **Reusability**: Phase logic can be used in other contexts
4. **Maintainability**: Changes to one phase don't affect others
5. **Readability**: Each module is under 200 lines

### Phase Independence

Each phase should:
- Have a clear input/output contract
- Not depend on other phases' internal state
- Be testable in isolation
- Be replaceable with alternative implementations

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing workflows | Extensive integration testing |
| Phase coupling | Clear interfaces, no shared mutable state |
| Circular dependencies | Phases depend only on pure modules and coordinator |
| Performance regression | Benchmark key paths |
| Incomplete extraction | Ensure all phase logic is moved, not copied |

## Success Metrics

- `executor.rs` reduced from ~1,600 to ~1,000 lines (37% reduction)
- 4 new phase modules created, each under 200 lines
- Zero test failures
- Performance within 2% of baseline
- Each phase independently testable
- Clear separation of concerns

## Documentation Requirements

### Code Documentation

- Each phase module must have:
  - Module-level documentation explaining purpose
  - Function documentation with examples
  - Clear description of inputs and outputs
  - Error conditions and handling

### User Documentation

No user-facing changes - internal refactoring only.

### Architecture Updates

Update ARCHITECTURE.md to document:
- Phase module organization
- Phase execution flow
- Interfaces between phases
- How to add new phase types

## Migration and Compatibility

### Breaking Changes

None - All changes are internal implementation details.

### Rollback Plan

Each phase extraction is a separate commit, allowing selective rollback if needed.

### Deployment

Can be deployed immediately with no user impact. Changes are entirely internal.
