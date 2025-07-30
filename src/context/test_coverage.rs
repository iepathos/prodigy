//! Test coverage analysis and gap detection

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Trait for test coverage analysis
#[async_trait::async_trait]
pub trait TestCoverageAnalyzer: Send + Sync {
    /// Analyze test coverage in the project
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap>;

    /// Update coverage based on changed files
    async fn update_coverage(
        &self,
        project_path: &Path,
        current: &TestCoverageMap,
        changed_files: &[PathBuf],
    ) -> Result<TestCoverageMap>;
}

/// Test coverage information for the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCoverageMap {
    pub file_coverage: HashMap<PathBuf, FileCoverage>,
    pub untested_functions: Vec<UntestedFunction>,
    pub critical_paths: Vec<CriticalPath>,
    pub overall_coverage: f64,
}

/// Coverage information for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCoverage {
    pub path: PathBuf,
    pub coverage_percentage: f64,
    pub tested_lines: u32,
    pub total_lines: u32,
    pub tested_functions: u32,
    pub total_functions: u32,
    pub has_tests: bool,
}

/// An untested function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UntestedFunction {
    pub file: PathBuf,
    pub name: String,
    pub line_number: u32,
    pub criticality: Criticality,
}

/// A critical path without tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPath {
    pub description: String,
    pub files: Vec<PathBuf>,
    pub risk_level: RiskLevel,
}

/// Criticality level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Criticality {
    High,
    Medium,
    Low,
}

/// Risk level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Critical,
    High,
    Medium,
    Low,
}

impl TestCoverageMap {
    /// Get coverage for a specific file
    pub fn get_file_coverage(&self, file: &Path) -> f64 {
        self.file_coverage
            .get(file)
            .map(|c| c.coverage_percentage)
            .unwrap_or(0.0)
    }

    /// Get critical gaps in coverage
    pub fn get_critical_gaps(&self) -> Vec<(String, f64)> {
        let mut gaps = Vec::new();

        for (path, coverage) in &self.file_coverage {
            // Consider files with < 50% coverage as critical if they're not test files
            let path_str = path.to_string_lossy();
            if !path_str.contains("test")
                && !path_str.contains("spec")
                && coverage.coverage_percentage < 0.5
            {
                gaps.push((path_str.to_string(), coverage.coverage_percentage));
            }
        }

        // Sort by coverage (lowest first)
        gaps.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        gaps
    }
}

/// Basic test coverage analyzer implementation
pub struct BasicTestCoverageAnalyzer;

