//! Claude-specific stream processor for real-time output handling

use super::processor::StreamProcessor;
use super::types::StreamSource;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handler for Claude streaming events
#[async_trait]
pub trait ClaudeStreamHandler: Send + Sync {
    /// Handle a tool invocation event
    async fn on_tool_invocation(
        &self,
        tool_name: &str,
        tool_id: &str,
        parameters: &Value,
    ) -> Result<()>;

    /// Handle token usage update
    async fn on_token_usage(&self, input: u64, output: u64, cache: u64) -> Result<()>;

    /// Handle a Claude message
    async fn on_message(&self, content: &str, message_type: &str) -> Result<()>;

    /// Handle session start
    async fn on_session_start(
        &self,
        session_id: &str,
        model: &str,
        tools: Vec<String>,
    ) -> Result<()>;

    /// Handle raw JSON event (for unknown event types)
    async fn on_raw_event(&self, event_type: &str, json: &Value) -> Result<()>;

    /// Handle non-JSON output line
    async fn on_text_line(&self, line: &str, source: StreamSource) -> Result<()>;
}

/// Stream processor for Claude's JSON streaming output
pub struct ClaudeJsonProcessor {
    handler: Arc<dyn ClaudeStreamHandler>,
    buffer: Arc<Mutex<String>>,
    print_to_console: bool,
}

impl ClaudeJsonProcessor {
    /// Create a new Claude JSON processor
    pub fn new(handler: Arc<dyn ClaudeStreamHandler>, print_to_console: bool) -> Self {
        Self {
            handler,
            buffer: Arc::new(Mutex::new(String::new())),
            print_to_console,
        }
    }

    /// Get the accumulated buffer content
    pub async fn get_buffer(&self) -> String {
        self.buffer.lock().await.clone()
    }
}

#[async_trait]
impl StreamProcessor for ClaudeJsonProcessor {
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
        // Accumulate lines for compatibility
        let mut buffer = self.buffer.lock().await;
        buffer.push_str(line);
        buffer.push('\n');
        drop(buffer);

        // Print to console if enabled for real-time feedback
        if self.print_to_console && source == StreamSource::Stdout {
            println!("{}", line);
        }

        // Skip empty lines
        if line.trim().is_empty() {
            return Ok(());
        }

        // Try to parse as JSON
        match serde_json::from_str::<Value>(line) {
            Ok(json) => {
                // Parse Claude event types
                if let Some(event_type) = json.get("event").and_then(|v| v.as_str()) {
                    match event_type {
                        "tool_use" => {
                            let tool_name = json
                                .get("tool_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let tool_id = json
                                .get("tool_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let parameters = json.get("parameters").cloned().unwrap_or(Value::Null);

                            self.handler
                                .on_tool_invocation(tool_name, tool_id, &parameters)
                                .await?;
                        }
                        "token_usage" => {
                            let input = json
                                .get("input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let output = json
                                .get("output_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let cache = json
                                .get("cache_read_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);

                            self.handler.on_token_usage(input, output, cache).await?;
                        }
                        "message" => {
                            let content =
                                json.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            let message_type =
                                json.get("type").and_then(|v| v.as_str()).unwrap_or("text");

                            self.handler.on_message(content, message_type).await?;
                        }
                        "session_started" => {
                            let session_id = json
                                .get("session_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let model = json
                                .get("model")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let tools = json
                                .get("tools")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default();

                            self.handler
                                .on_session_start(session_id, model, tools)
                                .await?;
                        }
                        _ => {
                            // Unknown event type, pass to raw handler
                            self.handler.on_raw_event(event_type, &json).await?;
                        }
                    }
                } else {
                    // JSON without event field
                    self.handler.on_raw_event("unknown", &json).await?;
                }
            }
            Err(_) => {
                // Not JSON, treat as text output
                self.handler.on_text_line(line, source).await?;
            }
        }

        Ok(())
    }

    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()> {
        tracing::debug!("Claude command completed with exit code: {:?}", exit_code);
        Ok(())
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        tracing::error!("Claude streaming error: {}", error);
        Ok(())
    }
}

/// Default implementation that logs events
pub struct LoggingClaudeHandler {
    prefix: String,
}

impl LoggingClaudeHandler {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

#[async_trait]
impl ClaudeStreamHandler for LoggingClaudeHandler {
    async fn on_tool_invocation(
        &self,
        tool_name: &str,
        tool_id: &str,
        parameters: &Value,
    ) -> Result<()> {
        tracing::info!("{}: Tool invoked: {} ({})", self.prefix, tool_name, tool_id);
        tracing::debug!("{}: Tool parameters: {}", self.prefix, parameters);
        Ok(())
    }

    async fn on_token_usage(&self, input: u64, output: u64, cache: u64) -> Result<()> {
        tracing::info!(
            "{}: Tokens - Input: {}, Output: {}, Cache: {}",
            self.prefix,
            input,
            output,
            cache
        );
        Ok(())
    }

    async fn on_message(&self, content: &str, message_type: &str) -> Result<()> {
        tracing::debug!("{}: Message ({}): {}", self.prefix, message_type, content);
        Ok(())
    }

    async fn on_session_start(
        &self,
        session_id: &str,
        model: &str,
        tools: Vec<String>,
    ) -> Result<()> {
        tracing::info!(
            "{}: Session started - ID: {}, Model: {}, Tools: {:?}",
            self.prefix,
            session_id,
            model,
            tools
        );
        Ok(())
    }

    async fn on_raw_event(&self, event_type: &str, json: &Value) -> Result<()> {
        tracing::trace!("{}: Raw event ({}): {}", self.prefix, event_type, json);
        Ok(())
    }

    async fn on_text_line(&self, line: &str, _source: StreamSource) -> Result<()> {
        tracing::trace!("{}: Text: {}", self.prefix, line);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_claude_json_processor() {
        let handler = Arc::new(LoggingClaudeHandler::new("test"));
        let processor = ClaudeJsonProcessor::new(handler, false);

        // Test tool invocation parsing
        let tool_json = r#"{"event": "tool_use", "tool_name": "Read", "tool_id": "123", "parameters": {"file": "test.rs"}}"#;
        processor
            .process_line(tool_json, StreamSource::Stdout)
            .await
            .unwrap();

        // Test token usage parsing
        let token_json = r#"{"event": "token_usage", "input_tokens": 100, "output_tokens": 50, "cache_read_tokens": 25}"#;
        processor
            .process_line(token_json, StreamSource::Stdout)
            .await
            .unwrap();

        // Test message parsing
        let msg_json = r#"{"event": "message", "content": "Hello", "type": "text"}"#;
        processor
            .process_line(msg_json, StreamSource::Stdout)
            .await
            .unwrap();

        // Test non-JSON line
        processor
            .process_line("Regular text output", StreamSource::Stdout)
            .await
            .unwrap();

        // Verify buffer accumulation
        let buffer = processor.get_buffer().await;
        assert!(buffer.contains("tool_use"));
        assert!(buffer.contains("token_usage"));
        assert!(buffer.contains("message"));
        assert!(buffer.contains("Regular text output"));
    }
}
