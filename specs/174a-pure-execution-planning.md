---
number: 174a
title: Pure Execution Planning Module
category: foundation
priority: high
status: draft
dependencies: [172, 173]
parent: 174
created: 2025-11-24
---

# Specification 174a: Pure Execution Planning Module

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 172 (Stillwater Foundation), Spec 173 (Parallel Execution Effects)
**Parent**: Spec 174 (Pure Core Extraction)

## Context

This is the first phase of Spec 174 (Pure Core Extraction). The orchestrator currently mixes execution planning logic with I/O operations in a 2,884 LOC file. This spec extracts the pure planning logic into testable, reusable functions.

**Scope**: Create pure execution planning module only. No orchestrator refactoring yet (that's in 174e).

## Objective

Extract execution planning logic from the orchestrator into pure, testable functions:
- Mode detection (DryRun, MapReduce, Iterative, Standard)
- Resource calculation (worktrees, memory, disk, concurrency limits)
- Phase determination (setup, map, reduce, commands)
- Create data structures for execution plans

## Requirements

### Functional Requirements

#### FR1: Execution Planning Data Structures
- **MUST** create `ExecutionPlan` struct with mode, resources, phases, parallel budget
- **MUST** create `ExecutionMode` enum: Standard, MapReduce, Iterative, DryRun
- **MUST** create `ResourceRequirements` struct with worktrees, memory, disk, concurrency
- **MUST** create `Phase` enum for execution phases

#### FR2: Mode Detection
- **MUST** implement `detect_execution_mode(config: &CookConfig) -> ExecutionMode`
- **MUST** detect dry run mode from config.command.dry_run
- **MUST** detect MapReduce mode from config.mapreduce.is_some()
- **MUST** detect Iterative mode from config.command.arguments.is_some()
- **MUST** default to Standard mode

#### FR3: Resource Calculation
- **MUST** implement `calculate_resources(config: &CookConfig, mode: &ExecutionMode) -> ResourceRequirements`
- **MUST** calculate worktrees: MapReduce = max_parallel + 1, Iterative = 1, else 0
- **MUST** estimate memory based on mode and iterations
- **MUST** estimate disk space for MapReduce
- **MUST** determine max concurrent commands

#### FR4: Phase Determination
- **MUST** implement `determine_phases(config: &CookConfig, mode: &ExecutionMode) -> Vec<Phase>`
- **MUST** return Setup, Map, Reduce phases for MapReduce mode
- **MUST** return Commands phase for Standard/Iterative modes
- **MUST** return DryRunAnalysis phase for DryRun mode

#### FR5: Execution Planning
- **MUST** implement `plan_execution(config: &CookConfig) -> ExecutionPlan`
- **MUST** compose mode detection, resource calculation, phase determination
- **MUST** calculate parallel budget from resources
- **MUST** be pure (no I/O, deterministic)

### Non-Functional Requirements

#### NFR1: Purity
- **MUST** have zero I/O operations (no file reads, network, etc.)
- **MUST** be deterministic (same input → same output)
- **MUST** have no side effects
- **MUST** pass clippy with no warnings

#### NFR2: Testability
- **MUST** achieve 100% test coverage
- **MUST** require zero mocking in tests
- **MUST** have tests that run in < 1ms per test
- **MUST** include property tests for determinism

## Acceptance Criteria

- [ ] Module created at `src/core/orchestration/`
- [ ] `execution_planning.rs` with `plan_execution()` function
- [ ] `mode_detection.rs` with `detect_execution_mode()` function
- [ ] `resource_allocation.rs` with `calculate_resources()` function
- [ ] All data structures defined (ExecutionPlan, ExecutionMode, ResourceRequirements, Phase)
- [ ] Unit tests achieve 100% coverage
- [ ] No mocking used in any test
- [ ] Property tests verify determinism
- [ ] All tests pass in < 100ms total
- [ ] `cargo fmt` and `cargo clippy` pass with no warnings
- [ ] Module properly exposed in `src/core/mod.rs`

## Technical Details

### Module Structure

```
src/core/orchestration/
├── mod.rs                    # Module exports
├── execution_planning.rs     # Main planning function
├── mode_detection.rs         # Mode classification
└── resource_allocation.rs    # Resource calculations
```

### Data Structures

```rust
// src/core/orchestration/execution_planning.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub mode: ExecutionMode,
    pub resource_needs: ResourceRequirements,
    pub phases: Vec<Phase>,
    pub parallel_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionMode {
    Standard,
    MapReduce,
    Iterative,
    DryRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRequirements {
    pub worktrees: usize,
    pub memory_estimate: usize,
    pub disk_space: usize,
    pub max_concurrent_commands: usize,
}

impl ResourceRequirements {
    pub fn minimal() -> Self {
        Self {
            worktrees: 0,
            memory_estimate: 0,
            disk_space: 0,
            max_concurrent_commands: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Setup(Vec<Command>),
    Map(MapConfig),
    Reduce(Vec<Command>),
    Commands(Vec<Command>),
    DryRunAnalysis,
}
```

### Core Functions

```rust
/// Pure: Plan execution from config
pub fn plan_execution(config: &CookConfig) -> ExecutionPlan {
    let mode = mode_detection::detect_execution_mode(config);
    let resource_needs = resource_allocation::calculate_resources(config, &mode);
    let phases = determine_phases(config, &mode);
    let parallel_budget = compute_parallel_budget(&resource_needs);

    ExecutionPlan {
        mode,
        resource_needs,
        phases,
        parallel_budget,
    }
}

/// Pure: Compute parallel execution budget
fn compute_parallel_budget(resources: &ResourceRequirements) -> usize {
    resources.max_concurrent_commands
}

/// Pure: Determine execution phases
fn determine_phases(config: &CookConfig, mode: &ExecutionMode) -> Vec<Phase> {
    match mode {
        ExecutionMode::MapReduce => {
            let mr = config.mapreduce.as_ref().unwrap();
            vec![
                Phase::Setup(mr.setup.clone()),
                Phase::Map(mr.map.clone()),
                Phase::Reduce(mr.reduce.clone()),
            ]
        }
        ExecutionMode::Standard | ExecutionMode::Iterative => {
            vec![Phase::Commands(config.commands.clone())]
        }
        ExecutionMode::DryRun => {
            vec![Phase::DryRunAnalysis]
        }
    }
}
```

```rust
// src/core/orchestration/mode_detection.rs

use crate::core::config::CookConfig;
use super::execution_planning::ExecutionMode;

/// Pure: Detect execution mode from config
pub fn detect_execution_mode(config: &CookConfig) -> ExecutionMode {
    if config.command.dry_run {
        ExecutionMode::DryRun
    } else if config.mapreduce.is_some() {
        ExecutionMode::MapReduce
    } else if config.command.arguments.is_some() {
        ExecutionMode::Iterative
    } else {
        ExecutionMode::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_dry_run_mode() {
        let config = CookConfig {
            command: Command { dry_run: true, ..Default::default() },
            ..Default::default()
        };
        assert_eq!(detect_execution_mode(&config), ExecutionMode::DryRun);
    }

    #[test]
    fn test_detect_mapreduce_mode() {
        let config = CookConfig {
            mapreduce: Some(MapReduceConfig::default()),
            ..Default::default()
        };
        assert_eq!(detect_execution_mode(&config), ExecutionMode::MapReduce);
    }

    #[test]
    fn test_detect_iterative_mode() {
        let config = CookConfig {
            command: Command {
                arguments: Some(vec!["arg1".to_string()]),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(detect_execution_mode(&config), ExecutionMode::Iterative);
    }

    #[test]
    fn test_detect_standard_mode() {
        let config = CookConfig::default();
        assert_eq!(detect_execution_mode(&config), ExecutionMode::Standard);
    }
}
```

```rust
// src/core/orchestration/resource_allocation.rs

use crate::core::config::CookConfig;
use super::execution_planning::{ExecutionMode, ResourceRequirements};

/// Pure: Calculate resource requirements
pub fn calculate_resources(
    config: &CookConfig,
    mode: &ExecutionMode,
) -> ResourceRequirements {
    match mode {
        ExecutionMode::MapReduce => calculate_mapreduce_resources(config),
        ExecutionMode::Iterative => calculate_iterative_resources(config),
        _ => ResourceRequirements::minimal(),
    }
}

fn calculate_mapreduce_resources(config: &CookConfig) -> ResourceRequirements {
    let mr = config.mapreduce.as_ref().unwrap();
    ResourceRequirements {
        worktrees: mr.max_parallel + 1, // +1 for parent
        memory_estimate: estimate_memory(mr.max_parallel),
        disk_space: estimate_disk_space(mr.max_parallel),
        max_concurrent_commands: mr.max_parallel,
    }
}

fn calculate_iterative_resources(config: &CookConfig) -> ResourceRequirements {
    let iterations = config.command.arguments.as_ref().map(|a| a.len()).unwrap_or(0);
    ResourceRequirements {
        worktrees: 1,
        memory_estimate: iterations * 50_000_000, // 50MB per iteration
        disk_space: 0,
        max_concurrent_commands: 1,
    }
}

fn estimate_memory(max_parallel: usize) -> usize {
    max_parallel * 100_000_000 // 100MB per parallel task
}

fn estimate_disk_space(max_parallel: usize) -> usize {
    max_parallel * 500_000_000 // 500MB per worktree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_mapreduce_resources() {
        let config = CookConfig {
            mapreduce: Some(MapReduceConfig {
                max_parallel: 10,
                ..Default::default()
            }),
            ..Default::default()
        };
        let mode = ExecutionMode::MapReduce;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources.worktrees, 11); // 10 + parent
        assert_eq!(resources.max_concurrent_commands, 10);
        assert!(resources.memory_estimate > 0);
        assert!(resources.disk_space > 0);
    }

    #[test]
    fn test_calculate_iterative_resources() {
        let config = CookConfig {
            command: Command {
                arguments: Some(vec!["a".into(), "b".into(), "c".into()]),
                ..Default::default()
            },
            ..Default::default()
        };
        let mode = ExecutionMode::Iterative;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources.worktrees, 1);
        assert_eq!(resources.max_concurrent_commands, 1);
        assert_eq!(resources.memory_estimate, 3 * 50_000_000);
    }

    #[test]
    fn test_minimal_resources_for_standard_mode() {
        let config = CookConfig::default();
        let mode = ExecutionMode::Standard;

        let resources = calculate_resources(&config, &mode);

        assert_eq!(resources, ResourceRequirements::minimal());
    }
}
```

## Testing Strategy

### Unit Tests (No Mocking!)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_execution_mapreduce() {
        let config = CookConfig {
            mapreduce: Some(MapReduceConfig {
                max_parallel: 10,
                setup: vec![],
                map: MapConfig::default(),
                reduce: vec![],
            }),
            commands: vec![],
            command: Command::default(),
        };

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::MapReduce);
        assert_eq!(plan.parallel_budget, 10);
        assert_eq!(plan.resource_needs.worktrees, 11);
        assert_eq!(plan.phases.len(), 3); // Setup, Map, Reduce
    }

    #[test]
    fn test_plan_execution_standard() {
        let config = CookConfig {
            commands: vec![Command::default()],
            ..Default::default()
        };

        let plan = plan_execution(&config);

        assert_eq!(plan.mode, ExecutionMode::Standard);
        assert_eq!(plan.parallel_budget, 1);
        assert_eq!(plan.phases.len(), 1);
    }
}
```

### Property Tests

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_planning_is_deterministic(max_parallel in 1usize..100) {
            let config = CookConfig {
                mapreduce: Some(MapReduceConfig {
                    max_parallel,
                    ..Default::default()
                }),
                ..Default::default()
            };

            let plan1 = plan_execution(&config);
            let plan2 = plan_execution(&config);

            // Pure function - same input, same output
            prop_assert_eq!(plan1, plan2);
        }

        #[test]
        fn prop_mode_detection_is_consistent(dry_run: bool) {
            let config = CookConfig {
                command: Command { dry_run, ..Default::default() },
                ..Default::default()
            };

            let mode1 = detect_execution_mode(&config);
            let mode2 = detect_execution_mode(&config);

            prop_assert_eq!(mode1, mode2);
        }
    }
}
```

