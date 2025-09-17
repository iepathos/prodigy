//! Retry state management for checkpoint persistence and restoration
//!
//! This module handles the preservation and restoration of retry state across workflow
//! interruptions and resumptions, ensuring retry logic functions correctly during resumed execution.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::cook::retry_v2::{BackoffStrategy, RetryConfig};

/// Enhanced retry checkpoint state for comprehensive persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryCheckpointState {
    /// Per-command retry states
    pub command_retry_states: HashMap<String, CommandRetryState>,
    /// Global retry configuration
    pub global_retry_config: Option<RetryConfig>,
    /// Execution history for all retries
    pub retry_execution_history: Vec<RetryExecution>,
    /// Circuit breaker states
    pub circuit_breaker_states: HashMap<String, CircuitBreakerState>,
    /// Correlation IDs for tracking
    pub retry_correlation_map: HashMap<String, String>,
    /// Timestamp of checkpoint
    pub checkpointed_at: DateTime<Utc>,
}

/// Retry state for an individual command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRetryState {
    /// Unique command identifier
    pub command_id: String,
    /// Number of attempts made
    pub attempt_count: u32,
    /// Maximum attempts allowed
    pub max_attempts: u32,
    /// Last attempt timestamp
    pub last_attempt_at: Option<DateTime<Utc>>,
    /// Next scheduled retry time
    pub next_retry_at: Option<DateTime<Utc>>,
    /// Current backoff state
    pub backoff_state: BackoffState,
    /// History of retry attempts
    pub retry_history: Vec<RetryAttempt>,
    /// Current retry strategy
    pub retry_config: Option<RetryConfig>,
    /// Whether circuit breaker is open
    pub is_circuit_broken: bool,
    /// Time when retry budget expires
    pub retry_budget_expires_at: Option<DateTime<Utc>>,
    /// Total time spent in retries
    pub total_retry_duration: Duration,
}

/// State of backoff strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffState {
    /// Backoff strategy being used
    pub strategy: BackoffStrategy,
    /// Current delay duration
    #[serde(with = "humantime_serde")]
    pub current_delay: Duration,
    /// Base delay for calculations
    #[serde(with = "humantime_serde")]
    pub base_delay: Duration,
    /// Maximum delay allowed
    #[serde(with = "humantime_serde")]
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Whether jitter is applied
    pub jitter_enabled: bool,
    /// Jitter factor (0.0 to 1.0)
    pub jitter_factor: f64,
    /// Last sequence values for fibonacci
    pub fibonacci_prev: Option<u64>,
    pub fibonacci_curr: Option<u64>,
}

/// Record of a single retry attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryAttempt {
    /// Attempt number (1-based)
    pub attempt_number: u32,
    /// When the attempt was executed
    pub executed_at: DateTime<Utc>,
    /// Duration of the attempt
    #[serde(with = "humantime_serde")]
    pub duration: Duration,
    /// Whether the attempt succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Backoff delay applied before this attempt
    #[serde(with = "humantime_serde")]
    pub backoff_applied: Duration,
    /// Exit code if applicable
    pub exit_code: Option<i32>,
}

/// Comprehensive retry execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryExecution {
    /// Command that was retried
    pub command_id: String,
    /// Correlation ID for tracking
    pub correlation_id: String,
    /// Start time of retry sequence
    pub started_at: DateTime<Utc>,
    /// End time of retry sequence
    pub completed_at: Option<DateTime<Utc>>,
    /// Total attempts made
    pub total_attempts: u32,
    /// Whether it ultimately succeeded
    pub succeeded: bool,
    /// Final error if failed
    pub final_error: Option<String>,
}

/// Circuit breaker state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    /// Current state of circuit breaker
    pub state: CircuitState,
    /// Number of consecutive failures
    pub failure_count: u32,
    /// Failure threshold before opening
    pub failure_threshold: u32,
    /// Last failure timestamp
    pub last_failure_at: Option<DateTime<Utc>>,
    /// Recovery timeout duration
    #[serde(with = "humantime_serde")]
    pub recovery_timeout: Duration,
    /// Maximum calls in half-open state
    pub half_open_max_calls: u32,
    /// Success count in half-open state
    pub half_open_success_count: u32,
}

