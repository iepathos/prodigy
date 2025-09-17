---
number: 90
title: MapReduce Phase Execution Module Extraction
category: optimization
priority: high
status: draft
dependencies: [87, 88, 89]
created: 2025-09-17
---

# Specification 90: MapReduce Phase Execution Module Extraction

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [87 - Agent Module, 88 - Command Module, 89 - Resource Module]

## Context

The MapReduce executor contains complex orchestration logic for executing setup, map, and reduce phases. The `execute_reduce_phase` method is 285 lines long with deeply nested logic for user interaction, variable interpolation, and command execution. This high-level orchestration is mixed with low-level implementation details, making it difficult to understand the overall flow and modify phase execution strategies.

## Objective

Extract phase execution logic into a dedicated module that provides clear orchestration for setup, map, and reduce phases. This will separate high-level workflow coordination from implementation details, making it easier to understand phase transitions, modify execution strategies, and add new phase types.

## Requirements

### Functional Requirements
- Extract setup phase execution and validation
- Separate map phase orchestration and item distribution
- Isolate reduce phase execution with result aggregation
- Support phase skipping and conditional execution
- Maintain phase transition state and checkpointing
- Preserve interactive user prompts for reduce phase

### Non-Functional Requirements
- Each phase executor should focus on orchestration only
- Delegate implementation details to other modules
- Support async execution without blocking
- Enable phase execution monitoring
- Maintain current execution semantics

## Acceptance Criteria

- [ ] Phase execution module created at `src/cook/execution/mapreduce/phases/`
- [ ] Setup phase executor in `phases/setup.rs`
- [ ] Map phase orchestrator in `phases/map.rs`
- [ ] Reduce phase executor in `phases/reduce.rs`
- [ ] Phase coordinator in `phases/coordinator.rs`
- [ ] `execute_reduce_phase` reduced to under 50 lines
- [ ] Main module reduced by approximately 500 lines
- [ ] All phase execution tests pass
- [ ] Phase transition logic clearly documented
- [ ] Support for custom phase types demonstrated

## Technical Details

### Implementation Approach

1. **Module Structure**:
   ```
   src/cook/execution/mapreduce/phases/
   ├── mod.rs          # Module exports and phase registry
   ├── coordinator.rs  # Phase transition and orchestration
   ├── setup.rs        # Setup phase execution
   ├── map.rs          # Map phase orchestration
   └── reduce.rs       # Reduce phase execution
   ```

2. **Key Extractions**:
   - `execute_setup_phase` → `setup.rs`
   - `execute_map_phase*` family → `map.rs`
   - `execute_reduce_phase` (285 lines) → `reduce.rs`
   - Phase transition logic → `coordinator.rs`

### Architecture Changes

- Implement phase executor trait for extensibility
- Use state machine for phase transitions
- Create phase context for sharing data between phases
- Separate orchestration from execution

### Data Structures

```rust
pub trait PhaseExecutor: Send + Sync {
    async fn execute(
        &self,
        context: &mut PhaseContext,
    ) -> Result<PhaseResult, PhaseError>;

    fn phase_type(&self) -> PhaseType;
}

pub struct PhaseContext {
    pub variables: HashMap<String, Value>,
    pub map_results: Option<Vec<AgentResult>>,
    pub checkpoint: Option<PhaseCheckpoint>,
    pub user_interaction: Arc<dyn UserInteraction>,
}

pub struct PhaseCoordinator {
    phases: Vec<Box<dyn PhaseExecutor>>,
    state_manager: Arc<dyn JobStateManager>,
}

pub enum PhaseTransition {
    Continue(PhaseType),
    Skip(PhaseType),
    Complete,
    Error(PhaseError),
}
```

### APIs and Interfaces

```rust
impl PhaseCoordinator {
    pub async fn execute_workflow(
        &self,
        config: &MapReduceConfig,
    ) -> Result<WorkflowResult, WorkflowError>;

    pub async fn resume_from_checkpoint(
        &self,
        checkpoint: PhaseCheckpoint,
    ) -> Result<WorkflowResult, WorkflowError>;
}

pub trait PhaseTransitionHandler {
    fn should_execute(&self, phase: PhaseType, context: &PhaseContext) -> bool;
    fn on_phase_complete(&self, phase: PhaseType, result: &PhaseResult);
    fn on_phase_error(&self, phase: PhaseType, error: &PhaseError) -> PhaseTransition;
}
```

## Dependencies

- **Prerequisites**:
  - Phase 1: Utils module (completed)
  - Phase 2: Agent module (spec 87)
  - Phase 3: Command module (spec 88)
  - Phase 4: Resource module (spec 89)
- **Affected Components**: MapReduceExecutor, state management, user interaction
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test each phase executor independently
- **Integration Tests**: Verify phase transitions and data flow
- **State Tests**: Validate checkpoint and resume functionality
- **Error Tests**: Ensure proper error handling between phases
- **User Interaction Tests**: Mock user input for reduce phase

## Documentation Requirements

- **Code Documentation**: Document phase lifecycle and transitions
- **Architecture Updates**: Phase execution flow diagrams
- **User Guide**: Customizing phase execution
- **Developer Guide**: Adding custom phases

## Implementation Notes

- Start with phase trait and context definition
- Implement setup phase first (simplest)
- Use composition to delegate to existing modules
- Ensure phase context is immutable where possible
- Consider using state machine library for transitions
- Maintain detailed phase execution logs

## Migration and Compatibility

- No changes to workflow file format
- Existing MapReduce workflows continue to work
- Internal refactoring maintains compatibility
- Phase execution remains semantically identical
- Consider feature flags for new phase types