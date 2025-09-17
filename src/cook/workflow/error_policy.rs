//! Workflow-level error handling policies
//!
//! Implements comprehensive workflow-level error handling directives that control
//! how the entire workflow responds to failures, including automatic DLQ routing,
//! failure thresholds, circuit breakers, and graceful degradation strategies.

use crate::cook::execution::dlq::{DeadLetterQueue, DeadLetteredItem};
use crate::cook::execution::errors::MapReduceError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Action to take when an individual work item fails
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemFailureAction {
    /// Send failed items to Dead Letter Queue
    #[default]
    Dlq,
    /// Retry the item immediately with backoff
    Retry,
    /// Skip the item and continue
    Skip,
    /// Stop the entire workflow on first failure
    Stop,
    /// Use a custom failure handler
    Custom(String),
}

/// Strategy for collecting and reporting errors
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCollectionStrategy {
    /// Collect all errors before reporting
    #[default]
    Aggregate,
    /// Report errors as they occur
    Immediate,
    /// Report errors in batches
    Batched { size: usize },
}

/// Circuit breaker configuration for preventing cascading failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of failures to trigger open state
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: usize,
    /// Number of successes to close the circuit
    #[serde(default = "default_success_threshold")]
    pub success_threshold: usize,
    /// Timeout before attempting to close circuit
    #[serde(default = "default_circuit_timeout", with = "humantime_serde")]
    pub timeout: Duration,
    /// Number of requests allowed in half-open state
    #[serde(default = "default_half_open_requests")]
    pub half_open_requests: usize,
}

fn default_failure_threshold() -> usize {
    5
}

fn default_success_threshold() -> usize {
    3
}

fn default_circuit_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_half_open_requests() -> usize {
    3
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: default_failure_threshold(),
            success_threshold: default_success_threshold(),
            timeout: default_circuit_timeout(),
            half_open_requests: default_half_open_requests(),
        }
    }
}

/// Retry configuration with backoff strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    /// Backoff strategy for retries
    #[serde(default)]
    pub backoff: BackoffStrategy,
}

fn default_max_attempts() -> u32 {
    3
}

/// Backoff strategy for retries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed { delay: Duration },
    /// Linear increase in delay
    Linear {
        initial: Duration,
        increment: Duration,
    },
    /// Exponential backoff
    Exponential { initial: Duration, multiplier: f64 },
    /// Fibonacci sequence delays
    Fibonacci { initial: Duration },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::Exponential {
            initial: Duration::from_secs(1),
            multiplier: 2.0,
        }
    }
}

/// Workflow-level error handling policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowErrorPolicy {
    /// Action to take when an item fails
    #[serde(default)]
    pub on_item_failure: ItemFailureAction,

    /// Continue processing after failures
    #[serde(default = "default_continue_on_failure")]
    pub continue_on_failure: bool,

    /// Maximum number of failures before stopping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_failures: Option<usize>,

    /// Failure rate threshold (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_threshold: Option<f64>,

    /// Error collection strategy
    #[serde(default)]
    pub error_collection: ErrorCollectionStrategy,

    /// Circuit breaker configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub circuit_breaker: Option<CircuitBreakerConfig>,

    /// Retry configuration for failed items
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_config: Option<RetryConfig>,
}

fn default_continue_on_failure() -> bool {
    true
}

impl Default for WorkflowErrorPolicy {
    fn default() -> Self {
        Self {
            on_item_failure: ItemFailureAction::default(),
            continue_on_failure: default_continue_on_failure(),
            max_failures: None,
            failure_threshold: None,
            error_collection: ErrorCollectionStrategy::default(),
            circuit_breaker: None,
            retry_config: None,
        }
    }
}

/// Action to take in response to a failure
#[derive(Debug, Clone)]
pub enum FailureAction {
    /// Continue processing
    Continue,
    /// Retry the item
    Retry(RetryConfig),
    /// Skip the item
    Skip,
    /// Stop the workflow
    Stop(String),
}

