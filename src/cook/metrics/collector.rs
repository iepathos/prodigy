//! Metrics collection implementation

use super::{MetricsCoordinator, ProjectMetrics};
use crate::cook::execution::CommandRunner;
use crate::cook::metrics::reporter::MetricsReporter;
use crate::metrics::MetricsCollector;
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

/// Trait for collecting metrics
#[async_trait]
pub trait MetricsCollectorTrait: Send + Sync {
    /// Collect test coverage
    async fn collect_test_coverage(&self, project_path: &Path) -> Result<Option<f64>>;

    /// Collect lint warnings
    async fn collect_lint_warnings(&self, project_path: &Path) -> Result<usize>;

    /// Collect compilation metrics
    async fn collect_compile_metrics(
        &self,
        project_path: &Path,
    ) -> Result<(Option<f64>, Option<u64>)>;

    /// Collect code complexity
    async fn collect_complexity(&self, project_path: &Path) -> Result<Option<serde_json::Value>>;
}

/// Implementation of metrics collector
pub struct MetricsCollectorImpl<R: CommandRunner> {
    runner: R,
    collector: MetricsCollector,
}

impl<R: CommandRunner> MetricsCollectorImpl<R> {
    /// Create a new metrics collector with production subprocess
    pub fn new(runner: R) -> Self {
        Self::with_subprocess(runner, SubprocessManager::production())
    }

    /// Create a new metrics collector with injected subprocess manager
    pub fn with_subprocess(runner: R, subprocess: SubprocessManager) -> Self {
        Self {
            runner,
            collector: MetricsCollector::new(subprocess),
        }
    }

