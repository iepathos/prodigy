//! Analysis coordination for cook operations
//!
//! Handles project analysis, context generation, and caching.

pub mod cache;
pub mod runner;

pub use cache::{AnalysisCache, AnalysisCacheImpl};
pub use runner::{AnalysisRunner, AnalysisRunnerImpl};

// Re-export types from context module to avoid duplication
pub use crate::context::{AnalysisMetadata, AnalysisResult};

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

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
