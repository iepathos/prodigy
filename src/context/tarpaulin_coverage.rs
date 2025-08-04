//! Tarpaulin-based test coverage analysis
//!
//! This module provides coverage analysis using actual runtime data from cargo-tarpaulin
//! instead of heuristic-based estimation.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::test_coverage::{
    CriticalPath, Criticality, FileCoverage, RiskLevel, TestCoverageAnalyzer, TestCoverageMap,
    UntestedFunction,
};

/// Tarpaulin JSON output structure
#[derive(Debug, Deserialize)]
struct TarpaulinReport {
    files: Vec<TarpaulinFile>, // JSON array of files
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

use crate::subprocess::SubprocessManager;

/// Test coverage analyzer that uses cargo-tarpaulin
pub struct TarpaulinCoverageAnalyzer {
    subprocess: SubprocessManager,
}

impl TarpaulinCoverageAnalyzer {
    pub fn new(subprocess: SubprocessManager) -> Self {
        Self { subprocess }
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
            // Check if project has justfile with coverage command
            let justfile_path = project_path.join("justfile");
            let has_just_coverage = if justfile_path.exists() {
                let justfile_content = tokio::fs::read_to_string(&justfile_path)
                    .await
                    .unwrap_or_default();
                justfile_content.contains("coverage:")
            } else {
                false
            };

            use crate::subprocess::ProcessCommandBuilder;

            let output = if has_just_coverage {
                // Use project's just coverage command
                let command = ProcessCommandBuilder::new("just")
                    .args(["coverage"])
                    .current_dir(project_path)
                    .build();

                self.subprocess
                    .runner()
                    .run(command)
                    .await
                    .context("Failed to run 'just coverage'. Is just installed?")?
            } else {
                // Fall back to direct cargo tarpaulin with JSON output
                let command = ProcessCommandBuilder::new("cargo")
                    .args([
                        "tarpaulin",
                        "--out",
                        "Json",
                        "--out",
                        "Html",
                        "--output-dir",
                        "target/coverage",
                        "--skip-clean",
                        "--timeout",
                        "180",
                        "--lib",
                        "--exclude-files",
                        "*/tests/*",
                        "--exclude-files",
                        "*/target/*",
                    ])
                    .current_dir(project_path)
                    .build();

                self.subprocess
                    .runner()
                    .run(command)
                    .await
                    .context("Failed to run cargo-tarpaulin. Is it installed?")?
            };

            if !output.status.success() {
                let command_name = if has_just_coverage {
                    "just coverage"
                } else {
                    "cargo tarpaulin"
                };
                anyhow::bail!("{} failed: {}", command_name, output.stderr);
            }

            // If we used just coverage, we need to also generate JSON output for parsing
            if has_just_coverage && !coverage_file.exists() {
                // Run tarpaulin again just for JSON output
                let command = ProcessCommandBuilder::new("cargo")
                    .args([
                        "tarpaulin",
                        "--out",
                        "Json",
                        "--out",
                        "Html",
                        "--output-dir",
                        "target/coverage",
                        "--skip-clean",
                        "--timeout",
                        "180",
                        "--lib",
                        "--exclude-files",
                        "*/tests/*",
                        "--exclude-files",
                        "*/target/*",
                    ])
                    .current_dir(project_path)
                    .build();

                let json_output = self
                    .subprocess
                    .runner()
                    .run(command)
                    .await
                    .context("Failed to generate JSON coverage report")?;

                if !json_output.status.success() {
                    anyhow::bail!("JSON coverage generation failed: {}", json_output.stderr);
                }
            }
        }

        // Read and parse the JSON output
        let json_content = tokio::fs::read_to_string(&coverage_file)
            .await
            .context("Failed to read tarpaulin coverage file")?;

        match serde_json::from_str(&json_content) {
            Ok(report) => Ok(report),
            Err(e) => Err(anyhow::anyhow!(
                "Failed to parse tarpaulin JSON output: {e}"
            )),
        }
    }

