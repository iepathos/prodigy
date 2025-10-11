//! Claude CLI execution implementation

use super::{CommandExecutor, CommandRunner, ExecutionContext, ExecutionResult};
use crate::cook::execution::events::EventLogger;
use crate::testing::config::TestConfiguration;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

/// Trait for executing Claude commands
#[async_trait]
pub trait ClaudeExecutor: Send + Sync {
    /// Execute a Claude command
    async fn execute_claude_command(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult>;

    /// Check if Claude CLI is available
    async fn check_claude_cli(&self) -> Result<bool>;

    /// Get Claude CLI version
    async fn get_claude_version(&self) -> Result<String>;
}

/// Implementation of Claude executor
pub struct ClaudeExecutorImpl<R: CommandRunner> {
    runner: R,
    test_config: Option<Arc<TestConfiguration>>,
    event_logger: Option<Arc<EventLogger>>,
    verbosity: u8,
}

impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    /// Create a new Claude executor
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            test_config: None,
            event_logger: None,
            verbosity: 0,
        }
    }

    /// Create a new Claude executor with test configuration
    pub fn with_test_config(runner: R, test_config: Arc<TestConfiguration>) -> Self {
        Self {
            runner,
            test_config: Some(test_config),
            event_logger: None,
            verbosity: 0,
        }
    }

    /// Set the event logger for streaming observability
    pub fn with_event_logger(mut self, event_logger: Arc<EventLogger>) -> Self {
        self.event_logger = Some(event_logger);
        self
    }

    /// Set the verbosity level for console output
    pub fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> ClaudeExecutor for ClaudeExecutorImpl<R> {
    async fn execute_claude_command(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        // Handle test mode
        let test_mode = self.test_config.as_ref().map_or(false, |c| c.test_mode);
        if test_mode {
            return self.handle_test_mode_execution(command).await;
        }

        // Check for streaming mode via environment variable
        let streaming_enabled = env_vars
            .get("PRODIGY_CLAUDE_STREAMING")
            .is_some_and(|v| v == "true");

        tracing::debug!(
            "Claude execution mode: streaming={}, env_var={:?}",
            streaming_enabled,
            env_vars.get("PRODIGY_CLAUDE_STREAMING")
        );

        if streaming_enabled {
            // Try streaming mode, even without event logger (output will still be captured)
            tracing::debug!("Using streaming mode for Claude command");
            self.execute_with_streaming(command, project_path, env_vars)
                .await
        } else {
            // Existing --print mode execution
            tracing::debug!("Using print mode for Claude command");
            self.execute_with_print(command, project_path, env_vars)
                .await
        }
    }

    async fn check_claude_cli(&self) -> Result<bool> {
        // Always return true in test mode
        let test_mode = self.test_config.as_ref().map_or(false, |c| c.test_mode);
        if test_mode {
            return Ok(true);
        }

        match self
            .runner
            .run_command("claude", &["--version".to_string()])
            .await
        {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }

    async fn get_claude_version(&self) -> Result<String> {
        let output = self
            .runner
            .run_command("claude", &["--version".to_string()])
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            anyhow::bail!("Failed to get Claude version")
        }
    }
}

impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    /// Execute Claude command with --print flag (legacy non-streaming mode)
    async fn execute_with_print(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        let mut context = ExecutionContext::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            context.working_directory = project_path.to_path_buf();
            context.env_vars = env_vars.clone();
        }

        // Check for timeout configuration passed via environment variable
        if let Some(timeout_str) = env_vars.get("PRODIGY_COMMAND_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                context.timeout_seconds = Some(timeout_secs);
                tracing::debug!("Claude command timeout set to {} seconds", timeout_secs);
            }
        }

        // Claude requires some input on stdin to work properly
        context.stdin = Some("".to_string());

        let args = vec![
            "--print".to_string(),
            "--dangerously-skip-permissions".to_string(),
            command.to_string(),
        ];
        tracing::debug!("Executing claude command with args: {:?}", args);

        let result = self
            .runner
            .run_with_context("claude", &args, &context)
            .await;

        match result {
            Ok(execution_result) => {
                if !execution_result.success {
                    // Claude command executed but failed
                    let error_details = if !execution_result.stderr.is_empty() {
                        format!("stderr: {}", execution_result.stderr)
                    } else if !execution_result.stdout.is_empty() {
                        format!("stdout: {}", execution_result.stdout)
                    } else {
                        format!("exit code: {:?}", execution_result.exit_code)
                    };

                    tracing::error!("Claude command '{}' failed - {}", command, error_details);

                    return Err(anyhow::anyhow!(
                        "Claude command '{}' failed: {}",
                        command,
                        error_details
                    ));
                }
                Ok(execution_result)
            }
            Err(e) => {
                tracing::error!("Claude command '{}' execution error: {:?}", command, e);
                Err(e.context(format!("Failed to execute Claude command: {}", command)))
            }
        }
    }

    /// Execute Claude command with --output-format stream-json for real-time observability
    async fn execute_with_streaming(
        &self,
        command: &str,
        project_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> Result<ExecutionResult> {
        // Record start time for log file detection
        let execution_start = SystemTime::now();

        // Build execution context using pure helper
        let mut context = build_execution_context(project_path, env_vars.clone());

        // Check for timeout configuration using pure helper
        if let Some(timeout_secs) = parse_timeout_from_env(&env_vars) {
            context.timeout_seconds = Some(timeout_secs);
            tracing::debug!("Claude command timeout set to {} seconds", timeout_secs);
        }

        // Build command args using pure helper function
        let args = build_streaming_claude_args(command);

        tracing::debug!(
            "Executing claude command in streaming mode with args: {:?}",
            args
        );

        // Determine if we should print to console using pure helper
        let print_to_console = should_print_to_console(&env_vars, self.verbosity);

        // Create stream processor using factory function
        let processor = create_stream_processor(
            self.event_logger.clone(),
            "agent-default".to_string(),
            print_to_console,
        );

        // Use the streaming interface
        let result = self
            .runner
            .run_with_streaming("claude", &args, &context, processor)
            .await;

        match result {
            Ok(mut execution_result) => {
                // Detect JSON log location after execution (success or failure)
                use crate::cook::execution::claude_log_detection::detect_json_log_location;

                if let Some(log_location) = detect_json_log_location(
                    project_path,
                    &execution_result.stdout,
                    execution_start,
                )
                .await
                {
                    // Store in metadata
                    execution_result =
                        execution_result.with_json_log_location(log_location.clone());

                    // Display log path to console - ALWAYS, regardless of verbosity
                    // This is critical for debugging failed commands
                    if execution_result.success {
                        println!("✅ Completed | Log: {}", log_location.display());
                    } else {
                        println!("❌ Failed | Log: {}", log_location.display());
                    }

                    // Log to tracing for debugging
                    tracing::info!("Claude JSON log saved to: {}", log_location.display());
                } else {
                    tracing::debug!("Could not detect Claude JSON log location");
                }

                if !execution_result.success {
                    // Claude command executed but failed - use pure functions for error formatting
                    let error_details = format_execution_error_details(&execution_result);
                    tracing::error!("Claude command '{}' failed - {}", command, error_details);

                    // Format error message with JSON log location if available
                    let error_msg = format_error_with_log_location(
                        command,
                        &error_details,
                        execution_result.json_log_location(),
                    );

                    return Err(anyhow::anyhow!(error_msg));
                }
                Ok(execution_result)
            }
            Err(e) => {
                tracing::error!(
                    "Claude streaming command '{}' execution error: {:?}",
                    command,
                    e
                );
                Err(e.context(format!("Failed to execute Claude command: {}", command)))
            }
        }
    }

    /// Handle test mode execution
    async fn handle_test_mode_execution(&self, command: &str) -> Result<ExecutionResult> {
        println!("[TEST MODE] Would execute Claude command: {command}");

        // Check if we should simulate no changes
        if let Some(config) = &self.test_config {
            let command_name = command.trim_start_matches('/');
            // Extract just the command name, ignoring arguments
            let command_name = command_name
                .split_whitespace()
                .next()
                .unwrap_or(command_name);
            if config
                .no_changes_commands
                .iter()
                .any(|cmd| cmd.trim() == command_name)
            {
                println!("[TEST MODE] Simulating no changes for: {command_name}");
                // Return success but the orchestrator will detect no commits were made
                return Ok(ExecutionResult {
                    success: true,
                    stdout: format!("Test mode - no changes for {command}"),
                    stderr: String::new(),
                    exit_code: Some(0),
                    metadata: HashMap::new(),
                });
            }
        }

        Ok(ExecutionResult {
            success: true,
            stdout: format!("Test mode execution of {command}"),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        })
    }
}

