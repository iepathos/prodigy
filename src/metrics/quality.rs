//! Code quality metrics collection

use crate::subprocess::{ProcessCommandBuilder, SubprocessManager};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::debug;

/// Quality metrics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub test_coverage: f32,
    pub type_coverage: f32,
    pub lint_warnings: u32,
    pub code_duplication: f32,
    pub doc_coverage: f32,
}

/// Analyzes code quality metrics
pub struct QualityAnalyzer {
    use_tarpaulin: bool,
    subprocess: SubprocessManager,
}

impl QualityAnalyzer {
    /// Create a new quality analyzer
    pub async fn new(subprocess: SubprocessManager) -> Self {
        // Check if cargo-tarpaulin is available
        let check_command = ProcessCommandBuilder::new("cargo")
            .args(["tarpaulin", "--version"])
            .build();

        let use_tarpaulin = subprocess
            .runner()
            .run(check_command)
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        Self {
            use_tarpaulin,
            subprocess,
        }
    }

    /// Analyze code quality metrics
    pub async fn analyze(&self, project_path: &Path) -> Result<QualityMetrics> {
        self.analyze_with_coverage(project_path, false).await
    }

    /// Analyze code quality metrics with optional coverage run
    pub async fn analyze_with_coverage(
        &self,
        project_path: &Path,
        run_coverage: bool,
    ) -> Result<QualityMetrics> {
        let mut metrics = QualityMetrics {
            test_coverage: 0.0,
            type_coverage: 0.0,
            lint_warnings: 0,
            code_duplication: 0.0,
            doc_coverage: 0.0,
        };

        // Get test coverage
        metrics.test_coverage = self.get_test_coverage(project_path, run_coverage).await?;

        // Get lint warnings count
        metrics.lint_warnings = self.get_lint_warnings(project_path).await?;

        // Get documentation coverage
        metrics.doc_coverage = self.get_doc_coverage(project_path).await?;

        // Get type coverage (simplified for now)
        metrics.type_coverage = self.estimate_type_coverage(project_path)?;

        Ok(metrics)
    }

