//! Metrics collection and reporting for cook operations
//!
//! Handles collection, storage, and reporting of code quality metrics.

pub mod collector;
pub mod reporter;

pub use collector::{MetricsCollectorImpl, MetricsCollectorTrait};
pub use reporter::{MetricsReporter, MetricsReporterImpl};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Collection of project metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetrics {
    /// Test coverage percentage
    pub test_coverage: Option<f64>,
    /// Type coverage percentage
    pub type_coverage: Option<f64>,
    /// Number of lint warnings
    pub lint_warnings: usize,
    /// Code duplication percentage
    pub code_duplication: Option<f64>,
    /// Documentation coverage percentage
    pub doc_coverage: Option<f64>,
    /// Benchmark results
    pub benchmark_results: Option<serde_json::Value>,
    /// Compilation time in seconds
    pub compile_time: Option<f64>,
    /// Binary size in bytes
    pub binary_size: Option<u64>,
    /// Cyclomatic complexity scores
    pub cyclomatic_complexity: Option<serde_json::Value>,
    /// Maximum nesting depth
    pub max_nesting_depth: Option<u32>,
    /// Total lines of code
    pub total_lines: Option<usize>,
    /// Technical debt score
    pub tech_debt_score: Option<f64>,
    /// Improvement velocity
    pub improvement_velocity: Option<f64>,
    /// Collection timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Associated iteration ID
    pub iteration_id: Option<String>,
}

impl Default for ProjectMetrics {
    fn default() -> Self {
        Self {
            test_coverage: None,
            type_coverage: None,
            lint_warnings: 0,
            code_duplication: None,
            doc_coverage: None,
            benchmark_results: None,
            compile_time: None,
            binary_size: None,
            cyclomatic_complexity: None,
            max_nesting_depth: None,
            total_lines: None,
            tech_debt_score: None,
            improvement_velocity: None,
            timestamp: chrono::Utc::now(),
            iteration_id: None,
        }
    }
}

/// Trait for metrics collection coordination
#[async_trait]
pub trait MetricsCoordinator: Send + Sync {
    /// Collect all available metrics
    async fn collect_all(&self, project_path: &Path) -> Result<ProjectMetrics>;

    /// Collect specific metric
    async fn collect_metric(&self, project_path: &Path, metric: &str) -> Result<serde_json::Value>;

    /// Store metrics
    async fn store_metrics(&self, project_path: &Path, metrics: &ProjectMetrics) -> Result<()>;

    /// Load historical metrics
    async fn load_history(&self, project_path: &Path) -> Result<Vec<ProjectMetrics>>;

    /// Generate metrics report
    async fn generate_report(
        &self,
        metrics: &ProjectMetrics,
        history: &[ProjectMetrics],
    ) -> Result<String>;
}
