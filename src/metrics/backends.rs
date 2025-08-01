//! Metrics collector backend implementations

use super::events::{
    AggregateResult, Aggregation, MetricEvent, MetricsCollector, MetricsQuery, MetricsReader,
    MetricsResult, Tags,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};

/// Configuration for collector backends
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CollectorConfig {
    File {
        path: PathBuf,
        rotate_size: Option<u64>,
        compress: Option<bool>,
    },
    Memory {
        max_events: Option<usize>,
    },
    Remote {
        endpoint: String,
        api_key: String,
        timeout: Option<Duration>,
    },
    Custom {
        name: String,
        config: serde_json::Value,
    },
}

/// File-based metrics collector
pub struct FileMetricsCollector {
    name: String,
    path: PathBuf,
    buffer: Arc<Mutex<Vec<MetricEvent>>>,
    flush_interval: Duration,
    rotate_size: Option<u64>,
    compress: bool,
}

impl FileMetricsCollector {
    /// Create a new file-based collector
    pub fn new(name: impl Into<String>, path: PathBuf) -> Self {
        Self {
            name: name.into(),
            path,
            buffer: Arc::new(Mutex::new(Vec::new())),
            flush_interval: Duration::from_secs(30),
            rotate_size: None,
            compress: false,
        }
    }

    /// Create with configuration
    pub fn with_config(name: impl Into<String>, config: &CollectorConfig) -> Result<Self> {
        match config {
            CollectorConfig::File {
                path,
                rotate_size,
                compress,
            } => Ok(Self {
                name: name.into(),
                path: path.clone(),
                buffer: Arc::new(Mutex::new(Vec::new())),
                flush_interval: Duration::from_secs(30),
                rotate_size: *rotate_size,
                compress: compress.unwrap_or(false),
            }),
            _ => Err(anyhow::anyhow!(
                "Invalid config type for FileMetricsCollector"
            )),
        }
    }

    /// Write events to file
    async fn write_events(&self, events: &[MetricEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create metrics directory")?;
        }

        // Check if we need to rotate the file
        if let Some(max_size) = self.rotate_size {
            if let Ok(metadata) = fs::metadata(&self.path).await {
                if metadata.len() > max_size {
                    self.rotate_file().await?;
                }
            }
        }

        // Append events to file
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .context("Failed to open metrics file")?;

        for event in events {
            let line = serde_json::to_string(event).context("Failed to serialize metric event")?;
            file.write_all(format!("{}\n", line).as_bytes())
                .await
                .context("Failed to write metric event")?;
        }

        file.flush().await.context("Failed to flush metrics file")?;

        Ok(())
    }

    /// Rotate the log file
    async fn rotate_file(&self) -> Result<()> {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let mut rotated_path = self.path.clone();
        rotated_path.set_extension(format!("{}.log", timestamp));

        fs::rename(&self.path, &rotated_path)
            .await
            .context("Failed to rotate metrics file")?;

        if self.compress {
            // TODO: Implement compression
            eprintln!("Compression not yet implemented for metrics files");
        }

        Ok(())
    }
}

#[async_trait]
impl MetricsCollector for FileMetricsCollector {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        let mut buffer = self.buffer.lock().await;
        buffer.push(event);

        // Auto-flush if buffer is getting large
        if buffer.len() >= 100 {
            let events = buffer.drain(..).collect::<Vec<_>>();
            drop(buffer); // Release lock before async operation
            self.write_events(&events).await?;
        }

        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let mut buffer = self.buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let events = buffer.drain(..).collect::<Vec<_>>();
        drop(buffer); // Release lock before async operation

