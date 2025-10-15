//! Health and metrics tracking for cook orchestrator
//!
//! Handles health score calculation, test coverage analysis, and code quality metrics.

use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::core::CookConfig;
use anyhow::Result;
use std::sync::Arc;

/// Health metrics manager for code quality tracking
pub struct HealthMetrics {
    user_interaction: Arc<dyn UserInteraction>,
}

impl HealthMetrics {
    /// Create a new HealthMetrics instance
    pub fn new(user_interaction: Arc<dyn UserInteraction>) -> Self {
        Self { user_interaction }
    }

    /// Display health score for the project
    pub async fn display_health_score(&self, config: &CookConfig) -> Result<()> {
        use crate::scoring::{display_health_score, BasicMetrics, ProjectHealthScore};

        // Create basic metrics from available data
        // For now, we'll use placeholder values - in a real implementation,
        // these would be collected from the actual project analysis
        let metrics = BasicMetrics {
            test_coverage: Self::get_test_coverage(&config.project_path).await.ok(),
            lint_warnings: Self::get_lint_warnings(&config.project_path).await.ok(),
            code_duplication: Self::get_code_duplication(&config.project_path).await.ok(),
            doc_coverage: None,       // Not readily available
            type_coverage: None,      // Not readily available
            complexity_average: None, // Not readily available
        };

        // Calculate and display the score
        let score = ProjectHealthScore::from_metrics(&metrics);
        let score_display = display_health_score(&score);

        self.user_interaction.display_info(&score_display);

        Ok(())
    }

    /// Get test coverage from the project (pure function)
    pub async fn get_test_coverage(project_path: &std::path::Path) -> Result<f64> {
        // Try to read coverage from common locations
        let coverage_file = project_path.join("target/coverage/info.lcov");
        if coverage_file.exists() {
            // Parse LCOV file (simplified - real implementation would be more thorough)
            let content = tokio::fs::read_to_string(&coverage_file).await?;
            let mut lines_hit = 0;
            let mut lines_total = 0;

            for line in content.lines() {
                if line.starts_with("LH:") {
                    lines_hit += line.trim_start_matches("LH:").parse::<i32>().unwrap_or(0);
                } else if line.starts_with("LF:") {
                    lines_total += line.trim_start_matches("LF:").parse::<i32>().unwrap_or(0);
                }
            }

            if lines_total > 0 {
                return Ok((lines_hit as f64 / lines_total as f64) * 100.0);
            }
        }

        // Default to 0 if no coverage data
        Ok(0.0)
    }

    /// Get lint warnings count (pure function)
    pub async fn get_lint_warnings(project_path: &std::path::Path) -> Result<u32> {
        // Try to run clippy and count warnings (simplified)
        let output = std::process::Command::new("cargo")
            .arg("clippy")
            .arg("--message-format=json")
            .current_dir(project_path)
            .output()?;

        let mut warning_count = 0;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) {
                if msg.get("level").and_then(|l| l.as_str()) == Some("warning") {
                    warning_count += 1;
                }
            }
        }

        Ok(warning_count)
    }

    /// Get code duplication percentage (pure function)
    pub async fn get_code_duplication(_project_path: &std::path::Path) -> Result<f32> {
        // This would integrate with a tool like rust-code-analysis or similar
        // For now, return a placeholder
        Ok(0.0)
    }
}
