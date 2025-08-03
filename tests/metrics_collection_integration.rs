use mmm::metrics::collector::MetricsCollector;
use mmm::subprocess::SubprocessManager;
use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_collection_integration() {
    let temp_dir = TempDir::new().unwrap();
    let subprocess = SubprocessManager::production();
    let collector = MetricsCollector::new(subprocess);

    // Collect metrics
    let result = collector
        .collect_metrics(temp_dir.path(), "test-iteration".to_string())
        .await;
    assert!(result.is_ok());

    let metrics = result.unwrap();
    assert!(metrics.test_coverage >= 0.0);
    assert!(metrics.lint_warnings >= 0);
}