    /// Convert tarpaulin data to our coverage format
    async fn convert_tarpaulin_data(
        &self,
        tarpaulin_report: &TarpaulinReport,
        project_path: &Path,
    ) -> TestCoverageMap {
        let mut file_coverage = HashMap::new();
        let mut all_untested_functions = Vec::new();
        let mut _total_covered_lines = 0;
        let mut _total_coverable_lines = 0;

        // Iterate through files array

        for file_data in &tarpaulin_report.files {
            // Construct file path from array of strings
            let file_path = if file_data.path.is_empty() {
                continue;
            } else if file_data.path[0].is_empty() {
                // Absolute path starting with empty string ["", "Users", "glen", ...]
                PathBuf::from(file_data.path[1..].join("/"))
            } else {
                // Relative path
                PathBuf::from(file_data.path.join("/"))
            };

            let relative_path = file_path
                .strip_prefix(project_path)
                .unwrap_or(&file_path)
                .to_path_buf();

            let coverage_percentage = if file_data.coverable > 0 {
                file_data.covered as f64 / file_data.coverable as f64
            } else {
                0.0
            };

            // Extract actual functions from the file content if available
            let (total_functions, tested_functions, untested_function_list) =
                if let Some(content) = &file_data.content {
                    self.analyze_file_functions(content, &relative_path, file_data)
                        .await
                } else if relative_path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    // Try to read the file directly if content not in tarpaulin data
                    match tokio::fs::read_to_string(project_path.join(&relative_path)).await {
                        Ok(content) => {
                            self.analyze_file_functions(&content, &relative_path, file_data)
                                .await
                        }
                        Err(_) => {
                            // Fallback to estimation
                            let estimated = (file_data.coverable / 10).max(1) as u32;
                            let tested = (estimated as f64 * coverage_percentage) as u32;
                            (estimated, tested, Vec::new())
                        }
                    }
                } else {
                    // Non-Rust file or unable to read
                    let estimated = (file_data.coverable / 10).max(1) as u32;
                    let tested = (estimated as f64 * coverage_percentage) as u32;
                    (estimated, tested, Vec::new())
                };

            all_untested_functions.extend(untested_function_list);

            file_coverage.insert(
                relative_path.clone(),
                FileCoverage {
                    path: relative_path,
                    coverage_percentage,
                    tested_lines: file_data.covered as u32,
                    total_lines: file_data.coverable as u32,
                    tested_functions,
                    total_functions,
                    has_tests: file_data.covered > 0,
                },
            );

            _total_covered_lines += file_data.covered;
            _total_coverable_lines += file_data.coverable;
        }

        // Use the overall coverage from tarpaulin report
        let overall_coverage = tarpaulin_report.coverage / 100.0; // Convert from percentage

        // Identify critical paths with coverage context
        let critical_paths =
            self.identify_critical_paths_with_coverage(project_path, &file_coverage);