impl BasicTestCoverageAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Find test files for a given source file
    fn find_test_files(&self, source_file: &Path, project_path: &Path) -> Vec<PathBuf> {
        let mut test_files = Vec::new();

        // Common test file patterns
        let file_stem = source_file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        // Check for inline tests
        test_files.push(source_file.to_path_buf());

        // Check for test module in same directory
        let test_mod = source_file.with_file_name(format!("{}_test.rs", file_stem));
        if test_mod.exists() {
            test_files.push(test_mod);
        }

        // Check for tests directory
        let tests_dir = project_path.join("tests");
        if tests_dir.exists() {
            let test_file = tests_dir.join(format!("{}_test.rs", file_stem));
            if test_file.exists() {
                test_files.push(test_file);
            }
        }

        test_files
    }

    /// Extract functions from Rust code
    fn extract_functions(&self, content: &str) -> Vec<(String, u32)> {
        let mut functions = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if (line.starts_with("pub fn")
                || line.starts_with("fn")
                || line.starts_with("pub async fn")
                || line.starts_with("async fn"))
                && !line.contains("#[test]")
            {
                if let Some(name_start) = line.find("fn ") {
                    let after_fn = &line[name_start + 3..];
                    if let Some(name) = after_fn.split(|c: char| c == '(' || c == '<').next() {
                        functions.push((name.trim().to_string(), line_num as u32 + 1));
                    }
                }
            }
        }

        functions
    }

    /// Extract test functions from Rust code
    fn extract_tests(&self, content: &str) -> Vec<String> {
        let mut tests = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            if line.contains("#[test]") {
                // Look at the next line for the function
                if i + 1 < lines.len() {
                    let next_line = lines[i + 1];
                    if let Some(name_start) = next_line.find("fn ") {
                        let after_fn = &next_line[name_start + 3..];
                        if let Some(name) = after_fn.split(|c: char| c == '(' || c == '<').next() {
                            tests.push(name.trim().to_string());
                        }
                    }
                }
            }
        }

        tests
    }

    /// Estimate coverage based on function names and test names
    fn estimate_coverage(&self, functions: &[(String, u32)], tests: &[String]) -> f64 {
        if functions.is_empty() {
            return 1.0; // No functions to test
        }

        let mut tested_count = 0;

        for (func_name, _) in functions {
            // Check if there's a test that likely tests this function
            let is_tested = tests.iter().any(|test_name| {
                test_name.contains(func_name)
                    || test_name.contains(&func_name.to_lowercase())
                    || (func_name.starts_with("get_") && test_name.contains(&func_name[4..]))
                    || (func_name.starts_with("set_") && test_name.contains(&func_name[4..]))
            });

            if is_tested {
                tested_count += 1;
            }
        }

        tested_count as f64 / functions.len() as f64
    }

    /// Determine criticality of a function
    fn determine_criticality(&self, func_name: &str, file_path: &Path) -> Criticality {
        let path_str = file_path.to_string_lossy();

        // High criticality patterns
        if func_name.contains("auth")
            || func_name.contains("security")
            || func_name.contains("payment")
            || func_name.contains("crypto")
            || path_str.contains("auth")
            || path_str.contains("security")
        {
            return Criticality::High;
        }

        // Medium criticality patterns
        if func_name.contains("save")
            || func_name.contains("delete")
            || func_name.contains("update")
            || func_name.contains("process")
            || path_str.contains("core")
            || path_str.contains("api")
        {
            return Criticality::Medium;
        }

        Criticality::Low
    }

    /// Identify critical paths
    fn identify_critical_paths(&self, project_path: &Path) -> Vec<CriticalPath> {
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
impl TestCoverageAnalyzer for BasicTestCoverageAnalyzer {
    async fn analyze_coverage(&self, project_path: &Path) -> Result<TestCoverageMap> {
        use walkdir::WalkDir;

        let mut file_coverage = HashMap::new();
        let mut all_untested_functions = Vec::new();
        let mut total_tested_lines = 0;
        let mut total_lines = 0;

        // Collect all test functions first
        let mut all_tests = Vec::new();
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let path = entry.path();
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let tests = self.extract_tests(&content);
                all_tests.extend(tests);
            }
        }

        // Analyze each source file
        for entry in WalkDir::new(project_path)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') && name != "target" && name != "node_modules"
            })
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
        {
            let path = entry.path();
            let relative_path = path.strip_prefix(project_path).unwrap_or(path);

            // Skip test files for coverage calculation
            let path_str = path.to_string_lossy();
            if path_str.contains("/tests/") || path_str.contains("_test.rs") {
                continue;
            }

            if let Ok(content) = tokio::fs::read_to_string(path).await {
                let functions = self.extract_functions(&content);
                let line_count = content.lines().count() as u32;

                // Find tests for this file
                let test_files = self.find_test_files(relative_path, project_path);
                let mut file_tests = Vec::new();

                for test_file in test_files {
                    if let Ok(test_content) = tokio::fs::read_to_string(&test_file).await {
                        file_tests.extend(self.extract_tests(&test_content));
                    }
                }

                // Estimate coverage
                let coverage = self.estimate_coverage(&functions, &file_tests);
                let tested_functions = (functions.len() as f64 * coverage) as u32;
                let tested_lines = (line_count as f64 * coverage) as u32;

                // Find untested functions
                for (func_name, line_num) in &functions {
                    let is_tested = file_tests.iter().any(|test| test.contains(func_name));
                    if !is_tested {
                        all_untested_functions.push(UntestedFunction {
                            file: relative_path.to_path_buf(),
                            name: func_name.clone(),
                            line_number: *line_num,
                            criticality: self.determine_criticality(func_name, relative_path),
                        });
                    }
                }

                file_coverage.insert(
                    relative_path.to_path_buf(),
                    FileCoverage {
                        path: relative_path.to_path_buf(),
                        coverage_percentage: coverage,
                        tested_lines,
                        total_lines: line_count,
                        tested_functions,
                        total_functions: functions.len() as u32,
                        has_tests: !file_tests.is_empty(),
                    },
                );

                total_tested_lines += tested_lines;
                total_lines += line_count;
            }
        }

        let overall_coverage = if total_lines > 0 {
            total_tested_lines as f64 / total_lines as f64
        } else {
            0.0
        };

        let critical_paths = self.identify_critical_paths(project_path);

        Ok(TestCoverageMap {
            file_coverage,
            untested_functions: all_untested_functions,
            critical_paths,
            overall_coverage,
        })
    }

    async fn update_coverage(
        &self,
        project_path: &Path,
        current: &TestCoverageMap,
        changed_files: &[PathBuf],
    ) -> Result<TestCoverageMap> {
        let mut updated_map = current.clone();

        // Re-analyze changed files and their test files
        for file in changed_files {
            if file.extension().and_then(|s| s.to_str()) == Some("rs") {
                let full_path = project_path.join(file);
                if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                    let functions = self.extract_functions(&content);
                    let line_count = content.lines().count() as u32;

                    // Find tests
                    let test_files = self.find_test_files(file, project_path);
                    let mut file_tests = Vec::new();

                    for test_file in test_files {
                        if let Ok(test_content) = tokio::fs::read_to_string(&test_file).await {
                            file_tests.extend(self.extract_tests(&test_content));
                        }
                    }

                    let coverage = self.estimate_coverage(&functions, &file_tests);
                    let tested_functions = (functions.len() as f64 * coverage) as u32;
                    let tested_lines = (line_count as f64 * coverage) as u32;

                    updated_map.file_coverage.insert(
                        file.clone(),
                        FileCoverage {
                            path: file.clone(),
                            coverage_percentage: coverage,
                            tested_lines,
                            total_lines: line_count,
                            tested_functions,
                            total_functions: functions.len() as u32,
                            has_tests: !file_tests.is_empty(),
                        },
                    );
                }
            }
        }

        // Recalculate overall coverage
        let (total_tested, total) = updated_map
            .file_coverage
            .values()
            .fold((0, 0), |(tested, total), cov| {
                (tested + cov.tested_lines, total + cov.total_lines)
            });

        updated_map.overall_coverage = if total > 0 {
            total_tested as f64 / total as f64
        } else {
            0.0
        };

        Ok(updated_map)
    }
}
