//! Session watcher for monitoring Claude JSONL files

use anyhow::Result;
use chrono::{DateTime, Utc};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::models::{SessionEvent, SessionIndex};
use crate::cook::execution::events::EventLogger;

/// Watches Claude session JSONL files for changes and indexes them
pub struct SessionWatcher {
    claude_projects_path: PathBuf,
    event_logger: Arc<EventLogger>,
    index: Arc<RwLock<SessionIndex>>,
    processed_lines: Arc<RwLock<HashSet<String>>>,
}

impl SessionWatcher {
    /// Create a new session watcher
    pub fn new(event_logger: Arc<EventLogger>) -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let claude_projects_path = PathBuf::from(home).join(".claude/projects");

        if !claude_projects_path.exists() {
            std::fs::create_dir_all(&claude_projects_path)?;
        }

        Ok(Self {
            claude_projects_path,
            event_logger,
            index: Arc::new(RwLock::new(SessionIndex::new())),
            processed_lines: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Get the session index
    pub fn index(&self) -> Arc<RwLock<SessionIndex>> {
        Arc::clone(&self.index)
    }

    /// Start watching for session file changes
    pub async fn watch(&self) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher = Watcher::new(tx, Config::default())?;

        watcher.watch(&self.claude_projects_path, RecursiveMode::Recursive)?;

        info!(
            "Watching Claude sessions at: {:?}",
            self.claude_projects_path
        );

        // Initial scan of existing files
        self.scan_existing_files().await?;

        // Watch for new events
        loop {
            match rx.recv() {
                Ok(Ok(event)) => match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                                if let Err(e) = self.process_session_file(&path).await {
                                    error!("Failed to process session file {:?}: {}", path, e);
                                }
                            }
                        }
                    }
                    _ => {}
                },
                Ok(Err(e)) => {
                    error!("Watch error: {}", e);
                }
                Err(e) => {
                    error!("Channel error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Scan existing JSONL files on startup
    pub async fn scan_existing_files(&self) -> Result<()> {
        let mut count = 0;
        for entry in walkdir::WalkDir::new(&self.claude_projects_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Err(e) = self.process_session_file(entry.path()).await {
                    warn!("Failed to process existing file {:?}: {}", entry.path(), e);
                } else {
                    count += 1;
                }
            }
        }
        info!("Indexed {} existing session files", count);
        Ok(())
    }

    /// Extract session ID from file path
    fn extract_session_id(path: &Path) -> Result<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Cannot extract session ID from path: {:?}", path))
    }

    /// Process a session JSONL file incrementally
    pub async fn process_session_file(&self, path: &Path) -> Result<()> {
        let session_id = Self::extract_session_id(path)?;
        debug!("Processing session file: {:?} (ID: {})", path, session_id);

        let events = self.parse_jsonl_incremental(path).await?;

        if events.is_empty() {
            return Ok(());
        }

        let mut index = self.index.write().await;
        for event in events {
            index.add_event(&session_id, event).await?;
        }

        // TODO: Log analytics event when EventLogger supports it
        debug!("Indexed session: {}", session_id);

        Ok(())
    }

    /// Parse JSONL file incrementally, only processing new lines
    async fn parse_jsonl_incremental(&self, path: &Path) -> Result<Vec<SessionEvent>> {
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut events = Vec::new();
        let mut processed_lines = self.processed_lines.write().await;

        while let Some(line) = lines.next_line().await? {
            // Create a unique key for this line (file + content hash)
            let line_key = format!("{:?}:{}", path, &line);

            if processed_lines.contains(&line_key) {
                continue;
            }

            match serde_json::from_str::<ClaudeLogEntry>(&line) {
                Ok(entry) => {
                    if let Some(event) = Self::convert_to_session_event(entry) {
                        events.push(event);
                    }
                    processed_lines.insert(line_key);
                }
                Err(e) => {
                    debug!("Failed to parse JSONL line: {}", e);
                }
            }
        }

        Ok(events)
    }

