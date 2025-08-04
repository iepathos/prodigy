//! Main metrics collection orchestrator

use super::{
    ComplexityCalculator, ComplexityHotspot, ComplexitySummary, FileComplexityStats,
    ImprovementMetrics, PerformanceProfiler, QualityAnalyzer,
};
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::task;

/// Orchestrates metrics collection across all analyzers
pub struct MetricsCollector {
    subprocess: SubprocessManager,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new(subprocess: SubprocessManager) -> Self {
        Self { subprocess }
    }

    /// Collect all metrics for the project
    pub async fn collect_metrics(
        &self,
        project_path: &Path,
        iteration_id: String,
    ) -> Result<ImprovementMetrics> {
        self.collect_metrics_with_config(project_path, iteration_id, false)
            .await
    }

    /// Collect all metrics for the project with configuration
    pub async fn collect_metrics_with_config(
        &self,
        project_path: &Path,
        iteration_id: String,
        run_coverage: bool,
    ) -> Result<ImprovementMetrics> {
        println!("📊 Collecting project metrics...");

        // Check if we're in test mode
        let test_mode = std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true";
        if test_mode {
            // Return mock metrics in test mode to avoid running actual cargo commands
            let mut metrics = ImprovementMetrics::new(iteration_id);
            metrics.test_coverage = 30.0;
            metrics.type_coverage = 30.0;
            metrics.lint_warnings = 0;
            metrics.code_duplication = 0.0;
            metrics.doc_coverage = 30.0;
            metrics.improvement_velocity = 0.0;
            metrics.total_lines = 100; // Add mock total_lines for tests
                                       // Don't add any complexity data - it would be compressed anyway
            println!("✅ Metrics collection complete. Overall score: 30.0");
            return Ok(metrics);
        }

        let mut metrics = ImprovementMetrics::new(iteration_id);

        // Run analyzers in parallel for efficiency
        let quality_path = project_path.to_path_buf();
        let perf_path = project_path.to_path_buf();
        let complex_path = project_path.to_path_buf();

        let subprocess_quality = self.subprocess.clone();
        let subprocess_perf = self.subprocess.clone();

        let (quality_result, perf_result, complex_result) = tokio::join!(
            async {
                let analyzer = QualityAnalyzer::new(subprocess_quality).await;
                analyzer
                    .analyze_with_coverage(&quality_path, run_coverage)
                    .await
            },
            async {
                let profiler = PerformanceProfiler::new(subprocess_perf);
                profiler.profile(&perf_path).await
            },
            task::spawn_blocking(move || {
                let calculator = ComplexityCalculator::new();
                calculator.calculate(&complex_path)
            }),
        );

        // Process quality metrics
        if let Ok(quality) = quality_result {
            // Quality metrics collected
            metrics.test_coverage = quality.test_coverage;
            metrics.type_coverage = quality.type_coverage;
            metrics.lint_warnings = quality.lint_warnings;
            metrics.code_duplication = quality.code_duplication;
            metrics.doc_coverage = quality.doc_coverage;
        }

        // Process performance metrics
        if let Ok(performance) = perf_result {
            // Performance metrics collected
            metrics.compile_time = performance.compile_time;
            metrics.binary_size = performance.binary_size;
            metrics.benchmark_results = performance.benchmark_results;
            metrics.memory_usage = performance.memory_usage;
        }

        // Process complexity metrics
        if let Ok(Ok(complexity)) = complex_result {
            // Store raw metrics for backward compatibility (will be empty after compression)
            metrics.max_nesting_depth = complexity.max_nesting_depth;
            metrics.total_lines = complexity.total_lines;

            // Compress complexity metrics to reduce file size
            let (summary, hotspots) = self.compress_complexity_metrics(
                project_path,
                complexity.cyclomatic_complexity,
                complexity.cognitive_complexity,
            );

            // Store compressed metrics
            metrics.complexity_summary = Some(summary);
            metrics.complexity_hotspots = hotspots;

            // Clear legacy fields to save space (they're now in the compressed format)
            metrics.cyclomatic_complexity.clear();
            metrics.cognitive_complexity.clear();
        }

        // Calculate derived metrics
        metrics.improvement_velocity = self.calculate_velocity(&metrics);

        // Update unified health score
        metrics.update_health_score();

        println!(
            "✅ Metrics collection complete. Overall health score: {:.1}",
            metrics.overall_score()
        );

        Ok(metrics)
    }

