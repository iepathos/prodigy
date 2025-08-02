//! Hybrid test coverage tracking that combines test coverage data with metric trends.
//!
//! This module provides a smarter approach to tracking test coverage by:
//! - Combining coverage data with metrics trends
//! - Prioritizing gaps in files with degrading quality metrics
//! - Tracking coverage impact on quality improvements
//! - Providing focused recommendations based on both coverage and metrics

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::test_coverage::TestCoverageMap;
use crate::metrics::MetricsSnapshot;

/// Coverage gap information for hybrid analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub file: PathBuf,
    pub functions: Vec<String>,
    pub coverage_percentage: f64,
    pub risk: String,
}

/// Hybrid coverage analyzer that combines test coverage with metrics data
#[async_trait::async_trait]
pub trait HybridCoverageAnalyzer: Send + Sync {
    /// Analyze coverage with metrics context
    async fn analyze_hybrid_coverage(
        &self,
        project_path: &Path,
        coverage_map: &TestCoverageMap,
        metrics_history: &[MetricsSnapshot],
    ) -> Result<HybridCoverageReport>;

    /// Get priority coverage gaps based on quality degradation
    fn get_priority_gaps(
        &self,
        report: &HybridCoverageReport,
        count: usize,
    ) -> Vec<PriorityCoverageGap>;
}

/// Combined coverage and metrics report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridCoverageReport {
    /// Standard coverage data
    pub coverage_map: TestCoverageMap,

    /// Quality-weighted coverage gaps
    pub priority_gaps: Vec<PriorityCoverageGap>,

    /// Coverage impact on quality metrics
    pub quality_correlation: QualityCorrelation,

    /// Files with both low coverage and degrading metrics
    pub critical_files: Vec<CriticalFile>,

    /// Overall hybrid score (0-100)
    pub hybrid_score: f64,
}

/// Coverage gap with quality context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityCoverageGap {
    /// Original coverage gap
    pub gap: CoverageGap,

    /// Quality metrics for this file
    pub quality_metrics: FileQualityMetrics,

    /// Priority score (higher = more important)
    pub priority_score: f64,

    /// Reason for prioritization
    pub priority_reason: String,
}

/// Quality metrics for a specific file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileQualityMetrics {
    pub file: PathBuf,
    pub complexity_trend: TrendDirection,
    pub lint_warnings_trend: TrendDirection,
    pub duplication_trend: TrendDirection,
    pub recent_changes: u32,
    pub bug_frequency: f64,
}

/// Trend direction for metrics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrendDirection {
    Improving,
    Stable,
    Degrading,
}

/// Correlation between coverage and quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityCorrelation {
    /// Files where coverage increase led to quality improvement
    pub positive_correlations: Vec<CorrelationEntry>,

    /// Files where low coverage correlates with issues
    pub negative_correlations: Vec<CorrelationEntry>,

    /// Overall correlation coefficient
    pub correlation_coefficient: f64,
}

/// Entry showing correlation between coverage and quality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationEntry {
    pub file: PathBuf,
    pub coverage_change: f64,
    pub quality_change: f64,
    pub timeframe: String,
}

/// File with critical coverage issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalFile {
    pub file: PathBuf,
    pub coverage_percentage: f64,
    pub complexity: u32,
    pub lint_warnings: u32,
    pub recent_bugs: u32,
    pub risk_score: f64,
}

/// Basic implementation of hybrid coverage analyzer
pub struct BasicHybridCoverageAnalyzer;

