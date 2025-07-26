use anyhow::{Context as _, Result};
use clap::Args;
use std::path::Path;
use std::time::Instant;

use super::{
    analyzer::ProjectAnalyzer,
    context::ContextBuilder,
    display,
    session::{ImproveOptions, ImproveSession, ImprovementType},
};

use crate::developer_experience::{
    ProgressDisplay, Phase, ResultSummary, QualityScore, ImpactMetrics,
    InterruptHandler, LivePreview, ChangeDecision,
    ErrorHandler, RollbackManager,
    SmartHelper, ContextualHelp,
    Achievement, AchievementManager, Streak, SuccessMessage,
    FastStartup, IncrementalProcessor,
};

#[derive(Debug, Args)]
pub struct ImproveCommand {
    /// Focus on specific area (e.g., tests, errors, perf)
    #[arg(long)]
    pub focus: Option<String>,

    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Automatically commit improvements
    #[arg(long)]
    pub auto_commit: bool,

    /// Show what would be improved without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Show detailed progress
    #[arg(short, long)]
    pub verbose: bool,

    /// Interactive preview mode
    #[arg(long)]
    pub preview: bool,

    /// Resume from previous session
    #[arg(long)]
    pub resume: bool,

    /// Conservative improvements only
    #[arg(long)]
    pub conservative: bool,

    /// Quick improvements only
    #[arg(long)]
    pub quick: bool,
}

impl From<ImproveCommand> for ImproveOptions {
    fn from(cmd: ImproveCommand) -> Self {
        Self {
            focus: cmd.focus,
            target_score: cmd.target,
            auto_commit: cmd.auto_commit,
            dry_run: cmd.dry_run,
            verbose: cmd.verbose,
        }
    }
}

pub async fn run(cmd: ImproveCommand) -> Result<()> {
    let options: ImproveOptions = cmd.into();

    // Show welcome message
    display::show_welcome();

    if options.dry_run {
        display::show_dry_run_notice();
    }

    // Analyze project
    let spinner = display::ProgressSpinner::new("Analyzing project...");
    let project_path = Path::new(".");
    let project = ProjectAnalyzer::analyze(project_path)
        .await
        .context("Failed to analyze project")?;
    spinner.success(&project.summary());

    // Show analysis results
    display::show_analysis_results(&project.summary(), &project.focus_areas);

    // Build context
    let spinner = display::ProgressSpinner::new("Building context...");
    let context = ContextBuilder::build(&project, project_path)
        .await
        .context("Failed to build context")?;
    spinner.success("Context built successfully");

    // Start improvement session
    let mut session = ImproveSession::start(project, context, options)
        .await
        .context("Failed to start improvement session")?;

    // Show initial review
    let review_summary = session.summary();
    display::show_review_results(&review_summary);

    // Run improvements
    if !session.is_good_enough() {
        let spinner = display::ProgressSpinner::new("Running improvements...");
        let result = session.run().await.context("Failed to run improvements")?;
        spinner.success("Improvements complete");

        // Show results
        display::show_results(&result);
    } else {
        println!("\n🎉 Your code already meets the target quality score!");
    }

    Ok(())
}
