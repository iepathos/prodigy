//! Isolated, pluggable metrics system for MMM
//!
//! This module provides a comprehensive metrics collection system with clear interfaces,
//! multiple backends, and comprehensive testing support. The system is designed to be
//! isolated from execution logic and pluggable.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::scoring::ProjectHealthScore;

// Core components
pub mod backends;
pub mod context;
pub mod events;
pub mod registry;
pub mod testing;

// Legacy components (for compatibility)
pub mod collector;
pub mod complexity;
pub mod history;
pub mod performance;
pub mod quality;
pub mod storage;

// Re-exports from new isolated system
pub use backends::{
    CollectorConfig, CompositeMetricsCollector, FileMetricsCollector, MemoryMetricsCollector,
};
pub use context::{MetricsContext, MetricsContextBuilder};
pub use events::{
    AggregateResult, Aggregation, MetricEvent, MetricsCollector as MetricsCollectorTrait,
    MetricsQuery, MetricsReader, MetricsResult, Tags, TimeRange,
};
pub use registry::{MetricsConfig, MetricsRegistry};
pub use testing::{create_disabled_registry, create_test_registry, MetricsAssert};

// Legacy re-exports (for backward compatibility)
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

    // Unified health score
    pub health_score: Option<ProjectHealthScore>,

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
            health_score: None,
            timestamp: Utc::now(),
            iteration_id,
        }
    }

    /// Calculate overall quality score using unified scoring system
    pub fn overall_score(&self) -> f32 {
        // Use cached health score if available
        if let Some(ref health_score) = self.health_score {
            return health_score.overall as f32;
        }

        // Otherwise calculate from current metrics
        let health_score = ProjectHealthScore::from_metrics(self);
        health_score.overall as f32
    }

    /// Update the unified health score
    pub fn update_health_score(&mut self) {
        self.health_score = Some(ProjectHealthScore::from_metrics(self));
    }
}

impl Default for ImprovementMetrics {
    fn default() -> Self {
        Self::new("default".to_string())
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
#[allow(async_fn_in_trait)]
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

/// Factory functions for creating common metrics configurations
pub mod factory {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Create a production metrics registry with file and memory collectors
    pub async fn create_production_registry(
        metrics_dir: PathBuf,
        config: Option<MetricsConfig>,
    ) -> Result<Arc<MetricsRegistry>> {
        let config = config.unwrap_or_default();
        let registry = Arc::new(MetricsRegistry::new(config));

        // Add file collector for persistence
        let file_path = metrics_dir.join("metrics.jsonl");
        let file_collector = Arc::new(FileMetricsCollector::new("file", file_path));
        registry.register_collector(file_collector.clone()).await;

        // Add memory collector for querying
        let memory_collector = Arc::new(MemoryMetricsCollector::new("memory"));
        registry.register_collector(memory_collector.clone()).await;
        registry.register_reader(memory_collector).await;

        Ok(registry)
    }

    /// Create a memory-only registry for testing
    pub async fn create_test_registry() -> (Arc<MetricsRegistry>, MetricsAssert) {
        crate::metrics::testing::create_test_registry().await
    }

    /// Create a disabled registry (no-op)
    pub fn create_disabled_registry() -> Arc<MetricsRegistry> {
        crate::metrics::testing::create_disabled_registry()
    }

    /// Create a metrics context with common tags
    pub fn create_context(
        registry: Arc<MetricsRegistry>,
        session_id: &str,
        component: &str,
    ) -> MetricsContext {
        MetricsContextBuilder::new(registry)
            .tag("session_id", session_id)
            .tag("component", component)
            .tag("version", env!("CARGO_PKG_VERSION"))
            .build()
    }
}