// Pure helper functions for configuration parsing

/// Parse timeout value from environment variables
/// Returns None if the environment variable is not set or contains an invalid value
fn parse_timeout_from_env(env_vars: &HashMap<String, String>) -> Option<u64> {
    env_vars
        .get("PRODIGY_COMMAND_TIMEOUT")
        .and_then(|timeout_str| timeout_str.parse::<u64>().ok())
}

/// Determine whether to print Claude output to console
/// Checks PRODIGY_CLAUDE_CONSOLE_OUTPUT environment variable first,
/// then falls back to verbosity level (>= 1)
fn should_print_to_console(env_vars: &HashMap<String, String>, verbosity: u8) -> bool {
    env_vars
        .get("PRODIGY_CLAUDE_CONSOLE_OUTPUT")
        .map(|v| v == "true")
        .unwrap_or(verbosity >= 1)
}

/// Create a stream processor based on event logger availability
/// Pure factory function that constructs the appropriate handler
fn create_stream_processor(
    event_logger: Option<Arc<EventLogger>>,
    agent_id: String,
    print_to_console: bool,
) -> Box<dyn crate::subprocess::streaming::StreamProcessor> {
    use crate::cook::execution::claude_stream_handler::{
        ConsoleClaudeHandler, EventLoggingClaudeHandler,
    };
    use crate::subprocess::streaming::ClaudeJsonProcessor;

    if let Some(logger) = event_logger {
        let handler = Arc::new(EventLoggingClaudeHandler::new(
            logger,
            agent_id,
            print_to_console,
        ));
        Box::new(ClaudeJsonProcessor::new(handler, print_to_console))
    } else {
        let handler = Arc::new(ConsoleClaudeHandler::new(agent_id));
        Box::new(ClaudeJsonProcessor::new(handler, print_to_console))
    }
}

/// Format execution error details from an ExecutionResult
/// Pure function that prioritizes stderr → stdout → exit code
fn format_execution_error_details(result: &ExecutionResult) -> String {
    if !result.stderr.is_empty() {
        format!("stderr: {}", result.stderr)
    } else if !result.stdout.is_empty() {
        format!("stdout: {}", result.stdout)
    } else {
        format!("exit code: {:?}", result.exit_code)
    }
}

/// Format error message with optional JSON log location
/// Pure function that constructs the final error message
fn format_error_with_log_location(
    command: &str,
    error_details: &str,
    log_location: Option<&str>,
) -> String {
    if let Some(log_path) = log_location {
        format!(
            "Claude command '{}' failed: {}\n📝 Full log: {}",
            command, error_details, log_path
        )
    } else {
        format!("Claude command '{}' failed: {}", command, error_details)
    }
}

/// Build command arguments for streaming Claude execution
/// Pure function that constructs the required args for --output-format stream-json mode
fn build_streaming_claude_args(command: &str) -> Vec<String> {
    vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--dangerously-skip-permissions".to_string(),
        command.to_string(),
    ]
}

