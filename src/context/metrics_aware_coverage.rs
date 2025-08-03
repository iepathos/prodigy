use super::test_coverage::{TestCoverageAnalyzer, TestCoverageMap, FileCoverage};
use crate::subprocess::SubprocessManager;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Coverage analyzer that loads test coverage from metrics if available,
/// otherwise falls back to tarpaulin analysis
pub struct MetricsAwareCoverageAnalyzer {
    tarpaulin_analyzer: super::tarpaulin_coverage::TarpaulinCoverageAnalyzer,
}

impl MetricsAwareCoverageAnalyzer {
    pub fn new(subprocess: SubprocessManager) -> Self {
        Self {
            tarpaulin_analyzer: super::tarpaulin_coverage::TarpaulinCoverageAnalyzer::new(subprocess),
        }
    }
    
    /// Try to load coverage data from metrics file
    async fn load_from_metrics(&self, project_path: &Path) -> Option<f64> {
        let metrics_file = project_path.join(".mmm/metrics/current.json");
        if !metrics_file.exists() {
            return None;
        }
        
        // Read metrics file
        match tokio::fs::read_to_string(&metrics_file).await {
            Ok(content) => {
                // Parse metrics
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(metrics) => {
                        // Extract test_coverage field
                        metrics.get("test_coverage")
                            .and_then(|v| v.as_f64())
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }
    
    /// Create a basic coverage map from metrics percentage
    fn create_coverage_map_from_metrics(&self, coverage_percentage: f64) -> TestCoverageMap {
        // Create a simple coverage map with the overall percentage
        // This is a placeholder - in a real system we'd want more detailed data
        let mut file_coverage = HashMap::new();
        
        // Add a summary entry
        file_coverage.insert(
            PathBuf::from("project_summary"),
            FileCoverage {
                path: PathBuf::from("project_summary"),
                coverage_percentage: coverage_percentage * 100.0,
                tested_lines: (coverage_percentage * 1000.0) as u32, // Estimate
                total_lines: 1000, // Estimate
                tested_functions: (coverage_percentage * 50.0) as u32, // Estimate
                total_functions: 50, // Estimate
                has_tests: coverage_percentage > 0.0,
            },
        );
        
        TestCoverageMap {
            file_coverage,
            untested_functions: Vec::new(),
            critical_paths: Vec::new(),
            overall_coverage: coverage_percentage,
        }
    }
}

#[async_trait::async_trait]
impl TestCoverageAnalyzer for MetricsAwareCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        // First try to load from metrics
        if let Some(coverage_pct) = self.load_from_metrics(project_path).await {
            eprintln!("📊 Loaded test coverage from metrics: {:.1}%", coverage_pct * 100.0);
            return Ok(self.create_coverage_map_from_metrics(coverage_pct));
        }
        
        // Fall back to tarpaulin analysis
        self.tarpaulin_analyzer.analyze_coverage(project_path).await
    }

    async fn update_coverage(
        &self,
        project_path: &Path,
        current: &TestCoverageMap,
        changed_files: &[PathBuf],
    ) -> Result<TestCoverageMap> {
        // For updates, always use tarpaulin since we need detailed file-level data
        self.tarpaulin_analyzer.update_coverage(project_path, current, changed_files).await
    }
}