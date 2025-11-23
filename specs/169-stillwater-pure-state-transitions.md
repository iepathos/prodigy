---
number: 169
title: Stillwater Pure State Transitions
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-11-23
---

# Specification 169: Stillwater Pure State Transitions

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MapReduce job state management in `src/cook/execution/state.rs` (1,856 lines) mixes business logic with I/O operations, making state transitions difficult to test and reason about.

**Current Problems**:
- State updates mixed with checkpoint persistence
- Cannot test state machine without file system
- Mutable state updates scattered throughout code
- Unclear separation between state logic and I/O

**Example**:
```rust
pub async fn handle_agent_completion(&mut self, result: AgentResult) -> Result<()> {
    self.completed_items.push(result.item_id);  // Mutation
    self.save_checkpoint().await?;               // I/O
    if self.pending_items.is_empty() {           // Logic
        self.complete_job().await?;              // More I/O
    }
}
```

## Objective

Separate state transition logic from I/O operations using Stillwater's Effect pattern, enabling pure, testable state functions and composable I/O operations.

## Requirements

### Functional Requirements

1. **Pure State Transitions**
   - Extract all state update logic to pure functions
   - State functions return new state (immutable updates)
   - No I/O in state transition functions

2. **Effect-Based I/O**
   - Wrap all I/O operations in `Effect<T, E, Env>` type
   - Environment contains storage dependencies
   - Lazy evaluation until `.run(env)` called

3. **Composable Operations**
   - Combine pure state updates with I/O via Effect chains
   - State transitions composable via functional patterns
   - Clear data flow through operations

### Non-Functional Requirements

1. **Testability**: 100% of state logic testable without I/O
2. **Performance**: No degradation vs current implementation
3. **Safety**: Immutable state updates prevent accidental mutation
4. **Clarity**: Clear separation between pure logic and effects

## Acceptance Criteria

- [ ] Pure state transition module created (`state/pure.rs`)
- [ ] All state update logic extracted to pure functions
- [ ] Effect-based I/O module created (`state/io.rs`)
- [ ] Environment struct defined for dependencies
- [ ] Checkpoint save/load wrapped in Effects
- [ ] State updates return new state (immutable)
- [ ] Pure functions have zero I/O dependencies
- [ ] 40-60 unit tests for pure state transitions
- [ ] Integration tests use mock environments
- [ ] Performance benchmarks show <5% overhead
- [ ] Documentation updated with architecture
- [ ] Migration guide for existing code

## Technical Details

### Implementation Approach

**Phase 1: Pure State Module**
```rust
// src/cook/execution/state/pure.rs

/// Pure state transition functions (no I/O, no side effects)

use crate::cook::execution::state::{JobState, WorkItem, AgentResult};

/// Apply agent completion to state (pure)
pub fn apply_agent_result(
    mut state: JobState,
    result: AgentResult,
) -> JobState {
    // Pure state update - returns new state
    state.completed_items.push(result.item_id);
    state.agent_results.insert(result.agent_id.clone(), result);
    state.active_agents.remove(&result.agent_id);
    state.items_processed += 1;

    // Pure phase transition logic
    if should_transition_to_reduce(&state) {
        state.phase = Phase::Reduce;
    }

    state
}

/// Determine if job should transition to reduce phase (pure)
pub fn should_transition_to_reduce(state: &JobState) -> bool {
    state.pending_items.is_empty() &&
    state.active_agents.is_empty() &&
    state.phase == Phase::Map
}

/// Determine if job is complete (pure)
pub fn is_job_complete(state: &JobState) -> bool {
    state.pending_items.is_empty() &&
    state.active_agents.is_empty() &&
    state.reduce_phase_completed
}

/// Calculate next work batch (pure)
pub fn next_batch(
    state: &JobState,
    batch_size: usize,
) -> Option<WorkBatch> {
    if state.pending_items.is_empty() {
        return None;
    }

    Some(WorkBatch {
        items: state.pending_items
            .iter()
            .take(batch_size)
            .cloned()
            .collect(),
        batch_id: state.next_batch_id,
    })
}

/// Move batch to active (pure)
pub fn activate_batch(
    mut state: JobState,
    batch: &WorkBatch,
) -> JobState {
    // Remove items from pending
    state.pending_items.retain(|item| {
        !batch.items.contains(item)
    });

    // Add to active agents
    for (item, agent_id) in batch.items.iter().zip(&batch.agent_ids) {
        state.active_agents.insert(
            agent_id.clone(),
            AgentInfo {
                item_id: item.id.clone(),
                started_at: Utc::now(),
            },
        );
    }

    state.next_batch_id += 1;
    state
}

/// Record agent failure (pure)
pub fn record_agent_failure(
    mut state: JobState,
    agent_id: &str,
    error: AgentError,
) -> JobState {
    if let Some(agent_info) = state.active_agents.remove(agent_id) {
        state.failed_items.push(FailedItem {
            item_id: agent_info.item_id,
            agent_id: agent_id.to_string(),
            error: error.to_string(),
            timestamp: Utc::now(),
        });
        state.items_failed += 1;
    }
    state
}
```