/// Build execution context for streaming Claude command
/// Pure function that constructs ExecutionContext with streaming enabled
fn build_execution_context(
    project_path: &Path,
    env_vars: HashMap<String, String>,
) -> ExecutionContext {
    ExecutionContext {
        working_directory: project_path.to_path_buf(),
        env_vars,
        capture_streaming: true,
        stdin: Some(String::new()), // Claude requires empty stdin
        ..ExecutionContext::default()
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> CommandExecutor for ClaudeExecutorImpl<R> {
    async fn execute(
        &self,
        command: &str,
        args: &[String],
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // For Claude commands, use the Claude-specific method
        if command == "claude" && args.len() == 1 {
            self.execute_claude_command(&args[0], &context.working_directory, context.env_vars)
                .await
        } else {
            // Fallback to regular command execution
            self.runner.run_with_context(command, args, &context).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::runner::tests::MockCommandRunner;

    // Phase 1: Tests for pure configuration functions

    #[test]
    fn test_parse_timeout_from_env_valid() {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_COMMAND_TIMEOUT".to_string(), "300".to_string());

        let result = parse_timeout_from_env(&env_vars);
        assert_eq!(result, Some(300));
    }

    #[test]
    fn test_parse_timeout_from_env_invalid() {
        let mut env_vars = HashMap::new();
        env_vars.insert("PRODIGY_COMMAND_TIMEOUT".to_string(), "invalid".to_string());

        let result = parse_timeout_from_env(&env_vars);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_timeout_from_env_missing() {
        let env_vars = HashMap::new();

        let result = parse_timeout_from_env(&env_vars);
        assert_eq!(result, None);
    }

    #[test]
    fn test_should_print_to_console_env_var_true() {
        let mut env_vars = HashMap::new();
        env_vars.insert(
            "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
            "true".to_string(),
        );

        // Should be true regardless of verbosity
        assert!(should_print_to_console(&env_vars, 0));
        assert!(should_print_to_console(&env_vars, 1));
    }

    #[test]
    fn test_should_print_to_console_env_var_false() {
        let mut env_vars = HashMap::new();
        env_vars.insert(
            "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
            "false".to_string(),
        );

        // Should be false regardless of verbosity
        assert!(!should_print_to_console(&env_vars, 0));
        assert!(!should_print_to_console(&env_vars, 1));
    }

    #[test]
    fn test_should_print_to_console_verbosity_high() {
        let env_vars = HashMap::new();

        // Should be true when verbosity >= 1
        assert!(should_print_to_console(&env_vars, 1));
        assert!(should_print_to_console(&env_vars, 2));
    }

    #[test]
    fn test_should_print_to_console_verbosity_low() {
        let env_vars = HashMap::new();

        // Should be false when verbosity < 1
        assert!(!should_print_to_console(&env_vars, 0));
    }

    // Phase 2: Tests for stream processor factory

    #[test]
    fn test_create_stream_processor_with_event_logger() {
        use crate::cook::execution::events::EventLogger;
        use std::sync::Arc;

        let event_logger = Arc::new(EventLogger::new(vec![]));
        let processor = create_stream_processor(Some(event_logger), "test-agent".to_string(), true);

        // Just verify we got a processor - the fact that it compiles and runs is enough
        // The actual behavior is tested in integration tests
        drop(processor); // Explicitly drop to show we're just testing creation
    }

    #[test]
    fn test_create_stream_processor_without_event_logger() {
        let processor = create_stream_processor(None, "test-agent".to_string(), false);

        // Just verify we got a processor
        drop(processor);
    }

    #[test]
    fn test_create_stream_processor_console_flags() {
        // Test with print_to_console true
        let processor_verbose = create_stream_processor(None, "test-agent".to_string(), true);
        drop(processor_verbose);

        // Test with print_to_console false
        let processor_quiet = create_stream_processor(None, "test-agent".to_string(), false);
        drop(processor_quiet);
    }

    // Phase 3: Tests for error formatting functions

    #[test]
    fn test_format_execution_error_details_with_stderr() {
        let result = ExecutionResult {
            success: false,
            stdout: "some output".to_string(),
            stderr: "error message".to_string(),
            exit_code: Some(1),
            metadata: HashMap::new(),
        };

        let details = format_execution_error_details(&result);
        assert_eq!(details, "stderr: error message");
    }

    #[test]
    fn test_format_execution_error_details_with_stdout_only() {
        let result = ExecutionResult {
            success: false,
            stdout: "output message".to_string(),
            stderr: String::new(),
            exit_code: Some(1),
            metadata: HashMap::new(),
        };

        let details = format_execution_error_details(&result);
        assert_eq!(details, "stdout: output message");
    }

    #[test]
    fn test_format_execution_error_details_with_neither() {
        let result = ExecutionResult {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(127),
            metadata: HashMap::new(),
        };

        let details = format_execution_error_details(&result);
        assert_eq!(details, "exit code: Some(127)");
    }

    #[test]
    fn test_format_error_with_log_location_present() {
        let error_msg = format_error_with_log_location(
            "/test-command",
            "stderr: some error",
            Some("/tmp/session-abc123.json"),
        );

        assert!(error_msg.contains("Claude command '/test-command' failed"));
        assert!(error_msg.contains("stderr: some error"));
        assert!(error_msg.contains("📝 Full log:"));
        assert!(error_msg.contains("/tmp/session-abc123.json"));
    }

    #[test]
    fn test_format_error_with_log_location_absent() {
        let error_msg = format_error_with_log_location("/test-command", "stderr: some error", None);

        assert_eq!(
            error_msg,
            "Claude command '/test-command' failed: stderr: some error"
        );
        assert!(!error_msg.contains("📝 Full log:"));
    }

    // Phase 4: Tests for command args builder

    #[test]
    fn test_build_streaming_claude_args() {
        let args = build_streaming_claude_args("/test-command");

        assert_eq!(args.len(), 5);
        assert_eq!(args[0], "--output-format");
        assert_eq!(args[1], "stream-json");
        assert_eq!(args[2], "--verbose");
        assert_eq!(args[3], "--dangerously-skip-permissions");
        assert_eq!(args[4], "/test-command");
    }

    #[test]
    fn test_build_streaming_claude_args_different_commands() {
        let args1 = build_streaming_claude_args("/prodigy-lint");
        assert_eq!(args1[4], "/prodigy-lint");

        let args2 = build_streaming_claude_args("/fix-issue");
        assert_eq!(args2[4], "/fix-issue");
    }

    #[test]
    fn test_build_streaming_claude_args_required_flags() {
        let args = build_streaming_claude_args("/any-command");

        // Verify all required flags are present
        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[tokio::test]
    async fn test_claude_verbosity_streaming() {
        // Test that verbosity level controls streaming output
        let runner = MockCommandRunner::new();

        // Test with verbosity 0 (default) - should NOT print to console
        let executor_quiet = ClaudeExecutorImpl::new(runner).with_verbosity(0);
        let env_vars: HashMap<String, String> = HashMap::new();

        // Check the internal print_to_console logic by checking if it would print
        let print_to_console_quiet = env_vars
            .get("PRODIGY_CLAUDE_CONSOLE_OUTPUT")
            .map(|v| v == "true")
            .unwrap_or_else(|| executor_quiet.verbosity >= 1);
        assert!(
            !print_to_console_quiet,
            "Verbosity 0 should not print to console"
        );

        // Test with verbosity 1 (-v) - should print to console
        let runner2 = MockCommandRunner::new();
        let executor_verbose = ClaudeExecutorImpl::new(runner2).with_verbosity(1);
        let print_to_console_verbose = env_vars
            .get("PRODIGY_CLAUDE_CONSOLE_OUTPUT")
            .map(|v| v == "true")
            .unwrap_or_else(|| executor_verbose.verbosity >= 1);
        assert!(
            print_to_console_verbose,
            "Verbosity 1 should print to console"
        );

        // Test override with environment variable
        let mut env_vars_override = HashMap::new();
        env_vars_override.insert(
            "PRODIGY_CLAUDE_CONSOLE_OUTPUT".to_string(),
            "true".to_string(),
        );
        let print_to_console_override = env_vars_override
            .get("PRODIGY_CLAUDE_CONSOLE_OUTPUT")
            .map(|v| v == "true")
            .unwrap_or(false); // Default to false when env var is not set
        assert!(
            print_to_console_override,
            "Environment variable should override verbosity"
        );
    }

    #[tokio::test]
    async fn test_claude_executor_check() {
        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "claude version 1.0.0".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let available = executor.check_claude_cli().await.unwrap();
        assert!(available);
    }

    #[tokio::test]
    async fn test_claude_executor_version() {
        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "claude version 1.0.0\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let version = executor.get_claude_version().await.unwrap();
        assert_eq!(version, "claude version 1.0.0");
    }

    #[tokio::test]
    async fn test_claude_command_execution() {
        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(ExecutionResult {
            success: true,
            stdout: "Command executed".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            metadata: HashMap::new(),
        });

        let executor = ClaudeExecutorImpl::new(mock_runner);
        let env_vars = HashMap::new();

        let result = executor
            .execute_claude_command("/test-command", Path::new("/tmp"), env_vars)
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.stdout, "Command executed");
    }
}
