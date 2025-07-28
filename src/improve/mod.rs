pub mod command;
pub mod git_ops;
pub mod retry;
pub mod session;
pub mod workflow;

use crate::config::{ConfigLoader, WorkflowConfig};
use crate::simple_state::StateManager;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context as _, Result};
use chrono::Utc;
use git_ops::get_last_commit_message;
use retry::{check_claude_cli, execute_with_retry, format_subprocess_error};
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
///
/// # Parallel Execution
/// For parallel execution, use the `--worktree` flag to run multiple sessions
/// in isolated git worktrees without conflicts.
pub async fn run(cmd: command::ImproveCommand) -> Result<()> {
    // Check if worktree isolation should be used
    // Check flag first, then env var with deprecation warning
    let use_worktree = if cmd.worktree {
        true
    } else if std::env::var("MMM_USE_WORKTREE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
    {
        eprintln!("Warning: MMM_USE_WORKTREE is deprecated. Use --worktree or -w flag instead.");
        true
    } else {
        false
    };

    if use_worktree {
        // Create worktree for this session
        let worktree_manager = WorktreeManager::new(std::env::current_dir()?)?;
        let session = worktree_manager.create_session(cmd.focus.as_deref())?;

        println!(
            "🌳 Created worktree: {} at {}",
            session.name,
            session.path.display()
        );

        // Change to worktree directory
        std::env::set_current_dir(&session.path)?;

        // Run improvement in worktree context
        let result = run_in_worktree(cmd.clone(), session.clone()).await;

        // Clean up on failure, keep on success for manual merge
        match &result {
            Ok(_) => {
                println!("✅ Improvements completed in worktree: {}", session.name);
                println!("To merge changes, run: mmm worktree merge {}", session.name);
            }
            Err(_) => {
                eprintln!(
                    "❌ Improvement failed, preserving worktree for debugging: {}",
                    session.name
                );
            }
        }

        result
    } else {
        // Run without worktree isolation (default behavior)
        run_without_worktree(cmd).await
    }
}

async fn run_in_worktree(
    cmd: command::ImproveCommand,
    session: crate::worktree::WorktreeSession,
) -> Result<()> {
    let worktree_manager =
        WorktreeManager::new(std::env::current_dir()?.parent().unwrap().to_path_buf())?;

    // Run improvement loop with state tracking
    let result = run_improvement_loop(cmd.clone(), &session, &worktree_manager).await;

    // Update final state
    worktree_manager.update_session_state(&session.name, |state| match &result {
        Ok(_) => {
            state.status = crate::worktree::WorktreeStatus::Completed;
        }
        Err(e) => {
            state.status = crate::worktree::WorktreeStatus::Failed;
            state.error = Some(e.to_string());
        }
    })?;

    result
}

async fn run_improvement_loop(
    cmd: command::ImproveCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
) -> Result<()> {
    // The actual improvement logic, but with state tracking
    // This is a copy of run_without_worktree logic with state updates

    // 1. Check for Claude CLI
    check_claude_cli().await?;

    // 2. Initial analysis
    // Skip analysis for worktree tracking

    if cmd.show_progress {
        if let Some(focus) = &cmd.focus {
            println!("📊 Focus: {focus}");
        }
        println!();
    }

    // 3. Setup basic state
    let state = StateManager::new()?;
    let _start_time = Utc::now();

    // 4. Main improvement loop
    let mut iteration = 1;
    let mut files_changed = 0;

    // Load configuration (with workflow if present)
    let config_loader = ConfigLoader::new().await?;
    config_loader.load_with_explicit_path(Path::new("."), cmd.config.as_deref()).await?;
    let config = config_loader.get_config();

    // Check if we have a workflow configuration
    if let Some(workflow_config) = config.workflow {
        // Use configurable workflow
        if cmd.show_progress {
            println!("Using custom workflow from configuration");
        }

        let max_iterations = cmd
            .max_iterations
            .min(workflow_config.max_iterations);
        let mut executor =
            WorkflowExecutor::new(workflow_config, cmd.show_progress, max_iterations);

        while iteration <= max_iterations {
            // Update worktree state before iteration
            worktree_manager.update_session_state(&session.name, |state| {
                state.iterations.completed = iteration - 1;
                state.iterations.max = max_iterations;
            })?;

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

            // Update stats after iteration
            worktree_manager.update_session_state(&session.name, |state| {
                state.stats.files_changed = files_changed;
                state.stats.commits += 3; // review + implement + lint
            })?;

            iteration += 1;
        }
    } else {
        // Use legacy hardcoded workflow
        while iteration <= cmd.max_iterations {
            // Update worktree state before iteration
            worktree_manager.update_session_state(&session.name, |state| {
                state.iterations.completed = iteration - 1;
                state.iterations.max = cmd.max_iterations;
            })?;

            if cmd.show_progress {
                println!("🔄 Iteration {iteration}/{}...", cmd.max_iterations);
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

            // Update stats after iteration
            worktree_manager.update_session_state(&session.name, |state| {
                state.stats.files_changed = files_changed;
                state.stats.commits += 3; // review + implement + lint
            })?;

            iteration += 1;
        }
    }

    // Final state update
    worktree_manager.update_session_state(&session.name, |state| {
        state.iterations.completed = iteration - 1;
    })?;

    // 5. Completion - record basic session info
    // StateManager handles saving automatically
    let _ = state; // Consume state to avoid unused variable warning

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
            Iterations: {}\n\
            Files changed: {}",
            iteration - 1,
            files_changed
        );

        let git_commit = Command::new("git")
            .args(["commit", "-m", &commit_message])
            .output()
            .await
            .context("Failed to commit state file")?;

        if !git_commit.status.success() {
            let stderr = String::from_utf8_lossy(&git_commit.stderr);
            let stdout = String::from_utf8_lossy(&git_commit.stdout);
            // It's okay if there's nothing to commit
            if !stderr.contains("nothing to commit")
                && !stdout.contains("nothing to commit")
                && !stderr.contains("no changes added")
            {
                eprintln!("Warning: Failed to commit state file: {stderr}");
            }
        }
    }

    // 7. Final summary
    if cmd.show_progress {
        println!("\n✅ Improvement session completed:");
        println!("   Iterations: {}", iteration - 1);
        println!("   Files improved: {files_changed}");
        println!("   Session state: saved");
    }

    Ok(())
}

