//! Optimized hybrid coverage report without duplication

use super::hybrid_coverage::{
    CriticalFile, HybridCoverageReport, PriorityCoverageGap, QualityCorrelation,
};
use serde::{Deserialize, Serialize};

/// Optimized hybrid coverage without duplicating test coverage data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizedHybridCoverage {
    /// Reference to test coverage file
    pub coverage_reference: String,

    /// Only top priority gaps (limit to 20)
    pub priority_gaps: Vec<PriorityCoverageGap>,

    /// Quality correlation summary
    pub quality_correlation: QualityCorrelationSummary,

    /// Only most critical files (limit to 10)
    pub critical_files: Vec<CriticalFile>,

    /// Overall hybrid score
    pub hybrid_score: f64,

    /// Key insights from the analysis
    pub insights: Vec<String>,
}

/// Summarized quality correlation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityCorrelationSummary {
    pub correlation_coefficient: f64,
    pub key_findings: Vec<String>,
}

impl OptimizedHybridCoverage {
    /// Create optimized version from full report
    pub fn from_report(report: &HybridCoverageReport) -> Self {
        // Take only top 20 priority gaps
        let mut priority_gaps = report.priority_gaps.clone();
        priority_gaps.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());
        priority_gaps.truncate(20);

        // Take only top 10 critical files
        let mut critical_files = report.critical_files.clone();
        critical_files.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap());
        critical_files.truncate(10);

        // Generate quality correlation summary
        let quality_correlation = QualityCorrelationSummary {
            correlation_coefficient: report.quality_correlation.correlation_coefficient,
            key_findings: Self::extract_key_findings(&report.quality_correlation),
        };

        // Generate insights
        let insights = Self::generate_insights(report, &priority_gaps, &critical_files);

        Self {
            coverage_reference: "test_coverage.json".to_string(),
            priority_gaps,
            quality_correlation,
            critical_files,
            hybrid_score: report.hybrid_score,
            insights,
        }
    }

    /// Extract key findings from correlation data
    fn extract_key_findings(correlation: &QualityCorrelation) -> Vec<String> {
        let mut findings = Vec::new();

        if correlation.correlation_coefficient > 0.7 {
            findings.push(
                "Strong positive correlation between test coverage and code quality".to_string(),
            );
        } else if correlation.correlation_coefficient < -0.3 {
            findings.push(
                "Negative correlation suggests tests may not be improving quality".to_string(),
            );
        }

        if !correlation.positive_correlations.is_empty() {
            findings.push(format!(
                "{} files showed quality improvements after coverage increase",
                correlation.positive_correlations.len()
            ));
        }

        if !correlation.negative_correlations.is_empty() {
            findings.push(format!(
                "{} files have low coverage correlated with quality issues",
                correlation.negative_correlations.len()
            ));
        }

        findings
    }

    /// Generate actionable insights
    fn generate_insights(
        report: &HybridCoverageReport,
        priority_gaps: &[PriorityCoverageGap],
        critical_files: &[CriticalFile],
    ) -> Vec<String> {
        let mut insights = Vec::new();

        // Coverage insights
        let avg_priority_score = if !priority_gaps.is_empty() {
            priority_gaps.iter().map(|g| g.priority_score).sum::<f64>() / priority_gaps.len() as f64
        } else {
            0.0
        };

        if avg_priority_score > 15.0 {
            insights.push(
                "High-priority coverage gaps correlate with degrading code quality".to_string(),
            );
        }

        // Critical file insights
        let high_risk_count = critical_files.iter().filter(|f| f.risk_score > 7.0).count();
        if high_risk_count > 3 {
            insights.push(format!(
                "{high_risk_count} critical files need immediate test coverage improvement"
            ));
        }

        // Trend insights
        let degrading_complexity = priority_gaps
            .iter()
            .filter(|g| {
                g.quality_metrics.complexity_trend
                    == super::hybrid_coverage::TrendDirection::Degrading
            })
            .count();
        if degrading_complexity > priority_gaps.len() / 3 {
            insights.push(
                "Many untested areas show increasing complexity - prioritize testing".to_string(),
            );
        }

        // Overall score insight
        if report.hybrid_score < 50.0 {
            insights.push(format!(
                "Hybrid score of {:.1} indicates significant quality risks from low coverage",
                report.hybrid_score
            ));
        }

        insights
    }
}