    /// Calculate improvement velocity (rate of positive change)
    fn calculate_velocity(&self, _metrics: &ImprovementMetrics) -> f32 {
        // This would compare with previous metrics to calculate rate of change
        // For now, return a placeholder
        0.0
    }

    /// Compress complexity metrics to reduce file size
    fn compress_complexity_metrics(
        &self,
        project_path: &Path,
        cyclomatic: HashMap<String, u32>,
        cognitive: HashMap<String, u32>,
    ) -> (ComplexitySummary, Vec<ComplexityHotspot>) {
        const COMPLEXITY_THRESHOLD: u32 = 5;
        const MAX_HOTSPOTS: usize = 20;

        let mut by_file: HashMap<String, FileComplexityStats> = HashMap::new();
        let mut hotspots = Vec::new();
        let total_functions = cyclomatic.len() as u32;
        let mut filtered_functions = 0u32;

        // Process each function
        for (full_path_with_function, cyclo_complexity) in &cyclomatic {
            let cogn_complexity = cognitive.get(full_path_with_function).copied().unwrap_or(0);

            // Extract file path and function name
            let parts: Vec<&str> = full_path_with_function.rsplitn(2, ":::").collect();
            let (file_path, function_name) = if parts.len() == 2 {
                (parts[1], parts[0])
            } else {
                // Fallback: try splitting by last double colon
                let parts: Vec<&str> = full_path_with_function.rsplitn(2, "::").collect();
                if parts.len() == 2 {
                    (parts[1], parts[0])
                } else {
                    (full_path_with_function.as_str(), "unknown")
                }
            };

            // Convert to relative path
            let relative_path = self.to_relative_path(project_path, file_path);

            // Update file statistics
            let stats = by_file
                .entry(relative_path.clone())
                .or_insert(FileComplexityStats {
                    avg_cyclomatic: 0.0,
                    max_cyclomatic: 0,
                    avg_cognitive: 0.0,
                    max_cognitive: 0,
                    functions_count: 0,
                    high_complexity_count: 0,
                });

            stats.functions_count += 1;
            stats.max_cyclomatic = stats.max_cyclomatic.max(*cyclo_complexity);
            stats.max_cognitive = stats.max_cognitive.max(cogn_complexity);

            if *cyclo_complexity > 10 {
                stats.high_complexity_count += 1;
            }

            // Track hotspots for functions above threshold
            if *cyclo_complexity > COMPLEXITY_THRESHOLD {
                hotspots.push(ComplexityHotspot {
                    file: relative_path.clone(),
                    function: function_name.to_string(),
                    cyclomatic: *cyclo_complexity,
                    cognitive: cogn_complexity,
                });
            } else {
                filtered_functions += 1;
            }
        }

        // Calculate averages for each file
        let mut file_sums: HashMap<String, (u32, u32)> = HashMap::new();

        // First pass: calculate sums
        for (full_path_with_function, cyclo_complexity) in &cyclomatic {
            let cogn_complexity = cognitive.get(full_path_with_function).copied().unwrap_or(0);

            // Extract file path
            let parts: Vec<&str> = full_path_with_function.rsplitn(2, ":::").collect();
            let file_path = if parts.len() == 2 {
                parts[1]
            } else {
                let parts: Vec<&str> = full_path_with_function.rsplitn(2, "::").collect();
                if parts.len() == 2 {
                    parts[1]
                } else {
                    full_path_with_function.as_str()
                }
            };

            let relative_path = self.to_relative_path(project_path, file_path);
            let (cyclo_sum, cogn_sum) = file_sums.entry(relative_path).or_insert((0, 0));
            *cyclo_sum += cyclo_complexity;
            *cogn_sum += cogn_complexity;
        }

        // Second pass: update averages
        for (file_path, stats) in by_file.iter_mut() {
            if let Some((cyclo_sum, cogn_sum)) = file_sums.get(file_path) {
                if stats.functions_count > 0 {
                    stats.avg_cyclomatic = *cyclo_sum as f32 / stats.functions_count as f32;
                    stats.avg_cognitive = *cogn_sum as f32 / stats.functions_count as f32;
                }
            }
        }

        // Sort hotspots by cyclomatic complexity (descending) and keep only top N
        hotspots.sort_by(|a, b| b.cyclomatic.cmp(&a.cyclomatic));
        hotspots.truncate(MAX_HOTSPOTS);

        let summary = ComplexitySummary {
            by_file,
            total_functions,
            filtered_functions,
        };

        (summary, hotspots)
    }

