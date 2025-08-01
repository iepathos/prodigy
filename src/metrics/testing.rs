//! Testing utilities for metrics

use super::backends::MemoryMetricsCollector;
use super::events::{MetricEvent, Tags};
use super::registry::{MetricsConfig, MetricsRegistry};
use std::sync::Arc;
use std::time::Duration;

/// Metrics assertions for testing
pub struct MetricsAssert {
    collector: Arc<MemoryMetricsCollector>,
}

impl MetricsAssert {
    /// Create a new metrics assert with a memory collector
    pub fn new() -> (Self, Arc<MetricsRegistry>) {
        let collector = Arc::new(MemoryMetricsCollector::new("test"));
        let registry = Arc::new(MetricsRegistry::new(MetricsConfig::default()));

        let assert = Self {
            collector: collector.clone(),
        };

        (assert, registry)
    }

    /// Create with existing collector
    pub fn with_collector(collector: Arc<MemoryMetricsCollector>) -> Self {
        Self { collector }
    }

    /// Get the underlying collector
    pub fn collector(&self) -> &Arc<MemoryMetricsCollector> {
        &self.collector
    }

    /// Get all events
    pub async fn events(&self) -> Vec<MetricEvent> {
        self.collector.get_events().await
    }

    /// Get event count
    pub async fn event_count(&self) -> usize {
        self.collector.event_count().await
    }

    /// Clear all events
    pub async fn clear(&self) {
        self.collector.clear().await;
    }

    /// Assert that a counter with specific name and value was recorded
    pub async fn assert_counter(&self, name: &str, expected_value: i64) {
        let events = self.events().await;
        let counter_events: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                MetricEvent::Counter { name: n, value, .. } if n == name => Some(*value),
                _ => None,
            })
            .collect();

        assert!(
            !counter_events.is_empty(),
            "No counter events found with name '{name}'"
        );

        let total_value: i64 = counter_events.iter().sum();
        assert_eq!(
            total_value, expected_value,
            "Counter '{name}' expected value {expected_value}, got {total_value}"
        );
    }

    /// Assert that a gauge with specific name and value was recorded
    pub async fn assert_gauge(&self, name: &str, expected_value: f64) {
        let events = self.events().await;
        let gauge_events: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                MetricEvent::Gauge { name: n, value, .. } if n == name => Some(*value),
                _ => None,
            })
            .collect();

        assert!(
            !gauge_events.is_empty(),
            "No gauge events found with name '{name}'"
        );

        // For gauges, we check the last value
        let last_value = gauge_events.last().unwrap();
        assert_eq!(
            *last_value, expected_value,
            "Gauge '{name}' expected value {expected_value}, got {last_value}"
        );
    }

    /// Assert that a timer with specific name was recorded
    pub async fn assert_timer_called(&self, name: &str) {
        let events = self.events().await;
        let timer_exists = events.iter().any(|e| match e {
            MetricEvent::Timer { name: n, .. } => n == name,
            _ => false,
        });

        assert!(timer_exists, "No timer events found with name '{name}'");
    }

    /// Assert that a timer was recorded within a duration range
    pub async fn assert_timer_duration(
        &self,
        name: &str,
        min_duration: Duration,
        max_duration: Duration,
    ) {
        let events = self.events().await;
        let timer_durations: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                MetricEvent::Timer {
                    name: n, duration, ..
                } if n == name => Some(*duration),
                _ => None,
            })
            .collect();

        assert!(
            !timer_durations.is_empty(),
            "No timer events found with name '{name}'"
        );

        for duration in &timer_durations {
            assert!(
                *duration >= min_duration && *duration <= max_duration,
                "Timer '{name}' duration {duration:?} not within range {min_duration:?} to {max_duration:?}"
            );
        }
    }

    /// Assert that a custom event with specific name was recorded
    pub async fn assert_custom(&self, name: &str) {
        let events = self.events().await;
        let custom_exists = events.iter().any(|e| match e {
            MetricEvent::Custom { name: n, .. } => n == name,
            _ => false,
        });

        assert!(custom_exists, "No custom events found with name '{name}'");
    }

    /// Assert that no metrics were recorded
    pub async fn assert_no_metrics(&self) {
        let count = self.event_count().await;
        assert_eq!(count, 0, "Expected no metrics, but found {count}");
    }

    /// Assert that exactly N events were recorded
    pub async fn assert_event_count(&self, expected_count: usize) {
        let count = self.event_count().await;
        assert_eq!(
            count, expected_count,
            "Expected {expected_count} events, but found {count}"
        );
    }

    /// Assert that an event with specific tags was recorded
    pub async fn assert_tags(&self, name: &str, expected_tags: &Tags) {
        let events = self.events().await;
        let matching_events: Vec<_> = events
            .iter()
            .filter(|e| {
                e.name() == name && {
                    let event_tags = e.tags();
                    expected_tags
                        .iter()
                        .all(|(k, v)| event_tags.get(k) == Some(v))
                }
            })
            .collect();

        assert!(
            !matching_events.is_empty(),
            "No events found with name '{name}' and tags {expected_tags:?}"
        );
    }

    /// Assert that events were recorded in chronological order
    pub async fn assert_chronological_order(&self) {
        let events = self.events().await;
        if events.len() < 2 {
            return; // Not enough events to check order
        }

        for window in events.windows(2) {
            let first_time = window[0].timestamp();
            let second_time = window[1].timestamp();
            assert!(
                first_time <= second_time,
                "Events not in chronological order: {first_time:?} should be <= {second_time:?}"
            );
        }
    }

    /// Get events matching a specific pattern
    pub async fn events_matching<F>(&self, predicate: F) -> Vec<MetricEvent>
    where
        F: Fn(&MetricEvent) -> bool,
    {
        let events = self.events().await;
        events.into_iter().filter(predicate).collect()
    }

    /// Print all events for debugging
    pub async fn debug_print_events(&self) {
        let events = self.events().await;
        println!("Recorded events ({}):", events.len());
        for (i, event) in events.iter().enumerate() {
            match event {
                MetricEvent::Counter {
                    name,
                    value,
                    tags,
                    timestamp,
                } => {
                    println!("  {i}: Counter '{name}' = {value} at {timestamp} {tags:?}");
                }
                MetricEvent::Gauge {
                    name,
                    value,
                    tags,
                    timestamp,
                } => {
                    println!("  {i}: Gauge '{name}' = {value} at {timestamp} {tags:?}");
                }
                MetricEvent::Timer {
                    name,
                    duration,
                    tags,
                    timestamp,
                } => {
                    println!("  {i}: Timer '{name}' = {duration:?} at {timestamp} {tags:?}");
                }
                MetricEvent::Custom {
                    name,
                    data,
                    tags,
                    timestamp,
                } => {
                    println!("  {i}: Custom '{name}' = {data:?} at {timestamp} {tags:?}");
                }
            }
        }
    }
}

