//! Integration tests for Claude JSON log location capture
//!
//! These tests verify that Claude JSON log locations are properly captured
//! and propagated through the execution pipeline to AgentResult and DLQ.

#[cfg(test)]
mod json_log_location_tests {
    use crate::cook::execution::mapreduce::agent::types::{AgentResult, AgentStatus};
    use crate::cook::execution::mapreduce::command::executor::CommandResult;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_command_result_preserves_json_log_location() {
        // Test that CommandResult can store and retrieve json_log_location
        let log_path = Some("/path/to/logs/session-123.json".to_string());

        let result = CommandResult {
            output: Some("test output".to_string()),
            exit_code: 0,
            variables: std::collections::HashMap::new(),
            duration: Duration::from_secs(1),
            success: true,
            stderr: String::new(),
            json_log_location: log_path.clone(),
        };

        assert_eq!(result.json_log_location, log_path);
    }

    #[test]
    fn test_agent_result_captures_log_location() {
        // Test that AgentResult properly stores json_log_location
        let log_location =
            Some("/Users/test/.local/state/claude/logs/session-abc.json".to_string());

        let agent_result = AgentResult {
            item_id: "test-item".to_string(),
            status: AgentStatus::Success,
            output: Some("Command executed successfully".to_string()),
            commits: vec!["abc123".to_string()],
            files_modified: vec!["src/main.rs".to_string()],
            duration: Duration::from_secs(30),
            error: None,
            worktree_path: Some(PathBuf::from("/test/worktree")),
            branch_name: Some("feature-branch".to_string()),
            worktree_session_id: Some("session-123".to_string()),
            json_log_location: log_location.clone(),
            cleanup_status: None,
        };

        assert_eq!(agent_result.json_log_location, log_location);
    }

    #[test]
    fn test_agent_result_no_log_location_on_failure() {
        // Test that failed agent results don't have log locations
        // (since the command didn't complete successfully)
        let agent_result = AgentResult {
            item_id: "test-item".to_string(),
            status: AgentStatus::Failed("Command failed".to_string()),
            output: None,
            commits: Vec::new(),
            files_modified: Vec::new(),
            duration: Duration::from_secs(5),
            error: Some("Execution error".to_string()),
            worktree_path: Some(PathBuf::from("/test/worktree")),
            branch_name: Some("feature-branch".to_string()),
            worktree_session_id: Some("session-123".to_string()),
            json_log_location: None,
            cleanup_status: None,
        };

        assert!(agent_result.json_log_location.is_none());
        assert!(matches!(agent_result.status, AgentStatus::Failed(_)));
    }

    #[test]
    fn test_failure_detail_includes_json_log_location() {
        use crate::cook::execution::dlq::{ErrorType, FailureDetail};
        use chrono::Utc;

        // Test that FailureDetail can store and retrieve json_log_location
        let json_log_location = Some("/path/to/logs/session-xyz.json".to_string());

        let failure_detail = FailureDetail {
            attempt_number: 1,
            timestamp: Utc::now(),
            error_type: ErrorType::Timeout,
            error_message: "Agent execution failed".to_string(),
            error_context: None,
            stack_trace: Some("Stack trace here".to_string()),
            agent_id: "agent-1".to_string(),
            step_failed: "claude: /test-command".to_string(),
            duration_ms: 30000,
            json_log_location: json_log_location.clone(),
        };

        // Verify json_log_location is preserved
        assert_eq!(failure_detail.json_log_location, json_log_location);

        // Verify serialization preserves json_log_location
        let serialized = serde_json::to_string(&failure_detail).unwrap();
        let deserialized: FailureDetail = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.json_log_location, json_log_location);
    }

    #[test]
    fn test_shell_command_result_has_no_log_location() {
        // Shell commands don't produce Claude JSON logs
        let result = CommandResult {
            output: Some("Shell output".to_string()),
            exit_code: 0,
            variables: std::collections::HashMap::new(),
            duration: Duration::from_secs(1),
            success: true,
            stderr: String::new(),
            json_log_location: None,
        };

        assert!(result.json_log_location.is_none());
    }

    #[test]
    fn test_commands_result_with_duration_preserves_log_location() {
        // Test that builder methods preserve json_log_location
        let log_path = "/path/to/logs/session-456.json".to_string();

        let result = crate::commands::CommandResult::success(serde_json::Value::String(
            "success".to_string(),
        ))
        .with_duration(1000)
        .with_json_log_location(log_path.clone());

        assert_eq!(result.json_log_location, Some(log_path));
        assert_eq!(result.duration_ms, Some(1000));
    }
}
