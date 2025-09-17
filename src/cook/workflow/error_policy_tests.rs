//! Tests for workflow-level error handling policies

use super::error_policy::*;
use crate::cook::execution::dlq::DeadLetterQueue;
use crate::cook::execution::errors::MapReduceError;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test error policy with default settings
    fn create_test_policy() -> WorkflowErrorPolicy {
        WorkflowErrorPolicy {
            on_item_failure: ItemFailureAction::Dlq,
            continue_on_failure: true,
            max_failures: Some(5),
            failure_threshold: Some(0.3),
            error_collection: ErrorCollectionStrategy::Aggregate,
            circuit_breaker: None,
            retry_config: None,
        }
    }

    /// Create a test MapReduceError
    fn create_test_error(message: &str) -> MapReduceError {
        MapReduceError::ProcessingError(message.to_string())
    }

    #[tokio::test]
    async fn test_error_policy_dlq_handling() {
        let policy = create_test_policy();
        let executor = ErrorPolicyExecutor::new(policy);

        // Create a test DLQ
        let dlq = DeadLetterQueue::new(PathBuf::from("/tmp/test-dlq"))
            .await
            .unwrap();
        let item = json!({"id": "test-item"});
        let error = create_test_error("Test processing error");

        // Handle the failure
        let action = executor
            .handle_item_failure("test-item", &item, &error, Some(&dlq))
            .await
            .unwrap();

        // Should continue processing since continue_on_failure is true
        assert!(matches!(action, FailureAction::Continue));

        // Check metrics
        let metrics = executor.get_metrics();
        assert_eq!(metrics.failed, 1);
        assert_eq!(metrics.total_items, 1);
    }

    #[tokio::test]
    async fn test_error_policy_skip_action() {
        let mut policy = create_test_policy();
        policy.on_item_failure = ItemFailureAction::Skip;
        let executor = ErrorPolicyExecutor::new(policy);

        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        let action = executor
            .handle_item_failure("test-item", &item, &error, None)
            .await
            .unwrap();

        assert!(matches!(action, FailureAction::Skip));
    }

    #[tokio::test]
    async fn test_error_policy_stop_action() {
        let mut policy = create_test_policy();
        policy.on_item_failure = ItemFailureAction::Stop;
        let executor = ErrorPolicyExecutor::new(policy);

        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        let action = executor
            .handle_item_failure("test-item", &item, &error, None)
            .await
            .unwrap();

        if let FailureAction::Stop(msg) = action {
            assert!(msg.contains("test-item"));
        } else {
            panic!("Expected Stop action");
        }
    }

    #[tokio::test]
    async fn test_error_policy_retry_action() {
        let mut policy = create_test_policy();
        policy.on_item_failure = ItemFailureAction::Retry;
        policy.retry_config = Some(RetryConfig {
            max_attempts: 3,
            backoff: BackoffStrategy::default(),
        });
        let executor = ErrorPolicyExecutor::new(policy);

        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        let action = executor
            .handle_item_failure("test-item", &item, &error, None)
            .await
            .unwrap();

        if let FailureAction::Retry(config) = action {
            assert_eq!(config.max_attempts, 3);
        } else {
            panic!("Expected Retry action");
        }
    }

    #[tokio::test]
    async fn test_max_failures_threshold() {
        let mut policy = create_test_policy();
        policy.max_failures = Some(2);
        let executor = ErrorPolicyExecutor::new(policy);

        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        // First failure - should continue
        let action1 = executor
            .handle_item_failure("item1", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action1, FailureAction::Continue));

        // Second failure - should continue (at limit)
        let action2 = executor
            .handle_item_failure("item2", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action2, FailureAction::Continue));

        // Third failure - should stop (exceeds limit)
        let action3 = executor
            .handle_item_failure("item3", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action3, FailureAction::Stop(_)));
    }

    #[tokio::test]
    async fn test_failure_rate_threshold() {
        let mut policy = create_test_policy();
        policy.failure_threshold = Some(0.25); // 25% failure rate
        let executor = ErrorPolicyExecutor::new(policy);

        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        // Record some successes
        for _ in 0..10 {
            executor.record_success();
        }

        // Record failures to reach 25% failure rate (3 failures out of 13 total = 23%)
        for i in 1..=3 {
            let action = executor
                .handle_item_failure(&format!("item{}", i), &item, &error, None)
                .await
                .unwrap();
            assert!(matches!(action, FailureAction::Continue));
        }

        // Next failure should trigger threshold (4 out of 14 = 28.5%)
        let action = executor
            .handle_item_failure("item4", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action, FailureAction::Stop(_)));
    }

    #[tokio::test]
    async fn test_continue_on_failure_false() {
        let mut policy = create_test_policy();
        policy.continue_on_failure = false;
        let executor = ErrorPolicyExecutor::new(policy);

        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        // First failure should stop workflow
        let action = executor
            .handle_item_failure("item1", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action, FailureAction::Stop(_)));
    }

    #[test]
    fn test_error_collection_aggregate() {
        let mut policy = create_test_policy();
        policy.error_collection = ErrorCollectionStrategy::Aggregate;
        let executor = ErrorPolicyExecutor::new(policy);

        // Collect some errors
        executor.collect_error("Error 1".to_string());
        executor.collect_error("Error 2".to_string());
        executor.collect_error("Error 3".to_string());

        // All errors should be collected
        let errors = executor.get_collected_errors();
        assert_eq!(errors.len(), 3);
        assert!(errors.contains(&"Error 1".to_string()));
        assert!(errors.contains(&"Error 2".to_string()));
        assert!(errors.contains(&"Error 3".to_string()));
    }

    #[test]
    fn test_error_collection_batched() {
        let mut policy = create_test_policy();
        policy.error_collection = ErrorCollectionStrategy::Batched { size: 2 };
        let executor = ErrorPolicyExecutor::new(policy);

        // Collect errors - batch should be reported at size 2
        executor.collect_error("Error 1".to_string());
        executor.collect_error("Error 2".to_string());

        // After batch report, errors should be cleared
        let errors = executor.get_collected_errors();
        assert_eq!(errors.len(), 0);

        // Add more errors
        executor.collect_error("Error 3".to_string());
        let errors = executor.get_collected_errors();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_circuit_breaker_basic() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_requests: 1,
        };

        let breaker = CircuitBreaker::new(config);

        // Initially closed
        assert!(!breaker.is_open());

        // Record failures to open circuit
        breaker.record_failure();
        assert!(!breaker.is_open()); // Still closed after 1 failure

        breaker.record_failure();
        assert!(breaker.is_open()); // Open after 2 failures

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));
        assert!(!breaker.is_open()); // Should be half-open now

        // Record successes to close
        breaker.record_success();
        breaker.record_success();
        assert!(!breaker.is_open()); // Closed again
    }

    #[tokio::test]
    async fn test_error_policy_with_circuit_breaker() {
        let mut policy = create_test_policy();
        policy.circuit_breaker = Some(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(100),
            half_open_requests: 1,
        });

        let executor = ErrorPolicyExecutor::new(policy);
        let item = json!({"id": "test-item"});
        let error = create_test_error("Test error");

        // First failure - circuit still closed
        let action1 = executor
            .handle_item_failure("item1", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action1, FailureAction::Continue));

        // Second failure - circuit opens
        let action2 = executor
            .handle_item_failure("item2", &item, &error, None)
            .await
            .unwrap();
        assert!(matches!(action2, FailureAction::Continue));

        // Third failure - circuit is open, should stop
        let action3 = executor
            .handle_item_failure("item3", &item, &error, None)
            .await
            .unwrap();
        if let FailureAction::Stop(msg) = action3 {
            assert!(msg.contains("Circuit breaker"));
        } else {
            panic!("Expected Stop due to circuit breaker");
        }
    }

    #[test]
    fn test_error_metrics_tracking() {
        let policy = create_test_policy();
        let executor = ErrorPolicyExecutor::new(policy);

        // Record mixed results
        executor.record_success();
        executor.record_success();
        executor.record_success();

        // Get metrics
        let metrics = executor.get_metrics();
        assert_eq!(metrics.successful, 3);
        assert_eq!(metrics.failed, 0);
        assert_eq!(metrics.total_items, 3);
        assert_eq!(metrics.failure_rate, 0.0);
    }

    #[test]
    fn test_failure_pattern_detection() {
        let policy = create_test_policy();
        let executor = ErrorPolicyExecutor::new(policy);

        // Simulate multiple timeout errors
        for _ in 0..3 {
            let _ = executor.update_metrics(&MapReduceError::Timeout);
        }

        let metrics = executor.get_metrics();

        // Should detect timeout pattern
        assert!(!metrics.failure_patterns.is_empty());
        let timeout_pattern = metrics
            .failure_patterns
            .iter()
            .find(|p| p.pattern_type.contains("Timeout"))
            .expect("Should detect timeout pattern");

        assert_eq!(timeout_pattern.frequency, 3);
        assert!(timeout_pattern.suggested_action.contains("timeout"));
    }
}