impl Default for MetricsAssert {
    fn default() -> Self {
        let (assert, _) = Self::new();
        assert
    }
}

/// Create a test metrics registry with memory collector
pub async fn create_test_registry() -> (Arc<MetricsRegistry>, MetricsAssert) {
    let collector = Arc::new(MemoryMetricsCollector::new("test"));
    let registry = Arc::new(MetricsRegistry::new(MetricsConfig::default()));
    registry.register_collector(collector.clone()).await;

    let assert = MetricsAssert::with_collector(collector);
    (registry, assert)
}

/// Create a disabled test metrics registry
pub fn create_disabled_registry() -> Arc<MetricsRegistry> {
    let config = MetricsConfig {
        enabled: false,
        ..Default::default()
    };
    Arc::new(MetricsRegistry::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_metrics_assert_counter() {
        let (registry, assert) = create_test_registry().await;

        registry
            .increment("test.counter", Tags::new())
            .await
            .unwrap();
        registry
            .increment("test.counter", Tags::new())
            .await
            .unwrap();

        assert.assert_counter("test.counter", 2).await;
        assert.assert_event_count(2).await;
    }

    #[tokio::test]
    async fn test_metrics_assert_gauge() {
        let (registry, assert) = create_test_registry().await;

        registry
            .gauge("test.gauge", 42.5, Tags::new())
            .await
            .unwrap();
        registry
            .gauge("test.gauge", 100.0, Tags::new())
            .await
            .unwrap();

        assert.assert_gauge("test.gauge", 100.0).await; // Should check last value
        assert.assert_event_count(2).await;
    }

    #[tokio::test]
    async fn test_metrics_assert_timer() {
        let (registry, assert) = create_test_registry().await;

        let result = registry
            .time("test.operation", Tags::new(), async {
                sleep(Duration::from_millis(10)).await;
                Ok::<_, anyhow::Error>(42)
            })
            .await
            .unwrap();

        assert_eq!(result, 42);
        assert.assert_timer_called("test.operation").await;
        assert
            .assert_timer_duration(
                "test.operation",
                Duration::from_millis(5),
                Duration::from_millis(100),
            )
            .await;
    }

    #[tokio::test]
    async fn test_metrics_assert_tags() {
        let (registry, assert) = create_test_registry().await;

        let mut tags = Tags::new();
        tags.insert("env".to_string(), "test".to_string());
        tags.insert("version".to_string(), "1.0".to_string());

        registry
            .increment("test.counter", tags.clone())
            .await
            .unwrap();

        assert.assert_tags("test.counter", &tags).await;
    }

    #[tokio::test]
    async fn test_metrics_assert_no_metrics() {
        let (_registry, assert) = create_test_registry().await;
        assert.assert_no_metrics().await;
    }

    #[tokio::test]
    async fn test_metrics_assert_chronological_order() {
        let (registry, assert) = create_test_registry().await;

        registry.increment("test.1", Tags::new()).await.unwrap();
        sleep(Duration::from_millis(1)).await;
        registry.increment("test.2", Tags::new()).await.unwrap();
        sleep(Duration::from_millis(1)).await;
        registry.increment("test.3", Tags::new()).await.unwrap();

        assert.assert_chronological_order().await;
    }

    #[tokio::test]
    #[should_panic(expected = "No counter events found")]
    async fn test_metrics_assert_counter_not_found() {
        let (_registry, assert) = create_test_registry().await;
        assert.assert_counter("missing.counter", 1).await;
    }
}