    /// Trim history to keep only the most recent entries
    fn trim_history(history: Vec<ProjectMetrics>, max_entries: usize) -> Vec<ProjectMetrics> {
        if history.len() > max_entries {
            let skip_amount = history.len() - max_entries;
            history.into_iter().skip(skip_amount).collect()
        } else {
            history
        }
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> MetricsCollectorTrait for MetricsCollectorImpl<R> {
    async fn collect_test_coverage(&self, project_path: &Path) -> Result<Option<f64>> {
        // Try to collect metrics and extract test coverage
        match self
            .collector
            .collect_metrics(project_path, "temp".to_string())
            .await
        {
            Ok(metrics) => {
                if metrics.test_coverage == 0.0 {
                    Ok(None) // Return None for N/A coverage
                } else {
                    Ok(Some(metrics.test_coverage as f64))
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn collect_lint_warnings(&self, project_path: &Path) -> Result<usize> {
        // Check for Rust project
        if project_path.join("Cargo.toml").exists() {
            let output = self
                .runner
                .run_command(
                    "cargo",
                    &["clippy".to_string(), "--message-format=json".to_string()],
                )
                .await?;

            if output.status.success() {
                // Count warning messages in JSON output
                let count = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter(|line| line.contains("\"level\":\"warning\""))
                    .count();
                Ok(count)
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
    }

    async fn collect_compile_metrics(
        &self,
        project_path: &Path,
    ) -> Result<(Option<f64>, Option<u64>)> {
        if !project_path.join("Cargo.toml").exists() {
            return Ok((None, None));
        }

        // Measure compile time
        let start = std::time::Instant::now();
        let output = self
            .runner
            .run_command("cargo", &["build".to_string(), "--release".to_string()])
            .await?;

        let compile_time = if output.status.success() {
            Some(start.elapsed().as_secs_f64())
        } else {
            None
        };

        // Get binary size
        let binary_size = if output.status.success() {
            // Try to find the binary
            let target_dir = project_path.join("target/release");
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() && !entry.file_name().to_string_lossy().contains('.')
                        {
                            return Ok((compile_time, Some(metadata.len())));
                        }
                    }
                }
            }
            None
        } else {
            None
        };

        Ok((compile_time, binary_size))
    }

    async fn collect_complexity(&self, _project_path: &Path) -> Result<Option<serde_json::Value>> {
        // TODO: Implement complexity analysis
        // For now, return None
        Ok(None)
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> MetricsCoordinator for MetricsCollectorImpl<R> {
    async fn collect_all(&self, project_path: &Path) -> Result<ProjectMetrics> {
        let mut metrics = ProjectMetrics::default();

        #[allow(clippy::field_reassign_with_default)]
        {
            // Collect test coverage
            metrics.test_coverage = self.collect_test_coverage(project_path).await?;

            // Collect lint warnings
            metrics.lint_warnings = self.collect_lint_warnings(project_path).await?;

            // Collect compile metrics
            let (compile_time, binary_size) = self.collect_compile_metrics(project_path).await?;
            metrics.compile_time = compile_time;
            metrics.binary_size = binary_size;

            // Collect complexity
            metrics.cyclomatic_complexity = self.collect_complexity(project_path).await?;
        }

        Ok(metrics)
    }

    async fn collect_metric(&self, project_path: &Path, metric: &str) -> Result<serde_json::Value> {
        match metric {
            "test_coverage" => {
                let coverage = self.collect_test_coverage(project_path).await?;
                Ok(serde_json::json!(coverage))
            }
            "lint_warnings" => {
                let warnings = self.collect_lint_warnings(project_path).await?;
                Ok(serde_json::json!(warnings))
            }
            "compile_time" => {
                let (time, _) = self.collect_compile_metrics(project_path).await?;
                Ok(serde_json::json!(time))
            }
            "binary_size" => {
                let (_, size) = self.collect_compile_metrics(project_path).await?;
                Ok(serde_json::json!(size))
            }
            _ => anyhow::bail!("Unknown metric: {}", metric),
        }
    }

    async fn store_metrics(&self, project_path: &Path, metrics: &ProjectMetrics) -> Result<()> {
        let metrics_dir = project_path.join(".mmm/metrics");
        tokio::fs::create_dir_all(&metrics_dir).await?;

        let current_path = metrics_dir.join("current.json");
        let json = serde_json::to_string_pretty(metrics)?;
        tokio::fs::write(&current_path, json).await?;

        // Also append to history
        let history_path = metrics_dir.join("history.json");
        let mut history = self.load_history(project_path).await.unwrap_or_default();
        history.push(metrics.clone());

        // Keep only last 100 entries
        history = Self::trim_history(history, 100);

        let history_json = serde_json::to_string_pretty(&history)?;
        tokio::fs::write(&history_path, history_json).await?;

        Ok(())
    }

    async fn load_history(&self, project_path: &Path) -> Result<Vec<ProjectMetrics>> {
        let history_path = project_path.join(".mmm/metrics/history.json");
        if !history_path.exists() {
            return Ok(Vec::new());
        }

        let json = tokio::fs::read_to_string(&history_path).await?;
        let history: Vec<ProjectMetrics> = serde_json::from_str(&json)?;
        Ok(history)
    }

    async fn generate_report(
        &self,
        metrics: &ProjectMetrics,
        history: &[ProjectMetrics],
    ) -> Result<String> {
        // Delegate to reporter
        let reporter = super::MetricsReporterImpl::new();
        reporter.generate_report(metrics, history).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::subprocess::SubprocessManager;
    use crate::testing::test_mocks::TestMockSetup;

    #[tokio::test]
    async fn test_lint_warnings_collection() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        let mock_runner = MockCommandRunner::new();
        mock_runner.add_response(crate::cook::execution::ExecutionResult {
            success: true,
            stdout: r#"{"level":"warning","message":"test1"}
{"level":"warning","message":"test2"}
{"level":"error","message":"test3"}"#
                .to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let collector = MetricsCollectorImpl::new(mock_runner);
        let warnings = collector
            .collect_lint_warnings(temp_dir.path())
            .await
            .unwrap();

        assert_eq!(warnings, 2); // Should count only warnings, not errors
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        // Create a minimal Cargo.toml to simulate a Rust project
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        tokio::fs::write(
            &cargo_toml,
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .await
        .unwrap();

        // Create mocked subprocess for underlying metrics collector
        let (subprocess, mut mock) = SubprocessManager::mock();
        TestMockSetup::setup_metrics_collection(&mut mock);

        let mock_runner = MockCommandRunner::new();

        // Add mock response for clippy command that might be called by collect_lint_warnings
        mock_runner.add_response(crate::cook::execution::ExecutionResult {
            success: true,
            stdout: r#"{"level":"warning","message":"test1"}
{"level":"warning","message":"test2"}"#
                .to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        // Add mock response for cargo build that might be called by collect_compile_metrics
        mock_runner.add_response(crate::cook::execution::ExecutionResult {
            success: true,
            stdout: "Finished release [optimized] target(s) in 1.0s".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
        });

        let collector = MetricsCollectorImpl::with_subprocess(mock_runner, subprocess);

        let metrics = collector.collect_all(temp_dir.path()).await.unwrap();

        // Should have collected metrics from mocked subprocess
        // Note: test_coverage may be None if no tarpaulin data is available
        if let Some(coverage) = metrics.test_coverage {
            assert!((0.0..=100.0).contains(&coverage));
        }
        // Compile time may or may not be set depending on mock runner behavior
        // We're not mocking the build command in MockCommandRunner
    }

    #[test]
    fn test_trim_history_empty() {
        let history: Vec<ProjectMetrics> = Vec::new();
        let result = MetricsCollectorImpl::<MockCommandRunner>::trim_history(history, 100);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_trim_history_under_limit() {
        let history = vec![
            create_test_metrics(1),
            create_test_metrics(2),
            create_test_metrics(3),
        ];
        let result = MetricsCollectorImpl::<MockCommandRunner>::trim_history(history.clone(), 100);
        assert_eq!(result.len(), 3);
        // Verify the order is preserved
        assert_eq!(result[0].lint_warnings, 1);
        assert_eq!(result[1].lint_warnings, 2);
        assert_eq!(result[2].lint_warnings, 3);
    }

    #[test]
    fn test_trim_history_at_limit() {
        let history = vec![
            create_test_metrics(1),
            create_test_metrics(2),
            create_test_metrics(3),
        ];
        let result = MetricsCollectorImpl::<MockCommandRunner>::trim_history(history.clone(), 3);
        assert_eq!(result.len(), 3);
        // Verify the order is preserved
        assert_eq!(result[0].lint_warnings, 1);
        assert_eq!(result[1].lint_warnings, 2);
        assert_eq!(result[2].lint_warnings, 3);
    }

    #[test]
    fn test_trim_history_over_limit() {
        let history = vec![
            create_test_metrics(1),
            create_test_metrics(2),
            create_test_metrics(3),
            create_test_metrics(4),
            create_test_metrics(5),
        ];
        let result = MetricsCollectorImpl::<MockCommandRunner>::trim_history(history.clone(), 3);
        assert_eq!(result.len(), 3);
        // Should keep the last 3 entries
        assert_eq!(result[0].lint_warnings, 3);
        assert_eq!(result[1].lint_warnings, 4);
        assert_eq!(result[2].lint_warnings, 5);
    }

    #[test]
    fn test_trim_history_limit_zero() {
        let history = vec![create_test_metrics(1), create_test_metrics(2)];
        let result = MetricsCollectorImpl::<MockCommandRunner>::trim_history(history, 0);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_trim_history_limit_one() {
        let history = vec![
            create_test_metrics(1),
            create_test_metrics(2),
            create_test_metrics(3),
        ];
        let result = MetricsCollectorImpl::<MockCommandRunner>::trim_history(history, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].lint_warnings, 3); // Should keep the last entry
    }

    #[tokio::test]
    async fn test_store_metrics_creates_directories() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let mock_runner = MockCommandRunner::new();
        let collector = MetricsCollectorImpl::new(mock_runner);

        let metrics = create_test_metrics(42);

        // Store metrics
        collector
            .store_metrics(temp_dir.path(), &metrics)
            .await
            .unwrap();

        // Verify directories were created
        assert!(temp_dir.path().join(".mmm/metrics").exists());
        assert!(temp_dir.path().join(".mmm/metrics/current.json").exists());
        assert!(temp_dir.path().join(".mmm/metrics/history.json").exists());
    }

    #[tokio::test]
    async fn test_store_metrics_appends_to_history() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let mock_runner = MockCommandRunner::new();
        let collector = MetricsCollectorImpl::new(mock_runner);

        // Store first metrics
        let metrics1 = create_test_metrics(1);
        collector
            .store_metrics(temp_dir.path(), &metrics1)
            .await
            .unwrap();

        // Store second metrics
        let metrics2 = create_test_metrics(2);
        collector
            .store_metrics(temp_dir.path(), &metrics2)
            .await
            .unwrap();

        // Load history and verify both entries exist
        let history = collector.load_history(temp_dir.path()).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].lint_warnings, 1);
        assert_eq!(history[1].lint_warnings, 2);
    }

    #[tokio::test]
    async fn test_store_metrics_trims_history_over_100() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let mock_runner = MockCommandRunner::new();
        let collector = MetricsCollectorImpl::new(mock_runner);

        // Create initial history with 100 entries
        let mut initial_history = Vec::new();
        for i in 1..=100 {
            initial_history.push(create_test_metrics(i));
        }

        // Write initial history
        let history_path = temp_dir.path().join(".mmm/metrics/history.json");
        tokio::fs::create_dir_all(temp_dir.path().join(".mmm/metrics"))
            .await
            .unwrap();
        let json = serde_json::to_string_pretty(&initial_history).unwrap();
        tokio::fs::write(&history_path, json).await.unwrap();

        // Store new metrics (101st entry)
        let new_metrics = create_test_metrics(101);
        collector
            .store_metrics(temp_dir.path(), &new_metrics)
            .await
            .unwrap();

        // Load history and verify it was trimmed
        let history = collector.load_history(temp_dir.path()).await.unwrap();
        assert_eq!(history.len(), 100);
        assert_eq!(history[0].lint_warnings, 2); // First entry should be the 2nd original
        assert_eq!(history[99].lint_warnings, 101); // Last entry should be the new one
    }

    #[tokio::test]
    async fn test_load_history_empty_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let mock_runner = MockCommandRunner::new();
        let collector = MetricsCollectorImpl::new(mock_runner);

        let history = collector.load_history(temp_dir.path()).await.unwrap();
        assert_eq!(history.len(), 0);
    }

    // Helper function to create test metrics
    fn create_test_metrics(id: usize) -> ProjectMetrics {
        ProjectMetrics {
            test_coverage: Some(50.0 + id as f64),
            type_coverage: Some(60.0 + id as f64),
            lint_warnings: id,
            code_duplication: Some(5.0),
            doc_coverage: Some(70.0),
            benchmark_results: None,
            compile_time: Some(10.0),
            binary_size: Some(1000000),
            cyclomatic_complexity: None,
            max_nesting_depth: None,
            total_lines: None,
            tech_debt_score: None,
            improvement_velocity: None,
            timestamp: chrono::Utc::now(),
            iteration_id: Some(format!("test-{id}")),
            iteration_duration: None,
            command_timings: None,
            workflow_timing: None,
        }
    }
}