**Phase 2: Effect-Based I/O**
```rust
// src/cook/execution/state/io.rs

use stillwater::Effect;
use super::pure;

/// Environment for state I/O operations
pub struct StateEnv {
    pub storage: Arc<dyn StorageBackend>,
    pub event_log: Arc<dyn EventLog>,
}

type StateEffect<T> = Effect<T, StateError, StateEnv>;

/// Save checkpoint (I/O wrapper)
pub fn save_checkpoint(state: JobState) -> StateEffect<()> {
    Effect::from_async(|env: &StateEnv| async move {
        let serialized = serde_json::to_string(&state)?;
        env.storage
            .write_checkpoint(&state.job_id, &serialized)
            .await?;

        // Log event
        env.event_log
            .log_checkpoint_saved(&state.job_id)
            .await?;

        Ok(())
    })
    .context(format!("Saving checkpoint for job {}", state.job_id))
}

/// Load checkpoint (I/O)
pub fn load_checkpoint(job_id: &str) -> StateEffect<JobState> {
    Effect::from_async(|env: &StateEnv| async move {
        let data = env.storage.read_checkpoint(job_id).await?;
        let state: JobState = serde_json::from_str(&data)?;
        Ok(state)
    })
    .context(format!("Loading checkpoint for job {}", job_id))
}

/// Update state and save (composition)
pub fn update_with_agent_result(
    state: JobState,
    result: AgentResult,
) -> StateEffect<JobState> {
    // Pure state update
    let new_state = pure::apply_agent_result(state, result);

    // Save updated state
    save_checkpoint(new_state.clone())
        .map(|_| new_state)
}

/// Complete agent batch (pure + I/O composition)
pub fn complete_batch(
    state: JobState,
    results: Vec<AgentResult>,
) -> StateEffect<JobState> {
    // Pure: apply all results
    let mut new_state = state;
    for result in results {
        new_state = pure::apply_agent_result(new_state, result);
    }

    // I/O: save checkpoint
    save_checkpoint(new_state.clone())
        .and_then(|_| {
            // Pure: check if transition needed
            if pure::should_transition_to_reduce(&new_state) {
                transition_to_reduce(new_state)
            } else {
                Effect::pure(new_state)
            }
        })
}

/// Transition to reduce phase (I/O)
fn transition_to_reduce(mut state: JobState) -> StateEffect<JobState> {
    state.phase = Phase::Reduce;

    Effect::from_async(|env: &StateEnv| async move {
        // Log phase transition
        env.event_log
            .log_phase_transition(&state.job_id, Phase::Reduce)
            .await?;

        Ok(state)
    })
    .context(format!("Transitioning job {} to reduce phase", state.job_id))
}
```

**Phase 3: Integration Layer**
```rust
// src/cook/execution/state/mod.rs

pub use pure::{
    apply_agent_result,
    is_job_complete,
    next_batch,
    // ... other pure functions
};

pub use io::{
    save_checkpoint,
    load_checkpoint,
    update_with_agent_result,
    StateEnv,
};

/// High-level state manager (orchestration)
pub struct JobStateManager {
    env: Arc<StateEnv>,
}

impl JobStateManager {
    pub fn new(env: Arc<StateEnv>) -> Self {
        Self { env }
    }

    /// Handle agent completion (uses pure + I/O composition)
    pub async fn handle_agent_completion(
        &self,
        state: JobState,
        result: AgentResult,
    ) -> Result<JobState, StateError> {
        io::update_with_agent_result(state, result)
            .run(&self.env)
            .await
    }

    /// Process batch completion (multiple agents)
    pub async fn complete_batch(
        &self,
        state: JobState,
        results: Vec<AgentResult>,
    ) -> Result<JobState, StateError> {
        io::complete_batch(state, results)
            .run(&self.env)
            .await
    }
}
```

### Architecture Changes

**New Module Structure**:
```
src/cook/execution/state/
├── mod.rs              (public API, re-exports)
├── pure.rs             (NEW - pure state transitions)
├── io.rs               (NEW - Effect-based I/O)
├── types.rs            (state data structures)
└── checkpoint.rs       (checkpoint versioning)
```

**Dependency Flow**:
```
High-level Orchestrator
        ↓
JobStateManager (thin wrapper)
        ↓
Effect Composition (io.rs)
        ↓
Pure Functions (pure.rs)  +  Storage/Logging (Env)
```