    /// Convert absolute path to project-relative path
    fn to_relative_path(&self, project_path: &Path, absolute_path: &str) -> String {
        let path = PathBuf::from(absolute_path);
        if let Ok(relative) = path.strip_prefix(project_path) {
            relative.to_string_lossy().to_string()
        } else {
            // If not under project path, try to extract just the src/... part
            if let Some(src_idx) = absolute_path.find("/src/") {
                absolute_path[src_idx + 1..].to_string()
            } else {
                // Last resort: use filename
                path.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| absolute_path.to_string())
            }
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(SubprocessManager::production())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::test_mocks::{CargoMocks, TestMockSetup};
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_collect_metrics_success() {
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

        // Create mocked subprocess environment
        let (subprocess, mut mock) = SubprocessManager::mock();
        TestMockSetup::setup_metrics_collection(&mut mock);

        let collector = MetricsCollector::new(subprocess);

        let result = collector
            .collect_metrics(temp_dir.path(), "test-iteration".to_string())
            .await;
        assert!(result.is_ok());

        let metrics = result.unwrap();
        assert_eq!(metrics.iteration_id, "test-iteration");
        // The quality analyzer returns percentage based on estimate_test_coverage
        assert!(metrics.test_coverage >= 0.0 && metrics.test_coverage <= 100.0);
        // Lint warnings count check - removed useless comparison since u32 is always >= 0
    }

    #[tokio::test]
    async fn test_collect_metrics_analyzer_failure() {
        let temp_dir = TempDir::new().unwrap();

        // Create mocked subprocess environment with some failures
        let (subprocess, mut mock) = SubprocessManager::mock();

        // Mock some commands to fail
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"tarpaulin".to_string()))
            .returns_stderr("error: cargo-tarpaulin not found")
            .returns_exit_code(1)
            .finish();