impl BasicHybridCoverageAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Calculate priority score for a coverage gap
    fn calculate_priority_score(&self, gap: &CoverageGap, metrics: &FileQualityMetrics) -> f64 {
        let mut score = 0.0;

        // Base score from coverage gap
        score += (100.0 - gap.coverage_percentage) / 10.0;

        // Boost for degrading trends
        if metrics.complexity_trend == TrendDirection::Degrading {
            score += 3.0;
        }
        if metrics.lint_warnings_trend == TrendDirection::Degrading {
            score += 2.0;
        }
        if metrics.duplication_trend == TrendDirection::Degrading {
            score += 2.0;
        }

        // Boost for recent changes (more changes = higher risk)
        score += (metrics.recent_changes as f64).min(5.0);

        // Boost for bug frequency
        score += metrics.bug_frequency * 5.0;

        score
    }

    /// Determine trend direction from metric values
    #[allow(dead_code)]
    fn calculate_trend(&self, values: &[f64]) -> TrendDirection {
        if values.len() < 2 {
            return TrendDirection::Stable;
        }

        let recent = values[values.len() - 1];
        let previous = values[values.len() - 2];

        if recent > previous * 1.1 {
            TrendDirection::Degrading
        } else if recent < previous * 0.9 {
            TrendDirection::Improving
        } else {
            TrendDirection::Stable
        }
    }

    /// Extract file-specific metrics from history
    fn extract_file_metrics(
        &self,
        file: &Path,
        _metrics_history: &[MetricsSnapshot],
    ) -> FileQualityMetrics {
        // This is a simplified implementation
        // In reality, you'd extract actual metrics from the history
        FileQualityMetrics {
            file: file.to_path_buf(),
            complexity_trend: TrendDirection::Stable,
            lint_warnings_trend: TrendDirection::Stable,
            duplication_trend: TrendDirection::Stable,
            recent_changes: 0,
            bug_frequency: 0.0,
        }
    }

    /// Calculate quality correlation between coverage and metrics
    fn calculate_quality_correlation(
        &self,
        _coverage_map: &TestCoverageMap,
        _metrics_history: &[MetricsSnapshot],
    ) -> QualityCorrelation {
        // Simplified implementation
        QualityCorrelation {
            positive_correlations: vec![],
            negative_correlations: vec![],
            correlation_coefficient: 0.0,
        }
    }

    /// Identify critical files needing immediate attention
    fn identify_critical_files(
        &self,
        coverage_map: &TestCoverageMap,
        _metrics_history: &[MetricsSnapshot],
    ) -> Vec<CriticalFile> {
        let mut critical_files = Vec::new();

        for (file, coverage) in &coverage_map.file_coverage {
            if coverage.coverage_percentage < 50.0 {
                // In a real implementation, extract actual metrics
                let risk_score = (100.0 - coverage.coverage_percentage) / 20.0;

                critical_files.push(CriticalFile {
                    file: file.clone(),
                    coverage_percentage: coverage.coverage_percentage,
                    complexity: 10,   // Placeholder
                    lint_warnings: 5, // Placeholder
                    recent_bugs: 2,   // Placeholder
                    risk_score,
                });
            }
        }

        // Sort by risk score
        critical_files.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap());
        critical_files
    }

    /// Calculate overall hybrid score
    fn calculate_hybrid_score(
        &self,
        coverage_map: &TestCoverageMap,
        priority_gaps: &[PriorityCoverageGap],
        critical_files: &[CriticalFile],
    ) -> f64 {
        // If we have no coverage data at all, return a more informative score
        if coverage_map.file_coverage.is_empty() {
            // No coverage data available - return 50.0 as a neutral score
            return 50.0;
        }
        
        let base_coverage = coverage_map.overall_coverage * 100.0;

        // Penalty for priority gaps
        let gap_penalty = priority_gaps.len() as f64 * 2.0;

        // Penalty for critical files
        let critical_penalty = critical_files.len() as f64 * 5.0;

        (base_coverage - gap_penalty - critical_penalty).clamp(0.0, 100.0)
    }
}

impl Default for BasicHybridCoverageAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HybridCoverageAnalyzer for BasicHybridCoverageAnalyzer {
    async fn analyze_hybrid_coverage(
        &self,
        _project_path: &Path,
        coverage_map: &TestCoverageMap,
        metrics_history: &[MetricsSnapshot],
    ) -> Result<HybridCoverageReport> {
        let mut priority_gaps = Vec::new();

        // Analyze each coverage gap with metrics context
        let critical_gaps = self.extract_critical_gaps(coverage_map);
        for gap in &critical_gaps {
            let metrics = self.extract_file_metrics(&gap.file, metrics_history);
            let priority_score = self.calculate_priority_score(gap, &metrics);

            let priority_reason = self.generate_priority_reason(&metrics);

            priority_gaps.push(PriorityCoverageGap {
                gap: gap.clone(),
                quality_metrics: metrics,
                priority_score,
                priority_reason,
            });
        }

        // Sort by priority score
        priority_gaps.sort_by(|a, b| b.priority_score.partial_cmp(&a.priority_score).unwrap());

        // Calculate correlations
        let quality_correlation = self.calculate_quality_correlation(coverage_map, metrics_history);

        // Identify critical files
        let critical_files = self.identify_critical_files(coverage_map, metrics_history);

        // Calculate hybrid score
        let hybrid_score =
            self.calculate_hybrid_score(coverage_map, &priority_gaps, &critical_files);

        Ok(HybridCoverageReport {
            coverage_map: coverage_map.clone(),
            priority_gaps,
            quality_correlation,
            critical_files,
            hybrid_score,
        })
    }

    fn get_priority_gaps(
        &self,
        report: &HybridCoverageReport,
        count: usize,
    ) -> Vec<PriorityCoverageGap> {
        report.priority_gaps.iter().take(count).cloned().collect()
    }
}

impl BasicHybridCoverageAnalyzer {
    /// Extract critical gaps from coverage map
    fn extract_critical_gaps(&self, coverage_map: &TestCoverageMap) -> Vec<CoverageGap> {
        let mut gaps = Vec::new();

        // Create gaps from files with low coverage
        for (file, coverage) in &coverage_map.file_coverage {
            if coverage.coverage_percentage < 50.0 {
                gaps.push(CoverageGap {
                    file: file.clone(),
                    functions: coverage_map
                        .untested_functions
                        .iter()
                        .filter(|f| f.file == *file)
                        .map(|f| f.name.clone())
                        .collect(),
                    coverage_percentage: coverage.coverage_percentage,
                    risk: if coverage.coverage_percentage < 30.0 {
                        "High"
                    } else {
                        "Medium"
                    }
                    .to_string(),
                });
            }
        }

        gaps
    }

