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
    async fn load_coverage_from_context(
        &self,
        project_path: &Path,
    ) -> Result<Option<TestCoverageMap>> {
        let context_dir = project_path.join(".mmm/context");
        let coverage_map_file = context_dir.join("test_coverage.json");
        let coverage_summary_file = context_dir.join("test_coverage_summary.json");

        // First try to load the full coverage map
        if coverage_map_file.exists() {
            let content = tokio::fs::read_to_string(&coverage_map_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to read coverage file: {}",
                        coverage_map_file.display()
                    )
                })?;

            if let Ok(coverage_map) = serde_json::from_str::<TestCoverageMap>(&content) {
                eprintln!(
                    "✅ Loaded test coverage data: {:.1}% overall coverage ({} files)",
                    coverage_map.overall_coverage * 100.0,
                    coverage_map.file_coverage.len()
                );
                return Ok(Some(coverage_map));
            }
        }

        // If no full map exists, try to load the summary
        if coverage_summary_file.exists() {
            let content = tokio::fs::read_to_string(&coverage_summary_file)
                .await
                .with_context(|| {
                    format!(
                        "Failed to read coverage summary file: {}",
                        coverage_summary_file.display()
                    )
                })?;

            if let Ok(summary) =
                serde_json::from_str::<crate::context::summary::TestCoverageSummary>(&content)
            {
                // Convert summary format to full format
                use crate::context::test_coverage::FileCoverage;
                use std::collections::HashMap;

                let mut file_coverage = HashMap::new();
                for (path, summary_cov) in summary.file_coverage {
                    file_coverage.insert(
                        path.clone(),
                        FileCoverage {
                            path,
                            coverage_percentage: summary_cov.coverage_percentage,
                            tested_lines: 0,
                            total_lines: 0,
                            tested_functions: 0,
                            total_functions: 0,
                            has_tests: summary_cov.has_tests,
                        },
                    );
                }

                let coverage = TestCoverageMap {
                    file_coverage,
                    untested_functions: Vec::new(),
                    critical_paths: Vec::new(),
                    overall_coverage: summary.overall_coverage,
                };

                eprintln!(
                    "✅ Loaded test coverage summary: {:.1}% overall coverage",
                    coverage.overall_coverage * 100.0
                );
                return Ok(Some(coverage));
            }
        }

        Ok(None)
    }
}

#[async_trait::async_trait]
impl TestCoverageAnalyzer for MetricsAwareCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        // Load coverage data from context directory (saved by metrics)
        eprintln!("📊 Loading test coverage data from metrics analysis...");

        // First check if there's a fresh tarpaulin report
        let tarpaulin_path = project_path.join("target/coverage/tarpaulin-report.json");
        if tarpaulin_path.exists() {
            if let Ok(metadata) = tokio::fs::metadata(&tarpaulin_path).await {
                if let Ok(modified) = metadata.modified() {
                    let age = std::time::SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or(std::time::Duration::from_secs(u64::MAX));

                    // If the report is less than 5 minutes old, use it
                    if age.as_secs() < 300 {
                        eprintln!("📊 Found recent tarpaulin report, using it for coverage data");
                        // Use TarpaulinCoverageAnalyzer to parse the report
                        let tarpaulin_analyzer =
                            super::tarpaulin_coverage::TarpaulinCoverageAnalyzer::new(
                                self.subprocess.clone(),
                            );
                        if let Ok(coverage) =
                            tarpaulin_analyzer.analyze_coverage(project_path).await
                        {
                            return Ok(coverage);
                        }
                    }
                }
            }
        }

        if let Some(coverage) = self.load_coverage_from_context(project_path).await? {
            return Ok(coverage);
        }

        // If no detailed coverage data exists, check metrics for overall percentage
        let metrics_file = project_path.join(".mmm/metrics/current.json");
        if metrics_file.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&metrics_file).await {
                if let Ok(metrics) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(test_coverage) =
                        metrics.get("test_coverage").and_then(|v| v.as_f64())
                    {
                        eprintln!("📊 Using test coverage from metrics: {test_coverage:.1}%");
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