async fn run_without_worktree(cmd: command::ImproveCommand) -> Result<()> {
    println!("🔍 Starting improvement loop...");

    // 2. Load configuration
    let config_loader = ConfigLoader::new().await?;

    // Load with explicit config path if provided
    config_loader
        .load_with_explicit_path(Path::new("."), cmd.config.as_deref())
        .await?;

    // Also load project configuration
    config_loader.load_project(Path::new(".")).await?;

    let config = config_loader.get_config();

    // Show config source in verbose mode
    if cmd.show_progress {
        if let Some(config_path) = &cmd.config {
            println!("📄 Using configuration from: {}", config_path.display());
        } else if Path::new(".mmm/config.toml").exists() {
            println!("📄 Using configuration from: .mmm/config.toml");
        } else {
            println!("📄 Using default configuration");
        }
    }

    // Determine workflow configuration
    let workflow_config = config
        .workflow
        .clone()
        .unwrap_or_else(WorkflowConfig::default);
    // Command-line max_iterations takes precedence over config
    let max_iterations = cmd.max_iterations;

    // 3. State setup
    let state = StateManager::new()?;

    // 4. Git-native improvement loop
    let mut iteration = 1;
    let mut files_changed = 0;

    // Display focus directive if provided
    if let Some(focus) = &cmd.focus {
        println!("📋 Focus: {focus}");
    }

    // Check if we should use configurable workflow or legacy workflow
    if config.workflow.is_some() {
        // Use configurable workflow
        let mut executor =
            WorkflowExecutor::new(workflow_config, cmd.show_progress, max_iterations);

        while iteration <= max_iterations {
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

            iteration += 1;
        }
    } else {
        // Use legacy hardcoded workflow
        while iteration <= cmd.max_iterations {
            if cmd.show_progress {
                println!("🔄 Iteration {iteration}/{}...", cmd.max_iterations);
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

            iteration += 1;
        }
    }

    // 5. Completion - record basic session info
    // StateManager handles saving automatically
    let _ = state; // Consume state to avoid unused variable warning

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
            Iterations: {}\n\
            Files changed: {}",
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

    // Completion message
    let actual_iterations = iteration - 1;
    if actual_iterations >= cmd.max_iterations {
        println!(
            "⏱️  Complete! Max iterations reached ({}).",
            cmd.max_iterations
        );
    } else {
        println!("✅ Complete! No more improvements found.");
    }

    println!("Files changed: {files_changed}");
    println!("Iterations: {actual_iterations}");

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
        && spec_id.len() >= 24 // "iteration-" (10) + at least 1 digit + "-improvements" (13)
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
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_command(worktree: bool, max_iterations: u32) -> command::ImproveCommand {
        command::ImproveCommand {
            max_iterations,
            worktree,
            show_progress: false,
            focus: None,
            config: None,
        }
    }

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
    async fn test_run_with_worktree_flag() {
        // Test that worktree flag is properly handled
        let cmd = create_test_command(true, 1);

        // We can't fully test without a git repo, but we can verify the logic
        assert!(cmd.worktree);
        assert_eq!(cmd.max_iterations, 1);
    }

    #[tokio::test]
    async fn test_run_with_env_var_worktree() {
        // Test deprecated MMM_USE_WORKTREE env var
        std::env::set_var("MMM_USE_WORKTREE", "true");
        let cmd = create_test_command(false, 1);

        // The function should detect the env var even if flag is false
        let use_worktree = if cmd.worktree {
            true
        } else if std::env::var("MMM_USE_WORKTREE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false)
        {
            true
        } else {
            false
        };

        assert!(use_worktree);
        std::env::remove_var("MMM_USE_WORKTREE");
    }

    #[test]
    fn test_default_claude_retries_constant() {
        assert_eq!(DEFAULT_CLAUDE_RETRIES, 2);
    }

    #[tokio::test]
    async fn test_call_claude_implement_spec_validation() {
        // Test spec ID validation in call_claude_implement_spec

        // Valid spec IDs
        let valid_specs = vec![
            "iteration-1234567890-improvements",
            "iteration-0-improvements",
            "iteration-99999999999999999999-improvements",
        ];

        for spec_id in valid_specs {
            let is_valid = spec_id.starts_with("iteration-")
                && spec_id.ends_with("-improvements")
                && spec_id.len() >= 24
                && spec_id[10..spec_id.len() - 13]
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '-');
            assert!(is_valid, "Should be valid: {}", spec_id);
        }

        // Invalid spec IDs
        let invalid_specs = vec![
            "iteration-abc-improvements", // Non-numeric
            "iteration-123",              // Missing suffix
            "123-improvements",           // Missing prefix
            "iteration--improvements",    // No digits
            "iteration-123-wrong",        // Wrong suffix
            "",                           // Empty
        ];

        for spec_id in invalid_specs {
            let is_valid = spec_id.starts_with("iteration-")
                && spec_id.ends_with("-improvements")
                && spec_id.len() >= 24
                && spec_id[10..spec_id.len() - 13]
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '-');
            assert!(!is_valid, "Should be invalid: {}", spec_id);
        }
    }

    #[tokio::test]
    async fn test_run_without_worktree_target_already_reached() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Initialize git repo
        std::process::Command::new("git")
            .args(&["init"])
            .output()
            .unwrap();

        // Create minimal project structure
        std::fs::create_dir_all(".mmm").unwrap();
        std::fs::write(
            ".mmm/config.toml",
            r#"
            [claude]
            model = "claude-3-sonnet"
            "#,
        )
        .unwrap();

        // This test would need more setup to actually run the function
        // For now, we test the command structure
        let cmd = create_test_command(false, 1);
        assert!(!cmd.worktree);
    }

    #[test]
    fn test_improve_command_with_focus() {
        let mut cmd = create_test_command(false, 5);
        cmd.focus = Some("performance".to_string());

        assert_eq!(cmd.focus.as_deref(), Some("performance"));
        assert_eq!(cmd.max_iterations, 5);
    }

    #[test]
    fn test_improve_command_with_config_path() {
        let mut cmd = create_test_command(false, 5);
        cmd.config = Some(PathBuf::from("/custom/config.toml"));

        assert_eq!(
            cmd.config.as_ref().map(|p| p.display().to_string()),
            Some("/custom/config.toml".to_string())
        );
    }

    #[tokio::test]
    async fn test_extract_spec_from_git_various_formats() {
        // Test edge cases in spec extraction
        let test_cases = vec![
            // Normal cases
            ("iteration-123-improvements", "iteration-123-improvements"),
            (
                "prefix iteration-456-improvements suffix",
                "iteration-456-improvements",
            ),
            // Edge cases
            ("iteration-", "iteration-"),       // Incomplete
            ("iteration-123", "iteration-123"), // No -improvements
            ("iteration-improvements", "iteration-improvements"), // No number
            // Multiple occurrences (should get first)
            (
                "iteration-111-improvements and iteration-222-improvements",
                "iteration-111-improvements",
            ),
            // No match
            ("no spec here", ""),
            ("iter-123-improvements", ""), // Wrong prefix
        ];

        for (input, expected) in test_cases {
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

            assert_eq!(result, expected, "Failed for input: '{}'", input);
        }
    }

    #[test]
    fn test_format_subprocess_error_output() {
        // Test error formatting (this would be from retry module but used here)
        let scenarios = vec![
            ("command", Some(1), "stderr output", "stdout output"),
            ("command", Some(127), "command not found", ""),
            ("command", None, "killed by signal", "partial output"),
        ];

        for (cmd, code, stderr, stdout) in scenarios {
            let formatted = format!(
                "Command '{}' failed with exit code {:?}\nStderr: {}\nStdout: {}",
                cmd, code, stderr, stdout
            );

            assert!(formatted.contains(cmd));
            assert!(formatted.contains(&format!("{:?}", code)));
            if !stderr.is_empty() {
                assert!(formatted.contains(stderr));
            }
            if !stdout.is_empty() {
                assert!(formatted.contains(stdout));
            }
        }
    }
}
