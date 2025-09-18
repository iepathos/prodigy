//! Claude stream handler for integrating with event logging

use crate::cook::execution::events::{EventLogger, MapReduceEvent};
use crate::subprocess::streaming::{ClaudeStreamHandler, StreamSource};
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;

/// Claude stream handler that logs events to EventLogger
pub struct EventLoggingClaudeHandler {
    event_logger: Arc<EventLogger>,
    agent_id: String,
    print_to_console: bool,
}

impl EventLoggingClaudeHandler {
    /// Create a new event logging handler
    pub fn new(event_logger: Arc<EventLogger>, agent_id: String, print_to_console: bool) -> Self {
        Self {
            event_logger,
            agent_id,
            print_to_console,
        }
    }
}

#[async_trait]
impl ClaudeStreamHandler for EventLoggingClaudeHandler {
    async fn on_tool_invocation(
        &self,
        tool_name: &str,
        tool_id: &str,
        parameters: &Value,
    ) -> Result<()> {
        if self.print_to_console {
            println!("🔧 Tool invoked: {}", tool_name);
        }

        let event = MapReduceEvent::ClaudeToolInvoked {
            agent_id: self.agent_id.clone(),
            tool_name: tool_name.to_string(),
            tool_id: tool_id.to_string(),
            parameters: parameters.clone(),
            timestamp: Utc::now(),
        };

        if let Err(e) = self.event_logger.log(event).await {
            tracing::warn!("Failed to log Claude tool event: {}", e);
        }
        Ok(())
    }

    async fn on_token_usage(&self, input: u64, output: u64, cache: u64) -> Result<()> {
        if self.print_to_console {
            println!(
                "📊 Tokens - Input: {}, Output: {}, Cache: {}",
                input, output, cache
            );
        }

        let event = MapReduceEvent::ClaudeTokenUsage {
            agent_id: self.agent_id.clone(),
            input_tokens: input,
            output_tokens: output,
            cache_tokens: cache,
        };

        if let Err(e) = self.event_logger.log(event).await {
            tracing::warn!("Failed to log Claude token usage event: {}", e);
        }
        Ok(())
    }

    async fn on_message(&self, content: &str, message_type: &str) -> Result<()> {
        if self.print_to_console && message_type == "text" {
            // Only print user-visible messages
            println!("{}", content);
        }

        let event = MapReduceEvent::ClaudeMessage {
            agent_id: self.agent_id.clone(),
            content: content.to_string(),
            message_type: message_type.to_string(),
        };

        if let Err(e) = self.event_logger.log(event).await {
            tracing::warn!("Failed to log Claude message event: {}", e);
        }
        Ok(())
    }

    async fn on_session_start(
        &self,
        session_id: &str,
        model: &str,
        tools: Vec<String>,
    ) -> Result<()> {
        if self.print_to_console {
            println!("🚀 Claude session started - Model: {}", model);
        }

        let event = MapReduceEvent::ClaudeSessionStarted {
            agent_id: self.agent_id.clone(),
            session_id: session_id.to_string(),
            model: model.to_string(),
            tools,
        };

        if let Err(e) = self.event_logger.log(event).await {
            tracing::warn!("Failed to log Claude session event: {}", e);
        }
        Ok(())
    }

    async fn on_raw_event(&self, event_type: &str, json: &Value) -> Result<()> {
        tracing::trace!("Claude raw event ({}): {}", event_type, json);
        Ok(())
    }

    async fn on_text_line(&self, line: &str, source: StreamSource) -> Result<()> {
        // Non-JSON lines are typically error messages or other output
        if source == StreamSource::Stderr {
            tracing::warn!("Claude stderr: {}", line);
            if self.print_to_console {
                eprintln!("{}", line);
            }
        } else {
            tracing::trace!("Claude text output: {}", line);
        }
        Ok(())
    }
}

/// Simple console-only handler for when event logging is not available
pub struct ConsoleClaudeHandler {
    agent_id: String,
}

impl ConsoleClaudeHandler {
    pub fn new(agent_id: String) -> Self {
        Self { agent_id }
    }
}

#[async_trait]
impl ClaudeStreamHandler for ConsoleClaudeHandler {
    async fn on_tool_invocation(
        &self,
        tool_name: &str,
        _tool_id: &str,
        _parameters: &Value,
    ) -> Result<()> {
        println!("[{}] 🔧 Tool invoked: {}", self.agent_id, tool_name);
        Ok(())
    }

    async fn on_token_usage(&self, input: u64, output: u64, cache: u64) -> Result<()> {
        println!(
            "[{}] 📊 Tokens - Input: {}, Output: {}, Cache: {}",
            self.agent_id, input, output, cache
        );
        Ok(())
    }

    async fn on_message(&self, content: &str, message_type: &str) -> Result<()> {
        if message_type == "text" {
            println!("{}", content);
        }
        Ok(())
    }

    async fn on_session_start(
        &self,
        _session_id: &str,
        model: &str,
        _tools: Vec<String>,
    ) -> Result<()> {
        println!(
            "[{}] 🚀 Claude session started - Model: {}",
            self.agent_id, model
        );
        Ok(())
    }

    async fn on_raw_event(&self, _event_type: &str, _json: &Value) -> Result<()> {
        // Silent for unknown events
        Ok(())
    }

    async fn on_text_line(&self, line: &str, source: StreamSource) -> Result<()> {
        if source == StreamSource::Stderr {
            eprintln!("{}", line);
        }
        Ok(())
    }
}
