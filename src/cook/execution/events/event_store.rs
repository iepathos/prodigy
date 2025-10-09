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
                    json_log_location: None,
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
        assert!(index_path.exists(), "Index file should exist on disk");

        // Verify index file can be deserialized
        let index_content = fs::read_to_string(&index_path).await.unwrap();
        let parsed_index: EventIndex =
            serde_json::from_str(&index_content).expect("Index should be valid JSON");
        assert_eq!(parsed_index.job_id, job_id, "Parsed index should match");
        assert_eq!(
            parsed_index.total_events, 3,
            "Parsed index should have correct count"
        );
    }

    #[tokio::test]
    async fn test_index_with_no_event_files() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path.clone());

        // Create job directory but no event files
        let job_id = "empty-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create a non-.jsonl file to ensure filtering works
        let non_event_file = events_dir.join("readme.txt");
        fs::write(&non_event_file, "This is not an event file")
            .await
            .unwrap();

        // Execute
        let result = store.index(job_id).await;

        // Assert
        assert!(result.is_ok(), "Index should succeed even with no events");
        let index = result.unwrap();

        assert_eq!(index.job_id, job_id);
        assert_eq!(index.total_events, 0, "Should have zero events");
        assert!(
            index.event_counts.is_empty(),
            "Event counts should be empty"
        );

        // Verify index file was created
        let index_path = events_dir.join("index.json");
        assert!(index_path.exists(), "Index file should be created");
    }

    #[tokio::test]
    async fn test_index_with_nonexistent_job() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path);

        // Execute with nonexistent job
        let job_id = "nonexistent-job";
        let result = store.index(job_id).await;

        // Assert - should return error when directory doesn't exist
        // (The save_index operation will fail when trying to write to nonexistent dir)
        assert!(
            result.is_err(),
            "Index should fail when job directory doesn't exist"
        );
    }

    #[tokio::test]
    async fn test_index_aggregates_multiple_event_files() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path.clone());

        let job_id = "multi-file-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create multiple event files with different timestamps
        let timestamp1 = Utc::now() - chrono::Duration::hours(2);
        let timestamp2 = Utc::now() - chrono::Duration::hours(1);
        let timestamp3 = Utc::now();

        // File 1: Earlier events
        let event_file1 = events_dir.join("events-001.jsonl");
        let events1 = vec![
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: timestamp1,
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
                    timestamp: timestamp1,
                },
                metadata: HashMap::new(),
            },
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: timestamp1,
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
        ];

        let mut content1 = String::new();
        for event in &events1 {
            content1.push_str(&serde_json::to_string(event).unwrap());
            content1.push('\n');
        }
        fs::write(&event_file1, &content1).await.unwrap();

        // File 2: Middle events
        let event_file2 = events_dir.join("events-002.jsonl");
        let events2 = vec![
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: timestamp2,
                correlation_id: "corr-3".to_string(),
                event: MapReduceEvent::AgentCompleted {
                    job_id: job_id.to_string(),
                    agent_id: "agent-1".to_string(),
                    duration: chrono::Duration::seconds(30),
                    commits: vec!["abc123".to_string()],
                    json_log_location: None,
                },
                metadata: HashMap::new(),
            },
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: timestamp2,
                correlation_id: "corr-4".to_string(),
                event: MapReduceEvent::AgentStarted {
                    job_id: job_id.to_string(),
                    agent_id: "agent-2".to_string(),
                    item_id: "item-2".to_string(),
                    worktree: "worktree-2".to_string(),
                    attempt: 1,
                },
                metadata: HashMap::new(),
            },
        ];

        let mut content2 = String::new();
        for event in &events2 {
            content2.push_str(&serde_json::to_string(event).unwrap());
            content2.push('\n');
        }
        fs::write(&event_file2, &content2).await.unwrap();

        // File 3: Latest events
        let event_file3 = events_dir.join("events-003.jsonl");
        let event3 = EventRecord {
            id: Uuid::new_v4(),
            timestamp: timestamp3,
            correlation_id: "corr-5".to_string(),
            event: MapReduceEvent::JobCompleted {
                job_id: job_id.to_string(),
                duration: chrono::Duration::hours(2),
                success_count: 10,
                failure_count: 0,
            },
            metadata: HashMap::new(),
        };

        let content3 = format!("{}\n", serde_json::to_string(&event3).unwrap());
        fs::write(&event_file3, &content3).await.unwrap();

        // Execute
        let result = store.index(job_id).await;

        // Assert
        assert!(result.is_ok(), "Index should succeed with multiple files");
        let index = result.unwrap();

        // Verify aggregation
        assert_eq!(index.job_id, job_id);
        assert_eq!(
            index.total_events, 5,
            "Should aggregate all events from all files"
        );

        // Verify event type counts
        assert_eq!(index.event_counts.get("job_started"), Some(&1));
        assert_eq!(index.event_counts.get("agent_started"), Some(&2));
        assert_eq!(index.event_counts.get("agent_completed"), Some(&1));
        assert_eq!(index.event_counts.get("job_completed"), Some(&1));

        // Verify time range spans all files
        let (start, end) = index.time_range;
        assert!(
            start <= timestamp1,
            "Time range start should be at or before earliest event"
        );
        assert!(
            end >= timestamp3,
            "Time range end should be at or after latest event"
        );

        // Verify file offsets were recorded
        assert_eq!(
            index.file_offsets.len(),
            5,
            "Should have file offsets for all events"
        );
    }

    #[tokio::test]
    async fn test_index_calculates_correct_time_range() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path.clone());

        let job_id = "time-range-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create events with specific, known timestamps
        let earliest_time = Utc::now() - chrono::Duration::days(1);
        let middle_time = Utc::now() - chrono::Duration::hours(12);
        let latest_time = Utc::now();

        let event_file = events_dir.join("events-001.jsonl");
        let events = vec![
            // Add events in non-chronological order to verify sorting logic
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: middle_time,
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
                timestamp: earliest_time,
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
                    timestamp: earliest_time,
                },
                metadata: HashMap::new(),
            },
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: latest_time,
                correlation_id: "corr-3".to_string(),
                event: MapReduceEvent::JobCompleted {
                    job_id: job_id.to_string(),
                    duration: chrono::Duration::days(1),
                    success_count: 10,
                    failure_count: 0,
                },
                metadata: HashMap::new(),
            },
        ];

        let mut file_content = String::new();
        for event in &events {
            file_content.push_str(&serde_json::to_string(event).unwrap());
            file_content.push('\n');
        }
        fs::write(&event_file, &file_content).await.unwrap();

        // Execute
        let result = store.index(job_id).await;

        // Assert
        assert!(result.is_ok(), "Index should succeed");
        let index = result.unwrap();

        let (start, end) = index.time_range;

        // Verify time range exactly matches earliest and latest timestamps
        assert_eq!(
            start, earliest_time,
            "Time range start should match earliest event timestamp"
        );
        assert_eq!(
            end, latest_time,
            "Time range end should match latest event timestamp"
        );

        // Verify the duration between start and end
        let duration = end - start;
        assert!(
            duration >= chrono::Duration::hours(23),
            "Duration should be approximately 24 hours"
        );
    }

    #[tokio::test]
    async fn test_index_handles_malformed_json() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path.clone());

        let job_id = "malformed-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create file with mix of valid and invalid JSON
        let event_file = events_dir.join("events-001.jsonl");

        let valid_event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
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
                timestamp: Utc::now(),
            },
            metadata: HashMap::new(),
        };

        // Write mix of valid and invalid lines
        let mut content = String::new();
        content.push_str(&serde_json::to_string(&valid_event).unwrap());
        content.push('\n');
        content.push_str("{ invalid json }\n"); // Malformed JSON
        content.push_str(&serde_json::to_string(&valid_event).unwrap());
        content.push('\n');
        content.push_str("not even close to json\n"); // Not JSON at all
        content.push_str(&serde_json::to_string(&valid_event).unwrap());
        content.push('\n');

        fs::write(&event_file, &content).await.unwrap();

        // Execute
        let result = store.index(job_id).await;

        // Assert - should succeed but skip invalid lines
        assert!(
            result.is_ok(),
            "Index should handle malformed JSON gracefully"
        );
        let index = result.unwrap();

        // Should only count the valid events (3 valid lines, 2 invalid)
        assert_eq!(
            index.total_events, 3,
            "Should skip malformed lines and count only valid events"
        );
        assert_eq!(
            index.event_counts.get("job_started"),
            Some(&3),
            "Should have counted all valid JobStarted events"
        );
    }

    #[tokio::test]
    async fn test_index_persists_and_deserializes_correctly() {
        // Setup
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        let store = FileEventStore::new(base_path.clone());

        let job_id = "persistence-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create test events
        let event_file = events_dir.join("events-001.jsonl");
        let timestamp = Utc::now();

        let event1 = EventRecord {
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
        };

        let event2 = EventRecord {
            id: Uuid::new_v4(),
            timestamp,
            correlation_id: "corr-2".to_string(),
            event: MapReduceEvent::AgentCompleted {
                job_id: job_id.to_string(),
                agent_id: "agent-1".to_string(),
                duration: chrono::Duration::seconds(30),
                commits: vec!["abc123".to_string(), "def456".to_string()],
                json_log_location: None,
            },
            metadata: HashMap::new(),
        };

        let mut content = String::new();
        content.push_str(&serde_json::to_string(&event1).unwrap());
        content.push('\n');
        content.push_str(&serde_json::to_string(&event2).unwrap());
        content.push('\n');
        fs::write(&event_file, &content).await.unwrap();

        // Execute - create index
        let result = store.index(job_id).await;
        assert!(result.is_ok(), "Index creation should succeed");
        let original_index = result.unwrap();

        // Verify index file exists
        let index_path = events_dir.join("index.json");
        assert!(index_path.exists(), "Index file should exist on disk");

        // Read and parse the index file
        let index_content = fs::read_to_string(&index_path).await.unwrap();
        let parsed_index: EventIndex =
            serde_json::from_str(&index_content).expect("Index file should contain valid JSON");

        // Verify all fields match
        assert_eq!(parsed_index.job_id, original_index.job_id);
        assert_eq!(parsed_index.total_events, original_index.total_events);
        assert_eq!(
            parsed_index.event_counts, original_index.event_counts,
            "Event counts should match"
        );
        assert_eq!(
            parsed_index.time_range, original_index.time_range,
            "Time range should match"
        );
        assert_eq!(
            parsed_index.file_offsets.len(),
            original_index.file_offsets.len(),
            "File offsets count should match"
        );

        // Verify JSON structure has expected fields
        let json_value: serde_json::Value =
            serde_json::from_str(&index_content).expect("Should parse as JSON value");
        assert!(
            json_value.get("job_id").is_some(),
            "Should have job_id field"
        );
        assert!(
            json_value.get("event_counts").is_some(),
            "Should have event_counts field"
        );
        assert!(
            json_value.get("time_range").is_some(),
            "Should have time_range field"
        );
        assert!(
            json_value.get("file_offsets").is_some(),
            "Should have file_offsets field"
        );
        assert!(
            json_value.get("total_events").is_some(),
            "Should have total_events field"
        );

        // Verify the JSON is pretty-printed (has newlines)
        assert!(
            index_content.contains('\n'),
            "Index should be pretty-printed"
        );
    }

    // Unit tests for helper functions

    #[test]
    fn test_update_time_range_first_event() {
        let event_time = Utc::now();
        let (start, end) = update_time_range(None, None, event_time);
        assert_eq!(start, Some(event_time));
        assert_eq!(end, Some(event_time));
    }

    #[test]
    fn test_update_time_range_earlier_than_start() {
        let current_start = Utc::now();
        let current_end = current_start + chrono::Duration::hours(1);
        let earlier_time = current_start - chrono::Duration::hours(1);

        let (start, end) = update_time_range(Some(current_start), Some(current_end), earlier_time);
        assert_eq!(start, Some(earlier_time));
        assert_eq!(end, Some(current_end));
    }

    #[test]
    fn test_update_time_range_later_than_end() {
        let current_start = Utc::now();
        let current_end = current_start + chrono::Duration::hours(1);
        let later_time = current_end + chrono::Duration::hours(1);

        let (start, end) = update_time_range(Some(current_start), Some(current_end), later_time);
        assert_eq!(start, Some(current_start));
        assert_eq!(end, Some(later_time));
    }

    #[test]
    fn test_update_time_range_within_range() {
        let current_start = Utc::now();
        let current_end = current_start + chrono::Duration::hours(2);
        let within_time = current_start + chrono::Duration::hours(1);

        let (start, end) = update_time_range(Some(current_start), Some(current_end), within_time);
        assert_eq!(start, Some(current_start));
        assert_eq!(end, Some(current_end));
    }

    #[test]
    fn test_increment_event_count_first_occurrence() {
        let mut counts = HashMap::new();
        increment_event_count(&mut counts, "job_started".to_string());
        assert_eq!(counts.get("job_started"), Some(&1));
    }

    #[test]
    fn test_increment_event_count_subsequent_occurrence() {
        let mut counts = HashMap::new();
        counts.insert("agent_completed".to_string(), 3);
        increment_event_count(&mut counts, "agent_completed".to_string());
        assert_eq!(counts.get("agent_completed"), Some(&4));
    }

    #[test]
    fn test_create_file_offset() {
        let timestamp = Utc::now();
        let event_id = Uuid::new_v4();
        let event = EventRecord {
            id: event_id,
            timestamp,
            correlation_id: "test-corr".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: "test-job".to_string(),
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
        };

        let file_path = PathBuf::from("/path/to/events.jsonl");
        let offset = create_file_offset(file_path.clone(), 1024, 42, &event);

        assert_eq!(offset.file_path, file_path);
        assert_eq!(offset.byte_offset, 1024);
        assert_eq!(offset.line_number, 42);
        assert_eq!(offset.event_id, event_id);
        assert_eq!(offset.timestamp, timestamp);
    }

    #[test]
    fn test_process_event_line_valid_json() {
        let timestamp = Utc::now();
        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp,
            correlation_id: "test-corr".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: "test-job".to_string(),
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
        };

        let line = serde_json::to_string(&event).unwrap();
        let mut index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 0,
        };
        let mut time_range = (None, None);

        process_event_line(
            &line,
            Path::new("/test.jsonl"),
            1,
            0,
            &mut index,
            &mut time_range,
        );

        assert_eq!(index.total_events, 1);
        assert_eq!(index.event_counts.get("job_started"), Some(&1));
        assert_eq!(index.file_offsets.len(), 1);
        assert!(time_range.0.is_some());
        assert!(time_range.1.is_some());
    }

    #[test]
    fn test_process_event_line_invalid_json() {
        let mut index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 0,
        };
        let mut time_range = (None, None);

        process_event_line(
            "invalid json",
            Path::new("/test.jsonl"),
            1,
            0,
            &mut index,
            &mut time_range,
        );

        assert_eq!(index.total_events, 0);
        assert!(index.event_counts.is_empty());
        assert!(index.file_offsets.is_empty());
        assert!(time_range.0.is_none());
        assert!(time_range.1.is_none());
    }

    #[tokio::test]
    async fn test_save_index_success() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.json");

        let index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 5,
        };

        let result = save_index(&index, &index_path).await;
        assert!(result.is_ok());
        assert!(index_path.exists());
    }

    #[tokio::test]
    async fn test_save_index_invalid_path() {
        let index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 5,
        };

        let result = save_index(&index, Path::new("/nonexistent/dir/index.json")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_event_file_multiple_events() {
        let temp_dir = TempDir::new().unwrap();
        let event_file = temp_dir.path().join("events.jsonl");

        let timestamp = Utc::now();
        let event1 = EventRecord {
            id: Uuid::new_v4(),
            timestamp,
            correlation_id: "corr-1".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: "test-job".to_string(),
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
        };

        let event2 = EventRecord {
            id: Uuid::new_v4(),
            timestamp,
            correlation_id: "corr-2".to_string(),
            event: MapReduceEvent::AgentCompleted {
                job_id: "test-job".to_string(),
                agent_id: "agent-1".to_string(),
                duration: chrono::Duration::seconds(30),
                commits: vec!["abc123".to_string()],
                json_log_location: None,
            },
            metadata: HashMap::new(),
        };

        let mut content = String::new();
        content.push_str(&serde_json::to_string(&event1).unwrap());
        content.push('\n');
        content.push_str(&serde_json::to_string(&event2).unwrap());
        content.push('\n');
        fs::write(&event_file, &content).await.unwrap();

        let mut index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 0,
        };
        let mut time_range = (None, None);

        let result = process_event_file(&event_file, &mut index, &mut time_range).await;
        assert!(result.is_ok());
        assert_eq!(index.total_events, 2);
    }

    #[tokio::test]
    async fn test_process_event_file_empty() {
        let temp_dir = TempDir::new().unwrap();
        let event_file = temp_dir.path().join("empty.jsonl");
        fs::write(&event_file, "").await.unwrap();

        let mut index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 0,
        };
        let mut time_range = (None, None);

        let result = process_event_file(&event_file, &mut index, &mut time_range).await;
        assert!(result.is_ok());
        assert_eq!(index.total_events, 0);
    }

    #[tokio::test]
    async fn test_process_event_file_mixed_valid_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let event_file = temp_dir.path().join("mixed.jsonl");

        let timestamp = Utc::now();
        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp,
            correlation_id: "corr-1".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: "test-job".to_string(),
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
        };

        let mut content = String::new();
        content.push_str(&serde_json::to_string(&event).unwrap());
        content.push('\n');
        content.push_str("invalid json line\n");
        content.push_str(&serde_json::to_string(&event).unwrap());
        content.push('\n');
        fs::write(&event_file, &content).await.unwrap();

        let mut index = EventIndex {
            job_id: "test-job".to_string(),
            event_counts: HashMap::new(),
            time_range: (Utc::now(), Utc::now()),
            file_offsets: Vec::new(),
            total_events: 0,
        };
        let mut time_range = (None, None);

        let result = process_event_file(&event_file, &mut index, &mut time_range).await;
        assert!(result.is_ok());
        assert_eq!(index.total_events, 2);
    }

    // Phase 1: Tests for empty and error cases

    #[tokio::test]
    async fn test_index_with_empty_directory_creates_empty_index() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "empty-dir-job";
        fs::create_dir_all(store.job_events_dir(job_id))
            .await
            .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.total_events, 0);
        assert!(result.event_counts.is_empty());
    }

    #[tokio::test]
    async fn test_index_fails_when_save_directory_missing() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "missing-dir-job";

        let result = store.index(job_id).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_index_with_only_invalid_json_events() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "invalid-events-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();
        let event_file = events_dir.join("events-001.jsonl");
        fs::write(&event_file, "invalid\n{bad:json}\n")
            .await
            .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.total_events, 0);
    }

    // Phase 2: Tests for time range calculation paths

    #[tokio::test]
    async fn test_index_time_range_with_single_event() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "single-event-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();
        let event_time = Utc::now();
        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: event_time,
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
                timestamp: event_time,
            },
            metadata: HashMap::new(),
        };
        fs::write(
            &events_dir.join("events-001.jsonl"),
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .await
        .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.time_range, (event_time, event_time));
    }

    #[tokio::test]
    async fn test_index_time_range_with_multiple_events() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "multi-time-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();
        let start_time = Utc::now() - chrono::Duration::hours(2);
        let end_time = Utc::now();
        let event1 = EventRecord {
            id: Uuid::new_v4(),
            timestamp: start_time,
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
                timestamp: start_time,
            },
            metadata: HashMap::new(),
        };
        let event2 = EventRecord {
            id: Uuid::new_v4(),
            timestamp: end_time,
            correlation_id: "corr-2".to_string(),
            event: MapReduceEvent::JobCompleted {
                job_id: job_id.to_string(),
                duration: chrono::Duration::hours(2),
                success_count: 10,
                failure_count: 0,
            },
            metadata: HashMap::new(),
        };
        let content = format!(
            "{}\n{}\n",
            serde_json::to_string(&event1).unwrap(),
            serde_json::to_string(&event2).unwrap()
        );
        fs::write(&events_dir.join("events-001.jsonl"), content)
            .await
            .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.time_range, (start_time, end_time));
    }

    #[tokio::test]
    async fn test_index_default_time_range_no_valid_events() {
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "no-valid-events-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();
        fs::write(&events_dir.join("events-001.jsonl"), "invalid\n")
            .await
            .unwrap();

        let result = store.index(job_id).await.unwrap();

        let (start, end) = result.time_range;
        let diff = (end - start).num_milliseconds().abs();
        assert!(
            diff < 100,
            "Time range should be nearly identical when no events"
        );
    }

    // Phase 1: Direct unit tests for index method orchestration

    #[tokio::test]
    async fn test_index_handles_files_in_sorted_order() {
        // Verify that index processes files in filename order (sorted by timestamp)
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "sorted-files-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Create files with timestamps in reverse order
        let time1 = Utc::now() - chrono::Duration::hours(3);
        let time2 = Utc::now() - chrono::Duration::hours(2);
        let time3 = Utc::now() - chrono::Duration::hours(1);

        // Intentionally create files in non-sorted order
        for (filename, timestamp) in [
            ("events-003.jsonl", time3),
            ("events-001.jsonl", time1),
            ("events-002.jsonl", time2),
        ] {
            let event = EventRecord {
                id: Uuid::new_v4(),
                timestamp,
                correlation_id: format!("corr-{}", filename),
                event: MapReduceEvent::AgentStarted {
                    job_id: job_id.to_string(),
                    agent_id: format!("agent-{}", filename),
                    item_id: "item-1".to_string(),
                    worktree: "worktree-1".to_string(),
                    attempt: 1,
                },
                metadata: HashMap::new(),
            };
            fs::write(
                events_dir.join(filename),
                format!("{}\n", serde_json::to_string(&event).unwrap()),
            )
            .await
            .unwrap();
        }

        let result = store.index(job_id).await.unwrap();

        // Verify all events are indexed
        assert_eq!(result.total_events, 3);
        // Verify file offsets are in sorted filename order
        assert_eq!(result.file_offsets.len(), 3);
        assert!(result.file_offsets[0]
            .file_path
            .to_str()
            .unwrap()
            .contains("events-001.jsonl"));
        assert!(result.file_offsets[1]
            .file_path
            .to_str()
            .unwrap()
            .contains("events-002.jsonl"));
        assert!(result.file_offsets[2]
            .file_path
            .to_str()
            .unwrap()
            .contains("events-003.jsonl"));
    }

    #[tokio::test]
    async fn test_index_idempotent_multiple_calls() {
        // Verify calling index multiple times produces same result
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "idempotent-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
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
                timestamp: Utc::now(),
            },
            metadata: HashMap::new(),
        };
        fs::write(
            events_dir.join("events-001.jsonl"),
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .await
        .unwrap();

        // Call index multiple times
        let result1 = store.index(job_id).await.unwrap();
        let result2 = store.index(job_id).await.unwrap();
        let result3 = store.index(job_id).await.unwrap();

        // Verify results are identical
        assert_eq!(result1.total_events, result2.total_events);
        assert_eq!(result2.total_events, result3.total_events);
        assert_eq!(result1.job_id, result2.job_id);
        assert_eq!(result1.event_counts, result2.event_counts);
        assert_eq!(result1.time_range, result2.time_range);

        // Verify index file is overwritten consistently
        let index_path = events_dir.join("index.json");
        assert!(index_path.exists());
        let index_content = fs::read_to_string(&index_path).await.unwrap();
        let parsed: EventIndex = serde_json::from_str(&index_content).unwrap();
        assert_eq!(parsed.total_events, result3.total_events);
    }

    #[tokio::test]
    async fn test_index_with_varying_event_sizes() {
        // Test index with events of different sizes (small and large payloads)
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "varying-sizes-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // Small event with minimal metadata
        let small_event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: "s".to_string(),
            event: MapReduceEvent::AgentStarted {
                job_id: job_id.to_string(),
                agent_id: "a".to_string(),
                item_id: "i".to_string(),
                worktree: "w".to_string(),
                attempt: 1,
            },
            metadata: HashMap::new(),
        };

        // Large event with extensive metadata
        let mut large_metadata = HashMap::new();
        for i in 0..100 {
            large_metadata.insert(
                format!("key_{}", i),
                serde_json::Value::String(format!("value_{}_with_some_data", i)),
            );
        }
        let large_event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: "large-correlation-id-with-lots-of-data".to_string(),
            event: MapReduceEvent::AgentCompleted {
                job_id: job_id.to_string(),
                agent_id: "agent-with-long-identifier".to_string(),
                duration: chrono::Duration::seconds(3600),
                commits: vec!["commit1".to_string(); 50],
                json_log_location: Some(
                    "/very/long/path/to/logs/session-id-here.json".to_string(),
                ),
            },
            metadata: large_metadata,
        };

        let content = format!(
            "{}\n{}\n",
            serde_json::to_string(&small_event).unwrap(),
            serde_json::to_string(&large_event).unwrap()
        );
        fs::write(events_dir.join("events-001.jsonl"), content)
            .await
            .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.total_events, 2);
        assert_eq!(result.event_counts.get("agent_started"), Some(&1));
        assert_eq!(result.event_counts.get("agent_completed"), Some(&1));
        // Verify byte offsets are tracked correctly
        assert_eq!(result.file_offsets.len(), 2);
        assert!(result.file_offsets[0].byte_offset < result.file_offsets[1].byte_offset);
    }

    #[tokio::test]
    async fn test_index_preserves_file_offset_metadata() {
        // Verify that file offsets contain correct metadata for event lookup
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "offset-metadata-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        let event_id = Uuid::new_v4();
        let event_time = Utc::now();
        let event = EventRecord {
            id: event_id,
            timestamp: event_time,
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
                timestamp: event_time,
            },
            metadata: HashMap::new(),
        };

        fs::write(
            events_dir.join("events-001.jsonl"),
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .await
        .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.file_offsets.len(), 1);
        let offset = &result.file_offsets[0];
        assert_eq!(offset.event_id, event_id);
        assert_eq!(offset.timestamp, event_time);
        assert_eq!(offset.line_number, 1);
        assert_eq!(offset.byte_offset, 0);
        assert!(offset.file_path.to_str().unwrap().contains("events-001.jsonl"));
    }

    #[tokio::test]
    async fn test_index_aggregates_multiple_files_different_sizes() {
        // Test index with multiple files containing varying numbers of events
        let temp_dir = TempDir::new().unwrap();
        let store = FileEventStore::new(temp_dir.path().to_path_buf());
        let job_id = "multi-size-files-job";
        let events_dir = store.job_events_dir(job_id);
        fs::create_dir_all(&events_dir).await.unwrap();

        // File 1: 1 event
        let event1 = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
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
                timestamp: Utc::now(),
            },
            metadata: HashMap::new(),
        };
        fs::write(
            events_dir.join("events-001.jsonl"),
            format!("{}\n", serde_json::to_string(&event1).unwrap()),
        )
        .await
        .unwrap();

        // File 2: 5 events
        let mut content2 = String::new();
        for i in 0..5 {
            let event = EventRecord {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                correlation_id: format!("corr-{}", i),
                event: MapReduceEvent::AgentStarted {
                    job_id: job_id.to_string(),
                    agent_id: format!("agent-{}", i),
                    item_id: format!("item-{}", i),
                    worktree: format!("worktree-{}", i),
                    attempt: 1,
                },
                metadata: HashMap::new(),
            };
            content2.push_str(&serde_json::to_string(&event).unwrap());
            content2.push('\n');
        }
        fs::write(events_dir.join("events-002.jsonl"), content2)
            .await
            .unwrap();

        // File 3: 10 events
        let mut content3 = String::new();
        for i in 0..10 {
            let event = EventRecord {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                correlation_id: format!("corr-complete-{}", i),
                event: MapReduceEvent::AgentCompleted {
                    job_id: job_id.to_string(),
                    agent_id: format!("agent-{}", i),
                    duration: chrono::Duration::seconds(30),
                    commits: vec![format!("commit-{}", i)],
                    json_log_location: None,
                },
                metadata: HashMap::new(),
            };
            content3.push_str(&serde_json::to_string(&event).unwrap());
            content3.push('\n');
        }
        fs::write(events_dir.join("events-003.jsonl"), content3)
            .await
            .unwrap();

        let result = store.index(job_id).await.unwrap();

        assert_eq!(result.total_events, 16); // 1 + 5 + 10
        assert_eq!(result.event_counts.get("job_started"), Some(&1));
        assert_eq!(result.event_counts.get("agent_started"), Some(&5));
        assert_eq!(result.event_counts.get("agent_completed"), Some(&10));
        assert_eq!(result.file_offsets.len(), 16);
    }
}
