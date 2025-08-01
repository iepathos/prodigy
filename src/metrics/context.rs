//! Metrics context for passing through execution

use super::events::{MetricEvent, Tags};
use super::registry::MetricsRegistry;
use anyhow::Result;
use std::sync::Arc;

/// Metrics context for passing through execution
#[derive(Clone)]
pub struct MetricsContext {
    registry: Arc<MetricsRegistry>,
    tags: Tags,
}

impl MetricsContext {
    /// Create a new metrics context
    pub fn new(registry: Arc<MetricsRegistry>, tags: Tags) -> Self {
        Self { registry, tags }
    }

    /// Create a child context with additional tags
    pub fn child(&self, additional_tags: Tags) -> Self {
        let mut combined_tags = self.tags.clone();
        combined_tags.extend(additional_tags);

        Self {
            registry: self.registry.clone(),
            tags: combined_tags,
        }
    }

    /// Record a metric event with context tags
    pub async fn record(&self, mut event: MetricEvent) -> Result<()> {
        // Merge context tags with event tags
        let event_tags = match &mut event {
            MetricEvent::Counter { tags, .. } => tags,
            MetricEvent::Gauge { tags, .. } => tags,
            MetricEvent::Timer { tags, .. } => tags,
            MetricEvent::Custom { tags, .. } => tags,
        };

        // Context tags take precedence over event tags
        for (key, value) in &self.tags {
            event_tags.insert(key.clone(), value.clone());
        }

        self.registry.record(event).await
    }

    /// Increment a counter with context tags
    pub async fn increment(&self, name: &str, additional_tags: Option<Tags>) -> Result<()> {
        let mut tags = self.tags.clone();
        if let Some(additional) = additional_tags {
            tags.extend(additional);
        }

        self.registry.increment(name, tags).await
    }

    /// Set a gauge value with context tags
    pub async fn gauge(&self, name: &str, value: f64, additional_tags: Option<Tags>) -> Result<()> {
        let mut tags = self.tags.clone();
        if let Some(additional) = additional_tags {
            tags.extend(additional);
        }

        self.registry.gauge(name, value, tags).await
    }

    /// Time an operation with context tags
    pub async fn time<F, R>(&self, name: &str, additional_tags: Option<Tags>, f: F) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>>,
    {
        let mut tags = self.tags.clone();
        if let Some(additional) = additional_tags {
            tags.extend(additional);
        }

        self.registry.time(name, tags, f).await
    }

    /// Get the underlying registry
    pub fn registry(&self) -> &Arc<MetricsRegistry> {
        &self.registry
    }

    /// Get the context tags
    pub fn tags(&self) -> &Tags {
        &self.tags
    }
}

/// Builder for creating metrics contexts
pub struct MetricsContextBuilder {
    registry: Arc<MetricsRegistry>,
    tags: Tags,
}

impl MetricsContextBuilder {
    /// Create a new builder
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self {
            registry,
            tags: Tags::new(),
        }
    }

    /// Add a tag
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add multiple tags
    pub fn tags(mut self, tags: Tags) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Build the context
    pub fn build(self) -> MetricsContext {
        MetricsContext::new(self.registry, self.tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{backends::MemoryMetricsCollector, registry::MetricsConfig};

    #[tokio::test]
    async fn test_metrics_context_tags() {
        let registry = Arc::new(MetricsRegistry::new(MetricsConfig::default()));
        let collector = Arc::new(MemoryMetricsCollector::new("test"));
        registry.register_collector(collector.clone()).await;

        let mut context_tags = Tags::new();
        context_tags.insert("component".to_string(), "test".to_string());

        let context = MetricsContext::new(registry, context_tags);

        let mut event_tags = Tags::new();
        event_tags.insert("operation".to_string(), "increment".to_string());

        context
            .increment("test.counter", Some(event_tags))
            .await
            .unwrap();

        let events = collector.get_events().await;
        assert_eq!(events.len(), 1);

        match &events[0] {
            MetricEvent::Counter { tags, .. } => {
                assert_eq!(tags.get("component"), Some(&"test".to_string()));
                assert_eq!(tags.get("operation"), Some(&"increment".to_string()));
            }
            _ => panic!("Expected counter event"),
        }
    }

    #[tokio::test]
    async fn test_metrics_context_child() {
        let registry = Arc::new(MetricsRegistry::new(MetricsConfig::default()));
        let collector = Arc::new(MemoryMetricsCollector::new("test"));
        registry.register_collector(collector.clone()).await;

        let mut parent_tags = Tags::new();
        parent_tags.insert("component".to_string(), "test".to_string());

        let parent_context = MetricsContext::new(registry, parent_tags);

        let mut child_tags = Tags::new();
        child_tags.insert("operation".to_string(), "child".to_string());

        let child_context = parent_context.child(child_tags);

        child_context.increment("test.counter", None).await.unwrap();

        let events = collector.get_events().await;
        assert_eq!(events.len(), 1);

        match &events[0] {
            MetricEvent::Counter { tags, .. } => {
                assert_eq!(tags.get("component"), Some(&"test".to_string()));
                assert_eq!(tags.get("operation"), Some(&"child".to_string()));
            }
            _ => panic!("Expected counter event"),
        }
    }

    #[tokio::test]
    async fn test_metrics_context_builder() {
        let registry = Arc::new(MetricsRegistry::new(MetricsConfig::default()));

        let context = MetricsContextBuilder::new(registry.clone())
            .tag("component", "test")
            .tag("version", "1.0")
            .build();

        assert_eq!(context.tags().get("component"), Some(&"test".to_string()));
        assert_eq!(context.tags().get("version"), Some(&"1.0".to_string()));
    }
}