### Data Structures

```rust
/// Job state (immutable updates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobState {
    pub job_id: String,
    pub phase: Phase,
    pub pending_items: Vec<WorkItem>,
    pub active_agents: HashMap<String, AgentInfo>,
    pub completed_items: Vec<String>,
    pub failed_items: Vec<FailedItem>,
    pub items_processed: usize,
    pub items_failed: usize,
    pub next_batch_id: u64,
    pub reduce_phase_completed: bool,
}

/// Work batch (pure value)
#[derive(Debug, Clone)]
pub struct WorkBatch {
    pub items: Vec<WorkItem>,
    pub batch_id: u64,
    pub agent_ids: Vec<String>,
}

/// Storage backend trait (dependency)
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn write_checkpoint(&self, job_id: &str, data: &str) -> Result<()>;
    async fn read_checkpoint(&self, job_id: &str) -> Result<String>;
}
```

### APIs and Interfaces

**Pure Functions API** (exported for testing):
```rust
pub mod pure {
    pub fn apply_agent_result(state: JobState, result: AgentResult) -> JobState;
    pub fn is_job_complete(state: &JobState) -> bool;
    pub fn next_batch(state: &JobState, batch_size: usize) -> Option<WorkBatch>;
    pub fn activate_batch(state: JobState, batch: &WorkBatch) -> JobState;
    pub fn record_agent_failure(state: JobState, agent_id: &str, error: AgentError) -> JobState;
}
```

**Effect API** (composition):
```rust
pub mod io {
    pub fn save_checkpoint(state: JobState) -> StateEffect<()>;
    pub fn load_checkpoint(job_id: &str) -> StateEffect<JobState>;
    pub fn update_with_agent_result(state: JobState, result: AgentResult) -> StateEffect<JobState>;
}
```

## Dependencies

### Prerequisites
- Stillwater library with `Effect<T, E, Env>` type
- Understanding of pure functions and effect composition

### Affected Components
- `src/cook/execution/mapreduce/coordination/executor.rs` - State usage
- `src/cook/execution/mapreduce/checkpoint_integration.rs` - Checkpoint management
- All state update call sites throughout MapReduce

### External Dependencies
- `stillwater = "0.1"` (Effect type)

## Testing Strategy

### Unit Tests (Pure Functions - No I/O)

```rust
#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn test_apply_agent_result() {
        let state = JobState {
            pending_items: vec![],
            active_agents: [(agent_id.clone(), agent_info)].into(),
            completed_items: vec![],
            items_processed: 0,
            ..Default::default()
        };

        let result = AgentResult {
            agent_id: agent_id.clone(),
            item_id: "item-1".to_string(),
            commits: vec!["abc123".to_string()],
        };

        let new_state = pure::apply_agent_result(state, result);

        assert_eq!(new_state.items_processed, 1);
        assert!(new_state.completed_items.contains(&"item-1".to_string()));
        assert!(!new_state.active_agents.contains_key(&agent_id));
    }

    #[test]
    fn test_should_transition_to_reduce() {
        let state_ready = JobState {
            pending_items: vec![],
            active_agents: HashMap::new(),
            phase: Phase::Map,
            ..Default::default()
        };

        assert!(pure::should_transition_to_reduce(&state_ready));

        let state_not_ready = JobState {
            pending_items: vec![test_work_item()],
            ..state_ready.clone()
        };

        assert!(!pure::should_transition_to_reduce(&state_not_ready));
    }

    #[test]
    fn test_next_batch() {
        let items: Vec<WorkItem> = (0..10).map(|i| test_work_item(i)).collect();
        let state = JobState {
            pending_items: items,
            ..Default::default()
        };

        let batch = pure::next_batch(&state, 5).unwrap();

        assert_eq!(batch.items.len(), 5);
        assert_eq!(batch.batch_id, 0);
    }

    #[test]
    fn test_activate_batch() {
        let items = vec![test_work_item(1), test_work_item(2)];
        let batch = WorkBatch {
            items: items.clone(),
            batch_id: 0,
            agent_ids: vec!["agent-1".to_string(), "agent-2".to_string()],
        };

        let state = JobState {
            pending_items: items.clone(),
            ..Default::default()
        };

        let new_state = pure::activate_batch(state, &batch);

        assert!(new_state.pending_items.is_empty());
        assert_eq!(new_state.active_agents.len(), 2);
        assert_eq!(new_state.next_batch_id, 1);
    }
}
```

### Integration Tests (Effects with Mock Environment)

