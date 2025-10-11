//! Failure handling and retry logic for workflow execution
//!
//! This module contains failure recovery strategies and retry mechanisms extracted
//! from WorkflowExecutor. It implements exponential backoff, jitter, and configurable
//! retry policies.
//!
//! ## Key Components
//!
//! - **Retry Logic**: Exponential backoff with jitter
//! - **Failure Handlers**: on_failure command execution
//! - **Recovery Strategies**: Logging vs Recovery mode
//! - **Error Matching**: Selective retry based on error patterns
//!
//! ## Design Principles
//!
//! 1. **Separation of Concerns**: Retry logic independent of command execution
//! 2. **Pure Retry Calculation**: Backoff delays are pure functions
//! 3. **Flexible Configuration**: Support multiple retry strategies
//! 4. **Observable**: Detailed logging and state tracking

use crate::cook::retry_state::RetryAttempt;
use crate::cook::retry_v2::{RetryConfig, RetryExecutor};
use crate::cook::workflow::on_failure::{HandlerStrategy, OnFailureConfig};
use std::time::Duration;

use super::StepResult;

// ============================================================================
// Pure Retry Calculation Functions
// ============================================================================

/// Calculate delay for a retry attempt with exponential backoff
///
/// This is a pure function that calculates delays deterministically.
/// Actual jitter is applied separately for testing purposes.
pub fn calculate_retry_delay(retry_config: &RetryConfig, attempt: u32) -> Duration {
    let executor = RetryExecutor::new(retry_config.clone());
    executor.calculate_delay(attempt)
}

/// Apply jitter to a delay duration
///
/// Adds randomness to prevent thundering herd problems.
#[allow(deprecated)] // rand::thread_rng deprecated in favor of rand::rng
pub fn apply_jitter(delay: Duration, jitter_factor: f64) -> Duration {
    use rand::Rng;
    let mut rng = rand::rng();
    let jitter_range = delay.as_secs_f64() * jitter_factor;
    let jitter = rng.random_range(-jitter_range..=jitter_range);
    let adjusted = delay.as_secs_f64() + jitter;
    Duration::from_secs_f64(adjusted.max(0.0))
}

/// Determine if an error should trigger a retry
///
/// Pure function that checks error message against retry patterns.
pub fn should_retry_error(error_message: &str, retry_config: &RetryConfig) -> bool {
    if retry_config.retry_on.is_empty() {
        return true; // Retry all errors if no specific matchers
    }

    retry_config
        .retry_on
        .iter()
        .any(|matcher| matcher.matches(error_message))
}

// ============================================================================
// Failure Recovery Types
// ============================================================================

/// Result of executing a failure handler
#[derive(Debug, Clone)]
pub struct FailureHandlerResult {
    pub success: bool,
    #[allow(dead_code)] // Used for handler output tracking
    pub outputs: Vec<String>,
    #[allow(dead_code)] // Reserved for future recovery tracking
    pub recovered: bool,
}

/// Context for retry execution
#[derive(Debug, Clone)]
pub struct RetryContext {
    #[allow(dead_code)] // Used for tracking but not directly read
    pub command_id: String,
    pub attempt: u32,
    pub max_attempts: u32,
    pub last_error: Option<String>,
}

impl RetryContext {
    pub fn new(command_id: String, max_attempts: u32) -> Self {
        Self {
            command_id,
            attempt: 0,
            max_attempts,
            last_error: None,
        }
    }

    pub fn next_attempt(&mut self) {
        self.attempt += 1;
    }

    pub fn record_error(&mut self, error: String) {
        self.last_error = Some(error);
    }

    pub fn should_continue(&self) -> bool {
        self.attempt < self.max_attempts
    }

    pub fn is_first_attempt(&self) -> bool {
        self.attempt == 0
    }
}

// ============================================================================
// Retry Attempt Tracking
// ============================================================================

/// Create a retry attempt record for tracking
pub fn create_retry_attempt(
    attempt_number: u32,
    duration: Duration,
    success: bool,
    error: Option<String>,
    backoff_applied: Duration,
    exit_code: Option<i32>,
) -> RetryAttempt {
    RetryAttempt {
        attempt_number,
        executed_at: chrono::Utc::now(),
        duration,
        success,
        error,
        backoff_applied,
        exit_code,
    }
}

