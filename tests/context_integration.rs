use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_storage_integration() {
    use mmm::metrics::storage::MetricsStorage;
    use mmm::metrics::ImprovementMetrics;
    use std::time::Duration;

    let temp_dir = TempDir::new().unwrap();
    let storage = MetricsStorage::new(temp_dir.path());

    // Create test metrics
    let metrics = ImprovementMetrics {
        test_coverage: 75.0,
        type_coverage: 80.0,
        doc_coverage: 65.0,
        lint_warnings: 5,
        code_duplication: 3.5,
        compile_time: Duration::from_secs(10),
        binary_size: 1024 * 1024,
        cyclomatic_complexity: std::collections::HashMap::new(),
        cognitive_complexity: std::collections::HashMap::new(),
        max_nesting_depth: 3,
        total_lines: 1000,
        timestamp: chrono::Utc::now(),
        iteration_id: "test-iteration".to_string(),
        benchmark_results: std::collections::HashMap::new(),
        memory_usage: std::collections::HashMap::new(),
        bugs_fixed: 2,
        features_added: 1,
        tech_debt_score: 4.5,
        improvement_velocity: 1.2,
        health_score: None,
    };

    // Save and verify
    assert!(storage.save_current(&metrics).is_ok());

    // Generate and save report
    let report = storage.generate_report(&metrics);
    assert!(storage.save_report(&report, &metrics.iteration_id).is_ok());
}
