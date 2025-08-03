use super::test_coverage::{TestCoverageAnalyzer, TestCoverageMap};
use crate::subprocess::SubprocessManager;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Coverage analyzer that loads test coverage from metrics/context data
/// Since metrics always runs before context analysis, we never need to run tarpaulin again
pub struct MetricsAwareCoverageAnalyzer {
    #[allow(dead_code)]
    subprocess: SubprocessManager,
}

impl MetricsAwareCoverageAnalyzer {
    pub fn new(subprocess: SubprocessManager) -> Self {
        Self { subprocess }
    }

    /// Load coverage data from the test_coverage.json file saved by metrics
    async fn load_coverage_from_context(&self, project_path: &Path) -> Result<Option<TestCoverageMap>> {
        let coverage_file = project_path.join(".mmm/context/test_coverage.json");
        
        if !coverage_file.exists() {
            eprintln!("⚠️  No test coverage data found at {}", coverage_file.display());
            return Ok(None);
        }

        // Read and parse the coverage data
        let content = tokio::fs::read_to_string(&coverage_file)
            .await
            .with_context(|| format!("Failed to read coverage file: {}", coverage_file.display()))?;
            
        let coverage: TestCoverageMap = serde_json::from_str(&content)
            .with_context(|| "Failed to parse test coverage data")?;
            
        eprintln!("✅ Loaded test coverage data: {:.1}% overall coverage", coverage.overall_coverage * 100.0);
        Ok(Some(coverage))
    }
}

#[async_trait::async_trait]
impl TestCoverageAnalyzer for MetricsAwareCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        // Load coverage data from context directory (saved by metrics)
        eprintln!("📊 Loading test coverage data from metrics analysis...");
        
        if let Some(coverage) = self.load_coverage_from_context(project_path).await? {
            return Ok(coverage);
        }
        
        // If no detailed coverage data exists, check metrics for overall percentage
        let metrics_file = project_path.join(".mmm/metrics/current.json");
        if metrics_file.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&metrics_file).await {
                if let Ok(metrics) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(test_coverage) = metrics.get("test_coverage").and_then(|v| v.as_f64()) {
                        eprintln!("📊 Using test coverage from metrics: {:.1}%", test_coverage);
                        // Create a simple coverage map with just the overall percentage
                        return Ok(TestCoverageMap {
                            file_coverage: Default::default(),
                            untested_functions: Vec::new(),
                            critical_paths: Vec::new(),
                            overall_coverage: test_coverage / 100.0, // Convert percentage to fraction
                        });
                    }
                }
            }
        }
        
        // If no coverage data exists at all, return empty coverage
        eprintln!("⚠️  No test coverage data available.");
        Ok(TestCoverageMap {
            file_coverage: Default::default(),
            untested_functions: Vec::new(),
            critical_paths: Vec::new(),
            overall_coverage: 0.0,
        })
    }

    async fn update_coverage(
        &self,
        project_path: &Path,
        _current: &TestCoverageMap,
        _changed_files: &[PathBuf],
    ) -> Result<TestCoverageMap> {
        // For incremental updates, just reload the coverage data
        // Metrics should have already updated it if needed
        self.analyze_coverage(project_path).await
    }
}