    /// Convert Claude log entry to session event
    fn convert_to_session_event(entry: ClaudeLogEntry) -> Option<SessionEvent> {
        match entry.event_type.as_str() {
            "system" => Some(SessionEvent::System {
                timestamp: entry.timestamp,
                message: entry.content.unwrap_or_default(),
            }),
            "assistant" => Some(SessionEvent::Assistant {
                timestamp: entry.timestamp,
                content: entry.content.unwrap_or_default(),
                model: entry.model,
            }),
            "tool_use" => Some(SessionEvent::ToolUse {
                timestamp: entry.timestamp,
                tool_name: entry.tool_name.unwrap_or_default(),
                parameters: entry.parameters.unwrap_or(serde_json::Value::Null),
            }),
            "tool_result" => Some(SessionEvent::ToolResult {
                timestamp: entry.timestamp,
                tool_name: entry.tool_name.unwrap_or_default(),
                result: entry.result.unwrap_or(serde_json::Value::Null),
                duration_ms: entry.duration_ms,
            }),
            "error" => Some(SessionEvent::Error {
                timestamp: entry.timestamp,
                error_type: entry.error_type.unwrap_or_else(|| "unknown".to_string()),
                message: entry.content.unwrap_or_default(),
            }),
            _ => None,
        }
    }

    /// Extract session metadata from a file
    pub async fn extract_session_metadata(&self, path: &Path) -> Result<SessionMetadata> {
        let session_id = Self::extract_session_id(path)?;
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        let mut metadata = SessionMetadata {
            session_id,
            project_path: path
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string(),
            jsonl_path: path.to_string_lossy().to_string(),
            started_at: None,
            completed_at: None,
            model: None,
            total_events: 0,
            total_tokens: TokenCount::default(),
        };

        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;

        while let Some(line) = lines.next_line().await? {
            if let Ok(entry) = serde_json::from_str::<ClaudeLogEntry>(&line) {
                metadata.total_events += 1;

                if first_timestamp.is_none() {
                    first_timestamp = Some(entry.timestamp);
                }
                last_timestamp = Some(entry.timestamp);

                if entry.model.is_some() && metadata.model.is_none() {
                    metadata.model = entry.model;
                }

                if let Some(usage) = entry.token_usage {
                    metadata.total_tokens.input += usage.input;
                    metadata.total_tokens.output += usage.output;
                    metadata.total_tokens.cache += usage.cache;
                }
            }
        }

        metadata.started_at = first_timestamp;
        metadata.completed_at = last_timestamp;

        Ok(metadata)
    }
}

/// Claude log entry structure from JSONL files
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeLogEntry {
    #[serde(rename = "type")]
    event_type: String,
    timestamp: DateTime<Utc>,
    content: Option<String>,
    model: Option<String>,
    tool_name: Option<String>,
    parameters: Option<serde_json::Value>,
    result: Option<serde_json::Value>,
    duration_ms: Option<u64>,
    error_type: Option<String>,
    token_usage: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenUsage {
    input: u64,
    output: u64,
    cache: u64,
}

/// Session metadata extracted from JSONL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub project_path: String,
    pub jsonl_path: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub total_events: usize,
    pub total_tokens: TokenCount,
}

/// Token count summary
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenCount {
    pub input: u64,
    pub output: u64,
    pub cache: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_extract_session_id() {
        let path = Path::new("/home/user/.claude/projects/myproject/session-123.jsonl");
        let id = SessionWatcher::extract_session_id(&path).unwrap();
        assert_eq!(id, "session-123");
    }

    #[tokio::test]
    async fn test_convert_log_entry() {
        let entry = ClaudeLogEntry {
            event_type: "tool_use".to_string(),
            timestamp: Utc::now(),
            content: None,
            model: None,
            tool_name: Some("Bash".to_string()),
            parameters: Some(serde_json::json!({"command": "ls"})),
            result: None,
            duration_ms: None,
            error_type: None,
            token_usage: None,
        };

        let event = SessionWatcher::convert_to_session_event(entry);
        assert!(matches!(event, Some(SessionEvent::ToolUse { .. })));
    }
}
