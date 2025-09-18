//! Progress event streaming for external consumers

use super::{AgentOperation, PhaseType};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error};

/// Progress events that can be streamed to consumers
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ProgressEvent {
    /// Workflow started
    #[serde(rename = "workflow_started")]
    WorkflowStarted {
        job_id: String,
        total_items: usize,
        timestamp: std::time::SystemTime,
    },
    /// Item completed
    #[serde(rename = "item_complete")]
    ItemComplete {
        item_id: String,
        total_completed: usize,
        percentage: f64,
    },
    /// Agent status update
    #[serde(rename = "agent_update")]
    AgentUpdate {
        agent_index: usize,
        #[serde(skip)]
        operation: AgentOperation,
        #[serde(skip)]
        timestamp: Instant,
    },
    /// Phase changed
    #[serde(rename = "phase_change")]
    PhaseChange {
        #[serde(skip)]
        phase: PhaseType,
        #[serde(skip)]
        timestamp: Instant,
    },
    /// Error occurred
    #[serde(rename = "error")]
    Error {
        message: String,
        failed_count: usize,
    },
    /// General message
    #[serde(rename = "message")]
    Message(String),
    /// Workflow completed
    #[serde(rename = "workflow_completed")]
    WorkflowCompleted {
        job_id: String,
        total_processed: usize,
        total_failed: usize,
        duration_secs: f64,
    },
    /// Metrics update
    #[serde(rename = "metrics")]
    Metrics {
        items_per_second: f64,
        agent_utilization: f64,
        estimated_remaining_secs: Option<f64>,
    },
}

/// Error type for streaming operations
#[derive(Debug)]
pub struct StreamError {
    message: String,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Stream error: {}", self.message)
    }
}

impl std::error::Error for StreamError {}

impl StreamError {
    /// Create a new stream error
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Trait for progress stream consumers
#[async_trait]
pub trait ProgressStreamConsumer: Send + Sync {
    /// Consume a progress event
    async fn consume(&mut self, event: ProgressEvent) -> Result<(), StreamError>;

    /// Called when the stream starts
    async fn on_start(&mut self) -> Result<(), StreamError> {
        Ok(())
    }

    /// Called when the stream ends
    async fn on_end(&mut self) -> Result<(), StreamError> {
        Ok(())
    }

    /// Get consumer name
    fn name(&self) -> &str;
}

/// Progress event streamer
pub struct ProgressStreamer {
    /// Broadcast sender for events
    sender: broadcast::Sender<ProgressEvent>,
    /// Active consumers
    consumers: Arc<RwLock<Vec<Box<dyn ProgressStreamConsumer>>>>,
    /// Whether streaming is active
    is_active: Arc<RwLock<bool>>,
}

impl ProgressStreamer {
    /// Create a new progress streamer
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);

        Self {
            sender,
            consumers: Arc::new(RwLock::new(Vec::new())),
            is_active: Arc::new(RwLock::new(true)),
        }
    }

    /// Subscribe to progress events
    pub fn subscribe(&self) -> broadcast::Receiver<ProgressEvent> {
        self.sender.subscribe()
    }

    /// Add a consumer
    pub async fn add_consumer(&self, mut consumer: Box<dyn ProgressStreamConsumer>) {
        // Notify consumer of start
        if let Err(e) = consumer.on_start().await {
            error!("Consumer {} failed to start: {}", consumer.name(), e);
            return;
        }

        let mut consumers = self.consumers.write().await;
        consumers.push(consumer);
    }

    /// Stream an event to all consumers
    pub async fn stream_event(&self, event: ProgressEvent) {
        // Check if streaming is active
        if !*self.is_active.read().await {
            return;
        }

        // Broadcast to subscribers
        if let Err(e) = self.sender.send(event.clone()) {
            debug!("No subscribers for progress event: {}", e);
        }

        // Send to consumers
        let mut consumers = self.consumers.write().await;
        let mut failed_indices = Vec::new();

        for (index, consumer) in consumers.iter_mut().enumerate() {
            if let Err(e) = consumer.consume(event.clone()).await {
                error!("Consumer {} failed: {}", consumer.name(), e);
                failed_indices.push(index);
            }
        }

        // Remove failed consumers
        for index in failed_indices.into_iter().rev() {
            consumers.remove(index);
        }
    }

    /// Stop streaming
    pub async fn stop(&self) {
        *self.is_active.write().await = false;

        // Notify consumers
        let mut consumers = self.consumers.write().await;
        for consumer in consumers.iter_mut() {
            if let Err(e) = consumer.on_end().await {
                error!("Consumer {} failed to end: {}", consumer.name(), e);
            }
        }

        consumers.clear();
    }

    /// Check if streaming is active
    pub async fn is_active(&self) -> bool {
        *self.is_active.read().await
    }

    /// Get consumer count
    pub async fn consumer_count(&self) -> usize {
        self.consumers.read().await.len()
    }
}