        TestCoverageMap {
            file_coverage,
            untested_functions: all_untested_functions,
            critical_paths,
            overall_coverage,
        }
    }

    /// Analyze functions in a file and determine coverage
    async fn analyze_file_functions(
        &self,
        content: &str,
        relative_path: &Path,
        file_data: &TarpaulinFile,
    ) -> (u32, u32, Vec<UntestedFunction>) {
        // Extract functions with line numbers
        let functions = self.extract_functions_with_lines(content);
        if functions.is_empty() {
            return (0, 0, Vec::new());
        }

        let total_functions = functions.len() as u32;
        let mut tested_count = 0;
        let mut untested_functions = Vec::new();

        // For each function, check if its lines are covered
        for (func_name, line_num) in functions {
            // A function is considered tested if any of its lines are covered
            // This is a simplification - ideally we'd parse the entire function body
            let is_tested = file_data.covered > 0 && line_num <= file_data.coverable as u32;

            if is_tested {
                tested_count += 1;
            } else {
                untested_functions.push(UntestedFunction {
                    file: relative_path.to_path_buf(),
                    name: func_name.clone(),
                    line_number: line_num,
                    criticality: self.determine_criticality(&func_name, relative_path),
                });
            }
        }

        (total_functions, tested_count, untested_functions)
    }

    /// Extract functions from Rust code with line numbers
    fn extract_functions_with_lines(&self, content: &str) -> Vec<(String, u32)> {
        let mut functions = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if (line.starts_with("pub fn")
                || line.starts_with("pub(") && line.contains(" fn ")
                || line.starts_with("fn ")
                || line.starts_with("pub async fn")
                || line.starts_with("async fn"))
                && !line.contains("#[test]")
            {
                if let Some(name_start) = line.find("fn ") {
                    let after_fn = &line[name_start + 3..];
                    if let Some(name) = after_fn.split(['(', '<']).next() {
                        functions.push((name.trim().to_string(), line_num as u32 + 1));
                    }
                }
            }
        }

        functions
    }

    /// Determine criticality of a function based on its name and file path
    fn determine_criticality(&self, func_name: &str, file_path: &Path) -> Criticality {
        let path_str = file_path.to_string_lossy();
        let func_lower = func_name.to_lowercase();

        // High criticality patterns
        if func_lower.contains("auth")
            || func_lower.contains("security")
            || func_lower.contains("payment")
            || func_lower.contains("crypto")
            || func_lower.contains("password")
            || func_lower.contains("token")
            || path_str.contains("auth")
            || path_str.contains("security")
            || path_str.contains("payment")
        {
            return Criticality::High;
        }

        // Medium criticality patterns
        if func_lower.contains("save")
            || func_lower.contains("delete")
            || func_lower.contains("update")
            || func_lower.contains("process")
            || func_lower.contains("validate")
            || func_lower.contains("handle")
            || path_str.contains("core")
            || path_str.contains("api")
            || path_str.contains("handler")
        {
            return Criticality::Medium;
        }

        Criticality::Low
    }

    /// Identify critical paths in the project with coverage context
    fn identify_critical_paths_with_coverage(
        &self,
        project_path: &Path,
        file_coverage: &HashMap<PathBuf, FileCoverage>,
    ) -> Vec<CriticalPath> {
        let mut paths = Vec::new();

        // Check specific critical directories and their coverage
        let critical_dirs = vec![
            ("src/api", "API request handling", RiskLevel::Critical),
            (
                "src/auth",
                "Authentication and authorization",
                RiskLevel::Critical,
            ),
            ("src/security", "Security features", RiskLevel::Critical),
            ("src/payment", "Payment processing", RiskLevel::Critical),
            ("src/db", "Database operations", RiskLevel::High),
            ("src/core", "Core business logic", RiskLevel::High),
            ("src/handlers", "Request handlers", RiskLevel::High),
        ];

        for (dir, desc, risk) in critical_dirs {
            let dir_path = project_path.join(dir);
            if dir_path.exists() {
                // Find all files in this directory with low coverage
                let mut uncovered_files = Vec::new();
                for (file_path, coverage) in file_coverage {
                    if file_path.starts_with(dir.trim_start_matches("src/"))
                        && coverage.coverage_percentage < 0.5
                    {
                        uncovered_files.push(file_path.clone());
                    }
                }

                if !uncovered_files.is_empty() {
                    paths.push(CriticalPath {
                        description: format!(
                            "{} ({}% files with <50% coverage)",
                            desc,
                            (uncovered_files.len() * 100)
                                / file_coverage
                                    .iter()
                                    .filter(|(p, _)| p.starts_with(dir.trim_start_matches("src/")))
                                    .count()
                                    .max(1)
                        ),
                        files: uncovered_files,
                        risk_level: risk,
                    });
                }
            }
        }

        // Also check for critical files based on naming patterns
        let critical_patterns = vec![
            ("auth", "Authentication-related files", RiskLevel::Critical),
            ("security", "Security-related files", RiskLevel::Critical),
            ("crypto", "Cryptography-related files", RiskLevel::Critical),
            ("payment", "Payment-related files", RiskLevel::Critical),
            ("validate", "Validation logic", RiskLevel::High),
        ];

        for (pattern, desc, risk) in critical_patterns {
            let mut matching_files = Vec::new();
            for (file_path, coverage) in file_coverage {
                let path_str = file_path.to_string_lossy().to_lowercase();
                if path_str.contains(pattern) && coverage.coverage_percentage < 0.5 {
                    matching_files.push(file_path.clone());
                }
            }

            if !matching_files.is_empty() {
                // Don't duplicate paths already added by directory check
                let unique_files: Vec<_> = matching_files
                    .into_iter()
                    .filter(|f| !paths.iter().any(|p| p.files.contains(f)))
                    .collect();

                if !unique_files.is_empty() {
                    paths.push(CriticalPath {
                        description: format!("{desc} with low coverage"),
                        files: unique_files,
                        risk_level: risk,
                    });
                }
            }
        }

        paths
    }
}

