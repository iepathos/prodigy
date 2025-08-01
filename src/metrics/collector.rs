//! Main metrics collection orchestrator

use super::{ComplexityCalculator, ImprovementMetrics, PerformanceProfiler, QualityAnalyzer};
use anyhow::Result;
use std::path::Path;
use tokio::task;

/// Orchestrates metrics collection across all analyzers
pub struct MetricsCollector {
    _quality_analyzer: QualityAnalyzer,
    _performance_profiler: PerformanceProfiler,
    _complexity_calculator: ComplexityCalculator,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            _quality_analyzer: QualityAnalyzer::new(),
            _performance_profiler: PerformanceProfiler::new(),
            _complexity_calculator: ComplexityCalculator::new(),
        }
    }

    /// Collect all metrics for the project
    pub async fn collect_metrics(
        &self,
        project_path: &Path,
        iteration_id: String,
    ) -> Result<ImprovementMetrics> {
        println!("📊 Collecting project metrics...");

        let mut metrics = ImprovementMetrics::new(iteration_id);

        // Run analyzers in parallel for efficiency
        let quality_path = project_path.to_path_buf();
        let perf_path = project_path.to_path_buf();
        let complex_path = project_path.to_path_buf();

        let (quality_result, perf_result, complex_result) = tokio::join!(
            task::spawn_blocking(move || {
                let analyzer = QualityAnalyzer::new();
                analyzer.analyze(&quality_path)
            }),
            task::spawn_blocking(move || {
                let profiler = PerformanceProfiler::new();
                profiler.profile(&perf_path)
            }),
            task::spawn_blocking(move || {
                let calculator = ComplexityCalculator::new();
                calculator.calculate(&complex_path)
            }),
        );

        // Process quality metrics
        if let Ok(Ok(quality)) = quality_result {
            // Quality metrics collected
            metrics.test_coverage = quality.test_coverage;
            metrics.type_coverage = quality.type_coverage;
            metrics.lint_warnings = quality.lint_warnings;
            metrics.code_duplication = quality.code_duplication;
            metrics.doc_coverage = quality.doc_coverage;
        }

        // Process performance metrics
        if let Ok(Ok(performance)) = perf_result {
            // Performance metrics collected
            metrics.compile_time = performance.compile_time;
            metrics.binary_size = performance.binary_size;
            metrics.benchmark_results = performance.benchmark_results;
            metrics.memory_usage = performance.memory_usage;
        }

        // Process complexity metrics
        if let Ok(Ok(complexity)) = complex_result {
            // Complexity metrics collected
            metrics.cyclomatic_complexity = complexity.cyclomatic_complexity;
            metrics.cognitive_complexity = complexity.cognitive_complexity;
            metrics.max_nesting_depth = complexity.max_nesting_depth;
            metrics.total_lines = complexity.total_lines;
        }

        // Calculate derived metrics
        metrics.tech_debt_score = self.calculate_tech_debt_score(&metrics);
        metrics.improvement_velocity = self.calculate_velocity(&metrics);

        println!(
            "✅ Metrics collection complete. Overall score: {:.1}",
            metrics.overall_score()
        );

        Ok(metrics)
    }

    /// Calculate technical debt score based on various metrics
    fn calculate_tech_debt_score(&self, metrics: &ImprovementMetrics) -> f32 {
        let mut score = 0.0;

        // Low test coverage increases debt
        score += (100.0 - metrics.test_coverage) * 0.3;

        // Lint warnings indicate debt
        score += (metrics.lint_warnings as f32 * 0.5).min(30.0);

        // High complexity increases debt
        let avg_complexity = if !metrics.cyclomatic_complexity.is_empty() {
            metrics.cyclomatic_complexity.values().sum::<u32>() as f32
                / metrics.cyclomatic_complexity.len() as f32
        } else {
            0.0
        };
        score += (avg_complexity * 2.0).min(20.0);

        // Low documentation increases debt
        score += (100.0 - metrics.doc_coverage) * 0.2;

        score.min(100.0)
    }

    /// Calculate improvement velocity (rate of positive change)
    fn calculate_velocity(&self, _metrics: &ImprovementMetrics) -> f32 {
        // This would compare with previous metrics to calculate rate of change
        // For now, return a placeholder
        0.0
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_collect_metrics_success() {
        let collector = MetricsCollector::new();
        let temp_dir = TempDir::new().unwrap();

        // Create a basic Rust project structure
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"
        "#,
        )
        .unwrap();
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let result = collector
            .collect_metrics(temp_dir.path(), "test-iteration".to_string())
            .await;
        assert!(result.is_ok());

        let metrics = result.unwrap();
        assert_eq!(metrics.iteration_id, "test-iteration");
        assert!(metrics.test_coverage >= 0.0);
    }

    #[tokio::test]
    async fn test_collect_metrics_analyzer_failure() {
        let collector = MetricsCollector::new();
        let temp_dir = TempDir::new().unwrap();

        // Create directory without Cargo.toml to trigger failures
        let result = collector
            .collect_metrics(temp_dir.path(), "test-iteration".to_string())
            .await;

        // Should still return metrics even with some analyzer failures
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics.test_coverage, 0.0);
    }
}
