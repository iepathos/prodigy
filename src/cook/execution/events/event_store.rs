//! Event storage and retrieval functionality

use super::{EventRecord, MapReduceEvent};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::{info, warn};
use uuid::Uuid;

/// Event filter for querying events
#[derive(Debug, Clone, Default)]
pub struct EventFilter {
    pub job_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_types: Vec<String>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub correlation_id: Option<String>,
    pub limit: Option<usize>,
}

/// Event index for quick lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventIndex {
    pub job_id: String,
    pub event_counts: HashMap<String, usize>,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
    pub file_offsets: Vec<FileOffset>,
    pub total_events: usize,
}

/// File offset information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOffset {
    pub file_path: PathBuf,
    pub byte_offset: u64,
    pub line_number: usize,
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

/// Event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub job_id: String,
    pub total_events: usize,
    pub event_counts: HashMap<String, usize>,
    pub success_count: usize,
    pub failure_count: usize,
    pub duration_ms: Option<i64>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

/// Event handler for replay functionality
pub type EventHandler = Box<dyn Fn(&EventRecord) -> Result<()> + Send + Sync>;

/// Event store trait for persistence and retrieval
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Append an event to the store
    async fn append(&self, event: MapReduceEvent) -> Result<()>;

    /// Query events with a filter
    async fn query(&self, filter: EventFilter) -> Result<Vec<EventRecord>>;

    /// Replay events for a job
    async fn replay(&self, job_id: &str, handler: EventHandler) -> Result<()>;

    /// Get aggregated statistics for a job
    async fn aggregate(&self, job_id: &str) -> Result<EventStats>;

    /// Create or update index for a job
    async fn index(&self, job_id: &str) -> Result<EventIndex>;
}

/// File-based event store implementation
#[allow(dead_code)]
pub struct FileEventStore {
    base_path: PathBuf,
}

#[allow(dead_code)]
impl FileEventStore {
    /// Create a new file event store
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Get the events directory for a job
    fn job_events_dir(&self, job_id: &str) -> PathBuf {
        self.base_path.join("events").join(job_id)
    }

    /// Find all event files for a job
    async fn find_event_files(&self, job_id: &str) -> Result<Vec<PathBuf>> {
        let dir = self.job_events_dir(job_id);

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        let mut entries = fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }

        // Sort files by name (which includes timestamp)
        files.sort();

        Ok(files)
    }

    /// Read events from a single file
    async fn read_events_from_file(&self, path: &Path) -> Result<Vec<EventRecord>> {
        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut events = Vec::new();

        while let Some(line) = lines.next_line().await? {
            match serde_json::from_str::<EventRecord>(&line) {
                Ok(event) => events.push(event),
                Err(e) => warn!("Failed to parse event line: {}", e),
            }
        }

        Ok(events)
    }

    /// Apply filter to an event
    fn matches_filter(&self, event: &EventRecord, filter: &EventFilter) -> bool {
        // Check job ID
        if let Some(ref job_id) = filter.job_id {
            if event.event.job_id() != job_id {
                return false;
            }
        }

        // Check agent ID
        if let Some(ref agent_id) = filter.agent_id {
            if event.event.agent_id() != Some(agent_id.as_str()) {
                return false;
            }
        }

        // Check event types
        if !filter.event_types.is_empty()
            && !filter
                .event_types
                .contains(&event.event.event_name().to_string())
        {
            return false;
        }

        // Check time range
        if let Some((start, end)) = filter.time_range {
            if event.timestamp < start || event.timestamp > end {
                return false;
            }
        }

        // Check correlation ID
        if let Some(ref correlation_id) = filter.correlation_id {
            if event.correlation_id != *correlation_id {
                return false;
            }
        }

        true
    }
}

// Pure helper functions for index operations

/// Update time range with a new event timestamp
fn update_time_range(
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
    event_time: DateTime<Utc>,
) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let new_start = match start {
        None => Some(event_time),
        Some(s) if event_time < s => Some(event_time),
        Some(s) => Some(s),
    };

    let new_end = match end {
        None => Some(event_time),
        Some(e) if event_time > e => Some(event_time),
        Some(e) => Some(e),
    };

    (new_start, new_end)
}

/// Increment event count for a given event type
fn increment_event_count(counts: &mut HashMap<String, usize>, event_name: String) {
    *counts.entry(event_name).or_insert(0) += 1;
}

/// Create a file offset record from event data
fn create_file_offset(
    file_path: PathBuf,
    byte_offset: u64,
    line_number: usize,
    event: &EventRecord,
) -> FileOffset {
    FileOffset {
        file_path,
        byte_offset,
        line_number,
        event_id: event.id,
        timestamp: event.timestamp,
    }
}

