---
number: 76
title: Retry State Management During Resume
category: foundation
priority: high
status: draft
dependencies: [72, 73]
created: 2025-01-16
---

# Specification 76: Retry State Management During Resume

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [72 - Resume with Error Recovery, 73 - MapReduce Resume Functionality]

## Context

Retry state during resume operations is currently untested and likely broken. The existing test was "neutered" to only check exit codes without validating that retry logic actually works correctly during resumed execution. This is a critical gap because retry mechanisms are essential for handling transient failures, and users expect retry state to be preserved and function correctly across resume operations.

Key issues with current retry state during resume:
1. Retry attempt counters may be reset during resume
2. Retry backoff timers are not preserved across interruptions
3. Retry configuration may not be properly restored
4. Failed command retry history is lost
5. Cross-agent retry coordination in MapReduce is untested

## Objective

Implement comprehensive retry state management during resume operations, ensuring retry attempt counters, backoff timers, retry configurations, and retry history are properly preserved and function correctly during resumed workflow execution.

## Requirements

### Functional Requirements

1. **Retry State Persistence**
   - Save retry attempt counters to checkpoints
   - Persist retry backoff timer states
   - Store retry configuration for each command
   - Maintain retry execution history
   - Preserve retry correlation IDs

2. **Retry State Restoration**
   - Restore retry attempt counters from checkpoints
   - Rebuild retry backoff timer states
   - Validate retry configuration consistency
   - Restore retry execution history
   - Maintain retry correlation across resume

3. **Command-Level Retry Management**
   - Track retry attempts for individual commands
   - Handle partial retry completion during interruption
   - Restore command retry configuration
   - Maintain retry attempt history per command
   - Support different retry strategies per command

4. **MapReduce Retry Coordination**
   - Coordinate retry state across multiple agents
   - Handle agent failures during retry operations
   - Preserve work item retry attempts
   - Synchronize retry state across worktrees
   - Manage DLQ integration with retry logic

5. **Retry Policy Enforcement**
   - Enforce maximum retry limits after resume
   - Apply retry backoff policies correctly
   - Handle retry timeout constraints
   - Support conditional retry logic
   - Maintain retry circuit breaker state

6. **Retry History and Observability**
   - Comprehensive retry execution logging
   - Retry attempt correlation tracking
   - Retry performance metrics collection
   - Retry failure pattern analysis
   - Cross-resume retry analytics

### Non-Functional Requirements

1. **Reliability**
   - Retry state is never lost during interruption/resume cycles
   - Retry logic behaves identically before and after resume
   - No duplicate retry execution after resume
   - Consistent retry behavior across all command types

2. **Performance**
   - Fast retry state restoration (< 2 seconds)
   - Minimal overhead for retry state persistence
   - Efficient retry coordination in MapReduce scenarios
   - Scalable retry management for large workflows

3. **Observability**
   - Clear logging of retry state restoration
   - Detailed retry execution tracing
   - Retry state validation reporting
   - Cross-resume retry correlation

## Acceptance Criteria

- [ ] Retry attempt counters are preserved accurately across resume operations
- [ ] Retry backoff timers continue correctly after resume
- [ ] Maximum retry limits are enforced correctly after resume
- [ ] Retry execution history is maintained across interruptions
- [ ] MapReduce retry coordination works across agent restarts
- [ ] Different retry strategies function correctly after resume
- [ ] Retry circuit breaker state is preserved and functional
- [ ] No duplicate retry execution occurs after resume
- [ ] Retry performance is unaffected by resume operations
- [ ] Comprehensive retry testing validates all scenarios

## Technical Details

