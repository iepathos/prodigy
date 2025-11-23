//! Integration tests for error context preservation

#[cfg(test)]
mod integration_tests {
    use crate::cook::error::{ContextResult, ResultExt};
    use stillwater::ContextError;

    #[test]
    fn test_context_error_basic() {
        fn inner_operation() -> Result<(), String> {
            Err("base error".to_string())
        }

        fn middle_operation() -> ContextResult<(), String> {
            inner_operation().context("middle operation")
        }

        let result = middle_operation();
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert_eq!(error.inner(), "base error");
        let trail = error.context_trail();
        assert_eq!(trail.len(), 1);
        assert!(trail.contains(&"middle operation".to_string()));
    }

    #[test]
    fn test_context_error_with_context() {
        fn process_item(id: &str) -> ContextResult<String, std::io::Error> {
            std::fs::read_to_string("nonexistent.txt")
                .with_context(|| format!("Processing item {}", id))
        }

        let result = process_item("test-123");
        assert!(result.is_err());

        let error = result.unwrap_err();
        let trail = error.context_trail();
        assert_eq!(trail.len(), 1);
        assert!(trail[0].contains("Processing item test-123"));
    }

    #[test]
    fn test_error_context_display() {
        // Test that we can create and format a ContextError
        let err = ContextError::new("File not found");
        let display = format!("{}", err);
        assert!(display.contains("File not found"));
    }

    #[test]
    fn test_dlq_error_context_integration() {
        use crate::cook::execution::dlq::FailureDetail;
        use chrono::Utc;

        // Simulate creating a FailureDetail with error context
        let error_context = vec![
            "Processing work item item-123".to_string(),
            "Executing map phase".to_string(),
            "Running MapReduce job".to_string(),
        ];

        let failure_detail = FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: crate::cook::execution::dlq::ErrorType::CommandFailed { exit_code: 1 },
            error_message: "Command failed".to_string(),
            error_context: Some(error_context.clone()),
            stack_trace: None,
            agent_id: "agent-1".to_string(),
            step_failed: "execute_command".to_string(),
            duration_ms: 1000,
            json_log_location: Some("/path/to/log.json".to_string()),
        };

        // Verify the error context is preserved
        assert!(failure_detail.error_context.is_some());
        let context = failure_detail.error_context.unwrap();
        assert_eq!(context.len(), 3);
        assert_eq!(context[0], "Processing work item item-123");
        assert_eq!(context[1], "Executing map phase");
        assert_eq!(context[2], "Running MapReduce job");
    }
}
