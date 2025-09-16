---
number: 62
title: Workflow-Level Error Handling Directives
category: foundation
priority: high
status: draft
dependencies: [58]
created: 2025-01-15
---

# Specification 62: Workflow-Level Error Handling Directives

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [58 - DLQ Reprocessing]

## Context

Current error handling in Prodigy MapReduce workflows is limited to command-level `on_failure` handlers. The whitepaper specifies workflow-level directives like `on_item_failure: dlq` and `continue_on_failure: true` that control how the entire workflow responds to failures. Without these, workflows cannot implement sophisticated failure strategies like continuing despite individual item failures or automatically routing failed items to the DLQ.

## Objective

Implement comprehensive workflow-level error handling directives that allow fine-grained control over failure behavior, including automatic DLQ routing, failure thresholds, circuit breakers, and graceful degradation strategies.

## Requirements

### Functional Requirements

1. **Workflow-Level Directives**
   - `on_item_failure: dlq|retry|skip|stop` - Action for failed items
   - `continue_on_failure: true|false` - Continue processing after failures
   - `max_failures: N` - Stop after N failures
   - `failure_threshold: 0.N` - Stop if failure rate exceeds threshold
   - `error_collection: aggregate|immediate` - Error reporting strategy

2. **Item Failure Strategies**
   - **DLQ**: Automatically send failed items to Dead Letter Queue
   - **Retry**: Immediate retry with backoff
   - **Skip**: Log and continue with next item
   - **Stop**: Halt entire workflow on first failure
   - **Custom**: User-defined failure handler

3. **Failure Policies**
   - Per-phase failure limits
   - Cascading failure prevention
   - Partial success handling
   - Failure rate monitoring
   - Circuit breaker with auto-recovery

4. **Error Aggregation**
   - Collect all errors before reporting
   - Group errors by type/pattern
   - Generate failure summary reports
   - Export error metrics
   - Create actionable error diagnostics

5. **Recovery Mechanisms**
   - Automatic rollback on threshold breach
   - Savepoint creation before risky operations
   - Compensating actions for failures
   - Graceful degradation options
   - Manual intervention hooks

### Non-Functional Requirements

1. **Performance**
   - Minimal overhead for error tracking
   - Efficient error aggregation
   - Fast failure detection

2. **Observability**
   - Real-time failure metrics
   - Detailed error logs
   - Failure pattern analysis

3. **Reliability**
   - Consistent error handling across phases
   - Atomic failure state updates
   - Prevent error handling loops

## Acceptance Criteria

- [ ] `on_item_failure: dlq` automatically routes failed items to DLQ
- [ ] `continue_on_failure: true` allows workflow to continue despite failures
- [ ] `max_failures: N` stops workflow after N failures
- [ ] `failure_threshold: 0.3` stops if 30% of items fail
- [ ] Circuit breaker activates after repeated failures
- [ ] Error aggregation provides comprehensive failure report
- [ ] Partial success is properly tracked and reported
- [ ] Recovery mechanisms can be triggered automatically
- [ ] All error directives work correctly together
- [ ] Performance impact is less than 2% overhead
- [ ] Error patterns are automatically detected and reported

## Technical Details

### Implementation Approach

```rust
pub struct WorkflowErrorPolicy {
    pub on_item_failure: ItemFailureAction,
    pub continue_on_failure: bool,
    pub max_failures: Option<usize>,
    pub failure_threshold: Option<f64>,
    pub error_collection: ErrorCollectionStrategy,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

impl WorkflowErrorPolicy {
    pub fn handle_item_failure(&mut self, item: &WorkItem, error: &Error) -> Result<FailureAction> {
        self.update_metrics(error);

        // Check circuit breaker
        if let Some(ref mut breaker) = self.circuit_breaker {
            if breaker.is_open() {
                return Ok(FailureAction::Stop("Circuit breaker open".into()));
            }
        }

        // Check thresholds
        if self.should_stop_on_threshold() {
            return Ok(FailureAction::Stop("Failure threshold exceeded".into()));
        }

        // Apply item failure strategy
        match self.on_item_failure {
            ItemFailureAction::Dlq => {
                self.send_to_dlq(item, error).await?;
                Ok(FailureAction::Continue)
            }
            ItemFailureAction::Retry => Ok(FailureAction::Retry(self.get_retry_config())),
            ItemFailureAction::Skip => Ok(FailureAction::Skip),
            ItemFailureAction::Stop => Ok(FailureAction::Stop(error.to_string())),
        }
    }
}
```