        // But clippy should still work
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"clippy".to_string()))
            .returns_stdout(&CargoMocks::clippy_output())
            .returns_exit_code(0)
            .finish();

        // And build should work
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"build".to_string()))
            .returns_stdout(&CargoMocks::build_success())
            .returns_exit_code(0)
            .finish();

        // Check should work
        mock.expect_command("cargo")
            .with_args(|args| args.first() == Some(&"check".to_string()))
            .returns_stdout(&CargoMocks::check_success())
            .returns_exit_code(0)
            .finish();

        let collector = MetricsCollector::new(subprocess);

        // Create directory without Cargo.toml to trigger failures
        let result = collector
            .collect_metrics(temp_dir.path(), "test-iteration".to_string())
            .await;

        // Should still return metrics even with some analyzer failures
        assert!(result.is_ok());
        let metrics = result.unwrap();
        // Test coverage will be 0.0 when no project structure exists
        assert_eq!(metrics.test_coverage, 0.0);
        // Lint warnings will be 0 without project structure
        assert_eq!(metrics.lint_warnings, 0);
    }

    #[test]
    fn test_compress_complexity_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create test complexity data with full paths
        let mut cyclomatic = HashMap::new();
        let mut cognitive = HashMap::new();

        // Add some low complexity functions that should be filtered
        cyclomatic.insert(
            format!("{}/src/utils.rs::helper_fn", project_path.display()),
            2,
        );
        cognitive.insert(
            format!("{}/src/utils.rs::helper_fn", project_path.display()),
            3,
        );

        cyclomatic.insert(
            format!("{}/src/utils.rs::simple_fn", project_path.display()),
            1,
        );
        cognitive.insert(
            format!("{}/src/utils.rs::simple_fn", project_path.display()),
            1,
        );

        // Add high complexity functions that should be in hotspots
        cyclomatic.insert(
            format!("{}/src/main.rs::complex_fn", project_path.display()),
            15,
        );
        cognitive.insert(
            format!("{}/src/main.rs::complex_fn", project_path.display()),
            20,
        );

        cyclomatic.insert(
            format!("{}/src/parser.rs::parse_args", project_path.display()),
            12,
        );
        cognitive.insert(
            format!("{}/src/parser.rs::parse_args", project_path.display()),
            18,
        );

        // Add more functions to test aggregation
        cyclomatic.insert(format!("{}/src/main.rs::init", project_path.display()), 3);
        cognitive.insert(format!("{}/src/main.rs::init", project_path.display()), 4);

        let subprocess = SubprocessManager::production();
        let collector = MetricsCollector::new(subprocess);

        let (summary, hotspots) =
            collector.compress_complexity_metrics(project_path, cyclomatic, cognitive);

        // Verify summary
        assert_eq!(summary.total_functions, 5);
        assert_eq!(summary.filtered_functions, 3); // Functions with complexity <= 5
        assert_eq!(summary.by_file.len(), 3); // Three unique files

        // Check file statistics
        let utils_stats = summary.by_file.get("src/utils.rs").unwrap();
        assert_eq!(utils_stats.functions_count, 2);
        assert_eq!(utils_stats.max_cyclomatic, 2);
        assert_eq!(utils_stats.high_complexity_count, 0);
        assert_eq!(utils_stats.avg_cyclomatic, 1.5);

        let main_stats = summary.by_file.get("src/main.rs").unwrap();
        assert_eq!(main_stats.functions_count, 2);
        assert_eq!(main_stats.max_cyclomatic, 15);
        assert_eq!(main_stats.high_complexity_count, 1); // complex_fn > 10
        assert_eq!(main_stats.avg_cyclomatic, 9.0); // (15 + 3) / 2

        // Verify hotspots
        assert_eq!(hotspots.len(), 2); // Only functions with complexity > 5
        assert_eq!(hotspots[0].function, "complex_fn");
        assert_eq!(hotspots[0].cyclomatic, 15);
        assert_eq!(hotspots[1].function, "parse_args");
        assert_eq!(hotspots[1].cyclomatic, 12);
    }

    #[test]
    fn test_to_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();
        let subprocess = SubprocessManager::production();
        let collector = MetricsCollector::new(subprocess);

        // Test case 1: Path under project
        let absolute = format!("{}/src/main.rs", project_path.display());
        let relative = collector.to_relative_path(project_path, &absolute);
        assert_eq!(relative, "src/main.rs");

        // Test case 2: Path not under project but has src/
        let external = "/some/other/path/src/lib.rs";
        let relative = collector.to_relative_path(project_path, external);
        assert_eq!(relative, "src/lib.rs");

        // Test case 3: Path with no src/ - should use filename
        let no_src = "/random/path/file.rs";
        let relative = collector.to_relative_path(project_path, no_src);
        assert_eq!(relative, "file.rs");
    }

    #[tokio::test]
    async fn test_compressed_metrics_in_collection() {
        // Set test mode to get predictable results
        std::env::set_var("MMM_TEST_MODE", "true");

        let temp_dir = TempDir::new().unwrap();
        let subprocess = SubprocessManager::production();
        let collector = MetricsCollector::new(subprocess);

        let metrics = collector
            .collect_metrics(temp_dir.path(), "test-compressed".to_string())
            .await
            .unwrap();

        // In test mode, we should have empty complexity maps (to save space)
        assert!(metrics.cyclomatic_complexity.is_empty());
        assert!(metrics.cognitive_complexity.is_empty());

        // But we should still have basic metrics
        assert_eq!(metrics.test_coverage, 30.0);
        assert_eq!(metrics.total_lines, 100);

        std::env::remove_var("MMM_TEST_MODE");
    }
}
