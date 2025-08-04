use chrono::Utc;
use mmm::metrics::backends::{CollectorConfig, FileMetricsCollector, MemoryMetricsCollector};
use mmm::metrics::events::{Aggregation, MetricsQuery, MetricsReader};
use mmm::metrics::{MetricEvent, MetricsRegistry, Tags};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_collection_and_reporting() {
    let temp_dir = TempDir::new().unwrap();
    let config = CollectorConfig::File {
        path: temp_dir.path().join("metrics.log"),
        rotate_size: None,
        compress: Some(false),
    };

    let collector = Arc::new(FileMetricsCollector::with_config("test", &config).unwrap());
    let registry = MetricsRegistry::new(mmm::metrics::MetricsConfig::default());
    registry.register_collector(collector.clone()).await;

    // Record various metrics
    registry
        .record(MetricEvent::counter("test.count", 10, Tags::new()))
        .await
        .unwrap();
    registry
        .record(MetricEvent::gauge("test.gauge", 3.14159, Tags::new()))
        .await
        .unwrap();
    registry
        .record(MetricEvent::timer(
            "test.duration",
            Duration::from_secs(5),
            Tags::new(),
        ))
        .await
        .unwrap();

    // Flush to ensure writes
    registry.flush().await.unwrap();

    // Verify file contains metrics
    let metrics_file = temp_dir.path().join("metrics.log");
    assert!(metrics_file.exists());

    let content = tokio::fs::read_to_string(&metrics_file).await.unwrap();
    assert!(content.contains("test.count"));
    assert!(content.contains("test.gauge"));
    assert!(content.contains("test.duration"));
}

#[tokio::test]
async fn test_metrics_with_tags_and_filtering() {
    let collector = Arc::new(MemoryMetricsCollector::new("test"));
    let registry = MetricsRegistry::new(mmm::metrics::MetricsConfig::default());
    registry.register_collector(collector.clone()).await;

    // Record metrics with different tags
    let mut prod_tags = Tags::new();
    prod_tags.insert("env".to_string(), "prod".to_string());
    prod_tags.insert("region".to_string(), "us-east".to_string());

    let mut test_tags = Tags::new();
    test_tags.insert("env".to_string(), "test".to_string());
    test_tags.insert("region".to_string(), "us-west".to_string());

    registry
        .record(MetricEvent::counter(
            "api.requests",
            1000,
            prod_tags.clone(),
        ))
        .await
        .unwrap();
    registry
        .record(MetricEvent::counter("api.requests", 500, test_tags.clone()))
        .await
        .unwrap();
    registry
        .record(MetricEvent::counter("api.errors", 10, prod_tags.clone()))
        .await
        .unwrap();
    registry
        .record(MetricEvent::counter("api.errors", 5, test_tags.clone()))
        .await
        .unwrap();

    // Query prod metrics only
    let mut query_tags = Tags::new();
    query_tags.insert("env".to_string(), "prod".to_string());

    let query = MetricsQuery {
        metric_names: vec!["api.requests".to_string()],
        time_range: None,
        tags: Some(query_tags),
        aggregation: None,
    };

    let result = collector.query(query).await.unwrap();
    assert_eq!(result.count, 1);
    assert_eq!(result.events.len(), 1);

    match &result.events[0] {
        MetricEvent::Counter { value, .. } => assert_eq!(*value, 1000),
        _ => panic!("Expected counter event"),
    }
}

