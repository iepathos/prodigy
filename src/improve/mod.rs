pub mod command;
pub mod session;

use crate::analyzer::ProjectAnalyzer;
use crate::simple_state::StateManager;
use anyhow::{anyhow, Context as _, Result};
use chrono::Utc;
use std::path::Path;
use tokio::process::Command;

/// Run the improve command to automatically enhance code quality
///
/// # Arguments
/// * `cmd` - The improve command with optional target score, verbosity, and focus directive
///
/// # Returns
/// Result indicating success or failure of the improvement process
///
/// # Errors
/// Returns an error if:
/// - Project analysis fails
/// - Claude CLI is not available
/// - File operations fail
/// - Git operations fail
///
/// # Thread Safety
/// This function performs git operations sequentially and is not designed for concurrent
/// execution. If running multiple instances, ensure they operate on different repositories
/// to avoid git conflicts.
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

    // Display focus directive on first iteration if provided
    if let Some(focus) = &cmd.focus {
        println!("📋 Focus: {focus} (initial analysis)");
    }

    while current_score < cmd.target && iteration <= 10 {
        if cmd.show_progress {
            println!("🔄 Iteration {iteration}/10...");
        }

        // Step 1: Generate review spec and commit
        let focus_for_iteration = if iteration == 1 {
            cmd.focus.as_deref()
        } else {
            None
        };
        let review_success =
            call_claude_code_review(cmd.show_progress, focus_for_iteration).await?;
        if !review_success {
            if cmd.show_progress {
                println!("Review failed - stopping iterations");
            }
            break;
        }

        // Step 2: Extract spec ID from latest commit
        let spec_id = extract_spec_from_git(cmd.show_progress).await?;
        if spec_id.is_empty() {
            if cmd.show_progress {
                println!("No issues found - stopping iterations");
            }
            break;
        }

        // Step 3: Implement fixes and commit
        let implement_success = call_claude_implement_spec(&spec_id, cmd.show_progress).await?;
        if !implement_success {
            if cmd.show_progress {
                println!("Implementation failed for iteration {iteration}");
            }
        } else {
            files_changed += 1;
        }

        // Step 4: Run linting/formatting and commit
        call_claude_lint(cmd.show_progress).await?;

        // Step 5: Re-analyze project
        let new_analysis = analyzer.analyze(Path::new(".")).await?;
        current_score = new_analysis.health_score;
        if cmd.show_progress {
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

async fn call_claude_code_review(verbose: bool, focus: Option<&str>) -> Result<bool> {
    println!("🤖 Running /mmm-code-review...");

    // First check if claude command exists
    let claude_check = Command::new("which")
        .arg("claude")
        .output()
        .await
        .context("Failed to check for Claude CLI")?;

    if !claude_check.status.success() {
        return Err(anyhow!(
            "Claude CLI not found. Please install Claude CLI: https://claude.ai/cli"
        ));
    }

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-code-review") // The custom command for code review
        .env("MMM_AUTOMATION", "true"); // Signals to /mmm-code-review to run in automated mode

    // Pass focus directive via environment variable on first iteration
    if let Some(focus_directive) = focus {
        cmd.env("MMM_FOCUS", focus_directive);
    }

    let output = cmd
        .output()
        .await
        .context("Failed to execute Claude CLI for review")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if verbose {
            eprintln!("Claude CLI stderr: {stderr}");
        }
        return Err(anyhow!(
            "Claude CLI failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    if verbose {
        println!("✅ Code review completed");
    }

    Ok(true)
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
    println!("🔧 Running /mmm-implement-spec {spec_id}...");

    let output = Command::new("claude")
        .arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-implement-spec") // The custom command for spec implementation
        .arg(spec_id) // The spec ID to implement (e.g., "iteration-123-improvements")
        .env("MMM_AUTOMATION", "true") // Signals to /mmm-implement-spec to run in automated mode
        .output()
        .await
        .context("Failed to execute Claude CLI for implementation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if verbose {
            eprintln!("Claude CLI stderr: {stderr}");
        }
        return Err(anyhow!(
            "Claude CLI failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    if verbose {
        println!("✅ Implementation completed");
    }

    Ok(true)
}

async fn call_claude_lint(verbose: bool) -> Result<bool> {
    println!("🧹 Running /mmm-lint...");

    let output = Command::new("claude")
        .arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-lint") // The custom command for linting and formatting
        .env("MMM_AUTOMATION", "true") // Signals to /mmm-lint to run in automated mode
        .output()
        .await
        .context("Failed to execute Claude CLI for linting")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if verbose {
            eprintln!("Claude CLI stderr: {stderr}");
        }
        return Err(anyhow!(
            "Claude CLI failed with exit code {}: {}",
            output.status.code().unwrap_or(-1),
            stderr
        ));
    }

    if verbose {
        println!("✅ Linting completed");
    }

    Ok(true)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_extract_spec_from_git_commit_message() {
        // Test parsing spec ID from commit message
        let test_cases = vec![
            (
                "review: generate improvement spec for iteration-1234567890-improvements",
                "iteration-1234567890-improvements",
            ),
            (
                "review: generate improvement spec for iteration-9876543210-improvements with extra text",
                "iteration-9876543210-improvements",
            ),
            (
                "some other commit message without spec",
                "",
            ),
            (
                "partial iteration- without complete spec",
                "iteration-",
            ),
        ];

        for (input, expected) in test_cases {
            // Simulate the parsing logic from extract_spec_from_git
            let result = if let Some(spec_start) = input.find("iteration-") {
                let spec_part = &input[spec_start..];
                if let Some(spec_end) = spec_part.find(' ') {
                    spec_part[..spec_end].to_string()
                } else {
                    spec_part.to_string()
                }
            } else {
                String::new()
            };

            assert_eq!(result, expected, "Failed for input: {input}");
        }
    }
}