impl Default for ProgressStreamer {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON Lines consumer for writing events to a file
pub struct JsonLinesConsumer {
    writer: tokio::io::BufWriter<tokio::fs::File>,
    event_count: usize,
}

impl JsonLinesConsumer {
    /// Create a new JSON Lines consumer
    pub async fn new(path: impl AsRef<std::path::Path>) -> Result<Self, StreamError> {
        let file = tokio::fs::File::create(path)
            .await
            .map_err(|e| StreamError::new(format!("Failed to create file: {}", e)))?;

        Ok(Self {
            writer: tokio::io::BufWriter::new(file),
            event_count: 0,
        })
    }
}

#[async_trait]
impl ProgressStreamConsumer for JsonLinesConsumer {
    async fn consume(&mut self, event: ProgressEvent) -> Result<(), StreamError> {
        use tokio::io::AsyncWriteExt;

        let json = serde_json::to_string(&event)
            .map_err(|e| StreamError::new(format!("Failed to serialize: {}", e)))?;

        self.writer
            .write_all(json.as_bytes())
            .await
            .map_err(|e| StreamError::new(format!("Write failed: {}", e)))?;

        self.writer
            .write_all(b"\n")
            .await
            .map_err(|e| StreamError::new(format!("Write failed: {}", e)))?;

        self.event_count += 1;

        // Flush periodically
        if self.event_count % 100 == 0 {
            self.writer
                .flush()
                .await
                .map_err(|e| StreamError::new(format!("Flush failed: {}", e)))?;
        }

        Ok(())
    }

    async fn on_end(&mut self) -> Result<(), StreamError> {
        use tokio::io::AsyncWriteExt;

        self.writer
            .flush()
            .await
            .map_err(|e| StreamError::new(format!("Final flush failed: {}", e)))?;

        Ok(())
    }

    fn name(&self) -> &str {
        "JSON Lines Consumer"
    }
}

/// WebSocket consumer for real-time progress streaming
pub struct WebSocketConsumer {
    endpoint: String,
    #[allow(dead_code)]
    client_id: String,
}

impl WebSocketConsumer {
    /// Create a new WebSocket consumer
    pub fn new(endpoint: impl Into<String>, client_id: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            client_id: client_id.into(),
        }
    }
}

#[async_trait]
impl ProgressStreamConsumer for WebSocketConsumer {
    async fn consume(&mut self, event: ProgressEvent) -> Result<(), StreamError> {
        // Implementation would send events via WebSocket
        debug!("Would send to WebSocket {}: {:?}", self.endpoint, event);
        Ok(())
    }

    fn name(&self) -> &str {
        "WebSocket Consumer"
    }
}

/// Metrics aggregator consumer
pub struct MetricsAggregator {
    events: Vec<ProgressEvent>,
    start_time: Option<Instant>,
}

impl MetricsAggregator {
    /// Create a new metrics aggregator
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            start_time: None,
        }
    }

    /// Get aggregated metrics
    pub fn get_metrics(&self) -> AggregatedMetrics {
        let total_events = self.events.len();
        let completed_items = self
            .events
            .iter()
            .filter(|e| matches!(e, ProgressEvent::ItemComplete { .. }))
            .count();
        let errors = self
            .events
            .iter()
            .filter(|e| matches!(e, ProgressEvent::Error { .. }))
            .count();

        let duration = self.start_time.map(|s| s.elapsed()).unwrap_or_default();

        AggregatedMetrics {
            total_events,
            completed_items,
            errors,
            duration,
        }
    }
}

impl Default for MetricsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProgressStreamConsumer for MetricsAggregator {
    async fn consume(&mut self, event: ProgressEvent) -> Result<(), StreamError> {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
        self.events.push(event);
        Ok(())
    }

    fn name(&self) -> &str {
        "Metrics Aggregator"
    }
}

/// Aggregated metrics from progress events
#[derive(Debug, Clone)]
pub struct AggregatedMetrics {
    /// Total events received
    pub total_events: usize,
    /// Completed items
    pub completed_items: usize,
    /// Error count
    pub errors: usize,
    /// Total duration
    pub duration: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_streamer() {
        let streamer = ProgressStreamer::new();

        // Add a metrics aggregator
        let aggregator = Box::new(MetricsAggregator::new());
        streamer.add_consumer(aggregator).await;

        // Stream events
        streamer
            .stream_event(ProgressEvent::Message("Test".to_string()))
            .await;
        streamer
            .stream_event(ProgressEvent::ItemComplete {
                item_id: "item1".to_string(),
                total_completed: 1,
                percentage: 10.0,
            })
            .await;

        assert_eq!(streamer.consumer_count().await, 1);

        // Stop streaming
        streamer.stop().await;
        assert!(!streamer.is_active().await);
        assert_eq!(streamer.consumer_count().await, 0);
    }

    #[tokio::test]
    async fn test_metrics_aggregator() {
        let mut aggregator = MetricsAggregator::new();

        // Consume events
        aggregator
            .consume(ProgressEvent::ItemComplete {
                item_id: "1".to_string(),
                total_completed: 1,
                percentage: 50.0,
            })
            .await
            .unwrap();

        aggregator
            .consume(ProgressEvent::Error {
                message: "Test error".to_string(),
                failed_count: 1,
            })
            .await
            .unwrap();

        let metrics = aggregator.get_metrics();
        assert_eq!(metrics.total_events, 2);
        assert_eq!(metrics.completed_items, 1);
        assert_eq!(metrics.errors, 1);
    }

    #[tokio::test]
    async fn test_broadcast_subscription() {
        let streamer = ProgressStreamer::new();
        let mut receiver = streamer.subscribe();

        // Stream an event
        let event = ProgressEvent::Message("Test broadcast".to_string());
        streamer.stream_event(event.clone()).await;

        // Receive event
        let received = receiver.recv().await.unwrap();
        matches!(received, ProgressEvent::Message(msg) if msg == "Test broadcast");
    }
}
