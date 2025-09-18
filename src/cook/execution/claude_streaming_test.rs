//! Tests for Claude streaming functionality

#[cfg(test)]
mod tests {
    use crate::cook::execution::claude::{ClaudeExecutor, ClaudeExecutorImpl};
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::cook::execution::ExecutionResult;
    use std::collections::HashMap;
    use std::path::Path;

    #[tokio::test]
    async fn test_claude_streaming_mode_enabled() {
        let mock_runner = MockCommandRunner::new();

        // Simulate Claude streaming JSON output
        let streaming_output = r#"{"event": "session_started", "session_id": "sess_123", "model": "claude-3"}
{"event": "tool_use", "tool_name": "Read", "tool_id": "tool_456", "parameters": {"file": "test.rs"}}
{"event": "message", "content": "Processing file...", "type": "text"}
{"event": "token_usage", "input_tokens": 100, "output_tokens": 50, "cache_read_tokens": 25}
Result: File processed successfully"#;

        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: streaming_output.to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("session_started"));
        assert!(result.stdout.contains("tool_use"));
        assert!(result.stdout.contains("File processed successfully"));
    }

    #[tokio::test]
    async fn test_claude_streaming_mode_disabled() {
        let mock_runner = MockCommandRunner::new();

        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "Command executed in print mode".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "false".to_string());

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "Command executed in print mode");
    }

    #[tokio::test]
    async fn test_claude_streaming_fallback_on_error() {
        let mock_runner = MockCommandRunner::new();

        // Simulate mixed JSON and non-JSON output
        let mixed_output = r#"{"event": "session_started", "session_id": "sess_123"}
ERROR: Something went wrong
{"event": "message", "content": "Recovering...", "type": "text"}
Done"#;

        mock_runner.add_response(ExecutionResult {
            success: false,
            stdout: mixed_output.to_string(),
            stderr: "Error occurred".to_string(),
            exit_code: Some(1),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.stdout.contains("session_started"));
        assert!(result.stdout.contains("ERROR"));
        assert_eq!(result.stderr, "Error occurred");
    }

    #[tokio::test]
    async fn test_claude_processor_json_parsing() {
        use crate::subprocess::streaming::{
            ClaudeJsonProcessor, LoggingClaudeHandler, StreamProcessor, StreamSource,
        };
        use std::sync::Arc;

        let handler = Arc::new(LoggingClaudeHandler::new("test"));
        let processor = ClaudeJsonProcessor::new(handler, false);

        // Test various Claude event types
        let events = vec![
            r#"{"event": "tool_use", "tool_name": "Bash", "tool_id": "123", "parameters": {"command": "ls"}}"#,
            r#"{"event": "token_usage", "input_tokens": 200, "output_tokens": 100, "cache_read_tokens": 50}"#,
            r#"{"event": "message", "content": "Hello", "type": "text"}"#,
            r#"{"event": "session_started", "session_id": "sess_456", "model": "claude-3", "tools": ["Read", "Write"]}"#,
            r#"Regular text output line"#,
        ];

        for event in events {
            processor
                .process_line(event, StreamSource::Stdout)
                .await
                .unwrap();
        }

        // Verify buffer accumulation
        let buffer = processor.get_buffer().await;
        assert!(buffer.contains("tool_use"));
        assert!(buffer.contains("token_usage"));
        assert!(buffer.contains("message"));
        assert!(buffer.contains("session_started"));
        assert!(buffer.contains("Regular text output"));
    }
}
