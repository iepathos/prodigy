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
        report.push_str(&format!("═══════════════════════════════════\n\n"));

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
        report.push_str("\n");

        report.push_str("🚀 Performance Metrics:\n");
        report.push_str(&format!(
            "  • Compile Time: {:.1}s\n",
            metrics.compile_time.as_secs_f32()
        ));
        report.push_str(&format!(
            "  • Binary Size: {:.1} MB\n",
            metrics.binary_size as f64 / 1_048_576.0
        ));
        report.push_str("\n");

        report.push_str("🧩 Complexity Metrics:\n");
        let avg_cyclomatic = if !metrics.cyclomatic_complexity.is_empty() {
            metrics.cyclomatic_complexity.values().sum::<u32>() as f32
                / metrics.cyclomatic_complexity.len() as f32
        } else {
            0.0
        };
        report.push_str(&format!(
            "  • Avg Cyclomatic Complexity: {:.1}\n",
            avg_cyclomatic
        ));
        report.push_str(&format!(
            "  • Max Nesting Depth: {}\n",
            metrics.max_nesting_depth
        ));
        report.push_str(&format!("  • Total Lines: {}\n", metrics.total_lines));
        report.push_str("\n");

        report.push_str("🎯 Overall Score: ");
        let score = metrics.overall_score();
        let score_emoji = match score as u32 {
            90..=100 => "🟢",
            70..=89 => "🟡",
            50..=69 => "🟠",
            _ => "🔴",
        };
        report.push_str(&format!("{} {:.1}/100\n", score_emoji, score));

        report
    }

    /// Save a metrics report
    pub fn save_report(&self, report: &str, iteration_id: &str) -> Result<()> {
        self.ensure_directory()?;

        let reports_dir = self.base_path.join("reports");
        std::fs::create_dir_all(&reports_dir)?;

        let report_path = reports_dir.join(format!("report-{}.txt", iteration_id));
        std::fs::write(&report_path, report).context("Failed to write metrics report")?;

        debug!("Saved metrics report to {:?}", report_path);
        Ok(())
    }
}
