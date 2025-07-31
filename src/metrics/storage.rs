//! Metrics storage and persistence

use super::{ImprovementMetrics, MetricsHistory};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Manages metrics storage on disk
pub struct MetricsStorage {
    base_path: PathBuf,
}

impl MetricsStorage {
    /// Create a new metrics storage instance
    pub fn new(project_path: &Path) -> Self {
        let base_path = project_path.join(".mmm").join("metrics");
        Self { base_path }
    }

    /// Ensure metrics directory exists
    pub fn ensure_directory(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_path).context("Failed to create metrics directory")?;
        Ok(())
    }

    /// Save current metrics snapshot
    pub fn save_current(&self, metrics: &ImprovementMetrics) -> Result<()> {
        self.ensure_directory()?;

        let current_path = self.base_path.join("current.json");
        let content =
            serde_json::to_string_pretty(metrics).context("Failed to serialize metrics")?;

        std::fs::write(&current_path, content).context("Failed to write current metrics")?;

        debug!("Saved current metrics to {:?}", current_path);
        Ok(())
    }

    /// Load current metrics if exists
    pub fn load_current(&self) -> Result<Option<ImprovementMetrics>> {
        let current_path = self.base_path.join("current.json");

        if !current_path.exists() {
            return Ok(None);
        }

        let content =
            std::fs::read_to_string(&current_path).context("Failed to read current metrics")?;

        let metrics = serde_json::from_str(&content).context("Failed to deserialize metrics")?;

        Ok(Some(metrics))
    }

    /// Save metrics history
    pub fn save_history(&self, history: &MetricsHistory) -> Result<()> {
        self.ensure_directory()?;

        let history_path = self.base_path.join("history.json");
        let content =
            serde_json::to_string_pretty(history).context("Failed to serialize metrics history")?;

        std::fs::write(&history_path, content).context("Failed to write metrics history")?;

        debug!(
            "Saved metrics history with {} snapshots",
            history.snapshots.len()
        );
        Ok(())
    }

    /// Load metrics history
    pub fn load_history(&self) -> Result<MetricsHistory> {
        let history_path = self.base_path.join("history.json");

        if !history_path.exists() {
            debug!("No existing metrics history found");
            return Ok(MetricsHistory::new());
        }

        let content =
            std::fs::read_to_string(&history_path).context("Failed to read metrics history")?;

        let history =
            serde_json::from_str(&content).context("Failed to deserialize metrics history")?;

        Ok(history)
    }

    /// Generate a simple text report
    pub fn generate_report(&self, metrics: &ImprovementMetrics) -> String {
        let mut report = String::new();

        report.push_str(&format!(
            "📊 Metrics Report - Iteration {}\n",
            metrics.iteration_id
        ));
        report.push_str("═══════════════════════════════════\n\n");

        report.push_str("📈 Quality Metrics:\n");
        report.push_str(&format!(
            "  • Test Coverage: {:.1}%\n",
            metrics.test_coverage
        ));
        report.push_str(&format!(
            "  • Type Coverage: {:.1}%\n",
            metrics.type_coverage
        ));
        report.push_str(&format!("  • Doc Coverage: {:.1}%\n", metrics.doc_coverage));
        report.push_str(&format!("  • Lint Warnings: {}\n", metrics.lint_warnings));
        report.push_str(&format!(
            "  • Code Duplication: {:.1}%\n",
            metrics.code_duplication
        ));
        report.push('\n');

        report.push_str("🚀 Performance Metrics:\n");
        report.push_str(&format!(
            "  • Compile Time: {:.1}s\n",
            metrics.compile_time.as_secs_f32()
        ));
        report.push_str(&format!(
            "  • Binary Size: {:.1} MB\n",
            metrics.binary_size as f64 / 1_048_576.0
        ));
        report.push('\n');

        report.push_str("🧩 Complexity Metrics:\n");
        let avg_cyclomatic = if !metrics.cyclomatic_complexity.is_empty() {
            metrics.cyclomatic_complexity.values().sum::<u32>() as f32
                / metrics.cyclomatic_complexity.len() as f32
        } else {
            0.0
        };
        report.push_str(&format!(
            "  • Avg Cyclomatic Complexity: {avg_cyclomatic:.1}\n"
        ));
        report.push_str(&format!(
            "  • Max Nesting Depth: {}\n",
            metrics.max_nesting_depth
        ));
        report.push_str(&format!("  • Total Lines: {}\n", metrics.total_lines));
        report.push('\n');

        report.push_str("🎯 Overall Score: ");
        let score = metrics.overall_score();
        let score_emoji = match score as u32 {
            90..=100 => "🟢",
            70..=89 => "🟡",
            50..=69 => "🟠",
            _ => "🔴",
        };
        report.push_str(&format!("{score_emoji} {score:.1}/100\n"));

        report
    }

    /// Save a metrics report
    pub fn save_report(&self, report: &str, iteration_id: &str) -> Result<()> {
        self.ensure_directory()?;

        let reports_dir = self.base_path.join("reports");
        std::fs::create_dir_all(&reports_dir)?;

        let report_path = reports_dir.join(format!("report-{iteration_id}.txt"));
        std::fs::write(&report_path, report).context("Failed to write metrics report")?;

        debug!("Saved metrics report to {:?}", report_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_metrics_storage_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        assert_eq!(
            storage.base_path,
            temp_dir.path().join(".mmm").join("metrics")
        );
    }

    #[test]
    fn test_ensure_directory() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        assert!(storage.ensure_directory().is_ok());
        assert!(storage.base_path.exists());
    }

    #[test]
    fn test_save_and_load_current_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        let metrics = ImprovementMetrics {
            test_coverage: 75.5,
            type_coverage: 85.0,
            doc_coverage: 60.0,
            lint_warnings: 5,
            code_duplication: 3.2,
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
            bugs_fixed: 0,
            features_added: 0,
            tech_debt_score: 5.0,
            improvement_velocity: 1.2,
        };

        // Save metrics
        assert!(storage.save_current(&metrics).is_ok());

        // Load metrics
        let loaded = storage.load_current().unwrap();
        assert!(loaded.is_some());
        let loaded_metrics = loaded.unwrap();
        assert_eq!(loaded_metrics.test_coverage, 75.5);
        assert_eq!(loaded_metrics.iteration_id, "test-iteration");
    }

    #[test]
    fn test_load_current_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        let result = storage.load_current().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_history() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        let mut history = MetricsHistory::new();
        history.add_snapshot(
            ImprovementMetrics {
                test_coverage: 70.0,
                type_coverage: 80.0,
                doc_coverage: 55.0,
                lint_warnings: 10,
                code_duplication: 5.0,
                compile_time: Duration::from_secs(15),
                binary_size: 2 * 1024 * 1024,
                cyclomatic_complexity: std::collections::HashMap::new(),
                cognitive_complexity: std::collections::HashMap::new(),
                max_nesting_depth: 4,
                total_lines: 1500,
                timestamp: chrono::Utc::now(),
                iteration_id: "history-test".to_string(),
                benchmark_results: std::collections::HashMap::new(),
                memory_usage: std::collections::HashMap::new(),
                bugs_fixed: 0,
                features_added: 0,
                tech_debt_score: 6.0,
                improvement_velocity: 1.0,
            },
            "test-commit-sha".to_string(),
        );

        assert!(storage.save_history(&history).is_ok());

        let loaded_history = storage.load_history().unwrap();
        assert_eq!(loaded_history.snapshots.len(), 1);
        assert_eq!(
            loaded_history.snapshots[0].metrics.iteration_id,
            "history-test"
        );
    }

    #[test]
    fn test_generate_report() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        let mut complexity = std::collections::HashMap::new();
        complexity.insert("main".to_string(), 5);
        complexity.insert("complex_fn".to_string(), 15);

        let metrics = ImprovementMetrics {
            test_coverage: 85.5,
            type_coverage: 90.0,
            doc_coverage: 70.0,
            lint_warnings: 2,
            code_duplication: 1.5,
            compile_time: Duration::from_secs(8),
            binary_size: 512 * 1024,
            cyclomatic_complexity: complexity,
            cognitive_complexity: std::collections::HashMap::new(),
            max_nesting_depth: 2,
            total_lines: 500,
            timestamp: chrono::Utc::now(),
            iteration_id: "report-test".to_string(),
            benchmark_results: std::collections::HashMap::new(),
            memory_usage: std::collections::HashMap::new(),
            bugs_fixed: 0,
            features_added: 0,
            tech_debt_score: 3.0,
            improvement_velocity: 1.5,
        };

        let report = storage.generate_report(&metrics);

        assert!(report.contains("report-test"));
        assert!(report.contains("85.5%"));
        assert!(report.contains("Test Coverage"));
        assert!(report.contains("Avg Cyclomatic Complexity: 10.0"));
        assert!(report.contains("Overall Score"));
    }

    #[test]
    fn test_save_report() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        let report = "Test Report Content\nLine 2";
        let iteration_id = "save-report-test";

        assert!(storage.save_report(report, iteration_id).is_ok());

        let report_path = storage
            .base_path
            .join("reports")
            .join(format!("report-{iteration_id}.txt"));

        assert!(report_path.exists());
        let saved_content = std::fs::read_to_string(report_path).unwrap();
        assert_eq!(saved_content, report);
    }

    #[test]
    fn test_generate_report_empty_complexity() {
        let temp_dir = TempDir::new().unwrap();
        let storage = MetricsStorage::new(temp_dir.path());

        let metrics = ImprovementMetrics {
            test_coverage: 50.0,
            type_coverage: 60.0,
            doc_coverage: 40.0,
            lint_warnings: 20,
            code_duplication: 10.0,
            compile_time: Duration::from_secs(20),
            binary_size: 4 * 1024 * 1024,
            cyclomatic_complexity: std::collections::HashMap::new(), // Empty
            cognitive_complexity: std::collections::HashMap::new(),
            max_nesting_depth: 5,
            total_lines: 2000,
            timestamp: chrono::Utc::now(),
            iteration_id: "empty-complexity".to_string(),
            benchmark_results: std::collections::HashMap::new(),
            memory_usage: std::collections::HashMap::new(),
            bugs_fixed: 0,
            features_added: 0,
            tech_debt_score: 8.0,
            improvement_velocity: 0.5,
        };

        let report = storage.generate_report(&metrics);
        assert!(report.contains("Avg Cyclomatic Complexity: 0.0"));
        assert!(report.contains("🟠")); // Orange emoji for medium-low score (50-69)
    }
}
