//! Enhanced retry strategies with configurable backoff
//!
//! This module provides comprehensive retry mechanisms with multiple backoff strategies,
//! jitter support, circuit breakers, and fine-grained control over retry behavior.

use anyhow::{anyhow, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Retry configuration with backoff strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    #[serde(default = "default_attempts")]
    pub attempts: u32,

    /// Backoff strategy
    #[serde(default)]
    pub backoff: BackoffStrategy,

    /// Initial delay between retries
    #[serde(default = "default_initial_delay", with = "humantime_serde")]
    pub initial_delay: Duration,

    /// Maximum delay between retries
    #[serde(default = "default_max_delay", with = "humantime_serde")]
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
    #[serde(default, with = "humantime_serde")]
    pub retry_budget: Option<Duration>,

    /// Action on final failure
    #[serde(default)]
    pub on_failure: FailureAction,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            attempts: default_attempts(),
            backoff: BackoffStrategy::default(),
            initial_delay: default_initial_delay(),
            max_delay: default_max_delay(),
            jitter: false,
            jitter_factor: default_jitter_factor(),
            retry_on: Vec::new(),
            retry_budget: None,
            on_failure: FailureAction::default(),
        }
    }
}

/// Backoff strategies for retry delays
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed,
    /// Linear increase in delay
    Linear {
        #[serde(with = "humantime_serde")]
        increment: Duration,
    },
    /// Exponential increase in delay
    Exponential {
        #[serde(default = "default_exponential_base")]
        base: f64,
    },
    /// Fibonacci sequence delays
    Fibonacci,
    /// Custom delay sequence
    Custom { delays: Vec<Duration> },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        BackoffStrategy::Exponential {
            base: default_exponential_base(),
        }
    }
}

/// Error patterns to match for retry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorMatcher {
    /// Network-related errors
    Network,
    /// Timeout errors
    Timeout,
    /// HTTP 5xx errors
    ServerError,
    /// Rate limiting errors
    RateLimit,
    /// Custom regex pattern
    Pattern(String),
}

impl ErrorMatcher {
    /// Check if an error message matches this matcher
    pub fn matches(&self, error_msg: &str) -> bool {
        let error_lower = error_msg.to_lowercase();
        match self {
            ErrorMatcher::Network => {
                error_lower.contains("network")
                    || error_lower.contains("connection")
                    || error_lower.contains("refused")
                    || error_lower.contains("unreachable")
            }
            ErrorMatcher::Timeout => {
                error_lower.contains("timeout") || error_lower.contains("timed out")
            }
            ErrorMatcher::ServerError => {
                error_lower.contains("500")
                    || error_lower.contains("502")
                    || error_lower.contains("503")
                    || error_lower.contains("504")
                    || error_lower.contains("server error")
            }
            ErrorMatcher::RateLimit => {
                error_lower.contains("rate limit")
                    || error_lower.contains("429")
                    || error_lower.contains("too many requests")
            }
            ErrorMatcher::Pattern(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(error_msg)
                } else {
                    false
                }
            }
        }
    }
}

/// Action to take on final failure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum FailureAction {
    /// Stop workflow execution
    #[default]
    Stop,
    /// Continue with next step
    Continue,
    /// Execute fallback command
    Fallback { command: String },
}

