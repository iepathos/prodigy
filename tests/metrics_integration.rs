mod common;

use mmm::context::{ContextAnalyzer, ProjectAnalyzer};
use mmm::metrics::{MetricsCollector, MetricsHistory};
use mmm::subprocess::SubprocessManager;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_metrics_collection_with_context() {
    // Initialize test environment (sets MMM_TEST_MODE=true)
    common::init_test_env();

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
    let subprocess = SubprocessManager::production();
    let collector = MetricsCollector::new(subprocess);
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

#[tokio::test]
async fn test_metrics_persistence_and_loading() {
    // Set test mode
    unsafe { std::env::set_var("MMM_TEST_MODE", "true") };

    let temp_dir = TempDir::new().unwrap();
    let metrics_dir = temp_dir.path().join(".mmm/metrics");
    fs::create_dir_all(&metrics_dir).unwrap();

    // Create and save metrics
    let subprocess = SubprocessManager::production();
    let collector = MetricsCollector::new(subprocess);

    // Create a simple project
    fs::create_dir_all(temp_dir.path().join("src")).unwrap();
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "test-metrics"
version = "0.1.0"
        "#,
    )
    .unwrap();

    // Collect metrics
    let metrics = collector
        .collect_metrics(temp_dir.path(), "test-iteration-1".to_string())
        .await
        .unwrap();

    // Save metrics
    let current_file = metrics_dir.join("current.json");
    fs::write(
        &current_file,
        serde_json::to_string_pretty(&metrics).unwrap(),
    )
    .unwrap();

    // Load metrics
    let loaded_content = fs::read_to_string(&current_file).unwrap();
    let loaded_metrics: mmm::metrics::ImprovementMetrics =
        serde_json::from_str(&loaded_content).unwrap();

    // Verify loaded metrics match original
    assert_eq!(loaded_metrics.iteration_id, metrics.iteration_id);
    assert_eq!(loaded_metrics.test_coverage, metrics.test_coverage);
}

#[tokio::test]
async fn test_metrics_trend_analysis() {
    // Set test mode
    unsafe { std::env::set_var("MMM_TEST_MODE", "true") };

    let mut history = MetricsHistory::new();

    // Add multiple snapshots with improving metrics
    for i in 1..=5 {
        let mut metrics = mmm::metrics::ImprovementMetrics {
            test_coverage: 50.0 + (i as f32 * 5.0),
            type_coverage: 60.0 + (i as f32 * 3.0),
            lint_warnings: 20 - (i * 2),
            code_duplication: 10.0 - (i as f32),
            doc_coverage: 30.0 + (i as f32 * 10.0),
            ..Default::default()
        };
        metrics.iteration_id = format!("iteration-{i}");

        history.add_snapshot(metrics, format!("commit-{i}"));
    }

    // Verify trends
    assert_eq!(history.snapshots.len(), 5);

    // Check that metrics are improving
    let first = &history.snapshots[0].metrics;
    let last = &history.snapshots[4].metrics;

    assert!(last.test_coverage > first.test_coverage);
    assert!(last.type_coverage > first.type_coverage);
    assert!(last.lint_warnings < first.lint_warnings);
    assert!(last.code_duplication < first.code_duplication);
    assert!(last.doc_coverage > first.doc_coverage);
}

#[tokio::test]
async fn test_metrics_with_real_rust_code() {
    // Set test mode
    unsafe { std::env::set_var("MMM_TEST_MODE", "true") };

    let temp_dir = TempDir::new().unwrap();

    // Create a more realistic Rust project
    fs::create_dir_all(temp_dir.path().join("src")).unwrap();
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[package]
name = "real-test"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
        "#,
    )
    .unwrap();

    // Create source files with various complexity levels
    fs::write(
        temp_dir.path().join("src/lib.rs"),
        r#"
//! A test library with various functions

use anyhow::Result;

/// Simple function with low complexity
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Medium complexity function
pub fn process_data(input: Vec<i32>) -> Result<Vec<i32>> {
    let mut result = Vec::new();
    for value in input {
        if value > 0 {
            result.push(value * 2);
        } else if value < 0 {
            result.push(value.abs());
        } else {
            result.push(0);
        }
    }
    Ok(result)
}

/// High complexity function (intentionally complex for testing)
pub fn complex_logic(x: i32, y: i32, flag: bool) -> i32 {
    match (x, y, flag) {
        (x, y, true) if x > 0 && y > 0 => x + y,
        (x, y, true) if x > 0 && y <= 0 => x - y,
        (x, y, false) if x <= 0 && y > 0 => y - x,
        (x, y, false) if x <= 0 && y <= 0 => x * y,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_process_data() {
        let input = vec![1, -2, 0, 3, -4];
        let result = process_data(input).unwrap();
        assert_eq!(result, vec![2, 2, 0, 6, 4]);
    }
}
"#,
    )
    .unwrap();

    // Collect metrics
    let subprocess = SubprocessManager::production();
    let collector = MetricsCollector::new(subprocess);
    let metrics = collector
        .collect_metrics(temp_dir.path(), "real-test-1".to_string())
        .await
        .unwrap();

    // Verify metrics are reasonable
    assert!(metrics.total_lines > 0);
    assert!(metrics.cyclomatic_complexity.values().any(|&v| v > 0));
}

#[tokio::test]
async fn test_metrics_report_generation() {
    // Set test mode
    unsafe { std::env::set_var("MMM_TEST_MODE", "true") };

    let temp_dir = TempDir::new().unwrap();
    let reports_dir = temp_dir.path().join(".mmm/metrics/reports");
    fs::create_dir_all(&reports_dir).unwrap();

    // Create metrics
    let metrics = mmm::metrics::ImprovementMetrics {
        test_coverage: 75.5,
        type_coverage: 82.3,
        lint_warnings: 5,
        code_duplication: 3.2,
        doc_coverage: 68.9,
        total_lines: 1234,
        iteration_id: "report-test-1".to_string(),
        ..Default::default()
    };

    // Generate report content
    let report = format!(
        r#"Metrics Report - {}

Code Quality:
- Test Coverage: {:.1}%
- Type Coverage: {:.1}%
- Documentation Coverage: {:.1}%
- Lint Warnings: {}
- Code Duplication: {:.1}%

Code Size:
- Total Lines: {}

Timestamp: {}
"#,
        metrics.iteration_id,
        metrics.test_coverage,
        metrics.type_coverage,
        metrics.doc_coverage,
        metrics.lint_warnings,
        metrics.code_duplication,
        metrics.total_lines,
        metrics.timestamp
    );

    // Save report
    let report_file = reports_dir.join(format!("report-{}.txt", metrics.iteration_id));
    fs::write(&report_file, report).unwrap();

    // Verify report exists and contains expected content
    assert!(report_file.exists());
    let saved_report = fs::read_to_string(&report_file).unwrap();
    assert!(saved_report.contains("Test Coverage: 75.5%"));
    assert!(saved_report.contains("Total Lines: 1234"));
}
