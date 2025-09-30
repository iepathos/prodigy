//! Event writer implementations for different output targets

use super::EventRecord;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Trait for writing events to various destinations
#[async_trait]
pub trait EventWriter: Send + Sync {
    /// Write a batch of events
    async fn write(&self, events: &[EventRecord]) -> Result<()>;

    /// Flush any buffered data
    async fn flush(&self) -> Result<()>;

    /// Clone the writer
    fn clone(&self) -> Box<dyn EventWriter>;
}

/// File-based event writer in JSONL format
pub struct JsonlEventWriter {
    file_path: PathBuf,
    writer: Arc<Mutex<Option<BufWriter<File>>>>,
    rotation_size: u64,
    current_size: Arc<Mutex<u64>>,
}

impl JsonlEventWriter {
    /// Create a new JSONL event writer
    pub async fn new(file_path: PathBuf) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create event directory")?;
        }

        // Open file for appending
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await
            .context("Failed to open event file")?;

        let metadata = file.metadata().await?;
        let current_size = metadata.len();

        let writer = BufWriter::new(file);

        Ok(Self {
            file_path,
            writer: Arc::new(Mutex::new(Some(writer))),
            rotation_size: 100 * 1024 * 1024, // 100MB
            current_size: Arc::new(Mutex::new(current_size)),
        })
    }

    /// Create a writer with custom rotation size
    pub async fn with_rotation(file_path: PathBuf, rotation_size: u64) -> Result<Self> {
        let mut writer = Self::new(file_path).await?;
        writer.rotation_size = rotation_size;
        Ok(writer)
    }

    /// Rotate the log file if needed
    async fn rotate_if_needed(&self) -> Result<()> {
        let current_size = *self.current_size.lock().await;

        if current_size >= self.rotation_size {
            let mut writer_guard = self.writer.lock().await;

            // Close current file
            if let Some(mut writer) = writer_guard.take() {
                writer.flush().await?;
            }

            // Create rotation filename
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let rotation_path = self
                .file_path
                .with_extension(format!("{}.jsonl.gz", timestamp));

            // Compress and move old file
            self.compress_and_move(&rotation_path).await?;

            // Open new file
            let file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&self.file_path)
                .await?;

            *writer_guard = Some(BufWriter::new(file));

            // Reset size counter
            let mut size_guard = self.current_size.lock().await;
            *size_guard = 0;

            info!("Rotated event log to {:?}", rotation_path);
        }

        Ok(())
    }

    /// Compress and move file for rotation
    async fn compress_and_move(&self, target: &Path) -> Result<()> {
        // For now, just rename the file
        // In production, we'd use flate2 or similar for actual compression
        let backup_path = self.file_path.with_extension("jsonl.bak");
        fs::rename(&self.file_path, &backup_path)
            .await
            .context("Failed to rotate event file")?;

        // TODO: Implement actual compression
        fs::rename(&backup_path, target)
            .await
            .context("Failed to move rotated file")?;

        Ok(())
    }
}

#[async_trait]
impl EventWriter for JsonlEventWriter {
    async fn write(&self, events: &[EventRecord]) -> Result<()> {
        self.rotate_if_needed().await?;

        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            let mut written_bytes = 0u64;

            for event in events {
                let json = serde_json::to_string(event)?;
                let line = format!("{}\n", json);
                let bytes = line.as_bytes();

                writer.write_all(bytes).await?;
                written_bytes += bytes.len() as u64;
            }

            // Update size counter
            let mut size_guard = self.current_size.lock().await;
            *size_guard += written_bytes;

            debug!("Wrote {} events ({} bytes)", events.len(), written_bytes);
        }

        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let mut writer_guard = self.writer.lock().await;
        if let Some(writer) = writer_guard.as_mut() {
            writer.flush().await?;
        }
        Ok(())
    }

    fn clone(&self) -> Box<dyn EventWriter> {
        Box::new(Self {
            file_path: self.file_path.clone(),
            writer: Arc::clone(&self.writer),
            rotation_size: self.rotation_size,
            current_size: Arc::clone(&self.current_size),
        })
    }
}

/// Generic file event writer that delegates to specific format writers
pub struct FileEventWriter {
    base_path: PathBuf,
    job_id: String,
    writer: Box<dyn EventWriter>,
}

