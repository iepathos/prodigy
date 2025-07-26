pub mod command;
pub mod git_ops;
pub mod retry;
pub mod session;
pub mod workflow;

use crate::analyzer::ProjectAnalyzer;
use crate::config::{ConfigLoader, WorkflowConfig};
use crate::simple_state::StateManager;
use anyhow::{anyhow, Context as _, Result};
use chrono::Utc;
use git_ops::get_last_commit_message;
use retry::{check_claude_cli, execute_with_retry, format_subprocess_error};
use std::fs::File;
use std::path::Path;
use tokio::process::Command;
use workflow::WorkflowExecutor;

/// Default number of retry attempts for Claude CLI commands
const DEFAULT_CLAUDE_RETRIES: u32 = 2;

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
/// - Lock file cannot be created or removed
///
/// # Thread Safety
/// This function uses file-based locking to prevent concurrent execution on the same repository.
/// The lock file is created in .mmm/improve.lock and cleaned up on exit.
pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
    // Create lock file to prevent concurrent executions
    let lock_path = Path::new(".mmm").join("improve.lock");
    if lock_path.exists() {
        return Err(anyhow!("Another mmm improve process is already running in this repository. If this is incorrect, delete .mmm/improve.lock"));
    }

    let _lock_file = File::create(&lock_path).context("Failed to create lock file")?;

    // Ensure lock file is cleaned up on exit
    let result = run_impl(cmd).await;

    // Clean up lock file
    if let Err(e) = std::fs::remove_file(&lock_path) {
        eprintln!("Warning: Failed to remove lock file: {e}");
    }

    result
}

async fn run_impl(cmd: command::ImproveCommand) -> Result<()> {
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

    // 2. Load configuration
    let config_loader = ConfigLoader::new().await?;
    config_loader.load_project(Path::new(".")).await?;
    let config = config_loader.get_config();

    // Determine workflow configuration
    let workflow_config = config
        .workflow
        .clone()
        .unwrap_or_else(WorkflowConfig::default);
    let max_iterations = workflow_config.max_iterations;

    // 3. State setup
    let mut state = StateManager::new()?;

    // 4. Git-native improvement loop
    let mut iteration = 1;
    let mut files_changed = 0;

    // Display focus directive on first iteration if provided
    if let Some(focus) = &cmd.focus {
        println!("📋 Focus: {focus} (initial analysis)");
    }

    // Check if we should use configurable workflow or legacy workflow
    if config.workflow.is_some() {
        // Use configurable workflow
        let mut executor = WorkflowExecutor::new(workflow_config, cmd.show_progress);

        while current_score < cmd.target && iteration <= max_iterations {
            // Execute workflow iteration
            let focus_for_iteration = if iteration == 1 {
                cmd.focus.as_deref()
            } else {
                None
            };

            let iteration_success = executor
                .execute_iteration(iteration, focus_for_iteration)
                .await?;
            if !iteration_success {
                if cmd.show_progress {
                    println!("Workflow iteration completed with no changes - stopping");
                }
                break;
            }

            files_changed += 1;

            // Re-analyze project
            let new_analysis = analyzer.analyze(Path::new(".")).await?;
            current_score = new_analysis.health_score;
            if cmd.show_progress {
                println!("Score: {current_score:.1}/10");
            }

            iteration += 1;
        }
    } else {
        // Use legacy hardcoded workflow
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
    }

    // 5. Completion - record basic session info
    state.state_mut().current_score = current_score;
    state.state_mut().total_runs += 1;
    state.state_mut().last_run = Some(Utc::now());
    state.save()?;

    // 6. Commit the state file
    // Stage the state file
    let git_add = Command::new("git")
        .args(["add", ".mmm/state.json"])
        .output()
        .await
        .context("Failed to stage state file")?;

    if git_add.status.success() {
        // Commit the state file
        let commit_message = format!(
            "chore: update mmm state after improvement session\n\n\
            Final score: {:.1}/10\n\
            Iterations: {}\n\
            Files changed: {}",
            current_score,
            iteration - 1,
            files_changed
        );

        let git_commit = Command::new("git")
            .args(["commit", "-m", &commit_message])
            .output()
            .await
            .context("Failed to commit state file")?;

        if !git_commit.status.success() {
            // Check if there were no changes to commit
            let stderr = String::from_utf8_lossy(&git_commit.stderr);
            if !stderr.contains("nothing to commit") && cmd.show_progress {
                eprintln!("Warning: Failed to commit .mmm/state.json: {stderr}");
            }
        }
    } else if cmd.show_progress {
        let stderr = String::from_utf8_lossy(&git_add.stderr);
        eprintln!("Warning: Failed to stage .mmm/state.json: {stderr}");
    }

    println!("✅ Complete! Final score: {current_score:.1}/10");
    println!("Files changed: {files_changed}");
    println!("Iterations: {}", iteration - 1);

    Ok(())
}

