//! Real metrics tracking for MMM improvements
//!
//! This module provides comprehensive metrics collection and tracking for Rust projects,
//! enabling data-driven decision making and validation of improvements.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub mod collector;
pub mod complexity;
pub mod history;
pub mod performance;
pub mod quality;
pub mod storage;

pub use collector::MetricsCollector;
pub use complexity::ComplexityCalculator;
pub use history::{MetricsHistory, MetricsSnapshot, MetricsTrends, Trend};
pub use performance::PerformanceProfiler;
pub use quality::QualityAnalyzer;
pub use storage::MetricsStorage;

/// Comprehensive metrics for a single improvement iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementMetrics {
    // Code quality metrics
    pub test_coverage: f32,
    pub type_coverage: f32,
    pub lint_warnings: u32,
    pub code_duplication: f32,
    pub doc_coverage: f32,

    // Performance metrics
    pub benchmark_results: HashMap<String, Duration>,
    pub memory_usage: HashMap<String, u64>, // in bytes
    pub compile_time: Duration,
    pub binary_size: u64, // in bytes

    // Complexity metrics
    pub cyclomatic_complexity: HashMap<String, u32>,
    pub cognitive_complexity: HashMap<String, u32>,
    pub max_nesting_depth: u32,
    pub total_lines: u32,

    // Progress metrics
    pub bugs_fixed: u32,
    pub features_added: u32,
    pub tech_debt_score: f32,
    pub improvement_velocity: f32,

    // Metadata
    pub timestamp: DateTime<Utc>,
    pub iteration_id: String,
}

impl ImprovementMetrics {
    /// Create a new metrics instance with default values
    pub fn new(iteration_id: String) -> Self {
        Self {
            test_coverage: 0.0,
            type_coverage: 0.0,
            lint_warnings: 0,
            code_duplication: 0.0,
            doc_coverage: 0.0,
            benchmark_results: HashMap::new(),
            memory_usage: HashMap::new(),
            compile_time: Duration::default(),
            binary_size: 0,
            cyclomatic_complexity: HashMap::new(),
            cognitive_complexity: HashMap::new(),
            max_nesting_depth: 0,
            total_lines: 0,
            bugs_fixed: 0,
            features_added: 0,
            tech_debt_score: 0.0,
            improvement_velocity: 0.0,
            timestamp: Utc::now(),
            iteration_id,
        }
    }

    /// Calculate an overall quality score (0-100)
    pub fn overall_score(&self) -> f32 {
        // Weighted average of different metrics
        let mut score = 0.0;
        let mut weight = 0.0;

        // Test coverage (weight: 30%)
        score += self.test_coverage * 0.3;
        weight += 0.3;

        // Code quality (weight: 20%)
        let quality_score = 100.0 - (self.lint_warnings as f32 * 2.0).min(100.0);
        score += quality_score * 0.2;
        weight += 0.2;

        // Documentation (weight: 15%)
        score += self.doc_coverage * 0.15;
        weight += 0.15;

        // Technical debt (weight: 20%)
        let debt_score = (100.0 - self.tech_debt_score).max(0.0);
        score += debt_score * 0.2;
        weight += 0.2;

        // Type coverage (weight: 15%)
        score += self.type_coverage * 0.15;
        weight += 0.15;

        if weight > 0.0 {
            score / weight
        } else {
            0.0
        }
    }
}

/// Comparison between two metrics snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsComparison {
    pub test_coverage_delta: f32,
    pub lint_warnings_delta: i32,
    pub complexity_delta: i32,
    pub performance_delta: f32,
    pub overall_improvement: f32,
}

/// Trait for metrics analysis
pub trait MetricsAnalyzer {
    async fn analyze(&self, project_path: &Path) -> Result<MetricsData>;
    fn get_baseline(&self) -> Option<MetricsData>;
    fn compare_with_baseline(&self, current: &MetricsData) -> MetricsComparison;
}

/// Generic metrics data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsData {
    pub name: String,
    pub value: serde_json::Value,
    pub unit: Option<String>,
}

/// Trait for metrics reporting
pub trait MetricsReporter {
    fn generate_report(&self, history: &MetricsHistory) -> String;
    fn get_summary(&self, current: &ImprovementMetrics) -> String;
    fn export_dashboard(&self, path: &Path) -> Result<()>;
}
