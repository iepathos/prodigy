//! Analysis runner implementation

use super::{AnalysisCoordinator, AnalysisMetadata, AnalysisResult};
use crate::context::{ContextAnalyzer, ProjectAnalyzer};
use crate::cook::execution::CommandRunner;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::path::Path;
use std::time::Instant;

/// Trait for running analysis
#[async_trait]
pub trait AnalysisRunner: Send + Sync {
    /// Run analysis with coverage option
    async fn run_analysis(&self, path: &Path, with_coverage: bool) -> Result<AnalysisResult>;

    /// Check if project type is supported
    async fn is_supported_project(&self, path: &Path) -> Result<bool>;
}

/// Implementation of analysis runner
pub struct AnalysisRunnerImpl<R: CommandRunner> {
    runner: R,
}

impl<R: CommandRunner> AnalysisRunnerImpl<R> {
    /// Create a new analysis runner
    pub fn new(runner: R) -> Self {
        Self { runner }
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> AnalysisRunner for AnalysisRunnerImpl<R> {
    async fn run_analysis(&self, path: &Path, with_coverage: bool) -> Result<AnalysisResult> {
        let start = Instant::now();

        // Use existing analyzer
        let project_analyzer = ProjectAnalyzer::new();

        // Run full context analysis
        let analysis = project_analyzer
            .analyze(path)
            .await
            .context("Failed to analyze project context")?;

        // Get test coverage if requested
        let test_coverage = if with_coverage {
            match self.get_rust_coverage(path).await {
                Ok(coverage) => Some(coverage),
                Err(e) => {
                    eprintln!("Warning: Failed to get test coverage: {e}");
                    None
                }
            }
        } else {
            None
        };

        Ok(AnalysisResult {
            dependency_graph: serde_json::to_value(&analysis.dependency_graph)?,
            architecture: serde_json::to_value(&analysis.architecture)?,
            conventions: serde_json::to_value(&analysis.conventions)?,
            technical_debt: serde_json::to_value(&analysis.technical_debt)?,
            test_coverage,
            metadata: AnalysisMetadata {
                timestamp: Utc::now(),
                duration_ms: start.elapsed().as_millis() as u64,
                files_analyzed: analysis.metadata.files_analyzed,
                incremental: false,
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        })
    }

    async fn is_supported_project(&self, path: &Path) -> Result<bool> {
        // Check for common project files that indicate a supported project
        let project_files = [
            "Cargo.toml",       // Rust
            "package.json",     // Node.js
            "pyproject.toml",   // Python
            "poetry.lock",      // Python Poetry
            "requirements.txt", // Python pip
            "go.mod",           // Go
            "pom.xml",          // Java Maven
            "build.gradle",     // Java Gradle
            "Gemfile",          // Ruby
            "composer.json",    // PHP
        ];

        for file in &project_files {
            if path.join(file).exists() {
                return Ok(true);
            }
        }

        // Also check if directory has source files
        let source_extensions = [
            "rs", "js", "ts", "py", "go", "java", "rb", "php", "cpp", "c", "h",
        ];

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if let Some(ext_str) = extension.to_str() {
                        if source_extensions.contains(&ext_str) {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }
}

impl<R: CommandRunner + 'static> AnalysisRunnerImpl<R> {
    /// Get Rust test coverage using cargo-tarpaulin
    async fn get_rust_coverage(&self, _path: &Path) -> Result<serde_json::Value> {
        // Check if cargo-tarpaulin is installed
        let check_output = self
            .runner
            .run_command("cargo", &["tarpaulin".to_string(), "--version".to_string()])
            .await;

        if check_output.is_err() || !check_output.unwrap().status.success() {
            anyhow::bail!("cargo-tarpaulin not installed");
        }

        // Run coverage
        let output = self
            .runner
            .run_command(
                "cargo",
                &[
                    "tarpaulin".to_string(),
                    "--out".to_string(),
                    "Json".to_string(),
                    "--skip-clean".to_string(),
                ],
            )
            .await?;

        if output.status.success() {
            let coverage_json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            Ok(coverage_json)
        } else {
            anyhow::bail!("Failed to run cargo-tarpaulin")
        }
    }
}

#[async_trait]
impl<R: CommandRunner + 'static> AnalysisCoordinator for AnalysisRunnerImpl<R> {
    async fn analyze_project(&self, project_path: &Path) -> Result<AnalysisResult> {
        self.run_analysis(project_path, false).await
    }

    async fn analyze_incremental(
        &self,
        project_path: &Path,
        _changed_files: &[String],
    ) -> Result<AnalysisResult> {
        // For now, fall back to full analysis
        // TODO: Implement incremental analysis
        self.run_analysis(project_path, false).await
    }

    async fn get_cached_analysis(&self, _project_path: &Path) -> Result<Option<AnalysisResult>> {
        // Cache implementation would go here
        Ok(None)
    }

    async fn save_analysis(&self, project_path: &Path, analysis: &AnalysisResult) -> Result<()> {
        let analysis_path = project_path.join(".mmm/context/analysis.json");
        if let Some(parent) = analysis_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(analysis)?;
        tokio::fs::write(&analysis_path, json).await?;
        Ok(())
    }

    async fn clear_cache(&self, project_path: &Path) -> Result<()> {
        let cache_path = project_path.join(".mmm/cache");
        if cache_path.exists() {
            tokio::fs::remove_dir_all(&cache_path).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::runner::tests::MockCommandRunner;
    use crate::cook::execution::ExecutionResult;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_analysis_runner_unsupported_project() {
        let temp_dir = TempDir::new().unwrap();
        let mock_runner = MockCommandRunner::new();
        let runner = AnalysisRunnerImpl::new(mock_runner);

        // Empty directory should not be supported
        let supported = runner.is_supported_project(temp_dir.path()).await.unwrap();
        assert!(!supported);
    }

    #[tokio::test]
    async fn test_coverage_check() {
        let mock_runner = MockCommandRunner::new();

        // Mock tarpaulin not installed
        mock_runner.add_response(ExecutionResult {
            success: false,
            stdout: String::new(),
            stderr: "command not found".to_string(),
            exit_code: Some(127),
        });

        let runner = AnalysisRunnerImpl::new(mock_runner);
        let result = runner.get_rust_coverage(Path::new("/tmp")).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cargo-tarpaulin not installed"));
    }
}