/// Error metrics tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    /// Total number of items processed
    pub total_items: usize,
    /// Number of successful items
    pub successful: usize,
    /// Number of failed items
    pub failed: usize,
    /// Number of skipped items
    pub skipped: usize,
    /// Current failure rate
    pub failure_rate: f64,
    /// Error types and their frequencies
    pub error_types: HashMap<String, usize>,
    /// Detected failure patterns
    pub failure_patterns: Vec<FailurePattern>,
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        Self {
            total_items: 0,
            successful: 0,
            failed: 0,
            skipped: 0,
            failure_rate: 0.0,
            error_types: HashMap::new(),
            failure_patterns: Vec::new(),
        }
    }
}

/// Pattern detected in failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Type of pattern detected
    pub pattern_type: String,
    /// Frequency of this pattern
    pub frequency: usize,
    /// Items affected by this pattern
    pub items: Vec<String>,
    /// Suggested action to address the pattern
    pub suggested_action: String,
}

/// Circuit breaker state
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed, normal operation
    Closed,
    /// Circuit is open, rejecting requests
    Open { since: Instant },
    /// Circuit is half-open, testing recovery
    HalfOpen { remaining_tests: usize },
}

/// Circuit breaker for preventing cascading failures
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<Mutex<CircuitState>>,
    consecutive_failures: Arc<Mutex<usize>>,
    consecutive_successes: Arc<Mutex<usize>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            consecutive_failures: Arc::new(Mutex::new(0)),
            consecutive_successes: Arc::new(Mutex::new(0)),
        }
    }

    /// Check if the circuit is open
    pub fn is_open(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Open { since } => {
                // Check if timeout has expired
                if since.elapsed() >= self.config.timeout {
                    *state = CircuitState::HalfOpen {
                        remaining_tests: self.config.half_open_requests,
                    };
                    false
                } else {
                    true
                }
            }
            CircuitState::HalfOpen { .. } => false,
            CircuitState::Closed => false,
        }
    }

    /// Record a successful operation
    pub fn record_success(&self) {
        let mut state = self.state.lock().unwrap();
        let mut successes = self.consecutive_successes.lock().unwrap();
        let mut failures = self.consecutive_failures.lock().unwrap();

        *failures = 0;
        *successes += 1;

        if let CircuitState::HalfOpen { .. } = *state {
            if *successes >= self.config.success_threshold {
                *state = CircuitState::Closed;
                info!("Circuit breaker closed after {} successes", successes);
            }
        }
    }

    /// Record a failed operation
    pub fn record_failure(&self) {
        let mut state = self.state.lock().unwrap();
        let mut failures = self.consecutive_failures.lock().unwrap();
        let mut successes = self.consecutive_successes.lock().unwrap();

        *successes = 0;
        *failures += 1;

        match *state {
            CircuitState::Closed => {
                if *failures >= self.config.failure_threshold {
                    *state = CircuitState::Open {
                        since: Instant::now(),
                    };
                    warn!("Circuit breaker opened after {} failures", failures);
                }
            }
            CircuitState::HalfOpen {
                mut remaining_tests,
            } => {
                remaining_tests -= 1;
                if remaining_tests == 0 {
                    *state = CircuitState::Open {
                        since: Instant::now(),
                    };
                    warn!("Circuit breaker re-opened after test failures");
                } else {
                    *state = CircuitState::HalfOpen { remaining_tests };
                }
            }
            _ => {}
        }
    }
}

/// Error policy executor that applies workflow-level error handling
pub struct ErrorPolicyExecutor {
    policy: WorkflowErrorPolicy,
    metrics: Arc<Mutex<ErrorMetrics>>,
    circuit_breaker: Option<CircuitBreaker>,
    collected_errors: Arc<Mutex<Vec<String>>>,
}

