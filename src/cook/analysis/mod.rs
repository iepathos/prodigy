//! Analysis coordination for cook operations
//!
//! Handles project analysis, context generation, and caching.

pub mod cache;
pub mod runner;

pub use cache::{AnalysisCache, AnalysisCacheImpl};
pub use runner::{AnalysisRunner, AnalysisRunnerImpl};

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of project analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Dependency graph
    pub dependency_graph: serde_json::Value,
    /// Architecture analysis
    pub architecture: serde_json::Value,
    /// Code conventions
    pub conventions: serde_json::Value,
    /// Technical debt
    pub technical_debt: serde_json::Value,
    /// Test coverage
    pub test_coverage: Option<serde_json::Value>,
    /// Analysis metadata
    pub metadata: AnalysisMetadata,
}

/// Metadata about the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisMetadata {
    /// When analysis was performed
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// How long analysis took
    pub duration_ms: u64,
    /// Number of files analyzed
    pub files_analyzed: usize,
    /// Whether incremental analysis was used
    pub incremental: bool,
    /// Analysis version
    pub version: String,
}

/// Trait for coordinating analysis operations
#[async_trait]
pub trait AnalysisCoordinator: Send + Sync {
    /// Run full project analysis
    async fn analyze_project(&self, project_path: &Path) -> Result<AnalysisResult>;

    /// Run incremental analysis
    async fn analyze_incremental(
        &self,
        project_path: &Path,
        changed_files: &[String],
    ) -> Result<AnalysisResult>;

    /// Get cached analysis if available
    async fn get_cached_analysis(&self, project_path: &Path) -> Result<Option<AnalysisResult>>;

    /// Save analysis results
    async fn save_analysis(&self, project_path: &Path, analysis: &AnalysisResult) -> Result<()>;

    /// Clear analysis cache
    async fn clear_cache(&self, project_path: &Path) -> Result<()>;
}