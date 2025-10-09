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

    /// Dispatch a JSON event to the appropriate handler method
    async fn dispatch_event(&self, json: &Value) -> Result<()> {
        if let Some(event_type) = json.get("event").and_then(|v| v.as_str()) {
            match event_type {
                "tool_use" => {
                    if let Some((tool_name, tool_id, parameters)) = parse_tool_use(json) {
                        self.handler
                            .on_tool_invocation(&tool_name, &tool_id, &parameters)
                            .await?;
                    }
                }
                "token_usage" => {
                    if let Some((input, output, cache)) = parse_token_usage(json) {
                        self.handler.on_token_usage(input, output, cache).await?;
                    }
                }
                "message" => {
                    if let Some((content, message_type)) = parse_message(json) {
                        self.handler.on_message(&content, &message_type).await?;
                    }
                }
                "session_started" => {
                    if let Some((session_id, model, tools)) = parse_session_start(json) {
                        self.handler
                            .on_session_start(&session_id, &model, tools)
                            .await?;
                    }
                }
                _ => {
                    // Unknown event type, pass to raw handler
                    self.handler.on_raw_event(event_type, json).await?;
                }
            }
        } else {
            // JSON without event field
            self.handler.on_raw_event("unknown", json).await?;
        }
        Ok(())
    }
}

// Pure helper functions for JSON field extraction

/// Extract a string field from JSON, returning a default if missing or invalid
fn extract_string_field<'a>(json: &'a Value, field: &str, default: &'a str) -> &'a str {
    json.get(field).and_then(|v| v.as_str()).unwrap_or(default)
}

/// Extract a u64 field from JSON, returning a default if missing or invalid
fn extract_u64_field(json: &Value, field: &str, default: u64) -> u64 {
    json.get(field).and_then(|v| v.as_u64()).unwrap_or(default)
}