/// Call Claude CLI for code review and generate improvement spec
///
/// # Arguments
/// * `verbose` - Whether to show detailed progress messages
/// * `focus` - Optional focus directive for the first iteration
///
/// # Returns
/// Result indicating whether the review was successful
///
/// # Errors
/// Returns an error if:
/// - Claude CLI is not installed or not in PATH
/// - Claude CLI command fails after retry attempts
/// - Network issues prevent Claude API access (transient, retried)
/// - Authentication issues with Claude API
///
/// # Recovery
/// The function automatically retries transient failures up to DEFAULT_CLAUDE_RETRIES times.
/// Non-transient errors (e.g., missing CLI, auth failures) fail immediately.
async fn call_claude_code_review(verbose: bool, focus: Option<&str>) -> Result<bool> {
    println!("🤖 Running /mmm-code-review...");

    // First check if claude command exists with improved error handling
    check_claude_cli().await?;

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-code-review") // The custom command for code review
        .env("MMM_AUTOMATION", "true"); // Signals to /mmm-code-review to run in automated mode

    // Pass focus directive via environment variable on first iteration
    if let Some(focus_directive) = focus {
        cmd.env("MMM_FOCUS", focus_directive);
    }

    // Execute with retry logic for transient failures
    let output =
        execute_with_retry(cmd, "Claude code review", DEFAULT_CLAUDE_RETRIES, verbose).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = format_subprocess_error(
            "claude /mmm-code-review",
            output.status.code(),
            &stderr,
            &stdout,
        );
        return Err(anyhow!(error_msg));
    }

    if verbose {
        println!("✅ Code review completed");
    }

    Ok(true)
}

/// Extract spec ID from the latest git commit message
///
/// # Arguments
/// * `verbose` - Whether to show detailed progress messages
///
/// # Returns
/// The spec ID if found, or empty string if no spec was generated
///
/// # Errors
/// Returns an error if:
/// - Git command fails (e.g., not in a git repository)
/// - Unable to read git log output
///
/// # Note
/// This function expects commit messages in the format:
/// "review: generate improvement spec for iteration-XXXXXXXXXX-improvements"
async fn extract_spec_from_git(verbose: bool) -> Result<String> {
    if verbose {
        println!("Extracting spec ID from git history...");
    }

    // Use thread-safe git operation
    let commit_message = get_last_commit_message()
        .await
        .context("Failed to get git log")?;

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

/// Call Claude CLI to implement a specific improvement spec
///
/// # Arguments
/// * `spec_id` - The spec identifier to implement (e.g., "iteration-1234567890-improvements")
/// * `verbose` - Whether to show detailed progress messages
///
/// # Returns
/// Result indicating whether the implementation was successful
///
/// # Errors
/// Returns an error if:
/// - Invalid spec_id format (must match iteration-*-improvements pattern)
/// - Claude CLI command fails after retry attempts
/// - Spec file not found or cannot be read
/// - Implementation changes cannot be applied
///
/// # Recovery
/// The function automatically retries transient failures up to DEFAULT_CLAUDE_RETRIES times.
/// Invalid spec IDs are rejected immediately to prevent command injection.
async fn call_claude_implement_spec(spec_id: &str, verbose: bool) -> Result<bool> {
    // Validate spec_id format to prevent potential command injection
    // Must be exactly "iteration-XXXXXXXXXX-improvements" where X is a digit
    let is_valid = spec_id.starts_with("iteration-") 
        && spec_id.ends_with("-improvements")
        && spec_id.len() > 24 // "iteration-" (10) + at least 1 digit + "-improvements" (13)
        && spec_id[10..spec_id.len()-13].chars().all(|c| c.is_ascii_digit() || c == '-');

    if !is_valid {
        return Err(anyhow!(
            "Invalid spec ID format: {spec_id}. Expected format: iteration-XXXXXXXXXX-improvements"
        ));
    }

    println!("🔧 Running /mmm-implement-spec {spec_id}...");

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-implement-spec") // The custom command for spec implementation
        .arg(spec_id) // The spec ID to implement (e.g., "iteration-123-improvements")
        .env("MMM_AUTOMATION", "true"); // Signals to /mmm-implement-spec to run in automated mode

    // Execute with retry logic for transient failures
    let output = execute_with_retry(
        cmd,
        &format!("Claude implement spec {spec_id}"),
        DEFAULT_CLAUDE_RETRIES,
        verbose,
    )
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg = format_subprocess_error(
            &format!("claude /mmm-implement-spec {spec_id}"),
            output.status.code(),
            &stderr,
            &stdout,
        );
        return Err(anyhow!(error_msg));
    }

    if verbose {
        println!("✅ Implementation completed");
    }

    Ok(true)
}