    /// Generate human-readable reason for prioritization
    fn generate_priority_reason(&self, metrics: &FileQualityMetrics) -> String {
        let mut reasons = Vec::new();

        if metrics.complexity_trend == TrendDirection::Degrading {
            reasons.push("increasing complexity");
        }
        if metrics.lint_warnings_trend == TrendDirection::Degrading {
            reasons.push("growing lint warnings");
        }
        if metrics.recent_changes > 5 {
            reasons.push("frequent changes");
        }
        if metrics.bug_frequency > 0.5 {
            reasons.push("high bug frequency");
        }

        if reasons.is_empty() {
            "Low coverage".to_string()
        } else {
            format!("Low coverage with {}", reasons.join(", "))
        }
    }
}

/// Integration with existing analysis system
impl HybridCoverageReport {
    /// Get actionable recommendations
    pub fn get_recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Top priority gaps
        for gap in self.priority_gaps.iter().take(5) {
            recommendations.push(format!(
                "Add tests for {} - {}",
                gap.gap.file.display(),
                gap.priority_reason
            ));
        }

        // Critical files
        for file in self.critical_files.iter().take(3) {
            recommendations.push(format!(
                "Critical: {} has only {:.1}% coverage with risk score {:.1}",
                file.file.display(),
                file.coverage_percentage,
                file.risk_score
            ));
        }

        recommendations
    }

    /// Export compact summary for context
    pub fn to_compact_summary(&self) -> HashMap<String, serde_json::Value> {
        let mut summary = HashMap::new();

        summary.insert(
            "hybrid_score".to_string(),
            serde_json::json!(self.hybrid_score),
        );

        summary.insert(
            "top_priority_gaps".to_string(),
            serde_json::json!(self
                .priority_gaps
                .iter()
                .take(10)
                .map(|gap| {
                    serde_json::json!({
                        "file": gap.gap.file.display().to_string(),
                        "coverage": gap.gap.coverage_percentage,
                        "priority": gap.priority_score,
                        "reason": gap.priority_reason,
                    })
                })
                .collect::<Vec<_>>()),
        );

        summary.insert(
            "critical_files".to_string(),
            serde_json::json!(self
                .critical_files
                .iter()
                .take(5)
                .map(|file| {
                    serde_json::json!({
                        "file": file.file.display().to_string(),
                        "coverage": file.coverage_percentage,
                        "risk": file.risk_score,
                    })
                })
                .collect::<Vec<_>>()),
        );

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::test_coverage::FileCoverage;

    #[tokio::test]
    async fn test_hybrid_coverage_analysis() {
        let analyzer = BasicHybridCoverageAnalyzer::new();

        // Create test coverage map
        let mut file_coverage = HashMap::new();
        file_coverage.insert(
            PathBuf::from("src/main.rs"),
            FileCoverage {
                path: PathBuf::from("src/main.rs"),
                coverage_percentage: 30.0,
                tested_lines: 30,
                total_lines: 100,
                tested_functions: 3,
                total_functions: 10,
                has_tests: true,
            },
        );

        let coverage_map = TestCoverageMap {
            overall_coverage: 0.3,
            file_coverage,
            untested_functions: vec![],
            critical_paths: vec![],
        };

        // Empty metrics history for test
        let metrics_history = vec![];

        let report = analyzer
            .analyze_hybrid_coverage(Path::new("."), &coverage_map, &metrics_history)
            .await
            .unwrap();

        assert!(!report.priority_gaps.is_empty());
        assert!(report.hybrid_score <= 100.0);

        let recommendations = report.get_recommendations();
        assert!(!recommendations.is_empty());
    }

    #[test]
    fn test_priority_score_calculation() {
        let analyzer = BasicHybridCoverageAnalyzer::new();

        let gap = CoverageGap {
            file: PathBuf::from("src/complex.rs"),
            functions: vec!["complex_function".to_string()],
            coverage_percentage: 20.0,
            risk: "High".to_string(),
        };

        let metrics = FileQualityMetrics {
            file: PathBuf::from("src/complex.rs"),
            complexity_trend: TrendDirection::Degrading,
            lint_warnings_trend: TrendDirection::Degrading,
            duplication_trend: TrendDirection::Stable,
            recent_changes: 10,
            bug_frequency: 0.8,
        };

        let score = analyzer.calculate_priority_score(&gap, &metrics);
        assert!(score > 10.0); // High priority due to multiple issues
    }

    #[test]
    fn test_trend_calculation() {
        let analyzer = BasicHybridCoverageAnalyzer::new();

        // Improving trend
        let improving = vec![10.0, 8.0, 6.0];
        assert_eq!(
            analyzer.calculate_trend(&improving),
            TrendDirection::Improving
        );

        // Degrading trend
        let degrading = vec![5.0, 7.0, 10.0];
        assert_eq!(
            analyzer.calculate_trend(&degrading),
            TrendDirection::Degrading
        );

        // Stable trend
        let stable = vec![10.0, 10.5, 10.2];
        assert_eq!(analyzer.calculate_trend(&stable), TrendDirection::Stable);
    }
}
