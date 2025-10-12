//! Test for Claude streaming log path display
//!
//! Reproduces the issue where the streaming log path is not displayed
//! when executing Claude commands in MapReduce setup phase.

#[cfg(test)]
mod tests {
    use crate::cook::execution::claude::{ClaudeExecutor, ClaudeExecutorImpl};
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::cook::execution::ExecutionResult;
    use std::collections::HashMap;
    use std::path::Path;

    #[tokio::test]
    async fn test_streaming_log_path_displayed_before_execution() {
        // This test reproduces the issue where users don't see the log path
        // before Claude execution starts

        // Initialize tracing to capture logs
        let _guard = init_test_tracing();

        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: r#"{"event":"session_started","session_id":"test-123"}
{"event":"message","content":"Hello"}
{"event":"token_usage","input_tokens":100,"output_tokens":50,"cache_read_tokens":25}"#
                .to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);

        // Simulate MapReduce setup phase environment
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());
        env_vars.insert("PRODIGY_AUTOMATION".to_string(), "true".to_string());

        // Execute Claude command
        let result = executor
            .execute_claude_command(
                "/prodigy-analyze-features-for-book --project test",
                Path::new("/tmp/test-worktree"),
                env_vars,
            )
            .await;

        // Verify execution succeeded
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.success);

        // Verify the log path was captured in metadata
        assert!(
            result.json_log_location().is_some(),
            "Expected json_log_location to be set in result metadata"
        );

        let log_path = result.json_log_location().unwrap();
        println!("📁 Log path that SHOULD have been displayed: {}", log_path);

        assert!(
            log_path.contains(".prodigy/logs/claude-streaming/"),
            "Expected log path to be in .prodigy/logs/claude-streaming/, got: {}",
            log_path
        );
        assert!(
            log_path.ends_with(".jsonl"),
            "Expected log path to end with .jsonl, got: {}",
            log_path
        );
    }

    fn init_test_tracing() -> tracing::subscriber::DefaultGuard {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_test_writer()
            .finish();
        tracing::subscriber::set_default(subscriber)
    }

    #[tokio::test]
    async fn test_non_streaming_mode_no_log_path() {
        // Verify that non-streaming mode doesn't create a log path
        // Note: Spec 129 changed default to streaming, so we must explicitly opt-out

        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "Command output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);

        // Explicitly opt-out of streaming mode (spec 129: streaming is now the default)
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "false".to_string());

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        // Non-streaming mode should not have a log location
        assert!(result.json_log_location().is_none());
    }

    #[tokio::test]
    async fn test_streaming_log_saved_to_file() {
        // Verify that the streaming JSON is actually saved to the file

        use std::fs;

        let mock_runner = MockCommandRunner::new();
        let test_json = r#"{"event":"session_started","session_id":"abc"}
{"event":"message","content":"Test message"}
{"event":"token_usage","input_tokens":10,"output_tokens":5,"cache_read_tokens":0}"#;

        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: test_json.to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);

        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_CLAUDE_STREAMING".to_string(), "true".to_string());

        let result = executor
            .execute_claude_command("/test", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        // Verify log file was created and contains the streaming JSON
        let log_path = result.json_log_location().unwrap();
        assert!(
            Path::new(log_path).exists(),
            "Log file should exist at: {}",
            log_path
        );

        let contents = fs::read_to_string(log_path).unwrap();
        assert_eq!(contents, test_json);
    }
}