### Implementation Approach

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryCheckpointState {
    pub command_retry_states: HashMap<String, CommandRetryState>,
    pub global_retry_config: RetryConfig,
    pub retry_execution_history: Vec<RetryExecution>,
    pub circuit_breaker_states: HashMap<String, CircuitBreakerState>,
    pub retry_correlation_map: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRetryState {
    pub command_id: String,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub backoff_state: BackoffState,
    pub retry_history: Vec<RetryAttempt>,
    pub current_strategy: RetryStrategy,
    pub is_circuit_broken: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffState {
    pub strategy: BackoffStrategy,
    pub current_delay: Duration,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub jitter: bool,
}

#[derive(Debug, Clone)]
pub struct RetryStateManager {
    checkpoint_state: RetryCheckpointState,
    retry_executor: Arc<RetryExecutor>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
    retry_scheduler: Arc<RetryScheduler>,
}

impl RetryStateManager {
    pub async fn restore_retry_state(
        &self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<RetryState> {
        // Load retry state from checkpoint
        let saved_state = checkpoint.retry_checkpoint_state
            .as_ref()
            .ok_or(ProdigyError::MissingRetryState)?;

        // Validate retry state consistency
        self.validate_retry_state_consistency(saved_state).await?;

        // Restore command retry states
        let mut retry_state = RetryState::new();

        for (command_id, command_retry_state) in &saved_state.command_retry_states {
            self.restore_command_retry_state(
                &mut retry_state,
                command_id,
                command_retry_state,
            ).await?;
        }

        // Restore circuit breaker states
        self.restore_circuit_breaker_states(
            &mut retry_state,
            &saved_state.circuit_breaker_states,
        ).await?;

        // Restore retry scheduler state
        self.restore_retry_scheduler_state(
            &saved_state.command_retry_states,
        ).await?;

        Ok(retry_state)
    }

    async fn restore_command_retry_state(
        &self,
        retry_state: &mut RetryState,
        command_id: &str,
        saved_state: &CommandRetryState,
    ) -> Result<()> {
        // Create command retry context
        let mut command_retry = CommandRetry::new(command_id.to_string());

        // Restore attempt count and limits
        command_retry.set_attempt_count(saved_state.attempt_count);
        command_retry.set_max_attempts(saved_state.max_attempts);

        // Restore backoff state
        let backoff = self.create_backoff_from_state(&saved_state.backoff_state)?;
        command_retry.set_backoff_strategy(backoff);

        // Restore retry schedule
        if let Some(next_retry_at) = saved_state.next_retry_at {
            if next_retry_at > Utc::now() {
                self.retry_scheduler.schedule_retry(
                    command_id.to_string(),
                    next_retry_at,
                ).await?;
            }
        }

        // Restore execution history
        for attempt in &saved_state.retry_history {
            command_retry.add_attempt_to_history(attempt.clone());
        }

        // Add to retry state
        retry_state.add_command_retry(command_id.to_string(), command_retry);

        Ok(())
    }

    pub async fn execute_retry_with_preserved_state(
        &self,
        command: &Command,
        retry_state: &RetryState,
    ) -> Result<CommandResult> {
        let command_id = &command.id;

        // Get retry state for this command
        let command_retry = retry_state.get_command_retry(command_id)
            .ok_or(ProdigyError::MissingCommandRetryState)?;

        // Check if retry is allowed
        if command_retry.attempt_count >= command_retry.max_attempts {
            return Err(ProdigyError::MaxRetriesExceeded(command_id.clone()));
        }

        // Check circuit breaker state
        if let Some(circuit_breaker) = self.circuit_breakers.get(command_id) {
            if circuit_breaker.is_open() {
                return Err(ProdigyError::CircuitBreakerOpen(command_id.clone()));
            }
        }

        // Apply backoff delay if needed
        if let Some(next_retry_at) = command_retry.next_retry_at {
            if next_retry_at > Utc::now() {
                let delay = next_retry_at - Utc::now();
                tokio::time::sleep(delay.to_std()?).await;
            }
        }

        // Execute retry attempt
        let attempt_start = Utc::now();
        let result = self.retry_executor.execute_with_retry(command).await;

        // Record retry attempt
        let retry_attempt = RetryAttempt {
            attempt_number: command_retry.attempt_count + 1,
            executed_at: attempt_start,
            duration: Utc::now() - attempt_start,
            success: result.is_ok(),
            error: result.as_ref().err().map(|e| e.to_string()),
        };

        // Update retry state
        self.update_retry_state_after_attempt(
            command_id,
            &retry_attempt,
            &result,
        ).await?;

        result
    }

    async fn coordinate_mapreduce_retry_state(
        &self,
        job_state: &MapReduceResumeState,
        retry_states: &HashMap<String, RetryState>,
    ) -> Result<CoordinatedRetryState> {
        let mut coordinated_state = CoordinatedRetryState::new();

        // Aggregate retry states from all agents
        for (agent_id, agent_retry_state) in retry_states {
            for (work_item_id, item_retry_state) in agent_retry_state.work_item_retries() {
                coordinated_state.merge_work_item_retry_state(
                    work_item_id.clone(),
                    item_retry_state.clone(),
                )?;
            }
        }

        // Restore DLQ retry coordination
        let dlq_items = self.load_dlq_retry_states(&job_state.job_id).await?;
        for dlq_item in dlq_items {
            coordinated_state.add_dlq_retry_state(dlq_item)?;
        }

        // Validate retry state consistency across agents
        self.validate_cross_agent_retry_consistency(&coordinated_state).await?;

        Ok(coordinated_state)
    }
}
```

### Architecture Changes

1. **Retry State Checkpoint System**
   - Enhanced checkpoint structure with comprehensive retry state
   - Cross-command retry state management
   - Circuit breaker state persistence

2. **Retry Scheduler Integration**
   - Persistent retry scheduling across resume
   - Backoff timer restoration
   - Retry coordination in distributed scenarios

3. **MapReduce Retry Coordination**
   - Cross-agent retry state synchronization
   - Work item retry tracking
   - DLQ integration with retry logic

4. **Retry Testing Framework**
   - Comprehensive retry scenario testing
   - Retry state validation tools
   - Cross-resume retry behavior verification

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttempt {
    pub attempt_number: u32,
    pub executed_at: DateTime<Utc>,
    pub duration: chrono::Duration,
    pub success: bool,
    pub error: Option<String>,
    pub backoff_applied: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Fixed { delay: Duration },
    Linear { base: Duration, increment: Duration },
    Exponential { base: Duration, multiplier: f64, max: Duration },
    Custom { formula: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    pub state: CircuitState,
    pub failure_count: u32,
    pub failure_threshold: u32,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub recovery_timeout: Duration,
    pub half_open_max_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CoordinatedRetryState {
    pub work_item_retries: HashMap<String, WorkItemRetryState>,
    pub dlq_retries: Vec<DlqRetryState>,
    pub cross_agent_consistency: ConsistencyState,
}
```

### Integration Points

1. **Checkpoint Manager Integration**
   - Save/load retry checkpoint state
   - Retry state validation
   - Cross-command retry management

2. **Command Execution Integration**
   - Retry state restoration before execution
   - Retry attempt coordination
   - Failure handling with retry logic

3. **MapReduce Executor Integration**
   - Cross-agent retry coordination
   - Work item retry management
   - DLQ retry integration

## Dependencies

- **Prerequisites**: [72 - Resume with Error Recovery, 73 - MapReduce Resume Functionality]
- **Affected Components**:
  - `src/cook/execution/retry.rs`
  - `src/cook/workflow/checkpoint.rs`
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/coordinators/agent_pool.rs`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Retry state persistence and restoration
  - Backoff timer state management
  - Circuit breaker state handling
  - Retry attempt coordination

- **Integration Tests**:
  - End-to-end retry behavior across resume
  - MapReduce retry coordination
  - Complex retry scenario validation
  - Cross-agent retry state synchronization

- **Property Tests**:
  - Retry state consistency across resume
  - Retry limit enforcement
  - Backoff timing accuracy

- **Performance Tests**:
  - Retry state restoration speed
  - Retry coordination overhead
  - Large-scale retry management

## Documentation Requirements

- **Code Documentation**:
  - Retry state management architecture
  - Cross-resume retry behavior
  - MapReduce retry coordination

- **User Documentation**:
  - Retry behavior during resume
  - Retry configuration best practices
  - Troubleshooting retry issues

- **Architecture Updates**:
  - Retry state persistence mechanisms
  - Cross-agent retry coordination
  - Resume retry flow diagrams

## Implementation Notes

1. **Consistency**: Ensure retry state is consistent across all agents and resume operations
2. **Timing**: Preserve exact timing behavior for backoff strategies
3. **Coordination**: Handle cross-agent retry coordination carefully
4. **Testing**: Comprehensive testing of all retry scenarios and edge cases
5. **Observability**: Rich logging and metrics for retry operations

## Migration and Compatibility

- Backward compatible with existing retry implementations
- Automatic migration of legacy retry state
- Graceful handling of missing retry data in old checkpoints
- Progressive rollout with comprehensive retry validation