### Architecture Changes

1. **Error Policy Module**
   - New `error_policy` module in workflow
   - Policy evaluation engine
   - Error metrics collector

2. **Integration Points**
   - Hook into MapReduceExecutor
   - Modify workflow executor
   - Update configuration parser

3. **Error Reporting**
   - Enhanced error aggregation
   - Pattern detection system
   - Report generation

### Data Structures

```rust
pub enum ItemFailureAction {
    Dlq,
    Retry { max_attempts: u32, backoff: BackoffStrategy },
    Skip,
    Stop,
    Custom(Box<dyn FailureHandler>),
}

pub enum ErrorCollectionStrategy {
    Aggregate,  // Collect all errors before reporting
    Immediate,  // Report errors as they occur
    Batched { size: usize }, // Report in batches
}

pub struct CircuitBreakerConfig {
    pub failure_threshold: usize,
    pub success_threshold: usize,
    pub timeout: Duration,
    pub half_open_requests: usize,
}

pub struct ErrorMetrics {
    pub total_items: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failure_rate: f64,
    pub error_types: HashMap<String, usize>,
    pub failure_patterns: Vec<FailurePattern>,
}

pub struct FailurePattern {
    pub pattern_type: String,
    pub frequency: usize,
    pub items: Vec<String>,
    pub suggested_action: String,
}
```

### APIs and Interfaces

```rust
pub trait ErrorHandler {
    fn handle_error(&mut self, error: Error, context: &ErrorContext) -> Result<FailureAction>;
    fn should_continue(&self) -> bool;
    fn get_metrics(&self) -> ErrorMetrics;
}

pub trait FailureRecovery {
    fn create_savepoint(&self) -> Result<SavepointId>;
    fn rollback_to_savepoint(&self, id: SavepointId) -> Result<()>;
    fn apply_compensation(&self, action: CompensationAction) -> Result<()>;
}
```

## Dependencies

- **Prerequisites**: [58 - DLQ Reprocessing] for DLQ integration
- **Affected Components**:
  - `src/cook/execution/mapreduce.rs`
  - `src/cook/workflow/executor.rs`
  - `src/cook/workflow/mod.rs`
  - `src/config/mapreduce.rs`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Error policy evaluation
  - Threshold calculations
  - Circuit breaker behavior
  - Error aggregation

- **Integration Tests**:
  - End-to-end failure scenarios
  - DLQ integration
  - Multi-phase error propagation
  - Recovery mechanisms

- **Chaos Testing**:
  - Random failure injection
  - Network partition simulation
  - Resource exhaustion
  - Cascading failures

- **User Acceptance**:
  - Production failure scenarios
  - Error report usefulness
  - Recovery time objectives

## Documentation Requirements

- **Code Documentation**:
  - Error handling directive reference
  - Policy configuration examples
  - Recovery strategy guide

- **User Documentation**:
  - Error handling best practices
  - Common failure patterns
  - Troubleshooting guide

- **Architecture Updates**:
  - Error handling flow diagrams
  - Circuit breaker state machine
  - Recovery mechanism design

## Implementation Notes

1. **State Management**: Ensure error state is consistent across retries
2. **Performance**: Use atomic counters for metrics to avoid locks
3. **Debugging**: Add detailed error context for troubleshooting
4. **Extensibility**: Allow custom error handlers via plugins
5. **Monitoring**: Export metrics to monitoring systems

## Migration and Compatibility

- Default to current behavior if no error directives specified
- Backward compatible with existing `on_failure` handlers
- Gradual migration path with deprecation warnings
- Configuration validation for conflicting directives