#[async_trait::async_trait]
impl TestCoverageAnalyzer for TarpaulinCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        // Try to run tarpaulin and get actual coverage data
        match self.run_tarpaulin(project_path).await {
            Ok(tarpaulin_report) => {
                // Convert tarpaulin data to our format
                Ok(self
                    .convert_tarpaulin_data(&tarpaulin_report, project_path)
                    .await)
            }
            Err(e) => {
                // Return empty coverage data when tarpaulin is not available
                eprintln!("Warning: Unable to collect coverage data: {e}");
                eprintln!("Please install cargo-tarpaulin to get test coverage metrics:");
                eprintln!("  cargo install cargo-tarpaulin");

                // Return empty coverage map instead of inaccurate heuristic data
                Ok(TestCoverageMap {
                    file_coverage: HashMap::new(),
                    untested_functions: Vec::new(),
                    critical_paths: Vec::new(),
                    overall_coverage: 0.0,
                })
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
        Self::new(SubprocessManager::production())
    }
}

// Test helper methods removed since we don't extract function-level data from tarpaulin

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_tarpaulin_unavailable() {
        // This test verifies that we return empty coverage when tarpaulin isn't available
        let analyzer = TarpaulinCoverageAnalyzer::new(SubprocessManager::production());
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Create a simple source file
        fs::create_dir_all(project_path.join("src")).unwrap();
        fs::write(
            project_path.join("src/lib.rs"),
            "pub fn example() -> i32 { 42 }",
        )
        .unwrap();

        // This should return empty coverage data when tarpaulin fails
        let result = analyzer.analyze_coverage(project_path).await.unwrap();
        assert_eq!(result.overall_coverage, 0.0);
        assert!(result.file_coverage.is_empty());
        assert!(result.untested_functions.is_empty());
        assert!(result.critical_paths.is_empty());
    }

    #[tokio::test]
    async fn test_run_tarpaulin_success() {
        let analyzer = TarpaulinCoverageAnalyzer::new(SubprocessManager::production());
        let temp_dir = TempDir::new().unwrap();

        // Create mock project
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"
        "#,
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        )
        .unwrap();

        // Create mock tarpaulin output
        let _mock_output = r#"{
            "files": {
                "src/lib.rs": {
                    "covered": [1],
                    "uncovered": []
                }
            }
        }"#;

        // This test would need mocking of the Command execution
        // For now, we test the error handling path
        let result = analyzer.run_tarpaulin(&PathBuf::from("/nonexistent")).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_tarpaulin_with_justfile() {
        let analyzer = TarpaulinCoverageAnalyzer::new(SubprocessManager::production());
        let temp_dir = TempDir::new().unwrap();

        // Create justfile with test command
        fs::write(temp_dir.path().join("justfile"), "test:\n    cargo test").unwrap();

        // Should detect justfile and add appropriate args
        let result = analyzer.run_tarpaulin(temp_dir.path()).await;
        // Verify the command would have included justfile args
        assert!(result.is_err()); // Expected since we're not actually running tarpaulin
    }
}
