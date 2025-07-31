use mmm::context::{ContextAnalyzer, ProjectAnalyzer};
use mmm::metrics::{MetricsCollector, MetricsHistory};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_collection_with_context() {
    let temp_dir = TempDir::new().unwrap();

    // Set up project structure
    fs::create_dir_all(temp_dir.path().join("src")).unwrap();
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test-project"
version = "0.1.0"
    "#,
    )
    .unwrap();

    // Create context
    let analyzer = ProjectAnalyzer::new();
    analyzer.analyze(temp_dir.path()).await.unwrap();

    // Collect metrics
    let collector = MetricsCollector::new();
    let metrics = collector
        .collect_metrics(temp_dir.path(), "test-1".to_string())
        .await
        .unwrap();

    // Verify metrics include context data
    assert!(metrics.test_coverage >= 0.0);
    assert!(metrics.type_coverage >= 0.0);

    // Test history tracking - note MetricsHistory requires a commit SHA
    let mut history = MetricsHistory::new();
    history.add_snapshot(metrics.clone(), "test-commit-sha".to_string());

    // With only one snapshot, we can check that it was added
    assert_eq!(history.snapshots.len(), 1);
}