#[tokio::test]
async fn test_metrics_aggregation() {
    let collector = Arc::new(MemoryMetricsCollector::new("test"));
    let registry = MetricsRegistry::new(mmm::metrics::MetricsConfig::default());
    registry.register_collector(collector.clone()).await;

    // Record multiple values for aggregation
    for i in 1..=10 {
        registry
            .record(MetricEvent::gauge("system.cpu", i as f64, Tags::new()))
            .await
            .unwrap();
    }

    let query = MetricsQuery {
        metric_names: vec!["system.cpu".to_string()],
        time_range: None,
        tags: None,
        aggregation: None,
    };

    // Test different aggregations
    let sum_result = collector
        .aggregate(query.clone(), Aggregation::Sum)
        .await
        .unwrap();
    assert_eq!(sum_result.value, 55.0); // 1+2+...+10 = 55

    let avg_result = collector
        .aggregate(query.clone(), Aggregation::Average)
        .await
        .unwrap();
    assert_eq!(avg_result.value, 5.5);

    let min_result = collector
        .aggregate(query.clone(), Aggregation::Min)
        .await
        .unwrap();
    assert_eq!(min_result.value, 1.0);

    let max_result = collector
        .aggregate(query.clone(), Aggregation::Max)
        .await
        .unwrap();
    assert_eq!(max_result.value, 10.0);

    let count_result = collector
        .aggregate(query.clone(), Aggregation::Count)
        .await
        .unwrap();
    assert_eq!(count_result.value, 10.0);
}

#[tokio::test]
async fn test_metrics_time_range_filtering() {
    let collector = Arc::new(MemoryMetricsCollector::new("test"));
    let registry = MetricsRegistry::new(mmm::metrics::MetricsConfig::default());
    registry.register_collector(collector.clone()).await;

    // Record events at different times (simulated)
    let _base_time = Utc::now();

    // Record some metrics
    registry
        .record(MetricEvent::counter("events.processed", 100, Tags::new()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    registry
        .record(MetricEvent::counter("events.processed", 200, Tags::new()))
        .await
        .unwrap();

    // Query all events
    let query_all = MetricsQuery {
        metric_names: vec!["events.processed".to_string()],
        time_range: None,
        tags: None,
        aggregation: None,
    };

    let result_all = collector.query(query_all).await.unwrap();
    assert_eq!(result_all.count, 2);
}

#[tokio::test]
async fn test_metrics_custom_events() {
    let collector = Arc::new(MemoryMetricsCollector::new("test"));
    let registry = MetricsRegistry::new(mmm::metrics::MetricsConfig::default());
    registry.register_collector(collector.clone()).await;

    // Record custom events
    let custom_data = serde_json::json!({
        "action": "user_login",
        "user_id": "12345",
        "success": true
    });

    registry
        .record(MetricEvent::custom(
            "user.actions",
            custom_data,
            Tags::new(),
        ))
        .await
        .unwrap();

    // Query custom events
    let query = MetricsQuery {
        metric_names: vec!["user.actions".to_string()],
        time_range: None,
        tags: None,
        aggregation: None,
    };

    let result = collector.query(query).await.unwrap();
    assert_eq!(result.count, 1);

    match &result.events[0] {
        MetricEvent::Custom { data, .. } => {
            assert_eq!(data["action"], "user_login");
            assert_eq!(data["user_id"], "12345");
            assert_eq!(data["success"], true);
        }
        _ => panic!("Expected custom event"),
    }
}

#[tokio::test]
async fn test_metrics_percentile_aggregation() {
    let collector = Arc::new(MemoryMetricsCollector::new("test"));
    let registry = MetricsRegistry::new(mmm::metrics::MetricsConfig::default());
    registry.register_collector(collector.clone()).await;

    // Record response times
    let response_times = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];
    for time in response_times {
        registry
            .record(MetricEvent::timer(
                "api.response_time",
                Duration::from_millis(time as u64),
                Tags::new(),
            ))
            .await
            .unwrap();
    }

    let query = MetricsQuery {
        metric_names: vec!["api.response_time".to_string()],
        time_range: None,
        tags: None,
        aggregation: None,
    };

    // Test percentiles
    let p50_result = collector
        .aggregate(query.clone(), Aggregation::Percentile(50.0))
        .await
        .unwrap();
    assert!(p50_result.value >= 0.04 && p50_result.value <= 0.06); // ~50ms in seconds

    let p95_result = collector
        .aggregate(query, Aggregation::Percentile(95.0))
        .await
        .unwrap();
    assert!(p95_result.value >= 0.09 && p95_result.value <= 0.11); // ~100ms in seconds
}
