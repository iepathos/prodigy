//! Metrics history tracking and trend analysis

use super::ImprovementMetrics;
use serde::{Deserialize, Serialize};

/// Historical metrics data with trends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsHistory {
    pub snapshots: Vec<MetricsSnapshot>,
    pub trends: MetricsTrends,
    pub baselines: MetricsBaselines,
}

impl MetricsHistory {
    /// Create a new metrics history
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            trends: MetricsTrends::default(),
            baselines: MetricsBaselines::default(),
        }
    }

    /// Add a new metrics snapshot
    pub fn add_snapshot(&mut self, metrics: ImprovementMetrics, commit_sha: String) {
        let iteration = self.snapshots.len() as u32 + 1;

        // Update baselines if this is the first snapshot
        if self.snapshots.is_empty() {
            self.baselines = MetricsBaselines::from_metrics(&metrics);
        }

        let snapshot = MetricsSnapshot {
            metrics,
            iteration,
            commit_sha,
        };

        self.snapshots.push(snapshot);
        self.update_trends();
    }

    /// Update trend calculations
    fn update_trends(&mut self) {
        if self.snapshots.len() < 2 {
            return;
        }

        let recent = &self.snapshots[self.snapshots.len() - 1].metrics;
        let previous = &self.snapshots[self.snapshots.len() - 2].metrics;

        // Coverage trend
        let coverage_delta = recent.test_coverage - previous.test_coverage;
        self.trends.coverage_trend = if coverage_delta > 0.5 {
            Trend::Improving(coverage_delta)
        } else if coverage_delta < -0.5 {
            Trend::Degrading(coverage_delta.abs())
        } else {
            Trend::Stable
        };

        // Complexity trend
        let recent_complexity: u32 = recent.cyclomatic_complexity.values().sum();
        let previous_complexity: u32 = previous.cyclomatic_complexity.values().sum();
        let complexity_delta = recent_complexity as f32 - previous_complexity as f32;

        self.trends.complexity_trend = if complexity_delta < -1.0 {
            Trend::Improving(complexity_delta.abs())
        } else if complexity_delta > 1.0 {
            Trend::Degrading(complexity_delta)
        } else {
            Trend::Stable
        };

        // Quality trend
        let recent_score = recent.overall_score();
        let previous_score = previous.overall_score();
        let quality_delta = recent_score - previous_score;

        self.trends.quality_trend = if quality_delta > 0.5 {
            Trend::Improving(quality_delta)
        } else if quality_delta < -0.5 {
            Trend::Degrading(quality_delta.abs())
        } else {
            Trend::Stable
        };
    }

    /// Get the latest metrics
    pub fn latest(&self) -> Option<&ImprovementMetrics> {
        self.snapshots.last().map(|s| &s.metrics)
    }

    /// Get metrics from N iterations ago
    pub fn get_previous(&self, iterations_ago: usize) -> Option<&ImprovementMetrics> {
        let len = self.snapshots.len();
        if iterations_ago < len {
            Some(&self.snapshots[len - 1 - iterations_ago].metrics)
        } else {
            None
        }
    }

    /// Calculate improvement velocity over last N iterations
    pub fn calculate_velocity(&self, iterations: usize) -> f32 {
        if self.snapshots.len() < 2 {
            return 0.0;
        }

        let start_idx = self.snapshots.len().saturating_sub(iterations);
        let end_idx = self.snapshots.len() - 1;

        if start_idx >= end_idx {
            return 0.0;
        }

        let start_score = self.snapshots[start_idx].metrics.overall_score();
        let end_score = self.snapshots[end_idx].metrics.overall_score();
        let iterations_count = (end_idx - start_idx) as f32;

        (end_score - start_score) / iterations_count
    }
}

impl Default for MetricsHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// A single metrics snapshot in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub metrics: ImprovementMetrics,
    pub iteration: u32,
    pub commit_sha: String,
}

/// Trend analysis for different metric categories
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsTrends {
    pub coverage_trend: Trend,
    pub complexity_trend: Trend,
    pub performance_trend: Trend,
    pub quality_trend: Trend,
}

/// Trend direction and magnitude
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Trend {
    Improving(f32), // Percentage improvement
    Stable,
    Degrading(f32), // Percentage degradation
}

impl Default for Trend {
    fn default() -> Self {
        Trend::Stable
    }
}

/// Baseline metrics for comparison
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetricsBaselines {
    pub initial_coverage: f32,
    pub initial_complexity: f32,
    pub initial_quality_score: f32,
    pub initial_lint_warnings: u32,
}

impl MetricsBaselines {
    /// Create baselines from initial metrics
    fn from_metrics(metrics: &ImprovementMetrics) -> Self {
        let avg_complexity = if !metrics.cyclomatic_complexity.is_empty() {
            metrics.cyclomatic_complexity.values().sum::<u32>() as f32
                / metrics.cyclomatic_complexity.len() as f32
        } else {
            0.0
        };

        Self {
            initial_coverage: metrics.test_coverage,
            initial_complexity: avg_complexity,
            initial_quality_score: metrics.overall_score(),
            initial_lint_warnings: metrics.lint_warnings,
        }
    }
}
