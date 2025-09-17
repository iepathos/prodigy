---
number: 92
title: MapReduce State Management Module Extraction
category: optimization
priority: medium
status: draft
dependencies: [90]
created: 2025-09-17
---

# Specification 92: MapReduce State Management Module Extraction

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: [90 - Phase Execution Module]

## Context

State management in the MapReduce executor includes job state persistence, checkpoint validation, and recovery logic. These responsibilities are partially implemented in the main module and partially delegated to external state managers. The checkpoint validation, state calculation, and persistence logic are mixed with execution code, making it difficult to ensure consistent state management and implement advanced features like partial recovery.

## Objective

Extract all state management functionality into a dedicated module that provides comprehensive job state tracking, checkpoint management, and recovery capabilities. This will ensure consistent state handling, enable more sophisticated recovery strategies, and provide better visibility into job state transitions.

## Requirements

### Functional Requirements
- Centralize job state persistence and retrieval
- Extract checkpoint creation and validation
- Implement state recovery and resume logic
- Track state transitions with audit trail
- Support partial state recovery
- Enable state querying and inspection

### Non-Functional Requirements
- Ensure atomic state transitions
- Support concurrent state access
- Minimize state persistence overhead
- Enable state compression for large jobs
- Maintain backward compatibility with existing state

## Acceptance Criteria

- [ ] State module created at `src/cook/execution/mapreduce/state/`
- [ ] State persistence logic in `state/persistence.rs`
- [ ] Checkpoint management in `state/checkpoint.rs`
- [ ] Recovery logic in `state/recovery.rs`
- [ ] State transitions in `state/transitions.rs`
- [ ] All state code removed from main module
- [ ] Main module reduced by approximately 200 lines
- [ ] State recovery works identically to current
- [ ] New state inspection API available
- [ ] Support for partial recovery demonstrated

## Technical Details

### Implementation Approach

1. **Module Structure**:
   ```
   src/cook/execution/mapreduce/state/
   ├── mod.rs          # Module exports and StateManager
   ├── persistence.rs  # State save/load operations
   ├── checkpoint.rs   # Checkpoint creation/validation
   ├── recovery.rs     # Resume and recovery logic
   └── transitions.rs  # State machine transitions
   ```

2. **Key Extractions**:
   - `execute_map_phase_with_state` → Integration with state module
   - `validate_checkpoint` → `checkpoint.rs`
   - `calculate_pending_items` → `recovery.rs`
   - State persistence logic → `persistence.rs`
   - State transition logic → `transitions.rs`

### Architecture Changes

- Implement state machine for job lifecycle
- Use event sourcing for state audit trail
- Create immutable state snapshots
- Separate state from execution logic

### Data Structures

```rust
pub struct StateManager {
    store: Arc<dyn StateStore>,
    transitions: StateMachine,
    audit_log: Arc<RwLock<Vec<StateEvent>>>,
}

pub trait StateStore: Send + Sync {
    async fn save(&self, state: &JobState) -> Result<(), StateError>;
    async fn load(&self, job_id: &str) -> Result<Option<JobState>, StateError>;
    async fn list(&self) -> Result<Vec<JobSummary>, StateError>;
}

pub struct JobState {
    pub id: String,
    pub phase: PhaseType,
    pub checkpoint: Option<Checkpoint>,
    pub processed_items: HashSet<String>,
    pub failed_items: Vec<String>,
    pub variables: HashMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Checkpoint {
    pub phase: PhaseType,
    pub items_processed: Vec<String>,
    pub agent_results: Vec<AgentResult>,
    pub timestamp: DateTime<Utc>,
    pub checksum: String,
}
```

### APIs and Interfaces

```rust
impl StateManager {
    pub async fn create_job(&self, config: &MapReduceConfig) -> Result<JobState, StateError>;

    pub async fn update_state<F>(&self, job_id: &str, updater: F) -> Result<JobState, StateError>
    where
        F: FnOnce(&mut JobState) -> Result<(), StateError>;

    pub async fn create_checkpoint(&self, job_id: &str) -> Result<Checkpoint, StateError>;

    pub async fn recover_from_checkpoint(&self, checkpoint: &Checkpoint)
        -> Result<RecoveryPlan, StateError>;

    pub fn get_state_history(&self, job_id: &str) -> Vec<StateEvent>;
}

pub struct RecoveryPlan {
    pub resume_phase: PhaseType,
    pub pending_items: Vec<Value>,
    pub skip_items: HashSet<String>,
}
```

## Dependencies

- **Prerequisites**:
  - Phase 1: Utils module (completed)
  - Phase 5: Phase execution module (spec 90)
- **Affected Components**: Phase execution, checkpoint validation, job recovery
- **External Dependencies**: serde, chrono

## Testing Strategy

- **Unit Tests**: Test state transitions and validation
- **Persistence Tests**: Verify save/load operations
- **Recovery Tests**: Test various recovery scenarios
- **Concurrency Tests**: Validate concurrent state updates
- **Compatibility Tests**: Ensure backward compatibility

## Documentation Requirements

- **Code Documentation**: State lifecycle documentation
- **Operations Guide**: State management and recovery
- **Architecture Updates**: State machine diagrams
- **Migration Guide**: Upgrading state format

## Implementation Notes

- Use versioned state format for compatibility
- Implement state compression for large jobs
- Add state validation on load
- Consider using SQLite for state storage
- Implement state garbage collection
- Add metrics for state operations

## Migration and Compatibility

- Support loading old state format
- Automatic migration on first load
- No changes to job execution semantics
- State inspection is read-only by default
- Consider state format versioning strategy