impl ErrorPolicyExecutor {
    /// Create a new error policy executor
    pub fn new(policy: WorkflowErrorPolicy) -> Self {
        let circuit_breaker = policy
            .circuit_breaker
            .as_ref()
            .map(|config| CircuitBreaker::new(config.clone()));

        Self {
            policy,
            metrics: Arc::new(Mutex::new(ErrorMetrics::default())),
            circuit_breaker,
            collected_errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Handle a failed work item
    pub async fn handle_item_failure(
        &self,
        item_id: &str,
        item: &serde_json::Value,
        error: &MapReduceError,
        dlq: Option<&DeadLetterQueue>,
    ) -> Result<FailureAction, MapReduceError> {
        // Update metrics
        self.update_metrics(error);

        // Check circuit breaker
        if let Some(ref breaker) = self.circuit_breaker {
            if breaker.is_open() {
                return Ok(FailureAction::Stop("Circuit breaker open".to_string()));
            }
            breaker.record_failure();
        }

        // Check failure thresholds
        if self.should_stop_on_threshold() {
            return Ok(FailureAction::Stop(
                "Failure threshold exceeded".to_string(),
            ));
        }

        // Apply item failure strategy
        match &self.policy.on_item_failure {
            ItemFailureAction::Dlq => {
                if let Some(dlq) = dlq {
                    self.send_to_dlq(item_id, item, error, dlq).await?;
                }
                Ok(FailureAction::Continue)
            }
            ItemFailureAction::Retry => {
                if let Some(ref retry_config) = self.policy.retry_config {
                    Ok(FailureAction::Retry(retry_config.clone()))
                } else {
                    Ok(FailureAction::Skip)
                }
            }
            ItemFailureAction::Skip => {
                debug!("Skipping failed item: {}", item_id);
                Ok(FailureAction::Skip)
            }
            ItemFailureAction::Stop => Ok(FailureAction::Stop(format!("Item {} failed", item_id))),
            ItemFailureAction::Custom(handler_name) => {
                warn!("Custom handler {} not implemented, skipping", handler_name);
                Ok(FailureAction::Skip)
            }
        }
    }

    /// Record a successful item
    pub fn record_success(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.successful += 1;
        metrics.total_items += 1;
        self.update_failure_rate(&mut metrics);

        if let Some(ref breaker) = self.circuit_breaker {
            breaker.record_success();
        }
    }

    /// Update error metrics
    pub fn update_metrics(&self, error: &MapReduceError) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.failed += 1;
        metrics.total_items += 1;

        // Track error types
        let error_type = format!("{:?}", error);
        *metrics.error_types.entry(error_type).or_insert(0) += 1;

        self.update_failure_rate(&mut metrics);
        self.detect_patterns(&mut metrics);
    }

    /// Update the failure rate
    fn update_failure_rate(&self, metrics: &mut ErrorMetrics) {
        if metrics.total_items > 0 {
            metrics.failure_rate = metrics.failed as f64 / metrics.total_items as f64;
        }
    }

    /// Detect failure patterns
    fn detect_patterns(&self, metrics: &mut ErrorMetrics) {
        // Simple pattern detection based on error frequencies
        for (error_type, count) in &metrics.error_types {
            if *count >= 3 {
                // Check if pattern already exists
                let exists = metrics
                    .failure_patterns
                    .iter()
                    .any(|p| p.pattern_type == *error_type);

                if !exists {
                    metrics.failure_patterns.push(FailurePattern {
                        pattern_type: error_type.clone(),
                        frequency: *count,
                        items: Vec::new(), // Would need to track actual items
                        suggested_action: self.suggest_action(error_type),
                    });
                }
            }
        }
    }

    /// Suggest action based on error pattern
    fn suggest_action(&self, error_type: &str) -> String {
        if error_type.contains("Timeout") {
            "Consider increasing timeout_per_agent".to_string()
        } else if error_type.contains("Network") {
            "Check network connectivity and retry settings".to_string()
        } else if error_type.contains("Permission") {
            "Verify file permissions and access rights".to_string()
        } else {
            "Review error logs for more details".to_string()
        }
    }

    /// Check if workflow should stop based on thresholds
    fn should_stop_on_threshold(&self) -> bool {
        let metrics = self.metrics.lock().unwrap();

        // Check max failures
        if let Some(max_failures) = self.policy.max_failures {
            if metrics.failed >= max_failures {
                warn!(
                    "Max failures reached: {} >= {}",
                    metrics.failed, max_failures
                );
                return true;
            }
        }

        // Check failure rate threshold
        if let Some(threshold) = self.policy.failure_threshold {
            if metrics.total_items >= 10 && metrics.failure_rate > threshold {
                warn!(
                    "Failure rate exceeded: {:.2}% > {:.2}%",
                    metrics.failure_rate * 100.0,
                    threshold * 100.0
                );
                return true;
            }
        }

        // Check if continue_on_failure is false and we have any failure
        if !self.policy.continue_on_failure && metrics.failed > 0 {
            warn!("Stopping due to failure (continue_on_failure=false)");
            return true;
        }

        false
    }

    /// Send an item to the Dead Letter Queue
    async fn send_to_dlq(
        &self,
        item_id: &str,
        item: &serde_json::Value,
        error: &MapReduceError,
        dlq: &DeadLetterQueue,
    ) -> Result<(), MapReduceError> {
        let now = chrono::Utc::now();
        let dlq_item = DeadLetteredItem {
            item_id: item_id.to_string(),
            item_data: item.clone(),
            first_attempt: now,
            last_attempt: now,
            failure_count: 1,
            failure_history: vec![crate::cook::execution::dlq::FailureDetail {
                attempt_number: 1,
                timestamp: now,
                error_type: crate::cook::execution::dlq::ErrorType::Unknown,
                error_message: error.to_string(),
                stack_trace: None,
                agent_id: "map-agent".to_string(),
                step_failed: "map_phase".to_string(),
                duration_ms: 0,
            }],
            error_signature: format!("{:?}", error),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };

        dlq.add(dlq_item)
            .await
            .map_err(|e| MapReduceError::DlqError(e.to_string()))?;

        info!("Sent failed item {} to DLQ", item_id);
        Ok(())
    }

    /// Get current error metrics
    pub fn get_metrics(&self) -> ErrorMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Get collected errors for aggregate reporting
    pub fn get_collected_errors(&self) -> Vec<String> {
        self.collected_errors.lock().unwrap().clone()
    }

    /// Collect an error for aggregate reporting
    pub fn collect_error(&self, error: String) {
        match self.policy.error_collection {
            ErrorCollectionStrategy::Aggregate => {
                self.collected_errors.lock().unwrap().push(error);
            }
            ErrorCollectionStrategy::Immediate => {
                // Report immediately
                warn!("Item failure: {}", error);
            }
            ErrorCollectionStrategy::Batched { size } => {
                let mut errors = self.collected_errors.lock().unwrap();
                errors.push(error);
                if errors.len() >= size {
                    // Report batch
                    warn!("Batch of {} errors collected", errors.len());
                    for err in errors.drain(..) {
                        warn!("  - {}", err);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_requests: 1,
        };

        let breaker = CircuitBreaker::new(config);

        // Initially closed
        assert!(!breaker.is_open());

        // Record failures to open circuit
        breaker.record_failure();
        breaker.record_failure();
        assert!(!breaker.is_open());
        breaker.record_failure();
        assert!(breaker.is_open());

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));
        assert!(!breaker.is_open()); // Should be half-open now

        // Record success to close
        breaker.record_success();
        breaker.record_success();
        assert!(!breaker.is_open());
    }

    #[test]
    fn test_error_policy_thresholds() {
        let policy = WorkflowErrorPolicy {
            max_failures: Some(5),
            failure_threshold: Some(0.3),
            ..Default::default()
        };

        let executor = ErrorPolicyExecutor::new(policy);

        // Record some successes and failures
        executor.record_success();
        executor.record_success();
        executor.record_success();

        let metrics = executor.get_metrics();
        assert_eq!(metrics.successful, 3);
        assert_eq!(metrics.failure_rate, 0.0);

        // Should not stop yet
        assert!(!executor.should_stop_on_threshold());
    }
}