/// Circuit breaker state enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed (normal operation)
    Closed,
    /// Circuit is open (rejecting calls)
    Open,
    /// Circuit is half-open (testing recovery)
    HalfOpen,
}

/// Manages retry state across checkpoint operations
pub struct RetryStateManager {
    /// Checkpoint state storage
    checkpoint_state: Arc<RwLock<Option<RetryCheckpointState>>>,
    /// Active command retry states
    command_states: Arc<RwLock<HashMap<String, CommandRetryState>>>,
    /// Circuit breakers
    circuit_breakers: Arc<RwLock<HashMap<String, CircuitBreakerState>>>,
}

impl Default for RetryStateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryStateManager {
    /// Create a new retry state manager
    pub fn new() -> Self {
        Self {
            checkpoint_state: Arc::new(RwLock::new(None)),
            command_states: Arc::new(RwLock::new(HashMap::new())),
            circuit_breakers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create checkpoint state for persistence
    pub async fn create_checkpoint_state(&self) -> Result<RetryCheckpointState> {
        let command_states = self.command_states.read().await;
        let circuit_breakers = self.circuit_breakers.read().await;

        let checkpoint = RetryCheckpointState {
            command_retry_states: command_states.clone(),
            global_retry_config: None, // Will be set by caller if needed
            retry_execution_history: Vec::new(), // Populated separately
            circuit_breaker_states: circuit_breakers.clone(),
            retry_correlation_map: HashMap::new(), // Populated separately
            checkpointed_at: Utc::now(),
        };

        Ok(checkpoint)
    }

    /// Restore retry state from checkpoint
    pub async fn restore_from_checkpoint(&self, checkpoint: &RetryCheckpointState) -> Result<()> {
        info!(
            "Restoring retry state from checkpoint at {}",
            checkpoint.checkpointed_at
        );

        // Validate checkpoint consistency
        self.validate_checkpoint_consistency(checkpoint)?;

        // Restore command retry states
        let mut command_states = self.command_states.write().await;
        for (command_id, state) in &checkpoint.command_retry_states {
            debug!(
                "Restoring retry state for command {}: {} attempts",
                command_id, state.attempt_count
            );

            // Adjust timing for elapsed time since checkpoint
            let mut restored_state = state.clone();
            if let Some(next_retry) = state.next_retry_at {
                let elapsed = Utc::now() - checkpoint.checkpointed_at;
                restored_state.next_retry_at = Some(next_retry + elapsed);
            }

            command_states.insert(command_id.clone(), restored_state);
        }

        // Restore circuit breaker states
        let mut circuit_breakers = self.circuit_breakers.write().await;
        for (command_id, cb_state) in &checkpoint.circuit_breaker_states {
            debug!(
                "Restoring circuit breaker for {}: {:?}",
                command_id, cb_state.state
            );

            // Check if circuit should transition based on elapsed time
            let mut restored_cb = cb_state.clone();
            if cb_state.state == CircuitState::Open {
                if let Some(last_failure) = cb_state.last_failure_at {
                    let elapsed = Utc::now() - last_failure;
                    if elapsed.num_seconds() as u64 >= cb_state.recovery_timeout.as_secs() {
                        restored_cb.state = CircuitState::HalfOpen;
                        restored_cb.half_open_success_count = 0;
                        info!(
                            "Circuit breaker for {} transitioned to half-open",
                            command_id
                        );
                    }
                }
            }

            circuit_breakers.insert(command_id.clone(), restored_cb);
        }

        // Store checkpoint for reference
        let mut checkpoint_state = self.checkpoint_state.write().await;
        *checkpoint_state = Some(checkpoint.clone());

        Ok(())
    }

    /// Get retry state for a specific command
    pub async fn get_command_retry_state(&self, command_id: &str) -> Option<CommandRetryState> {
        let states = self.command_states.read().await;
        states.get(command_id).cloned()
    }

    /// Update retry state after an attempt
    pub async fn update_retry_state(
        &self,
        command_id: &str,
        attempt: RetryAttempt,
        config: &RetryConfig,
    ) -> Result<()> {
        let mut states = self.command_states.write().await;

        let state = states
            .entry(command_id.to_string())
            .or_insert_with(|| CommandRetryState {
                command_id: command_id.to_string(),
                attempt_count: 0,
                max_attempts: config.attempts,
                last_attempt_at: None,
                next_retry_at: None,
                backoff_state: self.create_initial_backoff_state(config),
                retry_history: Vec::new(),
                retry_config: Some(config.clone()),
                is_circuit_broken: false,
                retry_budget_expires_at: config
                    .retry_budget
                    .map(|budget| Utc::now() + ChronoDuration::from_std(budget).unwrap()),
                total_retry_duration: Duration::from_secs(0),
            });

        // Update state
        state.attempt_count += 1;
        state.last_attempt_at = Some(attempt.executed_at);
        state.retry_history.push(attempt.clone());
        state.total_retry_duration += attempt.duration;

        // Calculate next retry time if not successful
        if !attempt.success && state.attempt_count < state.max_attempts {
            let next_delay = self.calculate_next_delay(&mut state.backoff_state)?;
            state.next_retry_at = Some(Utc::now() + ChronoDuration::from_std(next_delay)?);
            state.backoff_state.current_delay = next_delay;
        }

        // Update circuit breaker
        if !attempt.success {
            self.update_circuit_breaker(command_id, false).await?;
        } else {
            self.update_circuit_breaker(command_id, true).await?;
        }

        Ok(())
    }

    /// Check if command can retry
    pub async fn can_retry(&self, command_id: &str) -> Result<bool> {
        let states = self.command_states.read().await;
        let circuit_breakers = self.circuit_breakers.read().await;

        // Check command retry state
        if let Some(state) = states.get(command_id) {
            // Check attempt limit
            if state.attempt_count >= state.max_attempts {
                debug!("Command {} exceeded max attempts", command_id);
                return Ok(false);
            }

            // Check retry budget
            if let Some(expires_at) = state.retry_budget_expires_at {
                if Utc::now() >= expires_at {
                    debug!("Command {} retry budget expired", command_id);
                    return Ok(false);
                }
            }

            // Check circuit breaker
            if let Some(cb) = circuit_breakers.get(command_id) {
                if cb.state == CircuitState::Open {
                    debug!("Circuit breaker open for command {}", command_id);
                    return Ok(false);
                }
            }

            Ok(true)
        } else {
            // No retry state yet, can retry
            Ok(true)
        }
    }

    /// Calculate the next backoff delay
    fn calculate_next_delay(&self, backoff_state: &mut BackoffState) -> Result<Duration> {
        let base_delay = match &backoff_state.strategy {
            BackoffStrategy::Fixed => backoff_state.base_delay,
            BackoffStrategy::Linear { increment } => backoff_state.current_delay + *increment,
            BackoffStrategy::Exponential { base } => {
                let millis = (backoff_state.current_delay.as_millis() as f64 * base) as u64;
                Duration::from_millis(millis)
            }
            BackoffStrategy::Fibonacci => {
                let (prev, curr) = if let (Some(p), Some(c)) =
                    (backoff_state.fibonacci_prev, backoff_state.fibonacci_curr)
                {
                    (p, c)
                } else {
                    (1, 1)
                };

                let next = prev + curr;
                backoff_state.fibonacci_prev = Some(curr);
                backoff_state.fibonacci_curr = Some(next);

                Duration::from_secs(next)
            }
            BackoffStrategy::Custom { delays } => {
                // Use the delay for the current attempt, or last one if exceeded
                let index = backoff_state.current_delay.as_secs() as usize;
                delays
                    .get(index)
                    .or_else(|| delays.last())
                    .copied()
                    .unwrap_or(backoff_state.base_delay)
            }
        };

        // Apply jitter if enabled
        let delay = if backoff_state.jitter_enabled {
            let jitter_range = base_delay.as_millis() as f64 * backoff_state.jitter_factor;
            let jitter = rand::random::<f64>() * jitter_range - (jitter_range / 2.0);
            let millis = (base_delay.as_millis() as f64 + jitter).max(0.0) as u64;
            Duration::from_millis(millis)
        } else {
            base_delay
        };

        // Enforce max delay
        Ok(delay.min(backoff_state.max_delay))
    }

    /// Create initial backoff state from config
    fn create_initial_backoff_state(&self, config: &RetryConfig) -> BackoffState {
        BackoffState {
            strategy: config.backoff.clone(),
            current_delay: config.initial_delay,
            base_delay: config.initial_delay,
            max_delay: config.max_delay,
            multiplier: match &config.backoff {
                BackoffStrategy::Exponential { base } => *base,
                _ => 2.0,
            },
            jitter_enabled: config.jitter,
            jitter_factor: config.jitter_factor,
            fibonacci_prev: None,
            fibonacci_curr: None,
        }
    }

    /// Update circuit breaker state
    async fn update_circuit_breaker(&self, command_id: &str, success: bool) -> Result<()> {
        let mut breakers = self.circuit_breakers.write().await;

        let breaker = breakers.entry(command_id.to_string()).or_insert_with(|| {
            CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                failure_threshold: 5, // Default threshold
                last_failure_at: None,
                recovery_timeout: Duration::from_secs(60),
                half_open_max_calls: 3,
                half_open_success_count: 0,
            }
        });

        match breaker.state {
            CircuitState::Closed => {
                if !success {
                    breaker.failure_count += 1;
                    breaker.last_failure_at = Some(Utc::now());

                    if breaker.failure_count >= breaker.failure_threshold {
                        breaker.state = CircuitState::Open;
                        warn!("Circuit breaker opened for command {}", command_id);
                    }
                } else {
                    breaker.failure_count = 0;
                }
            }
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = breaker.last_failure_at {
                    let elapsed = Utc::now() - last_failure;
                    if elapsed.num_seconds() as u64 >= breaker.recovery_timeout.as_secs() {
                        breaker.state = CircuitState::HalfOpen;
                        breaker.half_open_success_count = 0;
                        info!("Circuit breaker half-open for command {}", command_id);
                    }
                }
            }
            CircuitState::HalfOpen => {
                if success {
                    breaker.half_open_success_count += 1;
                    if breaker.half_open_success_count >= breaker.half_open_max_calls {
                        breaker.state = CircuitState::Closed;
                        breaker.failure_count = 0;
                        info!("Circuit breaker closed for command {}", command_id);
                    }
                } else {
                    breaker.state = CircuitState::Open;
                    breaker.last_failure_at = Some(Utc::now());
                    warn!("Circuit breaker re-opened for command {}", command_id);
                }
            }
        }

        Ok(())
    }

