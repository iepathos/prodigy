//! Tarpaulin-based test coverage analysis
//!
//! This module provides coverage analysis using actual runtime data from cargo-tarpaulin
//! instead of heuristic-based estimation.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::test_coverage::{
    CriticalPath, FileCoverage, RiskLevel,
    TestCoverageAnalyzer, TestCoverageMap,
};

/// Tarpaulin JSON output structure
#[derive(Debug, Deserialize)]
struct TarpaulinReport {
    files: serde_json::Value, // The keys might be numbers or strings
    coverage: f64,
    #[allow(dead_code)]
    covered: usize,
    #[allow(dead_code)]
    coverable: usize,
}

#[derive(Debug, Deserialize)]
struct TarpaulinFile {
    path: Vec<String>,
    #[allow(dead_code)]
    content: Option<String>,
    covered: usize,
    coverable: usize,
    #[allow(dead_code)]
    traces: serde_json::Value, // Don't parse traces, we don't use them
}

// We don't need to parse traces since we're not using them

/// Test coverage analyzer that uses cargo-tarpaulin
pub struct TarpaulinCoverageAnalyzer {
    // We'll reimplement the needed methods directly
}

impl TarpaulinCoverageAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    /// Run cargo-tarpaulin and get coverage data
    async fn run_tarpaulin(&self, project_path: &Path) -> Result<TarpaulinReport> {
        let coverage_file = project_path.join("target/coverage/tarpaulin-report.json");
        
        // Check if we need to run tarpaulin or if recent results exist
        let should_run = if coverage_file.exists() {
            // Check if coverage data is older than 5 minutes
            let metadata = tokio::fs::metadata(&coverage_file).await?;
            let modified = metadata.modified()?;
            let age = std::time::SystemTime::now().duration_since(modified)?;
            age > std::time::Duration::from_secs(300)
        } else {
            true
        };

        if should_run {
            eprintln!("Running cargo-tarpaulin to collect coverage data...");
            
            // Run tarpaulin
            let output = Command::new("cargo")
                .args(&[
                    "tarpaulin",
                    "--skip-clean",
                    "--out", "Json",
                    "--output-dir", "target/coverage",
                ])
                .current_dir(project_path)
                .output()
                .context("Failed to run cargo-tarpaulin. Is it installed?")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("cargo-tarpaulin failed: {}", stderr);
            }
        }

        // Read and parse the JSON output
        eprintln!("Reading coverage data from: {}", coverage_file.display());
        let json_content = tokio::fs::read_to_string(&coverage_file)
            .await
            .context("Failed to read tarpaulin coverage file")?;
        
        match serde_json::from_str(&json_content) {
            Ok(report) => Ok(report),
            Err(e) => {
                eprintln!("JSON parse error: {}", e);
                Err(anyhow::anyhow!("Failed to parse tarpaulin JSON output: {}", e))
            }
        }
    }

    /// Convert tarpaulin data to our coverage format
    fn convert_tarpaulin_data(
        &self,
        tarpaulin_report: &TarpaulinReport,
        project_path: &Path,
    ) -> TestCoverageMap {
        let mut file_coverage = HashMap::new();
        let all_untested_functions = Vec::new();
        let mut _total_covered_lines = 0;
        let mut _total_coverable_lines = 0;

        // Parse files from JSON value
        if let Some(files_obj) = tarpaulin_report.files.as_object() {
            for (_, file_value) in files_obj {
                let file_data: TarpaulinFile = match serde_json::from_value(file_value.clone()) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("Failed to parse file data: {}", e);
                        continue;
                    }
                };
                let file_path = PathBuf::from(file_data.path.join("/"));
                let relative_path = file_path
                    .strip_prefix(project_path)
                    .unwrap_or(&file_path)
                    .to_path_buf();

                // Skip test files in coverage calculation
                if self.is_test_file(&relative_path) {
                    continue;
                }

                // We can't extract function-level coverage from tarpaulin traces
                // So we'll estimate based on line coverage
                let coverage_percentage = if file_data.coverable > 0 {
                    file_data.covered as f64 / file_data.coverable as f64
                } else {
                    0.0
                };

                // Estimate function coverage based on line coverage
                // This is a rough approximation
                let estimated_functions = (file_data.coverable / 10).max(1) as u32; // Assume ~10 lines per function
                let tested_functions = (estimated_functions as f64 * coverage_percentage) as u32;

                file_coverage.insert(
                    relative_path.clone(),
                    FileCoverage {
                        path: relative_path,
                        coverage_percentage,
                        tested_lines: file_data.covered as u32,
                        total_lines: file_data.coverable as u32,
                        tested_functions,
                        total_functions: estimated_functions,
                        has_tests: file_data.covered > 0,
                    },
                );

                _total_covered_lines += file_data.covered;
                _total_coverable_lines += file_data.coverable;
            }
        }

        // Use the overall coverage from tarpaulin report
        let overall_coverage = tarpaulin_report.coverage / 100.0; // Convert from percentage
        eprintln!("Tarpaulin overall coverage: {:.2}%", tarpaulin_report.coverage);

        // Identify critical paths
        let critical_paths = self.identify_critical_paths_in_project(project_path);

        TestCoverageMap {
            file_coverage,
            untested_functions: all_untested_functions,
            critical_paths,
            overall_coverage,
        }
    }

    /// Check if a path is a test file
    fn is_test_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.contains("/tests/")
            || path_str.contains("_test.rs")
            || path_str.contains("test_")
            || path_str.ends_with("_test.rs")
    }

    // Removed determine_criticality since we can't extract function names from tarpaulin

    /// Identify critical paths in the project
    fn identify_critical_paths_in_project(&self, project_path: &Path) -> Vec<CriticalPath> {
        let mut paths = Vec::new();

        // API/HTTP handlers
        let api_path = project_path.join("src/api");
        if api_path.exists() {
            paths.push(CriticalPath {
                description: "API request handling".to_string(),
                files: vec![api_path],
                risk_level: RiskLevel::Critical,
            });
        }

        // Authentication
        let auth_path = project_path.join("src/auth");
        if auth_path.exists() {
            paths.push(CriticalPath {
                description: "Authentication and authorization".to_string(),
                files: vec![auth_path],
                risk_level: RiskLevel::Critical,
            });
        }

        // Database operations
        let db_path = project_path.join("src/db");
        if db_path.exists() {
            paths.push(CriticalPath {
                description: "Database operations".to_string(),
                files: vec![db_path],
                risk_level: RiskLevel::High,
            });
        }

        paths
    }
}