        self.write_events(&events).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// In-memory metrics collector for testing and development
pub struct MemoryMetricsCollector {
    name: String,
    events: Arc<RwLock<Vec<MetricEvent>>>,
    max_events: Option<usize>,
}

impl MemoryMetricsCollector {
    /// Create a new memory collector
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            events: Arc::new(RwLock::new(Vec::new())),
            max_events: Some(10000), // Default limit
        }
    }

    /// Create with configuration
    pub fn with_config(name: impl Into<String>, config: &CollectorConfig) -> Result<Self> {
        match config {
            CollectorConfig::Memory { max_events } => Ok(Self {
                name: name.into(),
                events: Arc::new(RwLock::new(Vec::new())),
                max_events: *max_events,
            }),
            _ => Err(anyhow::anyhow!(
                "Invalid config type for MemoryMetricsCollector"
            )),
        }
    }

    /// Get all stored events (for testing)
    pub async fn get_events(&self) -> Vec<MetricEvent> {
        self.events.read().await.clone()
    }

    /// Clear all stored events
    pub async fn clear(&self) {
        self.events.write().await.clear();
    }

    /// Get event count
    pub async fn event_count(&self) -> usize {
        self.events.read().await.len()
    }
}

#[async_trait]
impl MetricsCollector for MemoryMetricsCollector {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        let mut events = self.events.write().await;

        // Enforce max events limit
        if let Some(max) = self.max_events {
            if events.len() >= max {
                // Remove oldest events (FIFO)
                let remove_count = events.len() - max + 1;
                events.drain(0..remove_count);
            }
        }

        events.push(event);
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        // Memory collector doesn't need to flush
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl MetricsReader for MemoryMetricsCollector {
    async fn query(&self, query: MetricsQuery) -> Result<MetricsResult> {
        let events = self.events.read().await;
        let mut filtered_events = Vec::new();

        for event in events.iter() {
            // Filter by metric names
            if !query.metric_names.is_empty()
                && !query.metric_names.contains(&event.name().to_string())
            {
                continue;
            }

            // Filter by time range
            if let Some(time_range) = &query.time_range {
                let timestamp = event.timestamp();
                if timestamp < time_range.start || timestamp > time_range.end {
                    continue;
                }
            }

            // Filter by tags
            if let Some(query_tags) = &query.tags {
                let event_tags = event.tags();
                let mut matches = true;
                for (key, value) in query_tags {
                    if event_tags.get(key) != Some(value) {
                        matches = false;
                        break;
                    }
                }
                if !matches {
                    continue;
                }
            }

            filtered_events.push(event.clone());
        }

        Ok(MetricsResult {
            count: filtered_events.len(),
            events: filtered_events,
        })
    }

    async fn aggregate(
        &self,
        query: MetricsQuery,
        aggregation: Aggregation,
    ) -> Result<AggregateResult> {
        let result = self.query(query).await?;

        if result.events.is_empty() {
            return Ok(AggregateResult {
                value: 0.0,
                count: 0,
                aggregation,
            });
        }

        let values: Vec<f64> = result
            .events
            .iter()
            .map(|event| {
                match event {
                    MetricEvent::Counter { value, .. } => *value as f64,
                    MetricEvent::Gauge { value, .. } => *value,
                    MetricEvent::Timer { duration, .. } => duration.as_secs_f64(),
                    MetricEvent::Custom { .. } => 1.0, // Count custom events as 1
                }
            })
            .collect();

        let count = values.len();

        let aggregated_value = match aggregation {
            Aggregation::Sum => values.iter().sum(),
            Aggregation::Average => values.iter().sum::<f64>() / values.len() as f64,
            Aggregation::Min => values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            Aggregation::Max => values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
            Aggregation::Count => values.len() as f64,
            Aggregation::Percentile(p) => {
                let mut sorted_values = values;
                sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let idx = ((sorted_values.len() - 1) as f64 * p / 100.0).round() as usize;
                sorted_values[idx.min(sorted_values.len() - 1)]
            }
        };

        Ok(AggregateResult {
            value: aggregated_value,
            count,
            aggregation,
        })
    }
}

/// Composite collector that forwards to multiple collectors
pub struct CompositeMetricsCollector {
    name: String,
    collectors: Vec<Arc<dyn MetricsCollector>>,
}

impl CompositeMetricsCollector {
    /// Create a new composite collector
    pub fn new(name: impl Into<String>, collectors: Vec<Arc<dyn MetricsCollector>>) -> Self {
        Self {
            name: name.into(),
            collectors,
        }
    }
}

#[async_trait]
impl MetricsCollector for CompositeMetricsCollector {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        let mut errors = Vec::new();

