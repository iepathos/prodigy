---
number: 64
title: MapReduce Module Testing
category: testing
priority: critical
status: draft
dependencies: []
created: 2025-09-16
---

# Specification 64: MapReduce Module Testing

**Category**: testing
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The MapReduce execution module is a critical component with only 25.8% test coverage (429/1662 lines). This module handles parallel workflow execution, agent lifecycle management, and failure recovery. With 1,233 uncovered lines, improving its test coverage would provide the single largest impact on overall project coverage (~4.7% increase).

## Objective

Increase MapReduce module test coverage from 25.8% to 70%+ by implementing comprehensive unit and integration tests for agent management, work distribution, error handling, and state persistence.

## Requirements

### Functional Requirements
- Test agent lifecycle (spawn, execute, monitor, cleanup)
- Test work item distribution and load balancing algorithms
- Test error handling and retry mechanisms
- Test DLQ operations and failure tracking
- Test state persistence and job recovery
- Test progress tracking and event streaming
- Test resource limits and throttling
- Test graceful shutdown and interruption handling

### Non-Functional Requirements
- Tests must complete within 30 seconds total
- Tests must not require actual git operations (use mocks)
- Tests must be deterministic and reproducible
- Tests must handle concurrent execution scenarios

## Acceptance Criteria

- [ ] MapReduce module coverage reaches 70% or higher
- [ ] All agent lifecycle states are tested
- [ ] Work distribution algorithms are verified
- [ ] Error scenarios have comprehensive tests
- [ ] DLQ operations are fully tested
- [ ] State persistence and recovery work correctly
- [ ] Concurrent execution edge cases are covered
- [ ] All tests pass consistently in CI

## Technical Details

### Implementation Approach

#### Core Components to Test

1. **MapReduceExecutor**
   - `execute()` main entry point
   - Setup phase execution
   - Map phase orchestration
   - Reduce phase aggregation
   - Error propagation

2. **Agent Management**
   - Agent spawning with worktree isolation
   - Agent health monitoring
   - Agent failure detection
   - Agent cleanup and resource release
   - Concurrent agent limits

3. **Work Distribution**
   - Work item parsing from various sources
   - Filter and sort expressions
   - Load balancing across agents
   - Work item retry logic
   - Partial failure handling

4. **State Management**
   - Job state initialization
   - Progress updates
   - Checkpoint creation
   - State recovery on resume
   - Concurrent state access

5. **Error Handling**
   - Individual item failures
   - Agent crashes
   - Timeout handling
   - Network failures
   - Graceful degradation

### Test Structure

```rust
// tests/mapreduce/mod.rs
mod agent_lifecycle_tests;
mod work_distribution_tests;
mod error_handling_tests;
mod state_management_tests;
mod integration_tests;

// Mock implementations
pub struct MockWorktreeManager {
    spawn_behavior: SpawnBehavior,
    cleanup_called: Arc<AtomicBool>,
}

pub struct MockCommandExecutor {
    responses: HashMap<String, CommandResponse>,
    execution_log: Arc<Mutex<Vec<String>>>,
}

pub struct MockEventLogger {
    events: Arc<Mutex<Vec<MapReduceEvent>>>,
}
```

### Key Test Scenarios

```rust
#[tokio::test]
async fn test_agent_spawn_and_cleanup() {
    // Verify agents are properly spawned and cleaned up
}

#[tokio::test]
async fn test_work_distribution_with_failures() {
    // Test partial failures and DLQ population
}

#[tokio::test]
async fn test_concurrent_agent_execution() {
    // Verify max_parallel limits are respected
}

#[tokio::test]
async fn test_job_interruption_and_recovery() {
    // Test SIGINT handling and state preservation
}

#[tokio::test]
async fn test_filter_and_sort_expressions() {
    // Verify work item filtering and ordering
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: MapReduce executor, WorktreeManager, EventLogger
- **External Dependencies**: tokio for async testing

## Testing Strategy

- **Unit Tests**: Test individual components in isolation
- **Integration Tests**: Test complete MapReduce workflows
- **Concurrency Tests**: Verify thread-safe operations
- **Failure Tests**: Simulate various failure modes
- **Performance Tests**: Ensure reasonable execution times

## Documentation Requirements

- **Test Documentation**: Clear descriptions of test scenarios
- **Mock Documentation**: Document mock behavior and configuration
- **Coverage Reports**: Include in CI pipeline output

## Implementation Notes

### Testing Challenges

1. **Async Complexity**: MapReduce is heavily async, requiring careful test setup
2. **External Dependencies**: Must mock git operations and subprocess execution
3. **Concurrency**: Need to test race conditions and synchronization
4. **State Management**: Complex state transitions need verification

### Mock Strategy

```rust
impl MockWorktreeManager {
    pub fn new() -> Self {
        Self {
            spawn_behavior: SpawnBehavior::Success,
            cleanup_called: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_failure_after(n: usize) -> Self {
        Self {
            spawn_behavior: SpawnBehavior::FailAfter(n),
            cleanup_called: Arc::new(AtomicBool::new(false)),
        }
    }
}
```

## Migration and Compatibility

No changes to production code required; tests are purely additive.