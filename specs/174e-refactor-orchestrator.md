---
number: 174e
title: Refactor Orchestrator
category: foundation
priority: high
status: draft
dependencies: [174a, 174d]
parent: 174
created: 2025-11-24
---

# Specification 174e: Refactor Orchestrator

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Specs 174a (Pure Execution Planning), 174d (Effect Modules)
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the fifth phase of Spec 174. The orchestrator (`src/cook/orchestrator/core.rs`, 2,884 LOC) mixes planning, decision-making, and I/O. Now that we have pure planning (174a) and effects (174d), we refactor the orchestrator to use them.

**Goal**: Reduce orchestrator from 2,884 LOC to < 500 LOC by delegating to pure planning and effect composition.

## Objective

Refactor orchestrator to:
- Use pure execution planning from 174a
- Compose effects for I/O coordination
- Eliminate inlined business logic
- Focus on I/O coordination only

## Requirements

### Functional Requirements

#### FR1: Use Pure Planning
- **MUST** call `plan_execution()` from 174a at workflow start
- **MUST** use `ExecutionPlan` to drive all decisions
- **MUST** eliminate inlined mode detection logic
- **MUST** eliminate inlined resource calculation logic

#### FR2: Effect Composition
- **MUST** create `setup_environment_effect` for initialization
- **MUST** create `execute_plan_effect` for execution
- **MUST** create `finalize_session_effect` for cleanup
- **MUST** compose effects with `and_then`

#### FR3: LOC Reduction
- **MUST** reduce orchestrator core to < 500 LOC
- **MUST** move all business logic to pure modules
- **MUST** keep only I/O coordination in orchestrator

#### FR4: Functionality Preservation
- **MUST** maintain all existing orchestration features
- **MUST** pass all existing orchestrator tests
- **MUST** preserve error handling behavior

### Non-Functional Requirements

#### NFR1: Code Quality
- **MUST** reduce from 2,884 LOC to < 500 LOC
- **MUST** improve readability through separation
- **MUST** pass clippy with no warnings

#### NFR2: Performance
- **MUST** show no performance regression
- **MUST** maintain existing benchmarks

## Acceptance Criteria

- [ ] Orchestrator core reduced to < 500 LOC
- [ ] Uses `plan_execution()` from 174a
- [ ] All business logic moved to pure modules
- [ ] Effect composition for I/O coordination
- [ ] All existing tests pass
- [ ] No performance regression
- [ ] `cargo fmt` and `cargo clippy` pass

## Technical Details

### Refactored Orchestrator

```rust
// src/cook/orchestrator/core.rs (~400 LOC)

use stillwater::Effect;
use crate::core::orchestration::execution_planning;

impl CookOrchestrator {
    /// Run workflow (I/O coordination only)
    pub async fn run(&self, config: CookConfig) -> Result<ExecutionResult> {
        // Pure planning (no I/O)
        let plan = execution_planning::plan_execution(&config);

        // Effect composition (I/O)
        let effect = self.setup_environment(&plan)
            .and_then(|env| self.execute_plan(&plan, env))
            .and_then(|result| self.finalize_session(result))
            .context("Running workflow");

        // Execute at boundary
        effect.run_async(&self.dependencies).await
    }

    fn setup_environment(&self, plan: &ExecutionPlan)
        -> Effect<ExecutionEnvironment, CookError, Dependencies>
    {
        Effect::from_async_fn(|deps| async move {
            let session = deps.session_manager.create_session().await?;
            let worktree = if plan.resource_needs.worktrees > 0 {
                Some(deps.worktree_manager.create_parent().await?)
            } else {
                None
            };

            Ok(ExecutionEnvironment { session, worktree, variables: HashMap::new() })
        })
    }

    fn execute_plan(&self, plan: &ExecutionPlan, env: ExecutionEnvironment)
        -> Effect<ExecutionResult, CookError, Dependencies>
    {
        match plan.mode {
            ExecutionMode::MapReduce => self.execute_mapreduce(plan, env),
            ExecutionMode::Standard => self.execute_standard(plan, env),
            ExecutionMode::Iterative => self.execute_iterative(plan, env),
            ExecutionMode::DryRun => self.execute_dry_run(plan, env),
        }
    }

    fn finalize_session(&self, result: ExecutionResult)
        -> Effect<ExecutionResult, CookError, Dependencies>
    {
        Effect::from_async_fn(move |deps| async move {
            deps.session_manager.finalize_session(&result.session_id).await?;
            if let Some(worktree) = result.worktree {
                deps.worktree_manager.cleanup(&worktree).await?;
            }
            Ok(result)
        })
    }
}
```

## Testing Strategy

- Run full orchestrator test suite
- Verify all modes still work (MapReduce, Standard, Iterative, DryRun)
- Test error handling paths
- Benchmark performance

## Implementation Notes

### Migration Path
1. Add calls to pure planning functions
2. Create effect composition helpers
3. Gradually remove inlined logic
4. Move logic to pure modules
5. Verify LOC reduction
6. Run full test suite
7. Benchmark performance
8. Commit

### Critical Success Factors
1. **LOC reduction** - Must achieve < 500 LOC
2. **Test preservation** - All tests must pass
3. **No regression** - Performance maintained
4. **Clean composition** - Effect chains readable

## Dependencies

### Prerequisites
- **174a** - Pure execution planning (required)
- **174d** - Effect modules (required)

### Blocks
- None - can proceed after 174a and 174d

## Success Metrics

- [ ] All 7 acceptance criteria met
- [ ] LOC reduced from 2,884 to < 500
- [ ] All tests pass
- [ ] Zero performance regression
