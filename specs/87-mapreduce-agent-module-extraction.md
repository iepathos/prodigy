---
number: 87
title: MapReduce Agent Module Extraction
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 87: MapReduce Agent Module Extraction

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The MapReduce executor currently contains 65+ methods with significant complexity, particularly around agent lifecycle management. The agent-related functionality spans approximately 800 lines of tightly coupled code that handles agent creation, execution, retry logic, and result collection. This violates the single responsibility principle and makes the code difficult to test, understand, and maintain.

## Objective

Extract all agent-related functionality from the MapReduce executor into a dedicated agent module that provides clear interfaces for agent lifecycle management, execution, and result handling. This will reduce coupling, improve testability, and make the agent execution logic reusable.

## Requirements

### Functional Requirements
- Extract agent lifecycle management (creation, execution, cleanup)
- Separate agent execution logic with retry mechanisms
- Isolate agent result collection and aggregation
- Maintain backward compatibility with existing MapReduce workflows
- Preserve all current agent execution features and behaviors
- Support both standard and enhanced progress tracking modes

### Non-Functional Requirements
- Reduce method complexity to under 20 lines where possible
- Achieve 90%+ test coverage for extracted module
- Maintain or improve current performance characteristics
- Follow functional programming principles with clear separation of pure and effectful functions
- Ensure thread-safety for concurrent agent execution

## Acceptance Criteria

- [ ] Agent module created at `src/cook/execution/mapreduce/agent/`
- [ ] Core agent structs and traits extracted to `agent/types.rs`
- [ ] Agent lifecycle management in `agent/lifecycle.rs`
- [ ] Agent execution logic in `agent/execution.rs`
- [ ] Result handling in `agent/results.rs`
- [ ] All agent-related methods removed from main MapReduceExecutor
- [ ] Main module reduced by approximately 800 lines
- [ ] All 140+ existing MapReduce tests still pass
- [ ] New unit tests achieve 90%+ coverage of agent module
- [ ] No performance regression in agent execution
- [ ] Documentation updated for new module structure

## Technical Details

### Implementation Approach

1. **Module Structure**:
   ```
   src/cook/execution/mapreduce/agent/
   ├── mod.rs          # Module exports and public API
   ├── types.rs        # AgentConfig, AgentHandle, AgentError types
   ├── lifecycle.rs    # Creation, initialization, cleanup
   ├── execution.rs    # Execution logic with retry handling
   └── results.rs      # Result aggregation and transformation
   ```

2. **Key Extractions**:
   - `run_agent` (145 lines) → `execution.rs`
   - `run_agent_with_enhanced_progress` (151 lines) → `execution.rs`
   - `execute_agent_commands_with_progress` (127 lines) → `execution.rs`
   - `execute_agent_commands_with_progress_and_retry` (118 lines) → `execution.rs`
   - `finalize_agent_result` → `results.rs`
   - `handle_merge_and_cleanup` → `lifecycle.rs`

### Architecture Changes

- Introduce `AgentExecutor` trait for different execution strategies
- Create `AgentLifecycleManager` for managing agent state transitions
- Implement `AgentResultAggregator` for collecting and processing results
- Use dependency injection for worktree management and progress tracking

### Data Structures

```rust
pub struct AgentConfig {
    pub id: String,
    pub item_id: String,
    pub branch_name: String,
    pub max_retries: u32,
    pub timeout: Duration,
}

pub struct AgentHandle {
    pub config: AgentConfig,
    pub worktree_session: WorktreeSession,
    pub state: Arc<RwLock<AgentState>>,
}

pub trait AgentExecutor {
    async fn execute(&self, handle: AgentHandle, commands: Vec<WorkflowStep>)
        -> Result<AgentResult, AgentError>;
}
```

### APIs and Interfaces

```rust
pub trait AgentLifecycleManager {
    async fn create_agent(&self, config: AgentConfig) -> Result<AgentHandle>;
    async fn cleanup_agent(&self, handle: AgentHandle) -> Result<()>;
}

pub trait AgentResultAggregator {
    fn aggregate(&self, results: Vec<AgentResult>) -> AggregatedResults;
    fn to_interpolation_context(&self, results: &AggregatedResults) -> InterpolationContext;
}
```

## Dependencies

- **Prerequisites**: Phase 1 utils module extraction (completed)
- **Affected Components**: MapReduceExecutor, progress tracking, worktree management
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: Test each extracted component in isolation
- **Integration Tests**: Verify agent execution within MapReduce context
- **Performance Tests**: Benchmark agent execution throughput
- **Concurrency Tests**: Validate thread-safety with parallel agents

## Documentation Requirements

- **Code Documentation**: Document all public APIs with rustdoc
- **Architecture Updates**: Update ARCHITECTURE.md with new module structure
- **Migration Guide**: Document how to use new agent module independently

## Implementation Notes

- Start with extracting types and interfaces before moving implementation
- Maintain compatibility layer during transition
- Use feature flags if needed for gradual migration
- Consider using async traits with proper boxing for flexibility
- Ensure proper error context preservation during extraction

## Migration and Compatibility

- No breaking changes to public MapReduce API
- Internal refactoring only affects module structure
- Consider deprecation warnings for any exposed internal APIs
- Provide migration path for any custom agent implementations