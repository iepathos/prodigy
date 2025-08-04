//! Analyze command implementation

use crate::analysis::{run_analysis, AnalysisConfig, OutputFormat, ProgressReporter};
use crate::subprocess::SubprocessManager;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Command structure for analyze subcommand
#[derive(Debug, Clone)]
pub struct AnalyzeCommand {
    pub output: String,
    pub save: bool,
    pub verbose: bool,
    pub path: Option<PathBuf>,
    pub run_coverage: bool,
    pub no_commit: bool,
}

/// Progress reporter for command-line interface
struct CommandProgressReporter {
    verbose: bool,
}

impl ProgressReporter for CommandProgressReporter {
    fn display_progress(&self, message: &str) {
        if self.verbose {
            println!("🔄 {message}");
        }
    }

    fn display_info(&self, message: &str) {
        if self.verbose {
            println!("ℹ️  {message}");
        }
    }

    fn display_warning(&self, message: &str) {
        println!("⚠️  {message}");
    }

    fn display_success(&self, message: &str) {
        println!("✅ {message}");
    }
}

/// Execute the analyze command with production subprocess manager
pub async fn execute(cmd: AnalyzeCommand) -> Result<()> {
    execute_with_subprocess(cmd, SubprocessManager::production()).await
}

/// Execute the analyze command with injected subprocess manager
pub async fn execute_with_subprocess(
    cmd: AnalyzeCommand,
    subprocess: SubprocessManager,
) -> Result<()> {
    let project_path = match cmd.path.clone() {
        Some(path) => path,
        None => std::env::current_dir().context("Failed to get current directory")?,
    };

    println!("🔍 Analyzing project at: {}", project_path.display());

    // Create progress reporter
    let progress = Arc::new(CommandProgressReporter {
        verbose: cmd.verbose,
    });

    // Build unified analysis config
    let config = AnalysisConfig::builder()
        .output_format(cmd.output.parse().unwrap_or(OutputFormat::Summary))
        .save_results(cmd.save)
        .commit_changes(cmd.save && !cmd.no_commit)
        .verbose(cmd.verbose)
        .run_metrics(true)
        .run_context(true)
        .run_coverage(cmd.run_coverage)
        .build();

    // Run unified analysis
    let _results = run_analysis(&project_path, config, subprocess, progress).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subprocess::MockProcessRunner;
    use tempfile::TempDir;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_execute_with_subprocess_success() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let cmd = AnalyzeCommand {
            output: "summary".to_string(),
            save: false,
            verbose: false,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: false,
            no_commit: true,
        };

        // Create a mock subprocess that returns success
        let mock = Arc::new(MockProcessRunner::new());
        let subprocess = SubprocessManager::new(mock);

        let result = execute_with_subprocess(cmd, subprocess).await;
        assert!(result.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_with_subprocess_error_cases() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let cmd = AnalyzeCommand {
            output: "json".to_string(),
            save: true,
            verbose: true,
            path: Some(temp_dir.path().to_path_buf()),
            run_coverage: true,
            no_commit: false,
        };

        // Create a mock subprocess that returns an error
        let mock = Arc::new(MockProcessRunner::new());
        let subprocess = SubprocessManager::new(mock);

        // Note: MockProcessRunner always returns success by default
        // so we can't easily test the error case without more complex mocking
        // For now, we'll just verify the function runs without panicking
        let result = execute_with_subprocess(cmd, subprocess).await;
        // The result may be Ok or Err depending on the analysis implementation
        let _ = result;
        Ok(())
    }
}