    /// Get test coverage using cargo-tarpaulin
    async fn get_test_coverage(&self, project_path: &Path, run_coverage: bool) -> Result<f32> {
        if self.use_tarpaulin {
            debug!("Checking for cargo-tarpaulin coverage");

            let tarpaulin_path = project_path.join("target/coverage/tarpaulin-report.json");

            // If run_coverage is true, actually run tarpaulin
            if run_coverage {
                println!("🔬 Running cargo-tarpaulin for accurate test coverage...");

                // Create coverage directory if it doesn't exist
                if let Some(parent) = tarpaulin_path.parent() {
                    std::fs::create_dir_all(parent).ok();
                }

                // Ensure target/debug/deps exists before running tarpaulin
                let deps_dir = project_path.join("target/debug/deps");
                if let Err(e) = std::fs::create_dir_all(&deps_dir) {
                    eprintln!("⚠️  Failed to create target/debug/deps directory: {e}");
                }
                
                // Also create target/debug/.fingerprint
                let fingerprint_dir = project_path.join("target/debug/.fingerprint");
                if let Err(e) = std::fs::create_dir_all(&fingerprint_dir) {
                    eprintln!("⚠️  Failed to create target/debug/.fingerprint directory: {e}");
                }

                // Run tarpaulin with JSON output
                // Add --frozen to avoid updating dependencies and --lib to only test library
                // Check for MMM_SKIP_TARPAULIN env var for testing
                if std::env::var("MMM_SKIP_TARPAULIN").is_ok() {
                    eprintln!("⚠️  Skipping tarpaulin execution (MMM_SKIP_TARPAULIN set)");
                    return Ok(0.0); // Return 0.0 to indicate N/A
                }

                let tarpaulin_command = ProcessCommandBuilder::new("cargo")
                    .args([
                        "tarpaulin",
                        "--out",
                        "Json",
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

                match self.subprocess.runner().run(tarpaulin_command).await {
                    Ok(output) => {
                        if output.status.success() {
                            // Try to read the generated report
                            if let Ok(content) = std::fs::read_to_string(&tarpaulin_path) {
                                if let Ok(report) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    if let Some(coverage) =
                                        report.get("coverage").and_then(|c| c.as_f64())
                                    {
                                        println!(
                                            "✅ Test coverage analysis complete: {coverage:.1}%"
                                        );
                                        return Ok(coverage as f32);
                                    }
                                }
                            }
                        } else {
                            eprintln!(
                                "❌ cargo-tarpaulin failed with exit code: {:?}",
                                output.status.code()
                            );
                            eprintln!("   Error output: {}", output.stderr);
                            eprintln!("   💡 Try running 'cargo tarpaulin' manually to see detailed errors");
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to run cargo-tarpaulin: {e}");
                        eprintln!("   💡 Make sure cargo-tarpaulin is installed: cargo install cargo-tarpaulin");
                    }
                }
            }

            // Try to use existing tarpaulin coverage data (whether or not run_coverage is true)
            if tarpaulin_path.exists() {
                // Parse existing tarpaulin report
                if let Ok(content) = std::fs::read_to_string(&tarpaulin_path) {
                    if let Ok(report) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(coverage) = report.get("coverage").and_then(|c| c.as_f64()) {
                            debug!("Using existing tarpaulin coverage: {:.1}%", coverage);
                            return Ok(coverage as f32);
                        }
                    }
                }
            }
        }

        // No coverage data available
        eprintln!(
            "⚠️  No test coverage data available. Run 'cargo tarpaulin' to generate coverage data."
        );
        Ok(0.0) // Return 0.0 to indicate N/A
    }

    /// Get clippy warning count
    async fn get_lint_warnings(&self, project_path: &Path) -> Result<u32> {
        debug!("Running clippy to count warnings");

        let clippy_command = ProcessCommandBuilder::new("cargo")
            .args(["clippy", "--", "-W", "clippy::all", "--no-deps"])
            .current_dir(project_path)
            .build();

        let output = self
            .subprocess
            .runner()
            .run(clippy_command)
            .await
            .context("Failed to run clippy")?;

        let stderr = &output.stderr;

        // Count warning lines
        let warning_count = stderr
            .lines()
            .filter(|line| line.contains("warning:"))
            .count() as u32;

        Ok(warning_count)
    }

    /// Get documentation coverage
    async fn get_doc_coverage(&self, project_path: &Path) -> Result<f32> {
        debug!("Analyzing documentation coverage");

        let src_dir = project_path.join("src");
        if !src_dir.exists() {
            return Ok(0.0);
        }

        let mut total_items = 0;
        let mut documented_items = 0;

        // Walk through source files
        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let path = entry.path();
            let content = std::fs::read_to_string(path)?;
            let lines: Vec<&str> = content.lines().collect();

            for (i, line) in lines.iter().enumerate() {
                // Check for public items
                if line.starts_with("pub fn")
                    || line.starts_with("pub struct")
                    || line.starts_with("pub enum")
                    || line.starts_with("pub trait")
                    || line.starts_with("pub mod")
                {
                    total_items += 1;

                    // Check if documented (look for /// or //! above)
                    if i > 0
                        && (lines[i - 1].trim().starts_with("///")
                            || lines[i - 1].trim().starts_with("//!"))
                    {
                        documented_items += 1;
                    }
                }
            }
        }

        if total_items > 0 {
            Ok((documented_items as f32 / total_items as f32) * 100.0)
        } else {
            Ok(0.0)
        }
    }

    /// Estimate type coverage
    fn estimate_type_coverage(&self, project_path: &Path) -> Result<f32> {
        // Simplified estimation based on explicit type annotations
        let src_dir = project_path.join("src");
        if !src_dir.exists() {
            return Ok(0.0);
        }

        let mut total_bindings = 0;
        let mut typed_bindings = 0;

        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let path = entry.path();
            let content = std::fs::read_to_string(path)?;

            // Count let bindings
            total_bindings += content.matches("let ").count();

            // Count explicitly typed bindings
            typed_bindings += content
                .matches("let ")
                .count()
                .saturating_sub(content.matches("let mut").count())
                .saturating_sub(content.matches("let _").count());

            // Count function parameters with types (rough estimate)
            typed_bindings += content.matches(": ").count() / 2;
        }

        // Rust has good type inference, so assume 80% baseline
        let base_coverage = 80.0;
        if total_bindings > 0 {
            let explicit_coverage = (typed_bindings as f32 / total_bindings as f32) * 20.0;
            Ok(base_coverage + explicit_coverage)
        } else {
            Ok(base_coverage)
        }
    }
}

impl Default for QualityAnalyzer {
    fn default() -> Self {
        // Since new() is async, we can't use it in a sync default() method.
        // For simplicity, we'll use a production subprocess manager and assume tarpaulin is not available.
        Self {
            use_tarpaulin: false,
            subprocess: SubprocessManager::production(),
        }
    }
}
