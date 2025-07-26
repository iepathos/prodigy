pub mod command;
pub mod session;

use crate::analyzer::ProjectAnalyzer;
use crate::simple_state::StateManager;
use anyhow::{Context as _, Result};
use chrono::Utc;
use std::path::Path;
use tokio::process::Command;

pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
    println!("🔍 Analyzing project...");

    // 1. Initial analysis
    let analyzer = ProjectAnalyzer::new();
    let analysis = analyzer.analyze(Path::new(".")).await?;
    let mut current_score = analysis.health_score;

    println!("Current score: {current_score:.1}/10");

    if current_score >= cmd.target {
        println!("✅ Target already reached!");
        return Ok(());
    }

    // 2. State setup
    let mut state = StateManager::new()?;

    // 3. Git-native improvement loop
    let mut iteration = 1;
    let mut files_changed = 0;
    while current_score < cmd.target && iteration <= 10 {
        if cmd.verbose {
            println!("🔄 Iteration {iteration}/10...");
        }

        // Step 1: Generate review spec and commit
        let review_success = call_claude_code_review(cmd.verbose).await?;
        if !review_success {
            if cmd.verbose {
                println!("Review failed - stopping iterations");
            }
            break;
        }

        // Step 2: Extract spec ID from latest commit
        let spec_id = extract_spec_from_git(cmd.verbose).await?;
        if spec_id.is_empty() {
            if cmd.verbose {
                println!("No issues found - stopping iterations");
            }
            break;
        }

        // Step 3: Implement fixes and commit
        let implement_success = call_claude_implement_spec(&spec_id, cmd.verbose).await?;
        if !implement_success {
            if cmd.verbose {
                println!("Implementation failed for iteration {iteration}");
            }
        } else {
            files_changed += 1;
        }

        // Step 4: Run linting/formatting and commit
        call_claude_lint(cmd.verbose).await?;

        // Step 5: Re-analyze project
        let new_analysis = analyzer.analyze(Path::new(".")).await?;
        current_score = new_analysis.health_score;
        if cmd.verbose {
            println!("Score: {current_score:.1}/10");
        }

        iteration += 1;
    }

    // 4. Completion - record basic session info
    state.state_mut().current_score = current_score;
    state.state_mut().total_runs += 1;
    state.state_mut().last_run = Some(Utc::now());
    state.save()?;

    println!("✅ Complete! Final score: {current_score:.1}/10");
    println!("Files changed: {files_changed}");
    println!("Iterations: {}", iteration - 1);

    Ok(())
}

async fn call_claude_code_review(verbose: bool) -> Result<bool> {
    if verbose {
        println!("Calling Claude CLI for code review...");
    }

    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-code-review")
        .env("MMM_AUTOMATION", "true")
        .status()
        .await
        .context("Failed to execute Claude CLI for review")?;

    Ok(status.success())
}

async fn extract_spec_from_git(verbose: bool) -> Result<String> {
    if verbose {
        println!("Extracting spec ID from git history...");
    }

    let output = Command::new("git")
        .args(["log", "-1", "--pretty=format:%s"])
        .output()
        .await
        .context("Failed to get git log")?;

    let commit_message = String::from_utf8_lossy(&output.stdout);

    // Parse commit message like "review: generate improvement spec for iteration-1234567890-improvements"
    if let Some(spec_start) = commit_message.find("iteration-") {
        let spec_part = &commit_message[spec_start..];
        if let Some(spec_end) = spec_part.find(' ') {
            Ok(spec_part[..spec_end].to_string())
        } else {
            Ok(spec_part.to_string())
        }
    } else {
        Ok(String::new()) // No spec found
    }
}

async fn call_claude_implement_spec(spec_id: &str, verbose: bool) -> Result<bool> {
    if verbose {
        println!("Calling Claude CLI to implement spec: {spec_id}");
    }

    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-implement-spec")
        .arg(spec_id)
        .env("MMM_AUTOMATION", "true")
        .status()
        .await
        .context("Failed to execute Claude CLI for implementation")?;

    Ok(status.success())
}

async fn call_claude_lint(verbose: bool) -> Result<bool> {
    if verbose {
        println!("Calling Claude CLI for linting...");
    }

    let status = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-lint")
        .env("MMM_AUTOMATION", "true")
        .status()
        .await
        .context("Failed to execute Claude CLI for linting")?;

    Ok(status.success())
}