/// Retry executor with circuit breaker
pub struct RetryExecutor {
    config: RetryConfig,
    metrics: Arc<RwLock<RetryMetrics>>,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new(config: RetryConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(RwLock::new(RetryMetrics::default())),
            circuit_breaker: None,
        }
    }

    /// Create with circuit breaker enabled
    pub fn with_circuit_breaker(mut self, threshold: u32, recovery_timeout: Duration) -> Self {
        self.circuit_breaker = Some(Arc::new(CircuitBreaker::new(threshold, recovery_timeout)));
        self
    }

    /// Execute an operation with retry logic
    pub async fn execute_with_retry<F, Fut, T>(&self, operation: F, context: &str) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut attempt = 0;
        let mut total_delay = Duration::ZERO;
        let _start_time = Instant::now();

        loop {
            attempt += 1;

            // Check circuit breaker
            if let Some(cb) = &self.circuit_breaker {
                if cb.is_open().await {
                    warn!("Circuit breaker open for {}", context);
                    return Err(anyhow!("Circuit breaker open"));
                }
            }

            // Execute operation
            match operation().await {
                Ok(result) => {
                    if let Some(cb) = &self.circuit_breaker {
                        cb.record_success().await;
                    }
                    self.metrics.write().await.record_success(attempt);
                    return Ok(result);
                }
                Err(err) => {
                    let error_str = err.to_string();

                    if let Some(cb) = &self.circuit_breaker {
                        cb.record_failure().await;
                    }

                    // Check if we should retry
                    if !self.should_retry(&error_str, attempt, total_delay) {
                        self.metrics.write().await.record_failure(attempt);
                        return Err(err);
                    }

                    // Calculate delay
                    let delay = self.calculate_delay(attempt);
                    let jittered_delay = self.apply_jitter(delay);

                    // Check retry budget
                    if let Some(budget) = self.config.retry_budget {
                        if total_delay + jittered_delay > budget {
                            warn!("Retry budget exhausted for {}", context);
                            self.metrics.write().await.record_failure(attempt);
                            return Err(anyhow!("Retry budget exhausted"));
                        }
                    }

                    // Log retry
                    info!(
                        "Retrying {} (attempt {}/{}) after {:?}",
                        context, attempt, self.config.attempts, jittered_delay
                    );

                    // Wait before retry
                    tokio::time::sleep(jittered_delay).await;
                    total_delay += jittered_delay;
                    self.metrics
                        .write()
                        .await
                        .record_retry(attempt, jittered_delay);
                }
            }
        }
    }

    /// Check if we should retry based on error and attempt
    fn should_retry(&self, error_msg: &str, attempt: u32, _total_delay: Duration) -> bool {
        // Check max attempts
        if attempt >= self.config.attempts {
            return false;
        }

        // If no specific matchers, retry all errors
        if self.config.retry_on.is_empty() {
            return true;
        }

        // Check if error matches any retry pattern
        self.config
            .retry_on
            .iter()
            .any(|matcher| matcher.matches(error_msg))
    }

    /// Calculate delay for the given attempt
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = match &self.config.backoff {
            BackoffStrategy::Fixed => self.config.initial_delay,
            BackoffStrategy::Linear { increment } => {
                self.config.initial_delay + *increment * (attempt - 1)
            }
            BackoffStrategy::Exponential { base } => {
                let multiplier = base.powi(attempt as i32 - 1);
                Duration::from_secs_f64(self.config.initial_delay.as_secs_f64() * multiplier)
            }
            BackoffStrategy::Fibonacci => {
                let fib = fibonacci(attempt);
                self.config.initial_delay * fib
            }
            BackoffStrategy::Custom { delays } => delays
                .get(attempt as usize - 1)
                .copied()
                .unwrap_or(self.config.max_delay),
        };

        base_delay.min(self.config.max_delay)
    }

    /// Apply jitter to delay
    pub fn apply_jitter(&self, delay: Duration) -> Duration {
        if !self.config.jitter {
            return delay;
        }

        let mut rng = rand::rng();
        let jitter_range = delay.as_secs_f64() * self.config.jitter_factor;
        let jitter = rng.random_range(-jitter_range / 2.0..=jitter_range / 2.0);
        Duration::from_secs_f64((delay.as_secs_f64() + jitter).max(0.0))
    }

    /// Get retry metrics
    pub async fn metrics(&self) -> RetryMetrics {
        self.metrics.read().await.clone()
    }
}

/// Circuit breaker for failure protection
pub struct CircuitBreaker {
    failure_threshold: u32,
    recovery_timeout: Duration,
    state: Arc<RwLock<CircuitState>>,
    consecutive_failures: Arc<RwLock<u32>>,
}