    /// Validate checkpoint consistency
    fn validate_checkpoint_consistency(&self, checkpoint: &RetryCheckpointState) -> Result<()> {
        // Check for inconsistent retry states
        for (command_id, state) in &checkpoint.command_retry_states {
            if state.attempt_count > state.max_attempts + 1 {
                return Err(anyhow!(
                    "Inconsistent retry state for {}: attempts {} > max {}",
                    command_id,
                    state.attempt_count,
                    state.max_attempts
                ));
            }

            // Validate retry history
            if state.retry_history.len() as u32 != state.attempt_count {
                warn!(
                    "Retry history mismatch for {}: {} history entries vs {} attempts",
                    command_id,
                    state.retry_history.len(),
                    state.attempt_count
                );
            }
        }

        Ok(())
    }

    /// Clear retry state for a command
    pub async fn clear_command_state(&self, command_id: &str) {
        let mut states = self.command_states.write().await;
        let mut breakers = self.circuit_breakers.write().await;

        states.remove(command_id);
        breakers.remove(command_id);

        debug!("Cleared retry state for command {}", command_id);
    }

    /// Get summary of all retry states
    pub async fn get_retry_summary(&self) -> HashMap<String, (u32, u32, bool)> {
        let states = self.command_states.read().await;
        let breakers = self.circuit_breakers.read().await;

        let mut summary = HashMap::new();

        for (command_id, state) in states.iter() {
            let is_open = breakers
                .get(command_id)
                .map(|b| b.state == CircuitState::Open)
                .unwrap_or(false);

            summary.insert(
                command_id.clone(),
                (state.attempt_count, state.max_attempts, is_open),
            );
        }

        summary
    }
}

