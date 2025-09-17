//! Stream processor trait and implementations

use super::types::StreamSource;
use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use tokio::sync::mpsc;

/// Trait for processing stream output line by line
#[async_trait]
pub trait StreamProcessor: Send + Sync {
    /// Process a line from the stream
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()>;

    /// Handle stream completion
    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()>;

    /// Handle stream errors
    async fn on_error(&self, error: &anyhow::Error) -> Result<()>;
}

/// Stream processor for JSON lines
pub struct JsonLineProcessor {
    event_sender: mpsc::Sender<Value>,
    emit_events: bool,
}

impl JsonLineProcessor {
    /// Create a new JSON line processor
    pub fn new(event_sender: mpsc::Sender<Value>, emit_events: bool) -> Self {
        Self {
            event_sender,
            emit_events,
        }
    }
}

#[async_trait]
impl StreamProcessor for JsonLineProcessor {
    async fn process_line(&self, line: &str, _source: StreamSource) -> Result<()> {
        if !self.emit_events {
            return Ok(());
        }

        // Try to parse as JSON
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            // Send the parsed JSON event
            self.event_sender
                .send(json)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send JSON event: {}", e))?;
        }
        Ok(())
    }

    async fn on_complete(&self, _exit_code: Option<i32>) -> Result<()> {
        // Nothing special to do on completion
        Ok(())
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        tracing::warn!("JSON processor encountered error: {}", error);
        Ok(())
    }
}

/// Stream processor for pattern matching
pub struct PatternMatchProcessor {
    patterns: Vec<Regex>,
    event_sender: mpsc::Sender<PatternMatch>,
}

/// Pattern match event
#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub line: String,
    pub pattern: String,
    pub source: StreamSource,
    pub captures: Vec<String>,
}

impl PatternMatchProcessor {
    /// Create a new pattern match processor
    pub fn new(patterns: Vec<Regex>, event_sender: mpsc::Sender<PatternMatch>) -> Self {
        Self {
            patterns,
            event_sender,
        }
    }
}

#[async_trait]
impl StreamProcessor for PatternMatchProcessor {
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
        for pattern in &self.patterns {
            if let Some(captures) = pattern.captures(line) {
                let match_event = PatternMatch {
                    line: line.to_string(),
                    pattern: pattern.to_string(),
                    source,
                    captures: captures
                        .iter()
                        .skip(1) // Skip the full match
                        .filter_map(|m| m.map(|s| s.as_str().to_string()))
                        .collect(),
                };

                self.event_sender
                    .send(match_event)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to send pattern match event: {}", e))?;
            }
        }
        Ok(())
    }

    async fn on_complete(&self, _exit_code: Option<i32>) -> Result<()> {
        Ok(())
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        tracing::warn!("Pattern processor encountered error: {}", error);
        Ok(())
    }
}

/// Simple logging processor for debugging
pub struct LoggingProcessor {
    prefix: String,
}

impl LoggingProcessor {
    /// Create a new logging processor
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }
}

#[async_trait]
impl StreamProcessor for LoggingProcessor {
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
        tracing::debug!("{} [{:?}]: {}", self.prefix, source, line);
        Ok(())
    }

    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()> {
        tracing::debug!("{} completed with exit code: {:?}", self.prefix, exit_code);
        Ok(())
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        tracing::error!("{} error: {}", self.prefix, error);
        Ok(())
    }
}