#[derive(Debug, Clone)]
enum CircuitState {
    Closed,
    Open { until: Instant },
    HalfOpen,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            consecutive_failures: Arc::new(RwLock::new(0)),
        }
    }

    /// Check if circuit breaker is open
    pub async fn is_open(&self) -> bool {
        let mut state = self.state.write().await;
        match *state {
            CircuitState::Open { until } => {
                if Instant::now() > until {
                    // Transition to half-open
                    *state = CircuitState::HalfOpen;
                    debug!("Circuit breaker transitioning to half-open");
                    false
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    /// Record a successful operation
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;
        let mut failures = self.consecutive_failures.write().await;

        *failures = 0;
        if matches!(*state, CircuitState::HalfOpen) {
            *state = CircuitState::Closed;
            debug!("Circuit breaker closed after successful operation");
        }
    }

    /// Record a failed operation
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;
        let mut failures = self.consecutive_failures.write().await;

        *failures += 1;

        if *failures >= self.failure_threshold {
            let until = Instant::now() + self.recovery_timeout;
            *state = CircuitState::Open { until };
            warn!(
                "Circuit breaker opened after {} consecutive failures",
                failures
            );
        }
    }
}

/// Retry metrics for observability
#[derive(Debug, Clone, Default)]
pub struct RetryMetrics {
    pub total_attempts: u32,
    pub successful_attempts: u32,
    pub failed_attempts: u32,
    pub retries: Vec<(u32, Duration)>,
}

impl RetryMetrics {
    fn record_success(&mut self, attempt: u32) {
        self.total_attempts = attempt;
        self.successful_attempts += 1;
    }

    fn record_failure(&mut self, attempt: u32) {
        self.total_attempts = attempt;
        self.failed_attempts += 1;
    }

    fn record_retry(&mut self, attempt: u32, delay: Duration) {
        self.retries.push((attempt, delay));
    }
}

/// Calculate fibonacci number
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => {
            let mut a = 0;
            let mut b = 1;
            for _ in 2..=n {
                let temp = a + b;
                a = b;
                b = temp;
            }
            b
        }
    }
}

// Default functions for serde
fn default_attempts() -> u32 {
    3
}

fn default_initial_delay() -> Duration {
    Duration::from_secs(1)
}

fn default_max_delay() -> Duration {
    Duration::from_secs(30)
}

fn default_jitter_factor() -> f64 {
    0.3
}

