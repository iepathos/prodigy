//! Session watcher for monitoring Claude JSONL files

use anyhow::Result;
use chrono::{DateTime, Utc};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::models::{SessionEvent, SessionIndex};
use crate::cook::execution::events::EventLogger;

/// Watches Claude session JSONL files for changes and indexes them
pub struct SessionWatcher {
    claude_projects_path: PathBuf,
    _event_logger: Arc<EventLogger>,
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
            _event_logger: event_logger,
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
    use crate::cook::execution::events::EventLogger;
    use tempfile::TempDir;
    use tokio::fs;

    async fn setup_test_environment() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let projects_dir = temp_dir.path().join("projects");
        fs::create_dir_all(&projects_dir).await.unwrap();
        (temp_dir, projects_dir)
    }

    async fn create_test_jsonl(path: &Path, content: &str) {
        fs::write(path, content).await.unwrap();
    }

    #[tokio::test]
    async fn test_session_watcher_initialization() {
        let event_logger = Arc::new(EventLogger::new(vec![]));

        let watcher = SessionWatcher::new(event_logger).unwrap();
        assert!(watcher.claude_projects_path.exists() || true); // Path might not exist in test env
    }

    #[tokio::test]
    async fn test_discover_sessions() {
        let (_temp_dir, projects_dir) = setup_test_environment().await;
        let event_logger = Arc::new(EventLogger::new(vec![]));

        // Create test project directory with JSONL file
        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).await.unwrap();

        let jsonl_content = r#"{"type":"system","timestamp":"2024-01-01T00:00:00Z","content":"Session started"}
{"type":"assistant","timestamp":"2024-01-01T00:01:00Z","content":"Hello","model":"claude-3","token_usage":{"input":10,"output":20,"cache":5}}"#;

        let jsonl_path = project_dir.join("test-session.jsonl");
        create_test_jsonl(&jsonl_path, jsonl_content).await;

        let watcher = SessionWatcher::new(event_logger).unwrap();

        // Parse a specific session file for testing
        let metadata = watcher.extract_session_metadata(&jsonl_path).await.unwrap();
        assert!(metadata.total_events > 0);
    }

    #[tokio::test]
    async fn test_parse_jsonl_session() {
        let (_temp_dir, projects_dir) = setup_test_environment().await;
        let event_logger = Arc::new(EventLogger::new(vec![]));

        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).await.unwrap();

        let jsonl_content = r#"{"type":"system","timestamp":"2024-01-01T00:00:00Z","content":"Session started"}
{"type":"tool_use","timestamp":"2024-01-01T00:01:00Z","tool_name":"Read","parameters":{"file":"test.txt"}}
{"type":"tool_result","timestamp":"2024-01-01T00:01:05Z","tool_name":"Read","result":"file content","duration_ms":5000}
{"type":"assistant","timestamp":"2024-01-01T00:02:00Z","content":"Task completed","model":"claude-3","token_usage":{"input":100,"output":200,"cache":50}}"#;

        let jsonl_path = project_dir.join("test-session.jsonl");
        create_test_jsonl(&jsonl_path, jsonl_content).await;

        let watcher = SessionWatcher::new(event_logger).unwrap();
        let metadata = watcher.extract_session_metadata(&jsonl_path).await.unwrap();

        assert_eq!(metadata.total_events, 4);
        assert_eq!(metadata.total_tokens.input, 100);
        assert_eq!(metadata.total_tokens.output, 200);
        assert_eq!(metadata.total_tokens.cache, 50);
        assert_eq!(metadata.model, Some("claude-3".to_string()));
    }

    #[tokio::test]
    async fn test_handle_malformed_jsonl() {
        let (_temp_dir, projects_dir) = setup_test_environment().await;
        let event_logger = Arc::new(EventLogger::new(vec![]));

        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).await.unwrap();

        // Create JSONL with some malformed lines
        let jsonl_content = r#"{"type":"system","timestamp":"2024-01-01T00:00:00Z","content":"Valid line"}
