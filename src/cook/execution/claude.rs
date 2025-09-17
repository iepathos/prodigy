//! Claude CLI execution implementation

use super::{CommandExecutor, CommandRunner, ExecutionContext, ExecutionResult};
use crate::cook::execution::events::{EventLogger, MapReduceEvent};
use crate::testing::config::TestConfiguration;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
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
}

impl<R: CommandRunner> ClaudeExecutorImpl<R> {
    /// Create a new Claude executor
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            test_config: None,
            event_logger: None,
        }
    }

    /// Create a new Claude executor with test configuration
    pub fn with_test_config(runner: R, test_config: Arc<TestConfiguration>) -> Self {
        Self {
            runner,
            test_config: Some(test_config),
            event_logger: None,
        }
    }

    /// Set the event logger for streaming observability
    pub fn with_event_logger(mut self, event_logger: Arc<EventLogger>) -> Self {
        self.event_logger = Some(event_logger);
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

        if streaming_enabled && self.event_logger.is_some() {
            self.execute_with_streaming(command, project_path, env_vars)
                .await
        } else {
            // Existing --print mode execution
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

        if let Err(ref e) = result {
            tracing::error!("Claude command failed: {:?}", e);
        }

        result
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

        // For now, we'll fall back to non-streaming execution since CommandRunner
        // doesn't yet support streaming. This will be enhanced when the runner is updated.
        // TODO: Implement actual streaming support in CommandRunner
        let result = self
            .runner
            .run_with_context("claude", &args, &context)
            .await;

        if let Ok(ref exec_result) = result {
            // Parse the streaming JSON output and emit events
            self.parse_and_emit_streaming_output(&exec_result.stdout, "agent-default")
                .await;
        }

        if let Err(ref e) = result {
            tracing::error!("Claude streaming command failed: {:?}", e);
        }

        result
    }

    /// Parse streaming JSON output and emit Claude events
    async fn parse_and_emit_streaming_output(&self, output: &str, agent_id: &str) {
        if let Some(event_logger) = &self.event_logger {
            for line in output.lines() {
                if line.trim().is_empty() {
                    continue;
                }

                // Try to parse as JSON
                if let Ok(json) = serde_json::from_str::<Value>(line) {
                    // Parse different event types from Claude's stream-json format
                    if let Some(event_type) = json.get("event").and_then(|v| v.as_str()) {
                        match event_type {
                            "tool_use" => {
                                if let Some(tool_name) =
                                    json.get("tool_name").and_then(|v| v.as_str())
                                {
                                    let tool_id = json
                                        .get("tool_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let parameters =
                                        json.get("parameters").cloned().unwrap_or(Value::Null);

                                    let event = MapReduceEvent::ClaudeToolInvoked {
                                        agent_id: agent_id.to_string(),
                                        tool_name: tool_name.to_string(),
                                        tool_id,
                                        parameters,
                                        timestamp: Utc::now(),
                                    };

                                    if let Err(e) = event_logger.log(event).await {
                                        tracing::warn!("Failed to log Claude tool event: {}", e);
                                    }
                                }
                            }
                            "token_usage" => {
                                let input_tokens = json
                                    .get("input_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let output_tokens = json
                                    .get("output_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let cache_tokens = json
                                    .get("cache_read_tokens")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);

                                let event = MapReduceEvent::ClaudeTokenUsage {
                                    agent_id: agent_id.to_string(),
                                    input_tokens,
                                    output_tokens,
                                    cache_tokens,
                                };

                                if let Err(e) = event_logger.log(event).await {
                                    tracing::warn!("Failed to log Claude token usage event: {}", e);
                                }
                            }
                            "session_started" => {
                                let session_id = json
                                    .get("session_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let model = json
                                    .get("model")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let tools = json
                                    .get("tools")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                let event = MapReduceEvent::ClaudeSessionStarted {
                                    agent_id: agent_id.to_string(),
                                    session_id,
                                    model,
                                    tools,
                                };

                                if let Err(e) = event_logger.log(event).await {
                                    tracing::warn!("Failed to log Claude session event: {}", e);
                                }
                            }
                            "message" => {
                                if let Some(content) = json.get("content").and_then(|v| v.as_str())
                                {
                                    let message_type = json
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("text")
                                        .to_string();

                                    let event = MapReduceEvent::ClaudeMessage {
                                        agent_id: agent_id.to_string(),
                                        content: content.to_string(),
                                        message_type,
                                    };

                                    if let Err(e) = event_logger.log(event).await {
                                        tracing::warn!("Failed to log Claude message event: {}", e);
                                    }
                                }
                            }
                            _ => {
                                tracing::trace!("Unhandled Claude event type: {}", event_type);
                            }
                        }
                    }
                } else {
                    tracing::trace!("Non-JSON line in Claude output: {}", line);
                }
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