## Implementation Notes

### Critical Success Factors
1. **Zero I/O** - No file reads, network calls, database queries
2. **100% coverage** - Every function fully tested
3. **No mocking** - Tests are simple and direct
4. **Fast tests** - < 1ms per test

### Integration with Existing Code
- Module should compile independently
- Will be consumed by orchestrator in spec 174e
- Can be developed in parallel with 174b and 174c

### Migration Path
1. Create module structure
2. Define data structures
3. Implement mode detection
4. Implement resource calculation
5. Implement phase determination
6. Implement main planning function
7. Write comprehensive unit tests
8. Add property tests
9. Verify no I/O operations
10. Commit and close spec

## Dependencies

### Prerequisites
- Spec 172 (Stillwater Foundation) - for Effect types used later
- Spec 173 (Parallel Execution Effects) - for composition patterns

### Blocks
- Spec 174e (Refactor Orchestrator) - cannot start until this spec completes

### Parallel Work
- Can be developed in parallel with 174b (Pure Workflow Transformations)
- Can be developed in parallel with 174c (Pure Session Updates)

## Documentation Requirements

- Module-level documentation explaining "pure core" pattern
- Function documentation with examples
- Test documentation showing no mocking needed
- Update `src/core/mod.rs` to expose new module

## Success Metrics

- [ ] All 10 acceptance criteria met
- [ ] 100% test coverage achieved
- [ ] All tests pass in < 100ms
- [ ] Zero clippy warnings
- [ ] Module successfully imports in orchestrator (compile check)