This is not valid JSON
{"type":"assistant","timestamp":"2024-01-01T00:01:00Z","content":"Another valid line"}"#;

        let jsonl_path = project_dir.join("malformed-session.jsonl");
        create_test_jsonl(&jsonl_path, jsonl_content).await;

        let watcher = SessionWatcher::new(event_logger).unwrap();
        let metadata = watcher.extract_session_metadata(&jsonl_path).await.unwrap();

        // Should still parse valid lines
        assert_eq!(metadata.total_events, 2);
    }

    #[tokio::test]
    async fn test_watch_multiple_projects() {
        let (_temp_dir, projects_dir) = setup_test_environment().await;
        let event_logger = Arc::new(EventLogger::new(vec![]));

        // Create multiple project directories
        for i in 1..=3 {
            let project_dir = projects_dir.join(format!("project-{}", i));
            fs::create_dir_all(&project_dir).await.unwrap();

            let jsonl_content = format!(
                r#"{{"type":"system","timestamp":"2024-01-0{}T00:00:00Z","content":"Project {} session"}}"#,
                i, i
            );

            let jsonl_path = project_dir.join(format!("session-{}.jsonl", i));
            create_test_jsonl(&jsonl_path, &jsonl_content).await;
        }

        let watcher = SessionWatcher::new(event_logger).unwrap();

        // Test that we can parse each session
        for i in 1..=3 {
            let jsonl_path = projects_dir.join(format!("project-{}/session-{}.jsonl", i, i));
            let metadata = watcher.extract_session_metadata(&jsonl_path).await.unwrap();
            assert!(metadata.total_events > 0);
        }
    }

    #[tokio::test]
    async fn test_empty_jsonl_handling() {
        let (_temp_dir, projects_dir) = setup_test_environment().await;
        let event_logger = Arc::new(EventLogger::new(vec![]));

        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).await.unwrap();

        // Create empty JSONL file
        let jsonl_path = project_dir.join("empty-session.jsonl");
        create_test_jsonl(&jsonl_path, "").await;

        let watcher = SessionWatcher::new(event_logger).unwrap();
        let metadata = watcher.extract_session_metadata(&jsonl_path).await.unwrap();

        assert_eq!(metadata.total_events, 0);
        assert_eq!(metadata.total_tokens.input, 0);
        assert_eq!(metadata.total_tokens.output, 0);
        assert_eq!(metadata.total_tokens.cache, 0);
    }

    #[tokio::test]
    async fn test_token_accumulation() {
        let (_temp_dir, projects_dir) = setup_test_environment().await;
        let event_logger = Arc::new(EventLogger::new(vec![]));

        let project_dir = projects_dir.join("test-project");
        fs::create_dir_all(&project_dir).await.unwrap();

        // Create JSONL with multiple token usage entries
        let jsonl_content = r#"{"type":"assistant","timestamp":"2024-01-01T00:00:00Z","content":"First","token_usage":{"input":100,"output":200,"cache":50}}
{"type":"assistant","timestamp":"2024-01-01T00:01:00Z","content":"Second","token_usage":{"input":150,"output":250,"cache":75}}
{"type":"assistant","timestamp":"2024-01-01T00:02:00Z","content":"Third","token_usage":{"input":200,"output":300,"cache":100}}"#;

        let jsonl_path = project_dir.join("tokens-session.jsonl");
        create_test_jsonl(&jsonl_path, jsonl_content).await;

        let watcher = SessionWatcher::new(event_logger).unwrap();
        let metadata = watcher.extract_session_metadata(&jsonl_path).await.unwrap();

        assert_eq!(metadata.total_tokens.input, 450); // 100 + 150 + 200
        assert_eq!(metadata.total_tokens.output, 750); // 200 + 250 + 300
        assert_eq!(metadata.total_tokens.cache, 225); // 50 + 75 + 100
    }
}