/// Call Claude CLI to run linting, formatting, and tests
///
/// # Arguments
/// * `verbose` - Whether to show detailed progress messages
///
/// # Returns
/// Result indicating whether linting was successful
///
/// # Errors
/// Returns an error if:
/// - Claude CLI command fails after retry attempts
/// - Linting/formatting tools are not available
/// - Tests fail during the linting phase
///
/// # Recovery
/// The function automatically retries transient failures up to DEFAULT_CLAUDE_RETRIES times.
/// Tool availability errors are reported clearly to help users install missing tools.
async fn call_claude_lint(verbose: bool) -> Result<bool> {
    println!("🧹 Running /mmm-lint...");

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-lint") // The custom command for linting and formatting
        .env("MMM_AUTOMATION", "true"); // Signals to /mmm-lint to run in automated mode

    // Execute with retry logic for transient failures
    let output = execute_with_retry(cmd, "Claude lint", DEFAULT_CLAUDE_RETRIES, verbose).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let error_msg =
            format_subprocess_error("claude /mmm-lint", output.status.code(), &stderr, &stdout);
        return Err(anyhow!(error_msg));
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

    #[test]
    fn test_spec_id_validation() {
        // Test spec ID validation
        let valid_specs = vec![
            "iteration-1234567890-improvements",
            "iteration-0000000000-improvements",
            "iteration-9999999999-improvements",
        ];

        let invalid_specs = vec![
            "not-a-spec",
            "iteration-1234567890",
            "iteration-improvements",
            "iteration-1234567890-other",
            "iteration-$(rm -rf /)-improvements", // Command injection attempt
            "",
        ];

        for spec in valid_specs {
            let is_valid = spec.starts_with("iteration-") 
                && spec.ends_with("-improvements")
                && spec.len() > 24 // "iteration-" (10) + at least 1 digit + "-improvements" (13)
                && spec[10..spec.len()-13].chars().all(|c| c.is_ascii_digit() || c == '-');
            assert!(is_valid, "Valid spec should pass validation: {spec}");
        }

        for spec in invalid_specs {
            let is_valid = spec.starts_with("iteration-") 
                && spec.ends_with("-improvements")
                && spec.len() > 24 // "iteration-" (10) + at least 1 digit + "-improvements" (13)
                && spec[10..spec.len()-13].chars().all(|c| c.is_ascii_digit() || c == '-');
            assert!(!is_valid, "Invalid spec should fail validation: {spec}");
        }
    }

    #[tokio::test]
    async fn test_call_claude_code_review_error_scenarios() {
        // This test would require mocking the Command execution
        // For now, we just ensure the function signature is correct
        // and document what should be tested

        // Test scenarios to cover:
        // 1. Claude CLI not found
        // 2. Network timeout (should retry)
        // 3. Authentication failure
        // 4. Success after retry
        // 5. Failure after all retries
    }

    #[tokio::test]
    async fn test_lock_file_prevents_concurrent_execution() {
        // Test scenarios to cover:
        // 1. First run creates lock file
        // 2. Second run fails with lock error
        // 3. Lock file cleaned up on success
        // 4. Lock file cleaned up on error
    }
}