/// Coordinated retry state for MapReduce operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatedRetryState {
    /// Work item retry states
    pub work_item_retries: HashMap<String, WorkItemRetryState>,
    /// Failed items in DLQ
    pub dlq_retries: Vec<DlqRetryState>,
    /// Cross-agent consistency check result
    pub consistency_valid: bool,
}

/// Retry state for a work item in MapReduce
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItemRetryState {
    /// Work item identifier
    pub work_item_id: String,
    /// Agent processing this item
    pub agent_id: String,
    /// Retry attempt count
    pub attempt_count: u32,
    /// Last attempt timestamp
    pub last_attempt_at: DateTime<Utc>,
}

/// DLQ retry state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqRetryState {
    /// Work item in DLQ
    pub work_item_id: String,
    /// Number of times retried from DLQ
    pub dlq_retry_count: u32,
    /// When it entered DLQ
    pub entered_dlq_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_state_persistence_and_restoration() {
        let manager = RetryStateManager::new();

        // Create some retry state
        let attempt = RetryAttempt {
            attempt_number: 1,
            executed_at: Utc::now(),
            duration: Duration::from_secs(2),
            success: false,
            error: Some("Test error".to_string()),
            backoff_applied: Duration::from_secs(0),
            exit_code: Some(1),
        };

        let config = RetryConfig::default();
        manager
            .update_retry_state("test_cmd", attempt, &config)
            .await
            .unwrap();

        // Create checkpoint
        let checkpoint = manager.create_checkpoint_state().await.unwrap();
        assert_eq!(checkpoint.command_retry_states.len(), 1);

        // Create new manager and restore
        let new_manager = RetryStateManager::new();
        new_manager
            .restore_from_checkpoint(&checkpoint)
            .await
            .unwrap();

        // Verify state was restored
        let restored = new_manager.get_command_retry_state("test_cmd").await;
        assert!(restored.is_some());
        assert_eq!(restored.unwrap().attempt_count, 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_state_transitions() {
        let manager = RetryStateManager::new();

        // Simulate failures to open circuit
        for i in 0..5 {
            let attempt = RetryAttempt {
                attempt_number: i + 1,
                executed_at: Utc::now(),
                duration: Duration::from_secs(1),
                success: false,
                error: Some("Failed".to_string()),
                backoff_applied: Duration::from_secs(i as u64),
                exit_code: Some(1),
            };

            let config = RetryConfig::default();
            manager
                .update_retry_state("test_cmd", attempt, &config)
                .await
                .unwrap();
        }

        // Check circuit is open
        let can_retry = manager.can_retry("test_cmd").await.unwrap();
        assert!(!can_retry, "Circuit should be open after failures");

        // Verify state in checkpoint
        let checkpoint = manager.create_checkpoint_state().await.unwrap();
        let cb_state = checkpoint.circuit_breaker_states.get("test_cmd").unwrap();
        assert_eq!(cb_state.state, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_retry_budget_enforcement() {
        let manager = RetryStateManager::new();

        let mut config = RetryConfig::default();
        config.retry_budget = Some(Duration::from_secs(5));
        config.attempts = 100; // High limit to test budget

        // Create initial state with budget
        let attempt = RetryAttempt {
            attempt_number: 1,
            executed_at: Utc::now() - ChronoDuration::seconds(10), // 10 seconds ago
            duration: Duration::from_secs(1),
            success: false,
            error: Some("Failed".to_string()),
            backoff_applied: Duration::from_secs(0),
            exit_code: Some(1),
        };

        manager
            .update_retry_state("budget_cmd", attempt, &config)
            .await
            .unwrap();

        // Manually expire budget for testing
        {
            let mut states = manager.command_states.write().await;
            if let Some(state) = states.get_mut("budget_cmd") {
                state.retry_budget_expires_at = Some(Utc::now() - ChronoDuration::seconds(1));
            }
        }

        // Check retry is not allowed
        let can_retry = manager.can_retry("budget_cmd").await.unwrap();
        assert!(!can_retry, "Should not retry after budget expired");
    }
}
