pub mod command;
pub mod session;

use crate::analyzer::{AnalyzerResult, ProjectAnalyzer};
use crate::simple_state::StateManager;
use anyhow::{Context as _, Result};
use chrono::Utc;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
    println!("🔍 Analyzing project...");
    
    // 1. Initial analysis
    let analyzer = ProjectAnalyzer::new();
    let analysis = analyzer.analyze(Path::new(".")).await?;
    let mut current_score = analysis.health_score;
    
    println!("Current score: {:.1}/10", current_score);
    
    if current_score >= cmd.target {
        println!("✅ Target already reached!");
        return Ok(());
    }
    
    // 2. State setup
    let mut state = StateManager::new()?;
    
    // 3. Improvement loop
    let mut iteration = 1;
    let mut files_changed = 0;
    while current_score < cmd.target && iteration <= 10 {
        if cmd.verbose {
            println!("🔄 Iteration {}/10...", iteration);
        }
        
        // Call Claude CLI for review and implementation
        let improved = call_claude_improve(&analysis, cmd.verbose).await?;
        if improved {
            files_changed += 1;
            // Re-analyze to get new score
            let new_analysis = analyzer.analyze(Path::new(".")).await?;
            current_score = new_analysis.health_score;
            if cmd.verbose {
                println!("Score: {:.1}/10", current_score);
            }
        } else {
            if cmd.verbose {
                println!("No improvements made in this iteration");
            }
            break;
        }
        
        iteration += 1;
    }
    
    // 4. Completion - record basic session info
    state.state_mut().current_score = current_score;
    state.state_mut().total_runs += 1;
    state.state_mut().last_run = Some(Utc::now());
    state.save()?;
    
    println!("✅ Complete! Final score: {:.1}/10", current_score);
    println!("Files changed: {}", files_changed);
    println!("Iterations: {}", iteration - 1);
    
    Ok(())
}

async fn call_claude_improve(_analysis: &AnalyzerResult, verbose: bool) -> Result<bool> {
    if verbose {
        println!("Calling Claude CLI for code review...");
    }
    
    // Call claude /mmm-code-review
    let review_cmd = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-code-review")
        .env("MMM_AUTOMATION", "true")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to execute Claude CLI for review")?;

    let review_output = review_cmd
        .wait_with_output()
        .await
        .context("Failed to wait for Claude CLI review")?;

    if !review_output.status.success() {
        let stderr = String::from_utf8_lossy(&review_output.stderr);
        if verbose {
            println!("Claude review failed: {}", stderr);
        }
        return Ok(false);
    }

    let review_stdout = String::from_utf8_lossy(&review_output.stdout);
    if verbose {
        println!("Review completed");
    }

    // Check if there are issues to address
    if review_stdout.contains("No issues found") || review_stdout.trim().is_empty() {
        if verbose {
            println!("No issues found to address");
        }
        return Ok(false);
    }

    // Call claude /mmm-implement-spec for improvements
    if verbose {
        println!("Calling Claude CLI to implement improvements...");
    }
    
    let implement_cmd = Command::new("claude")
        .arg("--dangerously-skip-permissions")
        .arg("/mmm-implement-spec")
        .arg("improvements")
        .env("MMM_AUTOMATION", "true")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to execute Claude CLI for implementation")?;

    let implement_output = implement_cmd
        .wait_with_output()
        .await
        .context("Failed to wait for Claude CLI implementation")?;

    if !implement_output.status.success() {
        let stderr = String::from_utf8_lossy(&implement_output.stderr);
        if verbose {
            println!("Claude implementation failed: {}", stderr);
        }
        return Ok(false);
    }

    let implement_stdout = String::from_utf8_lossy(&implement_output.stdout);
    if verbose {
        println!("Implementation completed");
    }

    // Check if changes were made (simple heuristic)
    let changes_made = implement_stdout.contains("✓") || 
                      implement_stdout.contains("Modified:") || 
                      implement_stdout.contains("Created:") ||
                      implement_stdout.contains("Updated:");

    Ok(changes_made)
}