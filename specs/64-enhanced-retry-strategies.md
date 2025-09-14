---
number: 64
title: Enhanced Retry Strategies with Backoff
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 64: Enhanced Retry Strategies with Backoff

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The whitepaper emphasizes sophisticated retry mechanisms:
```yaml
retry:
  attempts: 3
  backoff: exponential
  on_failure: continue  # or 'stop'
```

Currently, Prodigy has basic retry support but lacks configurable backoff strategies, jitter, and fine-grained control over retry behavior. This limits its effectiveness for handling transient failures in real-world scenarios.

## Objective

Implement comprehensive retry strategies with configurable backoff algorithms, jitter, retry conditions, and per-step retry configuration to handle various failure scenarios gracefully.

## Requirements

### Functional Requirements
- Support multiple backoff strategies: fixed, linear, exponential, fibonacci
- Configurable initial delay and maximum delay
- Jitter support to prevent thundering herd
- Retry only on specific error types
- Per-step retry configuration
- Global retry defaults
- Retry budget to limit total retry time
- Circuit breaker pattern for repeated failures
- Detailed retry metrics and logging

### Non-Functional Requirements
- Minimal overhead for non-retry cases
- Predictable retry timing
- Thread-safe retry state management
- Clear visibility into retry behavior

## Acceptance Criteria

- [ ] `backoff: exponential` increases delay exponentially
- [ ] `backoff: linear` increases delay linearly
- [ ] `jitter: true` adds randomization to prevent synchronized retries
- [ ] `retry_on: ["timeout", "network"]` retries only specific errors
- [ ] `max_delay: 60s` caps maximum retry delay
- [ ] `retry_budget: 5m` limits total retry time
- [ ] Circuit breaker stops retries after threshold
- [ ] Retry metrics available in logs and progress
- [ ] Per-step retry overrides global settings
- [ ] Clear indication when retrying vs failing

## Technical Details

### Implementation Approach