/// Process a single event line and update index state
fn process_event_line(
    line: &str,
    file_path: &Path,
    line_number: usize,
    byte_offset: u64,
    index: &mut EventIndex,
    time_range: &mut (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
) {
    if let Ok(event) = serde_json::from_str::<EventRecord>(line) {
        index.total_events += 1;

        let event_name = event.event.event_name().to_string();
        increment_event_count(&mut index.event_counts, event_name);

        let (start, end) = update_time_range(time_range.0, time_range.1, event.timestamp);
        *time_range = (start, end);

        index.file_offsets.push(create_file_offset(
            file_path.to_path_buf(),
            byte_offset,
            line_number,
            &event,
        ));
    }
}

/// Save index to file
async fn save_index(index: &EventIndex, index_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(index)?;
    fs::write(index_path, json).await?;
    Ok(())
}

/// Process all events in a single file and update the index
async fn process_event_file(
    file_path: &PathBuf,
    index: &mut EventIndex,
    time_range: &mut (Option<DateTime<Utc>>, Option<DateTime<Utc>>),
) -> Result<()> {
    let file = File::open(file_path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut line_number = 0;
    let mut byte_offset = 0u64;

    while let Some(line) = lines.next_line().await? {
        line_number += 1;
        process_event_line(
            &line,
            file_path,
            line_number,
            byte_offset,
            index,
            time_range,
        );
        byte_offset += line.len() as u64 + 1; // +1 for newline
    }

    Ok(())
}

#[async_trait]
impl EventStore for FileEventStore {
    async fn append(&self, _event: MapReduceEvent) -> Result<()> {
        // This would typically be handled by the EventLogger
        // This method is here for the trait interface
        Err(anyhow::anyhow!("Use EventLogger for appending events"))
    }

    async fn query(&self, filter: EventFilter) -> Result<Vec<EventRecord>> {
        let job_id = filter
            .job_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Job ID required for query"))?;

        let files = self.find_event_files(job_id).await?;
        let mut all_events = Vec::new();

        for file in files {
            let events = self.read_events_from_file(&file).await?;
            for event in events {
                if self.matches_filter(&event, &filter) {
                    all_events.push(event);

                    // Apply limit if specified
                    if let Some(limit) = filter.limit {
                        if all_events.len() >= limit {
                            return Ok(all_events);
                        }
                    }
                }
            }
        }

        Ok(all_events)
    }

    async fn replay(&self, job_id: &str, handler: EventHandler) -> Result<()> {
        let files = self.find_event_files(job_id).await?;

        for file in files {
            let events = self.read_events_from_file(&file).await?;
            for event in events {
                handler(&event)?;
            }
        }

        Ok(())
    }

    async fn aggregate(&self, job_id: &str) -> Result<EventStats> {
        let files = self.find_event_files(job_id).await?;
        let mut stats = EventStats {
            job_id: job_id.to_string(),
            total_events: 0,
            event_counts: HashMap::new(),
            success_count: 0,
            failure_count: 0,
            duration_ms: None,
            time_range: None,
        };

        let mut start_time: Option<DateTime<Utc>> = None;
        let mut end_time: Option<DateTime<Utc>> = None;

        for file in files {
            let events = self.read_events_from_file(&file).await?;

            for event in events {
                stats.total_events += 1;

                // Count by event type
                let event_name = event.event.event_name().to_string();
                *stats.event_counts.entry(event_name).or_insert(0) += 1;

                // Track time range
                if start_time.is_none() || event.timestamp < start_time.unwrap() {
                    start_time = Some(event.timestamp);
                }
                if end_time.is_none() || event.timestamp > end_time.unwrap() {
                    end_time = Some(event.timestamp);
                }

                // Count successes and failures
                match &event.event {
                    MapReduceEvent::AgentCompleted { .. } => stats.success_count += 1,
                    MapReduceEvent::AgentFailed { .. } => stats.failure_count += 1,
                    _ => {}
                }
            }
        }

        // Calculate duration
        if let (Some(start), Some(end)) = (start_time, end_time) {
            stats.duration_ms = Some((end - start).num_milliseconds());
            stats.time_range = Some((start, end));
        }

        Ok(stats)
    }

    async fn index(&self, job_id: &str) -> Result<EventIndex> {
        let files = self.find_event_files(job_id).await?;
        let mut index = EventIndex {
            job_id: job_id.to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 0,
        };

        let mut time_range = (None, None);

        for file_path in files {
            process_event_file(&file_path, &mut index, &mut time_range).await?;
        }

        if let (Some(start), Some(end)) = time_range {
            index.time_range = (start, end);
        }

        let index_path = self.job_events_dir(job_id).join("index.json");
        save_index(&index, &index_path).await?;

        info!(
            "Created index for job {} with {} events",
            job_id, index.total_events
        );

        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;
    use tempfile::TempDir;

    #[tokio::test]
    #[ignore] // This test requires proper EventRecord serialization setup
    async fn test_event_store_query() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());

        // Create test events directory using the correct structure
        let job_id = "test-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create test event
        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: "test-correlation".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: job_id.to_string(),
                config: MapReduceConfig {
                    agent_timeout_secs: None,
                    continue_on_failure: false,
                    batch_size: None,
                    enable_checkpoints: true,
                    input: "test.json".to_string(),
                    json_path: "$.items".to_string(),
                    max_parallel: 5,
                    max_items: None,
                    offset: None,
                },
                total_items: 10,
                timestamp: Utc::now(),
            },
            metadata: HashMap::new(),
        };

        // Write event to a JSONL file
        let event_file = events_dir.join("test.jsonl");
        let json = serde_json::to_string(&event).unwrap();
        fs::write(&event_file, &json).await.unwrap();

        // Verify the file was created and read it back
        assert!(event_file.exists(), "Event file should exist");

        // Test that we can deserialize what we wrote
        let file_content = fs::read_to_string(&event_file).await.unwrap();
        let parsed: EventRecord = serde_json::from_str(&file_content)
            .expect("Should be able to parse the event we just wrote");
        assert_eq!(parsed.event.job_id(), job_id);

        // Verify we can find the event files
        let files = store.find_event_files(job_id).await.unwrap();
        assert_eq!(files.len(), 1, "Should find 1 event file");

        // Query events
        let filter = EventFilter {
            job_id: Some(job_id.to_string()),
            ..Default::default()
        };

        let results = store.query(filter).await.unwrap();
        assert_eq!(results.len(), 1, "Should find 1 event");
        assert_eq!(results[0].event.job_id(), job_id);
    }

    #[tokio::test]
    async fn test_index_creates_index_for_job_with_events() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path.clone());

        // Create test data
        let job_id = "test-job-index";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create several test events in a JSONL file
        let event_file = events_dir.join("events-001.jsonl");
        let timestamp = Utc::now();

        // Create multiple events to test aggregation
        let events = vec![
            EventRecord {
                id: Uuid::new_v4(),
                timestamp,
                correlation_id: "corr-1".to_string(),
                event: MapReduceEvent::JobStarted {
                    job_id: job_id.to_string(),
                    config: MapReduceConfig {
                        agent_timeout_secs: None,
                        continue_on_failure: false,
                        batch_size: None,
                        enable_checkpoints: true,
                        input: "test.json".to_string(),
                        json_path: "$.items".to_string(),
                        max_parallel: 5,
                        max_items: None,
                        offset: None,
                    },
                    total_items: 10,
                    timestamp,
                },
                metadata: HashMap::new(),
            },
            EventRecord {
                id: Uuid::new_v4(),
                timestamp,
                correlation_id: "corr-2".to_string(),
                event: MapReduceEvent::AgentStarted {
                    job_id: job_id.to_string(),
                    agent_id: "agent-1".to_string(),
                    item_id: "item-1".to_string(),
                    worktree: "worktree-1".to_string(),
                    attempt: 1,
                },
                metadata: HashMap::new(),
            },
            EventRecord {
                id: Uuid::new_v4(),
                timestamp,
                correlation_id: "corr-3".to_string(),
                event: MapReduceEvent::AgentCompleted {
                    job_id: job_id.to_string(),
                    agent_id: "agent-1".to_string(),
                    duration: chrono::Duration::seconds(30),
                    commits: vec!["abc123".to_string()],
                },
                metadata: HashMap::new(),
            },
        ];

        // Write events to file
        let mut file_content = String::new();
        for event in &events {
            file_content.push_str(&serde_json::to_string(event).unwrap());
            file_content.push('\n');
        }
        fs::write(&event_file, &file_content).await.unwrap();

        // Execute
        let result = store.index(job_id).await;

        // Assert
        assert!(result.is_ok(), "Index operation should succeed");
        let index = result.unwrap();

        // Verify index structure
        assert_eq!(index.job_id, job_id, "Job ID should match");
        assert_eq!(index.total_events, 3, "Should count all events");

        // Verify event counts by type (event names are in snake_case)
        assert!(
            index.event_counts.contains_key("job_started"),
            "Should have job_started count"
        );
        assert!(
            index.event_counts.contains_key("agent_started"),
            "Should have agent_started count"
        );
        assert!(
            index.event_counts.contains_key("agent_completed"),
            "Should have agent_completed count"
        );

        // Verify index file was created
        let index_path = events_dir.join("index.json");
        assert!(
            index_path.exists(),
            "Index file should exist on disk"
        );

        // Verify index file can be deserialized
        let index_content = fs::read_to_string(&index_path).await.unwrap();
        let parsed_index: EventIndex =
            serde_json::from_str(&index_content).expect("Index should be valid JSON");
        assert_eq!(
            parsed_index.job_id, job_id,
            "Parsed index should match"
        );
        assert_eq!(
            parsed_index.total_events, 3,
            "Parsed index should have correct count"
        );
    }
}