// ============================================================================
// Failure Handler Logic
// ============================================================================

/// Determine recovery strategy from failure handler result
pub fn determine_recovery_strategy(
    handler_result: &FailureHandlerResult,
    strategy: HandlerStrategy,
) -> bool {
    handler_result.success && strategy == HandlerStrategy::Recovery
}

/// Check if handler failure should be fatal
pub fn is_handler_failure_fatal(
    handler_success: bool,
    on_failure_config: &OnFailureConfig,
) -> bool {
    !handler_success && on_failure_config.handler_failure_fatal()
}

/// Build error message for retry exhaustion
pub fn build_retry_exhausted_message(
    step_name: &str,
    attempts: u32,
    last_error: Option<&str>,
) -> String {
    match last_error {
        Some(err) => format!(
            "Failed '{}' after {} attempts: {}",
            step_name, attempts, err
        ),
        None => format!("Failed '{}' after {} attempts", step_name, attempts),
    }
}

// ============================================================================
// Retry Decision Logic
// ============================================================================

/// Determine if a retry should be attempted
///
/// Pure function that encapsulates all retry decision logic.
pub fn should_attempt_retry(
    ctx: &RetryContext,
    error_message: &str,
    retry_config: &RetryConfig,
) -> bool {
    if !ctx.should_continue() {
        return false;
    }

    should_retry_error(error_message, retry_config)
}

/// Format retry progress message
pub fn format_retry_message(
    step_name: &str,
    attempt: u32,
    max_attempts: u32,
    delay: Duration,
) -> String {
    format!(
        "Retrying {} (attempt {}/{}) after {:?}",
        step_name, attempt, max_attempts, delay
    )
}

/// Format retry success message
pub fn format_retry_success_message(step_name: &str, attempts: u32) -> String {
    format!("'{}' succeeded after {} attempts", step_name, attempts)
}

/// Format retry failure message
pub fn format_retry_failure_message(attempt: u32, max_attempts: u32, error: &str) -> String {
    format!(
        "Command failed (attempt {}/{}): {}",
        attempt, max_attempts, error
    )
}

// ============================================================================
// Handler Strategy Helpers
// ============================================================================

/// Check if handlers should retry the original command
pub fn should_retry_after_handler(on_failure_config: &OnFailureConfig, success: bool) -> bool {
    on_failure_config.should_retry() && !success
}

/// Get max retries from on_failure config
pub fn get_handler_max_retries(on_failure_config: &OnFailureConfig) -> u32 {
    on_failure_config.max_retries()
}

/// Get handler timeout
#[allow(dead_code)] // Will be used in future refactoring phases
pub fn get_handler_timeout(on_failure_config: &OnFailureConfig) -> Option<u64> {
    on_failure_config.handler_timeout()
}

// ============================================================================
// Step Result Modification
// ============================================================================

/// Mark step as recovered after successful handler
pub fn mark_step_recovered(mut result: StepResult) -> StepResult {
    result.success = true;
    result.stderr.clear();
    result.exit_code = Some(0);
    result
}

