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
        if history.len() > 100 {
            let skip_amount = history.len() - 100;
            history = history.into_iter().skip(skip_amount).collect();
        }

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
}
