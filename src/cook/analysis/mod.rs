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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    struct MockAnalysisCoordinator;
    
    #[async_trait]
    impl AnalysisCoordinator for MockAnalysisCoordinator {
        async fn analyze_project(&self, _project_path: &Path) -> Result<AnalysisResult> {
            Ok(create_test_analysis_result())
        }
        
        async fn analyze_incremental(
            &self,
            _project_path: &Path,
            _changed_files: &[String],
        ) -> Result<AnalysisResult> {
            Ok(create_test_analysis_result())
        }
        
        async fn get_cached_analysis(&self, _project_path: &Path) -> Result<Option<AnalysisResult>> {
            Ok(None)
        }
        
        async fn save_analysis(&self, _project_path: &Path, _analysis: &AnalysisResult) -> Result<()> {
            Ok(())
        }
        
        async fn clear_cache(&self, _project_path: &Path) -> Result<()> {
            Ok(())
        }
    }
    
    fn create_test_analysis_result() -> AnalysisResult {
        AnalysisResult {
            dependency_graph: crate::context::DependencyGraph {
                nodes: Default::default(),
                edges: vec![],
                cycles: vec![],
                layers: vec![],
            },
            architecture: crate::context::ArchitectureInfo {
                patterns: vec![],
                layers: vec![],
                components: Default::default(),
                violations: vec![],
            },
            conventions: crate::context::ProjectConventions {
                naming_patterns: Default::default(),
                code_patterns: Default::default(),
                test_patterns: crate::context::conventions::TestingConventions {
                    test_file_pattern: "tests/".to_string(),
                    test_function_prefix: "test_".to_string(),
                    test_module_pattern: "#[cfg(test)]".to_string(),
                    assertion_style: "assert_eq!".to_string(),
                },
                project_idioms: vec![],
            },
            technical_debt: crate::context::TechnicalDebtMap {
                debt_items: vec![],
                hotspots: vec![],
                duplication_map: Default::default(),
                priority_queue: std::collections::BinaryHeap::new(),
            },
            test_coverage: None,
            hybrid_coverage: None,
            metadata: crate::context::AnalysisMetadata {
                timestamp: chrono::Utc::now(),
                duration_ms: 0,
                files_analyzed: 0,
                incremental: false,
                version: "0.1.0".to_string(),
            },
        }
    }
    
    #[tokio::test]
    async fn test_analysis_coordinator_trait() {
        let coordinator = MockAnalysisCoordinator;
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        
        // Test all trait methods
        let result = coordinator.analyze_project(path).await;
        assert!(result.is_ok());
        
        let incremental = coordinator.analyze_incremental(path, &["file.rs".to_string()]).await;
        assert!(incremental.is_ok());
        
        let cached = coordinator.get_cached_analysis(path).await;
        assert!(cached.is_ok());
        assert!(cached.unwrap().is_none());
        
        let save_result = coordinator.save_analysis(path, &create_test_analysis_result()).await;
        assert!(save_result.is_ok());
        
        let clear_result = coordinator.clear_cache(path).await;
        assert!(clear_result.is_ok());
    }
}