fn default_exponential_base() -> f64 {
    2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fibonacci_calculation() {
        assert_eq!(fibonacci(0), 0);
        assert_eq!(fibonacci(1), 1);
        assert_eq!(fibonacci(2), 1);
        assert_eq!(fibonacci(3), 2);
        assert_eq!(fibonacci(4), 3);
        assert_eq!(fibonacci(5), 5);
        assert_eq!(fibonacci(6), 8);
    }

    #[test]
    fn test_error_matcher_network() {
        let matcher = ErrorMatcher::Network;
        assert!(matcher.matches("Connection refused"));
        assert!(matcher.matches("Network unreachable"));
        assert!(matcher.matches("connection timeout"));
        assert!(!matcher.matches("Syntax error"));
    }

    #[test]
    fn test_error_matcher_timeout() {
        let matcher = ErrorMatcher::Timeout;
        assert!(matcher.matches("Operation timeout"));
        assert!(matcher.matches("Request timed out"));
        assert!(!matcher.matches("Network error"));
    }

    #[test]
    fn test_error_matcher_rate_limit() {
        let matcher = ErrorMatcher::RateLimit;
        assert!(matcher.matches("Rate limit exceeded"));
        assert!(matcher.matches("Error 429"));
        assert!(matcher.matches("Too many requests"));
        assert!(!matcher.matches("Server error"));
    }

    #[tokio::test]
    async fn test_retry_executor_success() {
        let config = RetryConfig::default();
        let executor = RetryExecutor::new(config);

        let result = executor
            .execute_with_retry(|| async { Ok::<_, anyhow::Error>(42) }, "test")
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let metrics = executor.metrics().await;
        assert_eq!(metrics.successful_attempts, 1);
        assert_eq!(metrics.failed_attempts, 0);
    }

    #[tokio::test]
    async fn test_retry_executor_with_retries() {
        let config = RetryConfig {
            attempts: 3,
            initial_delay: Duration::from_millis(10),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        let counter = Arc::new(RwLock::new(0));
        let counter_clone = counter.clone();

        let result = executor
            .execute_with_retry(
                || {
                    let counter = counter_clone.clone();
                    async move {
                        let mut count = counter.write().await;
                        *count += 1;
                        if *count < 3 {
                            Err(anyhow!("Temporary failure"))
                        } else {
                            Ok(*count)
                        }
                    }
                },
                "test",
            )
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        let metrics = executor.metrics().await;
        assert_eq!(metrics.total_attempts, 3);
        assert_eq!(metrics.retries.len(), 2);
    }

    #[tokio::test]
    async fn test_retry_executor_max_attempts_exceeded() {
        let config = RetryConfig {
            attempts: 2,
            initial_delay: Duration::from_millis(10),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        let result = executor
            .execute_with_retry(
                || async { Err::<i32, _>(anyhow!("Persistent failure")) },
                "test",
            )
            .await;

        assert!(result.is_err());

        let metrics = executor.metrics().await;
        assert_eq!(metrics.failed_attempts, 1);
        assert_eq!(metrics.total_attempts, 2);
    }

    #[test]
    fn test_backoff_fixed() {
        let config = RetryConfig {
            backoff: BackoffStrategy::Fixed,
            initial_delay: Duration::from_secs(2),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        assert_eq!(executor.calculate_delay(1), Duration::from_secs(2));
        assert_eq!(executor.calculate_delay(2), Duration::from_secs(2));
        assert_eq!(executor.calculate_delay(3), Duration::from_secs(2));
    }

    #[test]
    fn test_backoff_linear() {
        let config = RetryConfig {
            backoff: BackoffStrategy::Linear {
                increment: Duration::from_secs(2),
            },
            initial_delay: Duration::from_secs(1),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        assert_eq!(executor.calculate_delay(1), Duration::from_secs(1));
        assert_eq!(executor.calculate_delay(2), Duration::from_secs(3));
        assert_eq!(executor.calculate_delay(3), Duration::from_secs(5));
    }

    #[test]
    fn test_backoff_exponential() {
        let config = RetryConfig {
            backoff: BackoffStrategy::Exponential { base: 2.0 },
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(100),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        assert_eq!(executor.calculate_delay(1), Duration::from_secs(1));
        assert_eq!(executor.calculate_delay(2), Duration::from_secs(2));
        assert_eq!(executor.calculate_delay(3), Duration::from_secs(4));
        assert_eq!(executor.calculate_delay(4), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_fibonacci() {
        let config = RetryConfig {
            backoff: BackoffStrategy::Fibonacci,
            initial_delay: Duration::from_secs(1),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        assert_eq!(executor.calculate_delay(1), Duration::from_secs(1));
        assert_eq!(executor.calculate_delay(2), Duration::from_secs(1));
        assert_eq!(executor.calculate_delay(3), Duration::from_secs(2));
        assert_eq!(executor.calculate_delay(4), Duration::from_secs(3));
        assert_eq!(executor.calculate_delay(5), Duration::from_secs(5));
    }

    #[test]
    fn test_jitter_application() {
        let config = RetryConfig {
            jitter: true,
            jitter_factor: 0.5,
            initial_delay: Duration::from_secs(10),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        for _ in 0..10 {
            let jittered = executor.apply_jitter(Duration::from_secs(10));
            let secs = jittered.as_secs_f64();
            assert!(secs >= 5.0 && secs <= 15.0);
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(100));

        // Record failures to open circuit
        for _ in 0..3 {
            cb.record_failure().await;
        }

        assert!(cb.is_open().await);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open
        assert!(!cb.is_open().await);

        // Successful operation should close circuit
        cb.record_success().await;
        assert!(!cb.is_open().await);
    }

    #[tokio::test]
    async fn test_retry_with_specific_errors() {
        let config = RetryConfig {
            attempts: 3,
            initial_delay: Duration::from_millis(10),
            retry_on: vec![ErrorMatcher::Network],
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        // Network error should be retried
        let counter = Arc::new(RwLock::new(0));
        let counter_clone = counter.clone();

        let result = executor
            .execute_with_retry(
                || {
                    let counter = counter_clone.clone();
                    async move {
                        let mut count = counter.write().await;
                        *count += 1;
                        if *count < 2 {
                            Err(anyhow!("Connection refused"))
                        } else {
                            Ok(*count)
                        }
                    }
                },
                "test",
            )
            .await;

        assert!(result.is_ok());

        // Non-network error should not be retried
        let result = executor
            .execute_with_retry(|| async { Err::<i32, _>(anyhow!("Syntax error")) }, "test")
            .await;

        assert!(result.is_err());
        let metrics = executor.metrics().await;
        assert_eq!(metrics.total_attempts, 1); // No retry for non-network error
    }

    #[tokio::test]
    async fn test_retry_budget() {
        let config = RetryConfig {
            attempts: 10,
            initial_delay: Duration::from_millis(50),
            retry_budget: Some(Duration::from_millis(100)),
            ..Default::default()
        };
        let executor = RetryExecutor::new(config);

        let start = Instant::now();
        let result = executor
            .execute_with_retry(
                || async { Err::<i32, _>(anyhow!("Persistent failure")) },
                "test",
            )
            .await;

        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_millis(200));
    }
}
