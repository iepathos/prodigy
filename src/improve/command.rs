use super::{
    analyzer::ProjectAnalyzer,
    context::ContextBuilder,
    session::{ImproveOptions, ImproveSession},
};
use anyhow::{Context as _, Result};
use clap::Args;
use std::path::Path;

#[derive(Debug, Args)]
pub struct ImproveCommand {
    /// Target quality score (default: 8.0)
    #[arg(long, default_value = "8.0")]
    pub target: f32,

    /// Show detailed progress
    #[arg(short, long)]
    pub verbose: bool,
}

impl From<ImproveCommand> for ImproveOptions {
    fn from(cmd: ImproveCommand) -> Self {
        Self {
            target_score: cmd.target,
            verbose: cmd.verbose,
        }
    }
}

pub async fn run(cmd: ImproveCommand) -> Result<()> {
    let options: ImproveOptions = cmd.into();

    println!("Analyzing project...");
    let project_path = Path::new(".");
    let project = ProjectAnalyzer::analyze(project_path)
        .await
        .context("Failed to analyze project")?;
    println!("✓ {}", project.summary());

    println!("Building context...");
    let context = ContextBuilder::build(&project, project_path)
        .await
        .context("Failed to build context")?;
    println!("✓ Context built successfully");

    let mut session = ImproveSession::start(project, context, options)
        .await
        .context("Failed to start improvement session")?;

    let review_summary = session.summary();
    println!("Current score: {:.1}/10", review_summary.current_score);
    if review_summary.issues_found > 0 {
        println!("Issues found: {}", review_summary.issues_found);
    }

    if !session.is_good_enough() {
        println!("Running improvements...");
        let result = session.run().await.context("Failed to run improvements")?;
        println!("✓ Improvements complete");
        println!(
            "Score: {:.1} → {:.1} (+{:.1})",
            result.initial_score, result.final_score, result.improvement
        );
        println!("Files changed: {}", result.files_changed);
        println!("Iterations: {}", result.iterations);
    } else {
        println!("Your code already meets the target quality score!");
    }

    Ok(())
}