/// Append handler output to step result
pub fn append_handler_output(mut result: StepResult, handler_outputs: &[String]) -> StepResult {
    result.stdout.push_str("\n--- on_failure output ---\n");
    result.stdout.push_str(&handler_outputs.join("\n"));
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::retry_v2::{BackoffStrategy, ErrorMatcher};

    fn create_test_retry_config() -> RetryConfig {
        RetryConfig {
            attempts: 3,
            backoff: BackoffStrategy::Exponential { base: 2.0 },
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(10),
            jitter: true,
            jitter_factor: 0.1,
            retry_on: vec![],
            retry_budget: None,
            on_failure: Default::default(),
        }
    }

    #[test]
    fn test_calculate_retry_delay() {
        let config = create_test_retry_config();

        let delay1 = calculate_retry_delay(&config, 1);
        let delay2 = calculate_retry_delay(&config, 2);
        let delay3 = calculate_retry_delay(&config, 3);

        // Exponential backoff: each delay should be larger
        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
    }

    #[test]
    fn test_should_retry_error_no_matchers() {
        let config = create_test_retry_config();

        // With no matchers, should retry all errors
        assert!(should_retry_error("any error", &config));
        assert!(should_retry_error("timeout", &config));
    }

    #[test]
    fn test_should_retry_error_with_matchers() {
        let mut config = create_test_retry_config();
        config.retry_on = vec![ErrorMatcher::Pattern("timeout".to_string())];

        // Should only retry matching errors
        assert!(should_retry_error("connection timeout", &config));
        assert!(!should_retry_error("permission denied", &config));
    }

    #[test]
    fn test_retry_context_lifecycle() {
        let mut ctx = RetryContext::new("test-cmd".to_string(), 3);

        assert!(ctx.is_first_attempt());
        assert_eq!(ctx.attempt, 0);
        assert!(ctx.should_continue());

        ctx.next_attempt();
        assert_eq!(ctx.attempt, 1);
        assert!(!ctx.is_first_attempt());

        ctx.record_error("test error".to_string());
        assert_eq!(ctx.last_error, Some("test error".to_string()));
    }

    #[test]
    fn test_retry_context_exhaustion() {
        let mut ctx = RetryContext::new("test-cmd".to_string(), 2);

        assert!(ctx.should_continue());
        ctx.next_attempt();
        assert!(ctx.should_continue());
        ctx.next_attempt();
        assert!(!ctx.should_continue()); // Exhausted after 2 attempts
    }

    #[test]
    fn test_determine_recovery_strategy_recovery_mode() {
        let handler_result = FailureHandlerResult {
            success: true,
            outputs: vec!["fixed".to_string()],
            recovered: false,
        };

        assert!(determine_recovery_strategy(
            &handler_result,
            HandlerStrategy::Recovery
        ));
    }

    #[test]
    fn test_determine_recovery_strategy_fallback_mode() {
        let handler_result = FailureHandlerResult {
            success: true,
            outputs: vec!["fallback".to_string()],
            recovered: false,
        };

        assert!(!determine_recovery_strategy(
            &handler_result,
            HandlerStrategy::Fallback
        ));
    }

    #[test]
    fn test_mark_step_recovered() {
        let failed_result = StepResult {
            success: false,
            exit_code: Some(1),
            stdout: "output".to_string(),
            stderr: "error".to_string(),
            json_log_location: None,
        };

        let recovered = mark_step_recovered(failed_result);

        assert!(recovered.success);
        assert_eq!(recovered.exit_code, Some(0));
        assert!(recovered.stderr.is_empty());
        assert_eq!(recovered.stdout, "output");
    }

    #[test]
    fn test_append_handler_output() {
        let result = StepResult {
            success: false,
            exit_code: Some(1),
            stdout: "original".to_string(),
            stderr: "error".to_string(),
            json_log_location: None,
        };

        let outputs = vec!["handler1".to_string(), "handler2".to_string()];
        let modified = append_handler_output(result, &outputs);

        assert!(modified.stdout.contains("original"));
        assert!(modified.stdout.contains("on_failure output"));
        assert!(modified.stdout.contains("handler1"));
        assert!(modified.stdout.contains("handler2"));
    }

    #[test]
    fn test_build_retry_exhausted_message_with_error() {
        let msg = build_retry_exhausted_message("test_cmd", 3, Some("timeout"));
        assert!(msg.contains("test_cmd"));
        assert!(msg.contains("3 attempts"));
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn test_build_retry_exhausted_message_no_error() {
        let msg = build_retry_exhausted_message("test_cmd", 5, None);
        assert!(msg.contains("test_cmd"));
        assert!(msg.contains("5 attempts"));
        assert!(!msg.contains(":"));
    }

    #[test]
    fn test_format_retry_messages() {
        let retry_msg = format_retry_message("cmd", 2, 5, Duration::from_secs(1));
        assert!(retry_msg.contains("2/5"));
        assert!(retry_msg.contains("cmd"));

        let success_msg = format_retry_success_message("cmd", 3);
        assert!(success_msg.contains("cmd"));
        assert!(success_msg.contains("3 attempts"));

        let failure_msg = format_retry_failure_message(2, 3, "error");
        assert!(failure_msg.contains("2/3"));
        assert!(failure_msg.contains("error"));
    }
}