        for collector in &self.collectors {
            if let Err(e) = collector.record(event.clone()).await {
                errors.push(format!("{}: {}", collector.name(), e));
            }
        }

        if !errors.is_empty() {
            eprintln!("Composite collector errors: {}", errors.join(", "));
        }

        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        let mut errors = Vec::new();

        for collector in &self.collectors {
            if let Err(e) = collector.flush().await {
                errors.push(format!("{}: {}", collector.name(), e));
            }
        }

        if !errors.is_empty() {
            return Err(anyhow::anyhow!("Flush errors: {}", errors.join(", ")));
        }

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_memory_collector() {
        let collector = MemoryMetricsCollector::new("test");

        let event = MetricEvent::counter("test.counter", 42, Tags::new());
        collector.record(event).await.unwrap();

        assert_eq!(collector.event_count().await, 1);

        let events = collector.get_events().await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            MetricEvent::Counter { name, value, .. } => {
                assert_eq!(name, "test.counter");
                assert_eq!(*value, 42);
            }
            _ => panic!("Expected counter event"),
        }
    }

    #[tokio::test]
    async fn test_memory_collector_max_events() {
        let config = CollectorConfig::Memory {
            max_events: Some(2),
        };
        let collector = MemoryMetricsCollector::with_config("test", &config).unwrap();

        // Add 3 events, should only keep 2
        collector
            .record(MetricEvent::counter("test.1", 1, Tags::new()))
            .await
            .unwrap();
        collector
            .record(MetricEvent::counter("test.2", 2, Tags::new()))
            .await
            .unwrap();
        collector
            .record(MetricEvent::counter("test.3", 3, Tags::new()))
            .await
            .unwrap();

        assert_eq!(collector.event_count().await, 2);

        let events = collector.get_events().await;
        // Should have kept the last 2 events
        match &events[0] {
            MetricEvent::Counter { name, .. } => assert_eq!(name, "test.2"),
            _ => panic!("Expected counter event"),
        }
        match &events[1] {
            MetricEvent::Counter { name, .. } => assert_eq!(name, "test.3"),
            _ => panic!("Expected counter event"),
        }
    }

    #[tokio::test]
    async fn test_file_collector() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("metrics.log");

        let collector = FileMetricsCollector::new("test", file_path.clone());

        let event = MetricEvent::counter("test.counter", 42, Tags::new());
        collector.record(event).await.unwrap();
        collector.flush().await.unwrap();

        // Verify file was created and contains data
        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("test.counter"));
        assert!(content.contains("42"));
    }

    #[tokio::test]
    async fn test_memory_reader_query() {
        let collector = MemoryMetricsCollector::new("test");

        let mut tags = Tags::new();
        tags.insert("env".to_string(), "test".to_string());

        collector
            .record(MetricEvent::counter("test.counter", 42, tags.clone()))
            .await
            .unwrap();
        collector
            .record(MetricEvent::gauge("test.gauge", 3.14, tags))
            .await
            .unwrap();

        let query = MetricsQuery {
            metric_names: vec!["test.counter".to_string()],
            time_range: None,
            tags: None,
            aggregation: None,
        };

        let result = collector.query(query).await.unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.events.len(), 1);

        match &result.events[0] {
            MetricEvent::Counter { name, value, .. } => {
                assert_eq!(name, "test.counter");
                assert_eq!(*value, 42);
            }
            _ => panic!("Expected counter event"),
        }
    }

    #[tokio::test]
    async fn test_memory_reader_aggregate() {
        let collector = MemoryMetricsCollector::new("test");

        collector
            .record(MetricEvent::counter("test.counter", 10, Tags::new()))
            .await
            .unwrap();
        collector
            .record(MetricEvent::counter("test.counter", 20, Tags::new()))
            .await
            .unwrap();
        collector
            .record(MetricEvent::counter("test.counter", 30, Tags::new()))
            .await
            .unwrap();

        let query = MetricsQuery {
            metric_names: vec!["test.counter".to_string()],
            time_range: None,
            tags: None,
            aggregation: None,
        };

        let result = collector.aggregate(query, Aggregation::Sum).await.unwrap();
        assert_eq!(result.value, 60.0);
        assert_eq!(result.count, 3);
    }
}