1. **Retry Strategy Configuration**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct RetryConfig {
       /// Maximum retry attempts
       pub attempts: u32,

       /// Backoff strategy
       pub backoff: BackoffStrategy,

       /// Initial delay between retries
       #[serde(with = "duration_serde")]
       pub initial_delay: Duration,

       /// Maximum delay between retries
       #[serde(with = "duration_serde")]
       pub max_delay: Duration,

       /// Add jitter to delays
       #[serde(default)]
       pub jitter: bool,

       /// Jitter factor (0.0 to 1.0)
       #[serde(default = "default_jitter_factor")]
       pub jitter_factor: f64,

       /// Only retry on specific error types
       #[serde(default)]
       pub retry_on: Vec<ErrorMatcher>,

       /// Maximum total time for retries
       #[serde(with = "duration_serde")]
       pub retry_budget: Option<Duration>,

       /// Action on final failure
       #[serde(default)]
       pub on_failure: FailureAction,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum BackoffStrategy {
       Fixed,
       Linear { increment: Duration },
       Exponential { base: f64 },
       Fibonacci,
       Custom { delays: Vec<Duration> },
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum FailureAction {
       Stop,
       Continue,
       Fallback { command: String },
   }
   ```

2. **Retry Executor**:
   ```rust
   pub struct RetryExecutor {
       config: RetryConfig,
       metrics: RetryMetrics,
       circuit_breaker: CircuitBreaker,
   }

   impl RetryExecutor {
       pub async fn execute_with_retry<F, T>(
           &self,
           operation: F,
           context: &str,
       ) -> Result<T>
       where
           F: Fn() -> Future<Output = Result<T>> + Clone,
       {
           let mut attempt = 0;
           let mut total_delay = Duration::ZERO;
           let start_time = Instant::now();

           loop {
               attempt += 1;

               // Check circuit breaker
               if self.circuit_breaker.is_open() {
                   return Err(Error::CircuitBreakerOpen);
               }

               // Execute operation
               match operation().await {
                   Ok(result) => {
                       self.circuit_breaker.record_success();
                       self.metrics.record_success(attempt);
                       return Ok(result);
                   }
                   Err(err) => {
                       self.circuit_breaker.record_failure();

                       // Check if we should retry
                       if !self.should_retry(&err, attempt, total_delay) {
                           self.metrics.record_failure(attempt);
                           return Err(err);
                       }

                       // Calculate delay
                       let delay = self.calculate_delay(attempt);
                       let jittered_delay = self.apply_jitter(delay);

                       // Check retry budget
                       if let Some(budget) = self.config.retry_budget {
                           if total_delay + jittered_delay > budget {
                               return Err(Error::RetryBudgetExhausted);
                           }
                       }

                       // Log retry
                       info!(
                           "Retrying {} (attempt {}/{}) after {:?}",
                           context,
                           attempt,
                           self.config.attempts,
                           jittered_delay
                       );

                       // Wait before retry
                       tokio::time::sleep(jittered_delay).await;
                       total_delay += jittered_delay;
                   }
               }
           }
       }

       fn calculate_delay(&self, attempt: u32) -> Duration {
           let base_delay = match &self.config.backoff {
               BackoffStrategy::Fixed => self.config.initial_delay,
               BackoffStrategy::Linear { increment } => {
                   self.config.initial_delay + *increment * (attempt - 1)
               }
               BackoffStrategy::Exponential { base } => {
                   let multiplier = base.powi(attempt as i32 - 1);
                   self.config.initial_delay.mul_f64(multiplier)
               }
               BackoffStrategy::Fibonacci => {
                   let fib = fibonacci(attempt);
                   self.config.initial_delay * fib
               }
               BackoffStrategy::Custom { delays } => {
                   delays.get(attempt as usize - 1)
                       .copied()
                       .unwrap_or(self.config.max_delay)
               }
           };

           base_delay.min(self.config.max_delay)
       }

       fn apply_jitter(&self, delay: Duration) -> Duration {
           if !self.config.jitter {
               return delay;
           }

           let jitter_range = delay.as_secs_f64() * self.config.jitter_factor;
           let jitter = rand::random::<f64>() * jitter_range - (jitter_range / 2.0);
           Duration::from_secs_f64((delay.as_secs_f64() + jitter).max(0.0))
       }
   }
   ```

3. **Circuit Breaker**:
   ```rust
   pub struct CircuitBreaker {
       failure_threshold: u32,
       recovery_timeout: Duration,
       state: Arc<RwLock<CircuitState>>,
   }

   enum CircuitState {
       Closed,
       Open { until: Instant },
       HalfOpen,
   }

   impl CircuitBreaker {
       pub fn is_open(&self) -> bool {
           let state = self.state.read().unwrap();
           match *state {
               CircuitState::Open { until } => {
                   if Instant::now() > until {
                       // Try to transition to half-open
                       false
                   } else {
                       true
                   }
               }
               _ => false,
           }
       }
   }
   ```

### Architecture Changes
- Add `RetryExecutor` with configurable strategies
- Implement `CircuitBreaker` for failure protection
- Add `RetryMetrics` for observability
- Enhance workflow configuration with retry settings
- Update executors to use new retry system

### Data Structures
```yaml
# Global retry configuration
retry_defaults:
  attempts: 3
  backoff: exponential
  initial_delay: 1s
  max_delay: 30s
  jitter: true
  jitter_factor: 0.3
  retry_budget: 2m

# Per-step retry override
tasks:
  - name: "API call"
    shell: "curl https://api.example.com"
    retry:
      attempts: 5
      backoff:
        exponential:
          base: 2.0
      retry_on: ["network", "timeout", "5xx"]
      on_failure: continue

  - name: "Critical operation"
    claude: "/deploy-production"
    retry:
      attempts: 1  # No retry for critical ops
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/retry.rs` - Core retry logic
  - `src/config/workflow.rs` - Retry configuration
  - `src/cook/execution/` - Integration with executors
- **External Dependencies**: `rand` for jitter

## Testing Strategy

- **Unit Tests**:
  - Backoff calculation for each strategy
  - Jitter application
  - Circuit breaker state transitions
  - Retry budget enforcement
- **Integration Tests**:
  - End-to-end retry scenarios
  - Circuit breaker in action
  - Metrics collection
  - Per-step retry overrides
- **Simulation Tests**:
  - Thundering herd prevention with jitter
  - Circuit breaker effectiveness
  - Retry storm prevention

## Documentation Requirements

- **Code Documentation**: Document retry algorithms and patterns
- **User Documentation**:
  - Retry configuration guide
  - Backoff strategy selection
  - Circuit breaker tuning
  - Best practices for retry configuration
- **Architecture Updates**: Add retry flow diagrams

## Implementation Notes

- Use tokio::time for async delays
- Consider retry middleware pattern for reusability
- Implement retry metrics dashboard
- Support custom retry predicates
- Future: Adaptive retry based on success rates

## Migration and Compatibility

- Existing `retry: N` syntax continues to work
- Defaults to exponential backoff if not specified
- Clear upgrade path to advanced retry config
- No breaking changes to current workflows