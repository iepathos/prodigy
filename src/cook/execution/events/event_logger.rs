//! Event logger implementation for MapReduce

use super::{EventWriter, MapReduceEvent};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::interval;
use tracing::{debug, error, warn};
use uuid::Uuid;

/// A single event record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: String,
    pub event: MapReduceEvent,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, Value>,
}

/// Event logger for MapReduce jobs
pub struct EventLogger {
    writers: Vec<Box<dyn EventWriter>>,
    buffer: Arc<Mutex<Vec<EventRecord>>>,
    flush_interval: Duration,
    buffer_size_limit: usize,
    correlation_id: Arc<Mutex<String>>,
    event_counter: Arc<AtomicUsize>,
    shutdown: Arc<Mutex<bool>>,
}

impl EventLogger {
    /// Create a new event logger
    pub fn new(writers: Vec<Box<dyn EventWriter>>) -> Self {
        Self {
            writers,
            buffer: Arc::new(Mutex::new(Vec::new())),
            flush_interval: Duration::seconds(5),
            buffer_size_limit: 1000,
            correlation_id: Arc::new(Mutex::new(Uuid::new_v4().to_string())),
            event_counter: Arc::new(AtomicUsize::new(0)),
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a new event logger with custom configuration
    pub fn with_config(
        writers: Vec<Box<dyn EventWriter>>,
        flush_interval: Duration,
        buffer_size_limit: usize,
    ) -> Self {
        Self {
            writers,
            buffer: Arc::new(Mutex::new(Vec::new())),
            flush_interval,
            buffer_size_limit,
            correlation_id: Arc::new(Mutex::new(Uuid::new_v4().to_string())),
            event_counter: Arc::new(AtomicUsize::new(0)),
            shutdown: Arc::new(Mutex::new(false)),
        }
    }

    /// Log an event
    pub async fn log(&self, event: MapReduceEvent) -> Result<()> {
        let record = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: self.current_correlation_id().await,
            event,
            metadata: self.collect_metadata().await,
        };

        let mut buffer = self.buffer.lock().await;
        buffer.push(record);

        // Increment event counter
        let count = self.event_counter.fetch_add(1, Ordering::SeqCst) + 1;

        // Check if we should flush
        if buffer.len() >= self.buffer_size_limit {
            drop(buffer); // Release lock before flushing
            self.flush().await?;
        }

        debug!("Event logged (total: {})", count);
        Ok(())
    }

    /// Log an event with custom metadata
    pub async fn log_with_metadata(
        &self,
        event: MapReduceEvent,
        metadata: HashMap<String, Value>,
    ) -> Result<()> {
        let mut record = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: self.current_correlation_id().await,
            event,
            metadata: self.collect_metadata().await,
        };

        // Merge custom metadata
        record.metadata.extend(metadata);

        let mut buffer = self.buffer.lock().await;
        buffer.push(record);

        let count = self.event_counter.fetch_add(1, Ordering::SeqCst) + 1;

        if buffer.len() >= self.buffer_size_limit {
            drop(buffer);
            self.flush().await?;
        }

        debug!("Event logged with metadata (total: {})", count);
        Ok(())
    }

    /// Set the current correlation ID
    pub async fn set_correlation_id(&self, id: String) {
        let mut correlation_id = self.correlation_id.lock().await;
        *correlation_id = id;
    }

    /// Get the current correlation ID
    pub async fn current_correlation_id(&self) -> String {
        let correlation_id = self.correlation_id.lock().await;
        correlation_id.clone()
    }

    /// Flush buffered events to all writers
    pub async fn flush(&self) -> Result<()> {
        let mut buffer = self.buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let events: Vec<EventRecord> = buffer.drain(..).collect();
        drop(buffer); // Release lock early

        debug!("Flushing {} events", events.len());

        // Write to all writers sequentially
        // (Parallel would require Arc<dyn EventWriter> which complicates the design)
        let mut errors = Vec::new();
        for writer in &self.writers {
            if let Err(e) = writer.write(&events).await {
                errors.push(e);
            }
        }

        if !errors.is_empty() {
            error!("Failed to write to {} writers", errors.len());
            for e in &errors {
                error!("Writer error: {}", e);
            }
            return Err(anyhow::anyhow!(
                "Failed to write events: {} errors",
                errors.len()
            ));
        }

        // Flush all writers
        for writer in &self.writers {
            writer.flush().await?;
        }

        Ok(())
    }

    /// Start background flush task
    pub fn start_background_flush(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = interval(std::time::Duration::from_secs(
                self.flush_interval.num_seconds() as u64,
            ));
            loop {
                ticker.tick().await;

                let shutdown = *self.shutdown.lock().await;
                if shutdown {
                    debug!("Background flush task shutting down");
                    break;
                }

                if let Err(e) = self.flush().await {
                    warn!("Background flush failed: {}", e);
                }
            }
        })
    }

    /// Shutdown the logger
    pub async fn shutdown(&self) -> Result<()> {
        // Signal shutdown
        let mut shutdown = self.shutdown.lock().await;
        *shutdown = true;
        drop(shutdown);

        // Final flush
        self.flush().await?;

        Ok(())
    }

    /// Get event statistics
    pub fn stats(&self) -> EventStats {
        EventStats {
            total_events: self.event_counter.load(Ordering::SeqCst),
            buffer_size: 0, // Will be updated async
        }
    }

    /// Collect metadata for the current environment
    async fn collect_metadata(&self) -> HashMap<String, Value> {
        let mut metadata = HashMap::new();

        // Add system metadata
        metadata.insert("host".to_string(), Value::String(hostname()));
        metadata.insert("pid".to_string(), Value::Number(std::process::id().into()));

        // Add thread information
        if let Some(name) = std::thread::current().name() {
            metadata.insert("thread".to_string(), Value::String(name.to_string()));
        }

        metadata
    }
}

/// Event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub total_events: usize,
    pub buffer_size: usize,
}

/// Get the hostname
fn hostname() -> String {
    // Just use a simple placeholder since hostname isn't critical
    // In production, we could add the hostname crate as dependency
    "localhost".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_event_logger_basic() {
        let logger = EventLogger::new(vec![]);
        let event = MapReduceEvent::JobStarted {
            job_id: "test-job".to_string(),
            config: MapReduceConfig {
                input: PathBuf::from("test.json"),
                json_path: "$.items".to_string(),
                max_parallel: 5,
                timeout_per_agent: 300,
                retry_on_failure: 2,
                max_items: None,
                offset: None,
            },
            total_items: 10,
            timestamp: Utc::now(),
        };

        logger.log(event).await.unwrap();
        assert_eq!(logger.stats().total_events, 1);
    }

    #[tokio::test]
    async fn test_event_logger_with_metadata() {
        let logger = EventLogger::new(vec![]);
        let event = MapReduceEvent::AgentStarted {
            job_id: "test-job".to_string(),
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            worktree: "worktree-1".to_string(),
            attempt: 1,
        };

        let mut metadata = HashMap::new();
        metadata.insert(
            "custom_field".to_string(),
            Value::String("value".to_string()),
        );

        logger.log_with_metadata(event, metadata).await.unwrap();
        assert_eq!(logger.stats().total_events, 1);
    }

    #[tokio::test]
    async fn test_correlation_id() {
        let logger = EventLogger::new(vec![]);

        let initial_id = logger.current_correlation_id().await;
        assert!(!initial_id.is_empty());

        logger
            .set_correlation_id("custom-correlation-id".to_string())
            .await;
        let updated_id = logger.current_correlation_id().await;
        assert_eq!(updated_id, "custom-correlation-id");
    }
}
