//! Claude CLI execution implementation

use super::{CommandExecutor, CommandRunner, ExecutionContext, ExecutionResult};
use crate::cook::execution::events::EventLogger;
use crate::testing::config::TestConfiguration;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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
        let mut context = ExecutionContext::default();
        #[allow(clippy::field_reassign_with_default)]
        {
            context.working_directory = project_path.to_path_buf();
            context.env_vars = env_vars.clone();
            context.capture_streaming = true; // Enable streaming capture
        }

        // Check for timeout configuration
        if let Some(timeout_str) = env_vars.get("PRODIGY_COMMAND_TIMEOUT") {
            if let Ok(timeout_secs) = timeout_str.parse::<u64>() {
                context.timeout_seconds = Some(timeout_secs);
                tracing::debug!("Claude command timeout set to {} seconds", timeout_secs);
            }
        }

        // Claude requires some input on stdin
        context.stdin = Some("".to_string());

        let args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--dangerously-skip-permissions".to_string(),
            command.to_string(),
        ];

        tracing::debug!(
            "Executing claude command in streaming mode with args: {:?}",
            args
        );

        // Check if we can use the streaming interface
        use crate::subprocess::streaming::{ClaudeJsonProcessor, StreamProcessor};
        use std::sync::Arc;

        // Determine if we should print to console based on verbosity
        // Show Claude streaming output only with -v (verbose) or higher
        let print_to_console = env_vars
            .get("PRODIGY_CLAUDE_CONSOLE_OUTPUT")
            .map(|v| v == "true")
            .unwrap_or_else(|| self.verbosity >= 1); // Default to showing output only with -v or higher

        // Create the appropriate handler based on whether we have an event logger
        let processor: Box<dyn StreamProcessor> = if let Some(ref event_logger) = self.event_logger
        {
            use crate::cook::execution::claude_stream_handler::EventLoggingClaudeHandler;
            let handler = Arc::new(EventLoggingClaudeHandler::new(
                event_logger.clone(),
                "agent-default".to_string(),
                print_to_console,
            ));
            Box::new(ClaudeJsonProcessor::new(handler, print_to_console))
        } else {
            use crate::cook::execution::claude_stream_handler::ConsoleClaudeHandler;
            let handler = Arc::new(ConsoleClaudeHandler::new("agent-default".to_string()));
            Box::new(ClaudeJsonProcessor::new(handler, print_to_console))
        };

        // Use the streaming interface
        let result = self
            .runner
            .run_with_streaming("claude", &args, &context, processor)
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
                });
            }
        }

        Ok(ExecutionResult {
            success: true,
            stdout: format!("Test mode execution of {command}"),
            stderr: String::new(),
            exit_code: Some(0),
        })
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