/// Extract an array of strings from JSON, returning an empty vector if missing or invalid
fn extract_string_array(json: &Value, field: &str) -> Vec<String> {
    json.get(field)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// Pure parsing functions for each event type

/// Parse a tool_use event, returning (tool_name, tool_id, parameters)
fn parse_tool_use(json: &Value) -> Option<(String, String, Value)> {
    let tool_name = extract_string_field(json, "tool_name", "unknown").to_string();
    let tool_id = extract_string_field(json, "tool_id", "unknown").to_string();
    let parameters = json.get("parameters").cloned().unwrap_or(Value::Null);
    Some((tool_name, tool_id, parameters))
}

/// Parse a token_usage event, returning (input_tokens, output_tokens, cache_read_tokens)
fn parse_token_usage(json: &Value) -> Option<(u64, u64, u64)> {
    let input = extract_u64_field(json, "input_tokens", 0);
    let output = extract_u64_field(json, "output_tokens", 0);
    let cache = extract_u64_field(json, "cache_read_tokens", 0);
    Some((input, output, cache))
}

/// Parse a message event, returning (content, message_type)
fn parse_message(json: &Value) -> Option<(String, String)> {
    let content = extract_string_field(json, "content", "").to_string();
    let message_type = extract_string_field(json, "type", "text").to_string();
    Some((content, message_type))
}

/// Parse a session_started event, returning (session_id, model, tools)
fn parse_session_start(json: &Value) -> Option<(String, String, Vec<String>)> {
    let session_id = extract_string_field(json, "session_id", "unknown").to_string();
    let model = extract_string_field(json, "model", "unknown").to_string();
    let tools = extract_string_array(json, "tools");
    Some((session_id, model, tools))
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

        // Try to parse as JSON and dispatch to appropriate handler
        match serde_json::from_str::<Value>(line) {
            Ok(json) => self.dispatch_event(&json).await,
            Err(_) => self.handler.on_text_line(line, source).await,
        }
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
    use serde_json::json;

    #[test]
    fn test_extract_string_field() {
        let json = json!({"name": "test", "value": "hello"});

        assert_eq!(extract_string_field(&json, "name", "default"), "test");
        assert_eq!(extract_string_field(&json, "value", "default"), "hello");
        assert_eq!(extract_string_field(&json, "missing", "default"), "default");

        // Test with non-string value
        let json = json!({"number": 42});
        assert_eq!(extract_string_field(&json, "number", "default"), "default");
    }

    #[test]
    fn test_extract_u64_field() {
        let json = json!({"count": 42, "zero": 0});

        assert_eq!(extract_u64_field(&json, "count", 99), 42);
        assert_eq!(extract_u64_field(&json, "zero", 99), 0);
        assert_eq!(extract_u64_field(&json, "missing", 99), 99);

        // Test with non-number value
        let json = json!({"string": "not a number"});
        assert_eq!(extract_u64_field(&json, "string", 99), 99);
    }

    #[test]
    fn test_extract_string_array() {
        let json = json!({"tools": ["Read", "Write", "Edit"]});

        let result = extract_string_array(&json, "tools");
        assert_eq!(result, vec!["Read", "Write", "Edit"]);

        // Test with missing field
        let result = extract_string_array(&json, "missing");
        assert_eq!(result, Vec::<String>::new());

        // Test with mixed types in array
        let json = json!({"mixed": ["a", 1, "b", null, "c"]});
        let result = extract_string_array(&json, "mixed");
        assert_eq!(result, vec!["a", "b", "c"]);

        // Test with non-array value
        let json = json!({"not_array": "string"});
        let result = extract_string_array(&json, "not_array");
        assert_eq!(result, Vec::<String>::new());
    }

    #[test]
    fn test_parse_tool_use() {
        let json = json!({
            "tool_name": "Read",
            "tool_id": "tool_123",
            "parameters": {"file": "test.rs"}
        });

        let result = parse_tool_use(&json);
        assert!(result.is_some());
        let (name, id, params) = result.unwrap();
        assert_eq!(name, "Read");
        assert_eq!(id, "tool_123");
        assert_eq!(params.get("file").and_then(|v| v.as_str()), Some("test.rs"));

        // Test with missing fields (should use defaults)
        let json = json!({});
        let result = parse_tool_use(&json);
        assert!(result.is_some());
        let (name, id, params) = result.unwrap();
        assert_eq!(name, "unknown");
        assert_eq!(id, "unknown");
        assert_eq!(params, Value::Null);
    }

    #[test]
    fn test_parse_token_usage() {
        let json = json!({
            "input_tokens": 100,
            "output_tokens": 50,
            "cache_read_tokens": 25
        });

        let result = parse_token_usage(&json);
        assert!(result.is_some());
        let (input, output, cache) = result.unwrap();
        assert_eq!(input, 100);
        assert_eq!(output, 50);
        assert_eq!(cache, 25);

        // Test with missing fields (should use defaults)
        let json = json!({});
        let result = parse_token_usage(&json);
        assert!(result.is_some());
        let (input, output, cache) = result.unwrap();
        assert_eq!(input, 0);
        assert_eq!(output, 0);
        assert_eq!(cache, 0);
    }

    #[test]
    fn test_parse_message() {
        let json = json!({
            "content": "Hello world",
            "type": "text"
        });

        let result = parse_message(&json);
        assert!(result.is_some());
        let (content, msg_type) = result.unwrap();
        assert_eq!(content, "Hello world");
        assert_eq!(msg_type, "text");

        // Test with missing fields (should use defaults)
        let json = json!({});
        let result = parse_message(&json);
        assert!(result.is_some());
        let (content, msg_type) = result.unwrap();
        assert_eq!(content, "");
        assert_eq!(msg_type, "text");
    }

    #[test]
    fn test_parse_session_start() {
        let json = json!({
            "session_id": "sess_123",
            "model": "claude-3",
            "tools": ["Read", "Write", "Edit"]
        });

        let result = parse_session_start(&json);
        assert!(result.is_some());
        let (session_id, model, tools) = result.unwrap();
        assert_eq!(session_id, "sess_123");
        assert_eq!(model, "claude-3");
        assert_eq!(tools, vec!["Read", "Write", "Edit"]);

        // Test with missing fields (should use defaults)
        let json = json!({});
        let result = parse_session_start(&json);
        assert!(result.is_some());
        let (session_id, model, tools) = result.unwrap();
        assert_eq!(session_id, "unknown");
        assert_eq!(model, "unknown");
        assert_eq!(tools, Vec::<String>::new());
    }

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