#[async_trait::async_trait]
impl TestCoverageAnalyzer for TarpaulinCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        eprintln!("TarpaulinCoverageAnalyzer: Starting coverage analysis");
        // Try to run tarpaulin and get actual coverage data
        match self.run_tarpaulin(project_path).await {
            Ok(tarpaulin_report) => {
                // Convert tarpaulin data to our format
                Ok(self.convert_tarpaulin_data(&tarpaulin_report, project_path))
            }
            Err(e) => {
                eprintln!("Warning: Failed to get tarpaulin coverage data: {}", e);
                eprintln!("Falling back to heuristic-based coverage analysis...");
                
                // Fall back to basic analyzer
                let basic_analyzer = super::test_coverage::BasicTestCoverageAnalyzer::new();
                basic_analyzer.analyze_coverage(project_path).await
            }
        }
    }

    async fn update_coverage(
        &self,
        project_path: &Path,
        _current: &TestCoverageMap,
        _changed_files: &[PathBuf],
    ) -> Result<TestCoverageMap> {
        // For now, just re-run full analysis
        // In the future, we could be smarter about incremental updates
        self.analyze_coverage(project_path).await
    }
}

impl Default for TarpaulinCoverageAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// Test helper methods removed since we don't extract function-level data from tarpaulin

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[tokio::test]
    async fn test_tarpaulin_fallback() {
        // This test verifies that we fall back gracefully when tarpaulin isn't available
        let analyzer = TarpaulinCoverageAnalyzer::new();
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create a simple source file
        fs::create_dir_all(project_path.join("src")).unwrap();
        fs::write(
            project_path.join("src/lib.rs"),
            "pub fn example() -> i32 { 42 }",
        )
        .unwrap();

        // This should fall back to basic analyzer since we're not in a real project
        let result = analyzer.analyze_coverage(project_path).await;
        assert!(result.is_ok());
    }

}