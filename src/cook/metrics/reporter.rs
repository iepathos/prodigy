//! Metrics reporting implementation

use super::ProjectMetrics;
use anyhow::Result;
use async_trait::async_trait;

/// Trait for generating metrics reports
#[async_trait]
pub trait MetricsReporter: Send + Sync {
    /// Generate a text report
    async fn generate_report(
        &self,
        current: &ProjectMetrics,
        history: &[ProjectMetrics],
    ) -> Result<String>;

    /// Generate a JSON report
    async fn generate_json_report(
        &self,
        current: &ProjectMetrics,
        history: &[ProjectMetrics],
    ) -> Result<serde_json::Value>;

    /// Calculate trends
    async fn calculate_trends(&self, history: &[ProjectMetrics]) -> Result<MetricsTrends>;
}

/// Trends in metrics over time
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsTrends {
    pub coverage_trend: TrendDirection,
    pub complexity_trend: TrendDirection,
    pub performance_trend: TrendDirection,
    pub quality_trend: TrendDirection,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TrendDirection {
    Improving(f64),
    Stable,
    Degrading(f64),
    Unknown,
}

/// Implementation of metrics reporter
pub struct MetricsReporterImpl;

impl MetricsReporterImpl {
    pub fn new() -> Self {
        Self
    }

    fn format_metric(value: Option<f64>, suffix: &str) -> String {
        match value {
            Some(v) => format!("{:.1}{}", v, suffix),
            None => "N/A".to_string(),
        }
    }

    fn calculate_trend(current: Option<f64>, previous: Option<f64>) -> TrendDirection {
        match (current, previous) {
            (Some(curr), Some(prev)) => {
                let change = ((curr - prev) / prev) * 100.0;
                if change.abs() < 1.0 {
                    TrendDirection::Stable
                } else if change > 0.0 {
                    TrendDirection::Improving(change)
                } else {
                    TrendDirection::Degrading(change.abs())
                }
            }
            _ => TrendDirection::Unknown,
        }
    }
}

#[async_trait]
impl MetricsReporter for MetricsReporterImpl {
    async fn generate_report(
        &self,
        current: &ProjectMetrics,
        history: &[ProjectMetrics],
    ) -> Result<String> {
        let mut report = String::new();

        report.push_str("📊 Metrics Report\n");
        report.push_str("================\n\n");

        // Current metrics
        report.push_str("Current Metrics:\n");
        report.push_str(&format!(
            "- Test Coverage: {}\n",
            Self::format_metric(current.test_coverage, "%")
        ));
        report.push_str(&format!(
            "- Type Coverage: {}\n",
            Self::format_metric(current.type_coverage, "%")
        ));
        report.push_str(&format!("- Lint Warnings: {}\n", current.lint_warnings));
        report.push_str(&format!(
            "- Code Duplication: {}\n",
            Self::format_metric(current.code_duplication, "%")
        ));
        report.push_str(&format!(
            "- Doc Coverage: {}\n",
            Self::format_metric(current.doc_coverage, "%")
        ));
        report.push_str(&format!(
            "- Compile Time: {}\n",
            Self::format_metric(current.compile_time, "s")
        ));

        if let Some(size) = current.binary_size {
            report.push_str(&format!(
                "- Binary Size: {:.2} MB\n",
                size as f64 / 1_048_576.0
            ));
        } else {
            report.push_str("- Binary Size: N/A\n");
        }

        // Trends
        if !history.is_empty() {
            let trends = self.calculate_trends(history).await?;
            report.push_str("\nTrends:\n");
            report.push_str(&format!("- Coverage: {:?}\n", trends.coverage_trend));
            report.push_str(&format!("- Complexity: {:?}\n", trends.complexity_trend));
            report.push_str(&format!("- Performance: {:?}\n", trends.performance_trend));
            report.push_str(&format!("- Quality: {:?}\n", trends.quality_trend));
        }

        Ok(report)
    }

    async fn generate_json_report(
        &self,
        current: &ProjectMetrics,
        history: &[ProjectMetrics],
    ) -> Result<serde_json::Value> {
        let trends = if history.is_empty() {
            None
        } else {
            Some(self.calculate_trends(history).await?)
        };

        Ok(serde_json::json!({
            "current": current,
            "history_count": history.len(),
            "trends": trends,
            "timestamp": current.timestamp,
        }))
    }

    async fn calculate_trends(&self, history: &[ProjectMetrics]) -> Result<MetricsTrends> {
        if history.len() < 2 {
            return Ok(MetricsTrends {
                coverage_trend: TrendDirection::Unknown,
                complexity_trend: TrendDirection::Unknown,
                performance_trend: TrendDirection::Unknown,
                quality_trend: TrendDirection::Unknown,
            });
        }

        let current = &history[history.len() - 1];
        let previous = &history[history.len() - 2];

        // Coverage trend (higher is better)
        let coverage_trend = Self::calculate_trend(current.test_coverage, previous.test_coverage);

        // Performance trend (lower compile time is better)
        let performance_trend = match (current.compile_time, previous.compile_time) {
            (Some(curr), Some(prev)) => {
                let change = ((prev - curr) / prev) * 100.0; // Inverted for performance
                if change.abs() < 1.0 {
                    TrendDirection::Stable
                } else if change > 0.0 {
                    TrendDirection::Improving(change)
                } else {
                    TrendDirection::Degrading(change.abs())
                }
            }
            _ => TrendDirection::Unknown,
        };

        // Quality trend (fewer warnings is better)
        let quality_trend = if current.lint_warnings < previous.lint_warnings {
            TrendDirection::Improving(
                ((previous.lint_warnings - current.lint_warnings) as f64
                    / previous.lint_warnings as f64)
                    * 100.0,
            )
        } else if current.lint_warnings > previous.lint_warnings {
            TrendDirection::Degrading(
                ((current.lint_warnings - previous.lint_warnings) as f64
                    / previous.lint_warnings as f64)
                    * 100.0,
            )
        } else {
            TrendDirection::Stable
        };

        Ok(MetricsTrends {
            coverage_trend,
            complexity_trend: TrendDirection::Unknown, // TODO: Implement when complexity is available
            performance_trend,
            quality_trend,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_metrics(
        coverage: Option<f64>,
        warnings: usize,
        compile_time: Option<f64>,
    ) -> ProjectMetrics {
        ProjectMetrics {
            test_coverage: coverage,
            lint_warnings: warnings,
            compile_time,
            timestamp: Utc::now(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_report_generation() {
        let reporter = MetricsReporterImpl::new();
        let current = create_test_metrics(Some(75.5), 10, Some(12.3));
        let history = vec![];

        let report = reporter.generate_report(&current, &history).await.unwrap();

        assert!(report.contains("Test Coverage: 75.5%"));
        assert!(report.contains("Lint Warnings: 10"));
        assert!(report.contains("Compile Time: 12.3s"));
    }

    #[tokio::test]
    async fn test_trend_calculation() {
        let reporter = MetricsReporterImpl::new();
        let history = vec![
            create_test_metrics(Some(70.0), 20, Some(15.0)),
            create_test_metrics(Some(75.0), 15, Some(12.0)),
        ];

        let trends = reporter.calculate_trends(&history).await.unwrap();

        match trends.coverage_trend {
            TrendDirection::Improving(change) => assert!(change > 0.0),
            _ => panic!("Expected improving coverage trend"),
        }

        match trends.performance_trend {
            TrendDirection::Improving(change) => assert!(change > 0.0),
            _ => panic!("Expected improving performance trend"),
        }

        match trends.quality_trend {
            TrendDirection::Improving(change) => assert!(change > 0.0),
            _ => panic!("Expected improving quality trend"),
        }
    }

    #[tokio::test]
    async fn test_json_report() {
        let reporter = MetricsReporterImpl::new();
        let current = create_test_metrics(Some(80.0), 5, Some(10.0));
        let history = vec![current.clone()];

        let json_report = reporter
            .generate_json_report(&current, &history)
            .await
            .unwrap();

        assert_eq!(json_report["history_count"], 1);
        assert_eq!(json_report["current"]["test_coverage"], 80.0);
    }
}
