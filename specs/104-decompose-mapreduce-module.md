---
number: 104
title: Decompose Monolithic MapReduce Module
category: foundation
priority: high
status: draft
dependencies: [101, 102]
created: 2025-09-22
---

# Specification 104: Decompose Monolithic MapReduce Module

## Context

The `cook/execution/mapreduce/mod.rs` file is a massive 4,027-line module that mixes state management, execution coordination, agent lifecycle management, and result aggregation. This violates VISION.md principles of functional programming and single responsibility, making it difficult to maintain, test, and extend.

Current issues:
- Complex state machine with unclear transitions
- Mixed concerns: execution, coordination, state management
- 6-level deep directory nesting
- Difficult to test individual components
- Performance bottlenecks from monolithic design

## Objective

Decompose the MapReduce module into focused, composable components following functional programming principles, improving maintainability, testability, and performance while simplifying the architecture.

## Requirements

### Functional Requirements
- Extract state management into dedicated module
- Separate agent lifecycle management
- Extract result aggregation logic
- Create clear interfaces between components
- Maintain all current MapReduce functionality
- Improve parallel execution performance

### Non-Functional Requirements
- Each module under 500 lines
- Clear separation of pure functions from I/O
- Improved test coverage and maintainability
- Better error handling and recovery
- Flatten directory structure to 3-4 levels maximum

## Acceptance Criteria

- [ ] `mapreduce/mod.rs` reduced to under 800 lines
- [ ] State management extracted to `state/` module
- [ ] Agent management extracted to `agents/` module
- [ ] Result aggregation extracted to `aggregation/` module
- [ ] Directory nesting reduced to 4 levels maximum
- [ ] All MapReduce tests pass without modification
- [ ] Performance benchmarks show no regression
- [ ] Clear module boundaries with minimal coupling

## Technical Details

### Proposed Module Structure

```
cook/execution/mapreduce/
├── mod.rs                 # Core coordination (<800 lines)
├── state/
│   ├── machine.rs         # State machine logic
│   ├── transitions.rs     # State transition functions
│   └── persistence.rs     # State persistence
├── agents/
│   ├── lifecycle.rs       # Agent spawn/cleanup
│   ├── pool.rs           # Agent pool management
│   └── monitoring.rs     # Agent health monitoring
├── aggregation/
│   ├── reducer.rs        # Result reduction logic
│   ├── collector.rs      # Result collection
│   └── formatter.rs      # Output formatting
└── types.rs              # Shared types and traits
```

### Implementation Approach

1. **Phase 1: Extract Pure Functions**
   - Identify and extract stateless result aggregation
   - Extract pure state transition functions
   - Create shared type definitions

2. **Phase 2: Separate State Management**
   - Move state machine to dedicated module
   - Implement clear state persistence interface
   - Separate state transitions from I/O operations

3. **Phase 3: Decompose Execution Logic**
   - Extract agent lifecycle management
   - Separate coordination from execution
   - Implement functional composition patterns

### Functional Programming Patterns

```rust
// Before: Mixed concerns in monolithic module
impl MapReduceExecutor {
    async fn execute(&mut self, job: Job) -> Result<Output> {
        // 1000+ lines mixing state, agents, I/O, aggregation
    }
}

// After: Functional composition
pub async fn execute_mapreduce(
    job: Job,
    context: ExecutionContext,
) -> Result<Output> {
    let initial_state = state::initialize(&job)?;
    let agents = agents::spawn_pool(&job.config).await?;

    let execution_result = coordinate_execution(initial_state, agents, &job).await;

    match execution_result {
        Ok(results) => aggregation::reduce_results(results, &job.output_config),
        Err(error) => state::handle_failure(error, &job.recovery_config).await,
    }
}
```

### Directory Structure Simplification

```
// Before: 6-level nesting
cook/execution/mapreduce/command/types/execution/state.rs

// After: 4-level maximum
cook/execution/mapreduce/state/machine.rs
cook/execution/mapreduce/agents/pool.rs
```

## Dependencies

- **Spec 101**: Error handling foundation required
- **Spec 102**: Establishes patterns for module decomposition

## Testing Strategy

- Extract tests alongside module decomposition
- Add integration tests for module interactions
- Performance benchmarks for parallel execution
- Property-based tests for state machine transitions
- Load testing for agent pool management

## Documentation Requirements

- Document new MapReduce architecture
- Update development guide for adding MapReduce features
- Create troubleshooting guide for common issues
- Document performance optimization patterns
- Add examples of extending MapReduce functionality