```rust
#[cfg(test)]
mod io_tests {
    use super::*;

    struct MockStorage {
        checkpoints: Arc<Mutex<HashMap<String, String>>>,
    }

    impl MockStorage {
        fn new() -> Self {
            Self {
                checkpoints: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl StorageBackend for MockStorage {
        async fn write_checkpoint(&self, job_id: &str, data: &str) -> Result<()> {
            self.checkpoints
                .lock()
                .unwrap()
                .insert(job_id.to_string(), data.to_string());
            Ok(())
        }

        async fn read_checkpoint(&self, job_id: &str) -> Result<String> {
            self.checkpoints
                .lock()
                .unwrap()
                .get(job_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Checkpoint not found"))
        }
    }

    #[tokio::test]
    async fn test_save_checkpoint() {
        let mock_storage = Arc::new(MockStorage::new());
        let env = Arc::new(StateEnv {
            storage: mock_storage.clone(),
            event_log: Arc::new(MockEventLog::new()),
        });

        let state = JobState::new("job-123");

        let result = io::save_checkpoint(state.clone())
            .run(&env)
            .await;

        assert!(result.is_ok());
        assert_eq!(mock_storage.checkpoints.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_update_with_agent_result() {
        let env = Arc::new(test_state_env());
        let state = JobState::new("job-123");
        let result = test_agent_result();

        let new_state = io::update_with_agent_result(state, result)
            .run(&env)
            .await
            .unwrap();

        assert_eq!(new_state.items_processed, 1);
    }
}
```

### Performance Tests

```rust
#[test]
fn benchmark_pure_state_updates() {
    let state = create_large_job_state(10_000);
    let results: Vec<AgentResult> = (0..1000).map(|i| test_result(i)).collect();

    let start = Instant::now();

    let mut current_state = state;
    for result in results {
        current_state = pure::apply_agent_result(current_state, result);
    }

    let duration = start.elapsed();

    // Should complete in <100ms
    assert!(duration < Duration::from_millis(100));
}
```

## Documentation Requirements

### Code Documentation

```rust
/// Apply agent completion result to job state (pure function)
///
/// This is a pure function - it has no side effects and always produces
/// the same output for the same inputs. This makes it easy to test and
/// reason about.
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::state::pure;
///
/// let state = JobState::new("job-1");
/// let result = AgentResult { ... };
///
/// let new_state = pure::apply_agent_result(state, result);
///
/// assert_eq!(new_state.items_processed, 1);
/// ```
pub fn apply_agent_result(state: JobState, result: AgentResult) -> JobState { ... }
```

### Architecture Updates

Add to `ARCHITECTURE.md`:
```markdown
## State Management Architecture

### Pure State Transitions

All MapReduce job state updates use pure functions in `state/pure.rs`:

- **Immutable**: Functions return new state, never mutate
- **Testable**: No I/O dependencies, instant tests
- **Composable**: Small functions combine for complex logic
- **Predictable**: Same inputs always produce same outputs

### Effect-Based I/O

All state persistence uses Effects in `state/io.rs`:

- **Lazy**: No execution until `.run(env)` called
- **Composable**: Chain pure updates with I/O operations
- **Testable**: Mock environments for testing
- **Context**: Automatic error context preservation

### Separation Pattern

```
Pure Logic (state/pure.rs)  →  Effect Composition (state/io.rs)  →  Orchestration (mod.rs)
    ↓                              ↓                                    ↓
No I/O, instant tests        Mock environments               Real dependencies
```
```

## Implementation Notes

### Migration Strategy

**Phase 1: Extract Pure Functions** (Week 1)
- Create `state/pure.rs`
- Extract state transition logic
- Add comprehensive unit tests
- No integration changes yet

**Phase 2: Create Effect Layer** (Week 1)
- Create `state/io.rs`
- Wrap I/O operations in Effects
- Define StateEnv
- Add integration tests with mocks

**Phase 3: Update Integration** (Week 2)
- Update JobStateManager
- Migrate call sites gradually
- Run both old and new in parallel
- Validate correctness

### Edge Cases

- **Empty state updates**: Pure functions handle gracefully
- **Concurrent updates**: Immutable state prevents races
- **Checkpoint failures**: Effect composition handles errors
- **Phase transitions**: Pure logic separated from I/O

### Performance Considerations

- **No cloning overhead**: Clone only when necessary
- **Structural sharing**: Rust's ownership enables efficient updates
- **Effect boxing**: Minimal overhead (single allocation per chain)

## Migration and Compatibility

### Breaking Changes
None - internal refactoring only.

### Migration Path

1. Create pure functions alongside existing code
2. Add Effect wrappers for I/O
3. Gradually migrate call sites
4. Remove old implementation once validated
5. Update documentation

### Rollback Strategy

Pure functions are additive - can disable without breaking existing code.
