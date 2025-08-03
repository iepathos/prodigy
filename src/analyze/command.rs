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
            println!("🔄 {}", message);
        }
    }

    fn display_info(&self, message: &str) {
        if self.verbose {
            println!("ℹ️  {}", message);
        }
    }

    fn display_warning(&self, message: &str) {
        println!("⚠️  {}", message);
    }

    fn display_success(&self, message: &str) {
        println!("✅ {}", message);
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
    let progress = Arc::new(CommandProgressReporter { verbose: cmd.verbose });

    // Build unified analysis config
    let config = AnalysisConfig::builder()
        .output_format(OutputFormat::from_str(&cmd.output))
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