impl FileEventWriter {
    /// Create a new file event writer for a specific job
    pub async fn new(base_path: PathBuf, job_id: String) -> Result<Self> {
        let event_dir = base_path.join("events").join(&job_id);
        fs::create_dir_all(&event_dir)
            .await
            .context("Failed to create job event directory")?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let file_path = event_dir.join(format!("events-{}.jsonl", timestamp));

        let writer = Box::new(JsonlEventWriter::new(file_path).await?);

        Ok(Self {
            base_path,
            job_id,
            writer,
        })
    }

    /// Create an index file for quick event lookup
    pub async fn create_index(&self) -> Result<()> {
        let _index_path = self
            .base_path
            .join("events")
            .join(&self.job_id)
            .join("index.json");

        // TODO: Implement index creation
        debug!("Index creation not yet implemented");

        Ok(())
    }
}

#[async_trait]
impl EventWriter for FileEventWriter {
    async fn write(&self, events: &[EventRecord]) -> Result<()> {
        self.writer.write(events).await
    }

    async fn flush(&self) -> Result<()> {
        self.writer.flush().await
    }

    fn clone(&self) -> Box<dyn EventWriter> {
        Box::new(Self {
            base_path: self.base_path.clone(),
            job_id: self.job_id.clone(),
            writer: self.writer.clone(),
        })
    }
}

/// Stdout event writer for debugging
#[allow(dead_code)]
pub struct StdoutEventWriter;

#[async_trait]
impl EventWriter for StdoutEventWriter {
    async fn write(&self, events: &[EventRecord]) -> Result<()> {
        for event in events {
            let json = serde_json::to_string_pretty(event)?;
            println!("{}", json);
        }
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        // Stdout is auto-flushed
        Ok(())
    }

    fn clone(&self) -> Box<dyn EventWriter> {
        Box::new(StdoutEventWriter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::events::MapReduceEvent;
    use crate::cook::execution::mapreduce::MapReduceConfig;

    use tempfile::TempDir;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_jsonl_writer() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("events.jsonl");

        let writer = JsonlEventWriter::new(file_path.clone()).await.unwrap();

        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            correlation_id: "test-correlation".to_string(),
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
                timestamp: chrono::Utc::now(),
            },
            metadata: Default::default(),
        };

        writer.write(&[event]).await.unwrap();
        writer.flush().await.unwrap();

        // Verify file was written
        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("test-job"));
    }

    #[tokio::test]
    async fn test_write_empty_events() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("events.jsonl");

        let writer = JsonlEventWriter::new(file_path.clone()).await.unwrap();

        // Writing empty events should succeed without error
        writer.write(&[]).await.unwrap();
        writer.flush().await.unwrap();

        // File should exist but be empty (or only have initial content)
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_write_multiple_events() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("events.jsonl");

        let writer = JsonlEventWriter::new(file_path.clone()).await.unwrap();

        let events: Vec<EventRecord> = (0..3)
            .map(|i| EventRecord {
                id: Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                correlation_id: format!("test-correlation-{}", i),
                event: MapReduceEvent::JobStarted {
                    job_id: format!("test-job-{}", i),
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
                    timestamp: chrono::Utc::now(),
                },
                metadata: Default::default(),
            })
            .collect();

        writer.write(&events).await.unwrap();
        writer.flush().await.unwrap();

        // Verify all events were written
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(content.contains("test-job-0"));
        assert!(content.contains("test-job-1"));
        assert!(content.contains("test-job-2"));
    }

    #[tokio::test]
    async fn test_size_tracking_across_writes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("events.jsonl");

        let writer = JsonlEventWriter::new(file_path.clone()).await.unwrap();

        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            correlation_id: "test-correlation".to_string(),
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
                timestamp: chrono::Utc::now(),
            },
            metadata: Default::default(),
        };

        // Write same event multiple times
        writer.write(&[event.clone()]).await.unwrap();
        writer.write(&[event.clone()]).await.unwrap();
        writer.flush().await.unwrap();

        // Verify size tracking - should accumulate across writes
        let size = *writer.current_size.lock().await;
        assert!(size > 0, "Size should be tracked across writes");

        // Verify file content
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
    }
}
