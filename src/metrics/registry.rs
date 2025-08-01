//! Metrics registry for managing collectors

use super::events::{
    AggregateResult, Aggregation, MetricEvent, MetricsCollector, MetricsQuery, MetricsReader,
    MetricsResult, Tags,
};
use anyhow::Result;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Configuration for metrics registry
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Whether metrics collection is enabled
    pub enabled: bool,
    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
    /// Buffer size for batching events
    pub buffer_size: usize,
    /// Flush interval for buffered events
    pub flush_interval: Duration,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_rate: 1.0,
            buffer_size: 1000,
            flush_interval: Duration::from_secs(30),
        }
    }
}

/// Registry for managing multiple metrics collectors
pub struct MetricsRegistry {
    collectors: Arc<RwLock<Vec<Arc<dyn MetricsCollector>>>>,
    readers: Arc<RwLock<Vec<Arc<dyn MetricsReader>>>>,
    config: MetricsConfig,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub fn new(config: MetricsConfig) -> Self {
        Self {
            collectors: Arc::new(RwLock::new(Vec::new())),
            readers: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Register a metrics collector
    pub async fn register_collector(&self, collector: Arc<dyn MetricsCollector>) {
        let mut collectors = self.collectors.write().await;
        collectors.push(collector);
    }

    /// Register a metrics reader
    pub async fn register_reader(&self, reader: Arc<dyn MetricsReader>) {
        let mut readers = self.readers.write().await;
        readers.push(reader);
    }

    /// Record a metric event in all collectors
    pub async fn record(&self, event: MetricEvent) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Apply sampling if configured
        if self.config.sampling_rate < 1.0 {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            if rng.gen::<f64>() > self.config.sampling_rate {
                return Ok(());
            }
        }

        let collectors = self.collectors.read().await;
        let mut errors = Vec::new();

        // Record to all collectors, collecting errors but not failing
        for collector in collectors.iter() {
            if let Err(e) = collector.record(event.clone()).await {
                errors.push(format!("Collector '{}': {}", collector.name(), e));
            }
        }

        // Log errors but don't fail the operation
        if !errors.is_empty() {
            eprintln!("Metrics recording errors: {}", errors.join(", "));
        }

        Ok(())
    }

    /// Flush all collectors
    pub async fn flush(&self) -> Result<()> {
        let collectors = self.collectors.read().await;
        let mut errors = Vec::new();

        for collector in collectors.iter() {
            if let Err(e) = collector.flush().await {
                errors.push(format!("Collector '{}': {}", collector.name(), e));
            }
        }

        if !errors.is_empty() {
            return Err(anyhow::anyhow!("Flush errors: {}", errors.join(", ")));
        }

        Ok(())
    }

    /// Query metrics from first available reader
    pub async fn query(&self, query: MetricsQuery) -> Result<MetricsResult> {
        let readers = self.readers.read().await;

        if readers.is_empty() {
            return Err(anyhow::anyhow!("No metrics readers available"));
        }

        // Try readers in order until one succeeds
        for reader in readers.iter() {
            match reader.query(query.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    eprintln!("Reader query failed: {e}");
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!("All readers failed to execute query"))
    }

    /// Aggregate metrics from first available reader
    pub async fn aggregate(
        &self,
        query: MetricsQuery,
        aggregation: Aggregation,
    ) -> Result<AggregateResult> {
        let readers = self.readers.read().await;

        if readers.is_empty() {
            return Err(anyhow::anyhow!("No metrics readers available"));
        }

        // Try readers in order until one succeeds
        for reader in readers.iter() {
            match reader.aggregate(query.clone(), aggregation.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    eprintln!("Reader aggregation failed: {e}");
                    continue;
                }
            }
        }

        Err(anyhow::anyhow!("All readers failed to execute aggregation"))
    }

    /// Convenience method to increment a counter
    pub async fn increment(&self, name: &str, tags: Tags) -> Result<()> {
        let event = MetricEvent::counter(name, 1, tags);
        self.record(event).await
    }

    /// Convenience method to set a gauge value
    pub async fn gauge(&self, name: &str, value: f64, tags: Tags) -> Result<()> {
        let event = MetricEvent::gauge(name, value, tags);
        self.record(event).await
    }

    /// Convenience method to time an operation
    pub async fn time<F, R>(&self, name: &str, tags: Tags, f: F) -> Result<R>
    where
        F: Future<Output = Result<R>>,
    {
        let start = std::time::Instant::now();
        let result = f.await;
        let duration = start.elapsed();

        // Record timing regardless of result
        let timing_event = MetricEvent::timer(name, duration, tags);
        if let Err(e) = self.record(timing_event).await {
            eprintln!("Failed to record timing metric: {e}");
        }

        result
    }

    /// Get current configuration
    pub fn config(&self) -> &MetricsConfig {
        &self.config
    }

    /// Check if metrics are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get number of registered collectors
    pub async fn collector_count(&self) -> usize {
        self.collectors.read().await.len()
    }

    /// Get number of registered readers
    pub async fn reader_count(&self) -> usize {
        self.readers.read().await.len()
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new(MetricsConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::backends::MemoryMetricsCollector;

    #[tokio::test]
    async fn test_registry_record_disabled() {
        let config = MetricsConfig {
            enabled: false,
            ..Default::default()
        };
        let registry = MetricsRegistry::new(config);

        let result = registry.increment("test.counter", Tags::new()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_with_collector() {
        let registry = MetricsRegistry::default();
        let collector = Arc::new(MemoryMetricsCollector::new("test"));

        registry.register_collector(collector.clone()).await;

        assert_eq!(registry.collector_count().await, 1);

        let result = registry.increment("test.counter", Tags::new()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_timing() {
        let registry = MetricsRegistry::default();
        let collector = Arc::new(MemoryMetricsCollector::new("test"));

        registry.register_collector(collector.clone()).await;

        let result = registry
            .time("test.operation", Tags::new(), async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok::<_, anyhow::Error>(42)
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        // Verify timing event was recorded
        let events = collector.get_events().await;
        assert_eq!(events.len(), 1);
        match &events[0] {
            MetricEvent::Timer { name, .. } => {
                assert_eq!(name, "test.operation");
            }
            _ => panic!("Expected timer event"),
        }
    }
}
