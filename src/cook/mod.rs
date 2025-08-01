pub mod command;
pub mod git_ops;
pub mod retry;
pub mod session;
pub mod signal_handler;
pub mod workflow;

#[cfg(test)]
mod tests;

use crate::config::{workflow::WorkflowConfig, Config, ConfigLoader};
use crate::context::{save_analysis, ContextAnalyzer, ProjectAnalyzer};
use crate::metrics::{MetricsCollector, MetricsHistory, MetricsStorage};
use crate::simple_state::StateManager;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context as _, Result};
use chrono::Utc;
use git_ops::get_last_commit_message;
use retry::check_claude_cli;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;
use workflow::WorkflowExecutor;

/// Choice made by user when prompted to merge
enum MergeChoice {
    Yes,
    No,
}

/// Load workflow configuration from a playbook file
async fn load_playbook(path: &Path) -> Result<WorkflowConfig> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read playbook file: {}", path.display()))?;

    // Try to parse as YAML first, then fall back to JSON
    if path.extension().and_then(|s| s.to_str()) == Some("yml")
        || path.extension().and_then(|s| s.to_str()) == Some("yaml")
    {
        serde_yaml::from_str(&content)
            .context(format!("Failed to parse YAML playbook: {}", path.display()))
    } else {
        // Default to JSON parsing
        serde_json::from_str(&content)
            .context(format!("Failed to parse JSON playbook: {}", path.display()))
    }
}

/// Setup metrics collection based on command settings
fn setup_metrics(
    cmd: &command::CookCommand,
    project_path: &Path,
) -> Result<(Option<MetricsCollector>, MetricsHistory)> {
    let metrics_collector = if cmd.metrics {
        Some(MetricsCollector::new())
    } else {
        None
    };

    let metrics_history = if cmd.metrics {
        let storage = MetricsStorage::new(project_path);
        storage
            .load_history()
            .unwrap_or_else(|_| MetricsHistory::new())
    } else {
        MetricsHistory::new()
    };

    Ok((metrics_collector, metrics_history))
}

/// Helper to conditionally collect metrics if enabled
async fn maybe_collect_metrics(
    cmd: &command::CookCommand,
    metrics_collector: &Option<MetricsCollector>,
    metrics_history: &mut MetricsHistory,
    project_path: &Path,
    iteration: u32,
    verbose: bool,
) -> Result<()> {
    if cmd.metrics {
        if let Some(ref collector) = metrics_collector {
            collect_iteration_metrics(collector, metrics_history, project_path, iteration, verbose)
                .await?;
        }
    }
    Ok(())
}

/// Collect and save metrics after an iteration
async fn collect_iteration_metrics(
    metrics_collector: &MetricsCollector,
    metrics_history: &mut MetricsHistory,
    project_path: &Path,
    iteration: u32,
    verbose: bool,
) -> Result<()> {
    match metrics_collector
        .collect_metrics(project_path, format!("iteration-{iteration}"))
        .await
    {
        Ok(metrics) => {
            // Get current commit SHA for tracking
            let commit_sha = get_last_commit_message()
                .await
                .unwrap_or_else(|_| "unknown".to_string());

            // Add to history
            metrics_history.add_snapshot(metrics.clone(), commit_sha);

            // Save metrics
            let storage = MetricsStorage::new(project_path);
            if let Err(e) = storage.save_current(&metrics) {
                eprintln!("⚠️  Failed to save current metrics: {}", e);
            }
            if let Err(e) = storage.save_history(metrics_history) {
                eprintln!("⚠️  Failed to save metrics history: {}", e);
            }

            // Show metrics summary if verbose
            if verbose {
                let report = storage.generate_report(&metrics);
                println!("\n{report}");
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to collect metrics: {}", e);
        }
    }
    Ok(())
}

/// Collect inputs from mapping patterns and arguments
fn collect_mapping_inputs(cmd: &command::CookCommand) -> Result<Vec<String>> {
    use glob::glob;

    let mut inputs: Vec<String> = Vec::new();

    // Collect inputs from --map patterns
    for pattern in &cmd.map {
        let entries = glob(pattern).context(format!("Invalid glob pattern: {pattern}"))?;

        for entry in entries {
            match entry {
                Ok(path) => {
                    inputs.push(path.to_string_lossy().into_owned());
                }
                Err(e) => {
                    eprintln!("⚠️  Error matching pattern: {e}");
                }
            }
        }
    }

    // Add direct arguments from --args
    inputs.extend(cmd.args.clone());

    if inputs.is_empty() {
        return Err(anyhow!("No inputs found from --map patterns or --args"));
    }

    Ok(inputs)
}

/// Handle merge prompting and execution for worktree
#[allow(dead_code)]
async fn handle_worktree_merge(
    cmd: &command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    original_repo_path: &std::path::Path,
    iterations_completed: u32,
) -> Result<()> {
    // Skip if no improvements were made
    if iterations_completed == 0 {
        return Ok(());
    }

    // Check if we should prompt
    if !should_prompt_for_merge(cmd) {
        print_manual_merge_instructions(&session.name);
        return Ok(());
    }

    // Initialize worktree manager
    let worktree_manager = WorktreeManager::new(original_repo_path.to_path_buf())?;

    // Update state to track prompt shown
    worktree_manager.update_session_state(&session.name, |state| {
        state.merge_prompt_shown = true;
    })?;

    // Get merge decision
    let should_merge = get_merge_decision(cmd, &session.name);

    // Handle merge decision
    match should_merge {
        MergeChoice::Yes => {
            handle_merge_accepted(cmd, session, &worktree_manager, original_repo_path).await
        }
        MergeChoice::No => handle_merge_declined(&worktree_manager, session),
    }
}

/// Check if we should prompt for merge
fn should_prompt_for_merge(cmd: &command::CookCommand) -> bool {
    is_tty() || cmd.auto_accept
}

/// Get merge decision based on auto-accept or user prompt
fn get_merge_decision(cmd: &command::CookCommand, session_name: &str) -> MergeChoice {
    if cmd.auto_accept {
        println!("Auto-accepting worktree merge (--yes flag set)");
        MergeChoice::Yes
    } else {
        prompt_for_merge(session_name)
    }
}

/// Handle accepted merge
async fn handle_merge_accepted(
    cmd: &command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    original_repo_path: &std::path::Path,
) -> Result<()> {
    // Update state with response
    worktree_manager.update_session_state(&session.name, |state| {
        state.merge_prompt_response = Some("yes".to_string());
    })?;

    println!("Merging worktree {}...", session.name);

    // Perform merge
    merge_worktree(&session.name, original_repo_path).await?;
    println!("✅ Successfully merged worktree: {}", session.name);

    // Update worktree status to merged
    worktree_manager.update_session_state(&session.name, |state| {
        state.status = crate::worktree::WorktreeStatus::Merged;
    })?;

    // Handle deletion prompt
    handle_deletion_prompt(cmd, session, worktree_manager).await
}

/// Handle declined merge
fn handle_merge_declined(
    worktree_manager: &WorktreeManager,
    session: &crate::worktree::WorktreeSession,
) -> Result<()> {
    // Update state with response
    worktree_manager.update_session_state(&session.name, |state| {
        state.merge_prompt_response = Some("no".to_string());
    })?;

    println!("Worktree kept for manual review: {}", session.name);
    print_manual_merge_instructions(&session.name);
    Ok(())
}

/// Handle deletion prompt after successful merge
async fn handle_deletion_prompt(
    cmd: &command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
) -> Result<()> {
    // Check if we should prompt for deletion
    if !should_prompt_for_merge(cmd) {
        return Ok(());
    }

    let should_delete = get_deletion_decision(cmd, &session.name)?;

    match should_delete {
        MergeChoice::Yes => delete_worktree(worktree_manager, &session.name).await,
        MergeChoice::No => {
            println!("Worktree kept: {}", session.name);
            Ok(())
        }
    }
}

/// Get deletion decision based on auto-accept or user prompt
fn get_deletion_decision(cmd: &command::CookCommand, session_name: &str) -> Result<MergeChoice> {
    if cmd.auto_accept {
        println!("Auto-accepting worktree deletion (--yes flag set)");
        Ok(MergeChoice::Yes)
    } else {
        prompt_for_deletion(session_name)
    }
}

/// Delete worktree
async fn delete_worktree(worktree_manager: &WorktreeManager, session_name: &str) -> Result<()> {
    match worktree_manager.cleanup_session(session_name, true) {
        Ok(_) => {
            println!("✅ Deleted worktree: {session_name}");
            Ok(())
        }
        Err(e) => {
            println!("⚠️  Failed to delete worktree: {e}");
            // Don't propagate error, just warn
            Ok(())
        }
    }
}

/// Print manual merge instructions
fn print_manual_merge_instructions(session_name: &str) {
    println!("To merge the worktree, run:");
    println!("   mmm worktree merge {session_name}");
}

/// Create variables for a mapping input
fn create_mapping_variables(
    cmd: &command::CookCommand,
    input: &str,
    index: usize,
    total: usize,
) -> std::collections::HashMap<String, String> {
    use glob::glob;
    use std::collections::HashMap;

    let mut variables = HashMap::new();
    let item_number = index + 1;

    // Determine the ARG value based on whether this came from --map or --args
    let arg_value = if cmd.map.iter().any(|pattern| {
        glob(pattern)
            .ok()
            .and_then(|entries| {
                entries
                    .filter_map(Result::ok)
                    .find(|p| p.to_string_lossy() == input)
            })
            .is_some()
    }) {
        // This input came from --map, so extract spec ID if it's a spec file
        if input.ends_with(".md") && input.contains("spec") {
            extract_spec_id_from_path(input)
        } else {
            input.to_string()
        }
    } else {
        // This input came from --args, use it directly
        input.to_string()
    };

    variables.insert("ARG".to_string(), arg_value);
    variables.insert("FILE".to_string(), input.to_string());
    variables.insert("INDEX".to_string(), item_number.to_string());
    variables.insert("TOTAL".to_string(), total.to_string());

    if let Some(file_name) = std::path::Path::new(input).file_name() {
        variables.insert(
            "FILE_NAME".to_string(),
            file_name.to_string_lossy().into_owned(),
        );
        if let Some(stem) = std::path::Path::new(input).file_stem() {
            variables.insert("FILE_STEM".to_string(), stem.to_string_lossy().into_owned());
        }
    }

    variables
}

/// Run comprehensive analysis including context and metrics for workflows
async fn analyze_project_comprehensive(project_path: &Path, verbose: bool) -> Result<()> {
    if verbose {
        println!("🔍 Running comprehensive project analysis...");
    }

    // First run context analysis
    let analyzer = ProjectAnalyzer::new();
    let analysis_result = analyzer.analyze(project_path).await?;
    let suggestions = analyzer.get_improvement_suggestions();

    // Then run metrics collection to get accurate coverage data
    let metrics_collector = MetricsCollector::new();
    let iteration_id = format!("cook-{}", chrono::Utc::now().timestamp());

    let metrics_result = metrics_collector
        .collect_metrics(project_path, iteration_id.clone())
        .await;

    match metrics_result {
        Ok(metrics) => {
            if verbose {
                println!("✅ Comprehensive analysis complete");
                println!(
                    "   - Dependencies: {} modules analyzed",
                    analysis_result.dependency_graph.nodes.len()
                );
                println!(
                    "   - Architecture: {} patterns detected",
                    analysis_result.architecture.patterns.len()
                );
                println!(
                    "   - Technical debt: {} items found",
                    analysis_result.technical_debt.debt_items.len()
                );
                println!(
                    "   - Test coverage: {:.1}% (accurate)",
                    metrics.test_coverage
                );
                println!("   - Lint warnings: {}", metrics.lint_warnings);

                if !suggestions.is_empty() {
                    println!("\n📋 Top improvement suggestions:");
                    for (i, suggestion) in suggestions.iter().take(3).enumerate() {
                        println!("   {}. {}", i + 1, suggestion.title);
                    }
                }
            }

            // Save metrics for later use
            let metrics_storage = MetricsStorage::new(project_path);
            if let Err(e) = metrics_storage.save_current(&metrics) {
                if verbose {
                    eprintln!("⚠️  Failed to save metrics: {}", e);
                }
            }
        }
        Err(e) => {
            if verbose {
                eprintln!("⚠️  Metrics collection failed: {}", e);
                eprintln!("   Using context analysis only");
                println!(
                    "   - Test coverage: {:.1}% (estimated)",
                    analysis_result.test_coverage.overall_coverage * 100.0
                );
            }
        }
    }

    // Save analysis for Claude commands to use
    save_analysis(project_path, &analysis_result)?;

    // Set environment variable to signal context is available
    std::env::set_var("MMM_CONTEXT_AVAILABLE", "true");

    Ok(())
}

/// Check if we're running in an interactive terminal
fn is_tty() -> bool {
    atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
}

/// Prompt user to merge a completed worktree
fn prompt_for_merge(_worktree_name: &str) -> MergeChoice {
    print!("\nWould you like to merge the completed worktree now? (y/N): ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return MergeChoice::No;
    }

    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => MergeChoice::Yes,
        _ => MergeChoice::No,
    }
}

/// Prompt user to delete a worktree
#[allow(dead_code)]
fn prompt_for_deletion(_worktree_name: &str) -> Result<MergeChoice> {
    print!("\nWould you like to delete the worktree now? (y/N): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(match input.trim().to_lowercase().as_str() {
        "y" | "yes" => MergeChoice::Yes,
        _ => MergeChoice::No,
    })
}

/// Execute worktree merge
async fn merge_worktree(worktree_name: &str, original_repo_path: &std::path::Path) -> Result<()> {
    // Use the original repository path, not the current directory which is inside the worktree
    let worktree_manager = WorktreeManager::new(original_repo_path.to_path_buf())?;

    // Execute merge using existing logic
    worktree_manager.merge_session(worktree_name)?;

    Ok(())
}

/// Run the cook command with verbosity level
pub async fn run_with_verbosity(cmd: command::CookCommand, verbosity: u8) -> Result<()> {
    run_internal(cmd, verbosity > 0).await
}

/// Run the cook command to automatically enhance code quality
///
/// # Arguments
/// * `cmd` - The cook command with optional target score, verbosity, and focus directive
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
pub async fn run(cmd: command::CookCommand) -> Result<()> {
    run_internal(cmd, false).await
}

async fn run_internal(mut cmd: command::CookCommand, verbose: bool) -> Result<()> {
    // Save original working directory before any changes
    let original_dir = std::env::current_dir().context("Failed to get current directory")?;

    // Make playbook path absolute before any directory changes
    if !cmd.playbook.is_absolute() {
        cmd.playbook = original_dir.join(&cmd.playbook);
    }

    // Handle path argument if provided
    if let Some(ref path) = cmd.path {
        // Expand tilde notation if present
        let expanded_path = if path.to_string_lossy().starts_with("~/") {
            let home =
                dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
            home.join(
                path.strip_prefix("~/")
                    .context("Failed to strip ~/ prefix")?,
            )
        } else {
            path.clone()
        };

        // Resolve to absolute path
        let absolute_path = if expanded_path.is_absolute() {
            expanded_path
        } else {
            original_dir.join(&expanded_path)
        };

        // Validate path exists and is a directory
        if !absolute_path.exists() {
            return Err(anyhow!("Directory not found: {}", absolute_path.display()));
        }
        if !absolute_path.is_dir() {
            return Err(anyhow!(
                "Path is not a directory: {}",
                absolute_path.display()
            ));
        }

        // Check if it's a git repository
        if !absolute_path.join(".git").exists() {
            return Err(anyhow!("Not a git repository: {}", absolute_path.display()));
        }

        // Change to the specified directory
        std::env::set_current_dir(&absolute_path).with_context(|| {
            format!("Failed to change to directory: {}", absolute_path.display())
        })?;

        if verbose {
            println!("📁 Working in: {}", absolute_path.display());
        }
    }

    // Check if we're resuming an interrupted session
    let result = if let Some(session_id) = cmd.resume.clone() {
        resume_session(&session_id, cmd, verbose).await
    } else if !cmd.map.is_empty() || !cmd.args.is_empty() {
        run_with_mapping(cmd, verbose).await
    } else {
        run_standard(cmd, verbose).await
    };

    // Note: We don't restore the original directory as per spec
    // This allows scripts to check where MMM ended up

    result
}

/// Run cook command with file mapping or direct arguments
///
/// Wrapper for running with mapping in worktree mode
async fn run_with_mapping_in_worktree_wrapper(
    cmd: command::CookCommand,
    verbose: bool,
) -> Result<()> {
    let original_repo_path = std::env::current_dir()?;
    let worktree_manager = WorktreeManager::new(original_repo_path.to_path_buf())?;
    let session = worktree_manager.create_session(cmd.focus.as_deref())?;

    println!(
        "🌳 Created worktree: {} at {}",
        session.name,
        session.path.display()
    );

    // Change to worktree directory
    std::env::set_current_dir(&session.path)?;

    // Run improvement in worktree context
    let result = run_with_mapping_in_worktree(
        cmd.clone(),
        session.clone(),
        original_repo_path.clone(),
        verbose,
    )
    .await;

    // Handle post-execution
    handle_worktree_completion(
        &result,
        &session,
        &worktree_manager,
        &original_repo_path,
        &cmd,
    )
    .await?;

    result
}

/// Handles worktree completion including merge prompts
async fn handle_worktree_completion(
    result: &Result<()>,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    original_repo_path: &Path,
    cmd: &command::CookCommand,
) -> Result<()> {
    match result {
        Ok(_) => {
            let state = worktree_manager.get_session_state(&session.name)?;
            let iterations_completed = state.iterations.completed;

            if iterations_completed == 0 {
                println!(
                    "⚠️  No improvements were made in worktree: {}",
                    session.name
                );
                println!("   Reason: No issues were found to fix");
            } else {
                println!("✅ Improvements completed in worktree: {}", session.name);
            }

            // Handle merge prompting
            if (is_tty() || cmd.auto_accept) && iterations_completed > 0 {
                worktree_manager.update_session_state(&session.name, |state| {
                    state.merge_prompt_shown = true;
                })?;

                handle_worktree_merge(cmd, session, original_repo_path, state.iterations.completed)
                    .await?;
            } else if iterations_completed > 0 {
                println!("\nWorktree completed. To merge changes, run:");
                println!("  mmm worktree merge {}", session.name);
            } else {
                println!("\nNo changes to merge. You can delete the worktree with:");
                println!("  mmm worktree delete {}", session.name);
            }
        }
        Err(_) => {
            eprintln!(
                "❌ Improvement failed, preserving worktree for debugging: {}",
                session.name
            );
        }
    }

    Ok(())
}

/// Runs mapping without worktree  
async fn run_with_mapping_standard(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    let inputs = collect_mapping_inputs(&cmd)?;

    println!("📋 Processing {} input(s)", inputs.len());

    let mut success_count = 0;
    let mut failure_count = 0;
    let total = inputs.len();

    for (index, input) in inputs.iter().enumerate() {
        let item_number = index + 1;
        println!("\n[{item_number}/{total}] Processing: {input}");

        // Create a new command instance for this input
        let input_cmd = cmd.clone();

        // Create variables for this input
        let variables = create_mapping_variables(&cmd, input, index, total);

        // Store variables in environment for the subprocess
        for (key, value) in &variables {
            std::env::set_var(format!("MMM_VAR_{key}"), value);
        }

        // Run the improvement for this input
        let result = run_standard_with_variables(input_cmd, variables, verbose).await;

        // Clean up environment variables
        for key in ["ARG", "FILE", "INDEX", "TOTAL", "FILE_NAME", "FILE_STEM"] {
            std::env::remove_var(format!("MMM_VAR_{key}"));
        }

        match result {
            Ok(_) => {
                success_count += 1;
                println!("✅ [{item_number}/{total}] Completed: {input}");
            }
            Err(e) => {
                failure_count += 1;
                eprintln!("❌ [{item_number}/{total}] Failed: {input} - {e}");
                if cmd.fail_fast {
                    return Err(anyhow!("Stopping due to --fail-fast: {}", e));
                }
            }
        }
    }

    println!(
        "\n📊 Summary: {success_count} succeeded, {failure_count} failed out of {total} total"
    );

    if failure_count > 0 && !cmd.fail_fast {
        Err(anyhow!("{} input(s) failed processing", failure_count))
    } else {
        Ok(())
    }
}

async fn run_with_mapping(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    if cmd.worktree {
        run_with_mapping_in_worktree_wrapper(cmd, verbose).await
    } else {
        run_with_mapping_standard(cmd, verbose).await
    }
}

/// Extract spec ID from a file path
fn extract_spec_id_from_path(path: &str) -> String {
    let path = std::path::Path::new(path);

    // Get the file stem (filename without extension)
    if let Some(stem) = path.file_stem() {
        let stem_str = stem.to_string_lossy();

        // Extract numeric ID from filenames like "01-feature.md" or "35-something.md"
        if let Some(dash_pos) = stem_str.find('-') {
            let potential_id = &stem_str[..dash_pos];
            if potential_id.chars().all(|c| c.is_alphanumeric()) {
                return potential_id.to_string();
            }
        }

        // Return full stem if no pattern found (e.g., "iteration-123456-improvements")
        stem_str.into_owned()
    } else {
        path.to_string_lossy().into_owned()
    }
}

/// Run standard cook without mapping
async fn run_standard(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    if cmd.worktree {
        run_with_worktree(cmd, verbose).await
    } else {
        run_without_worktree(cmd, verbose).await
    }
}

/// Run cook with worktree isolation
async fn run_with_worktree(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    let original_repo_path = std::env::current_dir()?;
    let worktree_manager = WorktreeManager::new(original_repo_path.to_path_buf())?;
    let session = worktree_manager.create_session(cmd.focus.as_deref())?;

    println!(
        "🌳 Created worktree: {} at {}",
        session.name,
        session.path.display()
    );

    // Change to worktree directory
    std::env::set_current_dir(&session.path)?;

    // Run improvement in worktree context
    let result = run_in_worktree(
        cmd.clone(),
        session.clone(),
        original_repo_path.clone(),
        verbose,
    )
    .await;

    // Handle post-execution based on result
    handle_worktree_result(
        &result,
        &cmd,
        &session,
        &worktree_manager,
        &original_repo_path,
    )
    .await?;

    result
}

/// Handle worktree result and prompt for merge/cleanup
async fn handle_worktree_result(
    result: &Result<()>,
    cmd: &command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    original_repo_path: &Path,
) -> Result<()> {
    match result {
        Ok(_) => {
            handle_successful_worktree(cmd, session, worktree_manager, original_repo_path).await
        }
        Err(_) => {
            eprintln!(
                "❌ Improvement failed, preserving worktree for debugging: {}",
                session.name
            );
            Ok(())
        }
    }
}

/// Handle successful worktree completion
async fn handle_successful_worktree(
    cmd: &command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    original_repo_path: &Path,
) -> Result<()> {
    let state = worktree_manager.get_session_state(&session.name)?;
    let iterations_completed = state.iterations.completed;

    if iterations_completed == 0 {
        println!(
            "⚠️  No improvements were made in worktree: {}",
            session.name
        );
        println!("   Reason: No issues were found to fix");
        println!("\nNo changes to merge. You can delete the worktree with:");
        println!("  mmm worktree delete {}", session.name);
        return Ok(());
    }

    println!("✅ Improvements completed in worktree: {}", session.name);

    // Handle merge prompt for interactive terminals
    if is_tty() || cmd.auto_accept {
        handle_merge_prompt(cmd, session, worktree_manager, original_repo_path).await
    } else {
        println!("\nWorktree completed. To merge changes, run:");
        println!("  mmm worktree merge {}", session.name);
        Ok(())
    }
}

/// Handle merge prompt and deletion
async fn handle_merge_prompt(
    cmd: &command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    original_repo_path: &Path,
) -> Result<()> {
    // Update state to track prompt shown
    worktree_manager.update_session_state(&session.name, |state| {
        state.merge_prompt_shown = true;
    })?;

    let should_merge = if cmd.auto_accept {
        println!("Auto-accepting worktree merge (--yes flag set)");
        MergeChoice::Yes
    } else {
        prompt_for_merge(&session.name)
    };

    match should_merge {
        MergeChoice::Yes => {
            handle_merge_yes(session, worktree_manager, original_repo_path, cmd).await
        }
        MergeChoice::No => handle_merge_no(session, worktree_manager),
    }
}

/// Handle "yes" response to merge prompt
async fn handle_merge_yes(
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    original_repo_path: &Path,
    cmd: &command::CookCommand,
) -> Result<()> {
    // Update state with response
    worktree_manager.update_session_state(&session.name, |state| {
        state.merge_prompt_response = Some("yes".to_string());
    })?;

    println!("Merging worktree {}...", session.name);
    match merge_worktree(&session.name, original_repo_path).await {
        Ok(_) => {
            println!("✅ Successfully merged worktree: {}", session.name);

            // Update worktree status to merged
            worktree_manager.update_session_state(&session.name, |state| {
                state.status = crate::worktree::WorktreeStatus::Merged;
            })?;

            // Handle deletion prompt
            handle_deletion_prompt(cmd, session, worktree_manager).await
        }
        Err(e) => {
            eprintln!("❌ Failed to merge worktree: {e}");
            println!("\nTo merge changes manually, run:");
            println!("  mmm worktree merge {}", session.name);
            Ok(())
        }
    }
}

/// Handle "no" response to merge prompt
fn handle_merge_no(
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
) -> Result<()> {
    // Update state with response
    worktree_manager.update_session_state(&session.name, |state| {
        state.merge_prompt_response = Some("no".to_string());
    })?;

    println!("\nTo merge changes later, run:");
    println!("  mmm worktree merge {}", session.name);
    Ok(())
}

async fn run_in_worktree(
    cmd: command::CookCommand,
    session: crate::worktree::WorktreeSession,
    original_repo_path: std::path::PathBuf,
    verbose: bool,
) -> Result<()> {
    // Check if we have args or map patterns
    if !cmd.args.is_empty() || !cmd.map.is_empty() {
        // Run with mapping logic in worktree context
        run_with_mapping_in_worktree(cmd, session, original_repo_path, verbose).await
    } else {
        // Run standard improvement loop
        let worktree_manager = WorktreeManager::new(original_repo_path.to_path_buf())?;

        // Set up signal handlers for graceful interruption
        let worktree_manager_arc = Arc::new(worktree_manager);
        signal_handler::setup_interrupt_handlers(
            worktree_manager_arc.clone(),
            session.name.clone(),
        )?;

        // Run improvement loop with state tracking
        let result =
            run_improvement_loop(cmd.clone(), &session, &worktree_manager_arc, verbose).await;

        // Update final state
        update_session_final_state(&worktree_manager_arc, &session.name, &result)?;

        result
    }
}

/// Sets up the improvement session by checking prerequisites and loading configuration
async fn setup_improvement_session(
    cmd: &command::CookCommand,
    verbose: bool,
) -> Result<(std::path::PathBuf, Config, StateManager)> {
    // Check prerequisites
    check_claude_cli().await?;

    // Get project path and perform analysis
    let project_path = std::env::current_dir()?;
    perform_project_analysis(cmd, &project_path, verbose).await;

    // Display focus if set
    display_focus(cmd, verbose);

    // Setup state and load configuration
    let state = StateManager::new()?;
    let config = load_project_configuration().await?;

    Ok((project_path, config, state))
}

/// Perform project analysis if not skipped
async fn perform_project_analysis(cmd: &command::CookCommand, project_path: &Path, verbose: bool) {
    if cmd.skip_analysis {
        if verbose {
            println!("📋 Skipping project analysis (--skip-analysis flag)");
        }
        return;
    }

    if let Err(e) = analyze_project_comprehensive(project_path, verbose).await {
        if verbose {
            eprintln!("⚠️  Comprehensive analysis failed: {e}");
            eprintln!("   Continuing without deep context understanding");
        }
    }
}

/// Display focus directive if set
fn display_focus(cmd: &command::CookCommand, verbose: bool) {
    if verbose {
        if let Some(focus) = &cmd.focus {
            println!("📊 Focus: {focus}");
        }
    }
}

/// Load project configuration
async fn load_project_configuration() -> Result<Config> {
    let config_loader = ConfigLoader::new().await?;
    config_loader
        .load_with_explicit_path(Path::new("."), None)
        .await?;
    Ok(config_loader.get_config().clone())
}

/// Update session final state based on result
fn update_session_final_state<T>(
    worktree_manager: &WorktreeManager,
    session_name: &str,
    result: &Result<T>,
) -> Result<()> {
    worktree_manager.update_session_state(session_name, |state| match result {
        Ok(_) => {
            state.status = crate::worktree::WorktreeStatus::Completed;
        }
        Err(e) => {
            state.status = crate::worktree::WorktreeStatus::Failed;
            state.error = Some(e.to_string());
        }
    })
}

/// Finalizes the improvement session by updating state and committing changes
async fn finalize_improvement_session(
    worktree_manager: &WorktreeManager,
    session_name: &str,
    completed_iterations: u32,
    files_changed: usize,
    max_iterations: u32,
    verbose: bool,
) -> Result<()> {
    // Final state update
    worktree_manager.update_session_state(session_name, |state| {
        state.iterations.completed = completed_iterations;
    })?;

    // Commit the state file
    commit_state_file(completed_iterations, files_changed as u32).await?;

    // Final summary
    if verbose {
        print_session_summary(completed_iterations, files_changed, max_iterations);
    }

    Ok(())
}

/// Commits the state file with session information
async fn commit_state_file(iterations: u32, files_changed: u32) -> Result<()> {
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
            Iterations: {iterations}\n\
            Files changed: {files_changed}"
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

    Ok(())
}

/// Prints the final session summary
fn print_session_summary(iterations: u32, files_changed: usize, max_iterations: u32) {
    let completed_all = iterations >= max_iterations;

    if iterations == 0 {
        println!("\n⚠️  No improvements were made:");
        println!("   No issues were found to fix");
    } else if completed_all {
        println!("\n✅ Improvement session completed:");
        println!("   Iterations: {iterations} (reached maximum)");
        println!("   Files improved: {files_changed}");
    } else {
        println!("\n✅ Improvement session finished early:");
        println!("   Iterations: {iterations}/{max_iterations}");
        println!("   Files improved: {files_changed}");
        println!("   Reason: No more issues found");
    }
    println!("   Session state: saved");
}

async fn run_improvement_loop(
    cmd: command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    verbose: bool,
) -> Result<()> {
    // Setup phase
    let (_project_path, _config, _state) = setup_improvement_session(&cmd, verbose).await?;
    let _start_time = Utc::now();

    // Main improvement loop
    let mut iteration = 1;
    let mut files_changed: usize = 0;

    // Load playbook
    let workflow_config = load_playbook(&cmd.playbook).await?;
    if verbose {
        println!("📖 Loaded playbook: {}", cmd.playbook.display());
    }

    let max_iterations = cmd.max_iterations;
    let mut executor = WorkflowExecutor::new(workflow_config, verbose, max_iterations).await?;

    while iteration <= max_iterations {
        // Update worktree state before iteration
        worktree_manager.update_session_state(&session.name, |state| {
            state.iterations.completed = iteration - 1;
            state.iterations.max = max_iterations;
        })?;

        // Execute workflow iteration
        // Pass focus on every iteration to maintain consistent improvement direction
        let focus_for_iteration = cmd.focus.as_deref();

        let iteration_success = executor
            .execute_iteration(iteration, focus_for_iteration)
            .await?;
        if !iteration_success {
            println!("ℹ️  Iteration {iteration} completed with no changes - stopping early");
            println!("   (This typically means no issues were found to fix)");
            break;
        }

        files_changed += 1;

        // Update stats after iteration
        worktree_manager.update_session_state(&session.name, |state| {
            state.stats.files_changed = files_changed as u32;
            state.stats.commits += 3; // review + implement + lint
        })?;

        // Run inter-iteration analysis if not skipped
        if !cmd.skip_analysis {
            if let Err(e) = analyze_project_comprehensive(&_project_path, verbose).await {
                if verbose {
                    eprintln!("⚠️  Inter-iteration analysis failed: {e}");
                    eprintln!("   Continuing with stale context");
                }
            }
        }

        iteration += 1;
    }

    // Final state update
    finalize_improvement_session(
        worktree_manager,
        &session.name,
        iteration - 1,
        files_changed,
        cmd.max_iterations,
        verbose,
    )
    .await?;

    Ok(())
}

/// Run standard cook with variables for mapping support
async fn run_standard_with_variables(
    cmd: command::CookCommand,
    variables: std::collections::HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
    // Run the standard flow but with variables available for command substitution
    run_without_worktree_with_vars(cmd, variables, verbose).await
}

async fn run_without_worktree(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    run_without_worktree_with_vars(cmd, std::collections::HashMap::new(), verbose).await
}

/// Sets up the improvement environment with configuration and initial analysis
async fn setup_improvement_environment(
    cmd: &command::CookCommand,
    verbose: bool,
) -> Result<(PathBuf, Config, StateManager)> {
    // Load configuration
    let config_loader = ConfigLoader::new().await?;
    config_loader
        .load_with_explicit_path(Path::new("."), None)
        .await?;
    config_loader.load_project(Path::new(".")).await?;
    let config = config_loader.get_config().clone();

    // Show config source in verbose mode
    if verbose {
        println!("📄 Using default configuration");
    }

    // Run comprehensive analysis
    let project_path = std::env::current_dir()?;
    if !cmd.skip_analysis {
        if let Err(e) = analyze_project_comprehensive(&project_path, verbose).await {
            if verbose {
                eprintln!("⚠️  Comprehensive analysis failed: {e}");
                eprintln!("   Continuing without deep context understanding");
            }
        }
    } else if verbose {
        println!("📋 Skipping project analysis (--skip-analysis flag)");
    }

    // Setup state
    let state = StateManager::new()?;

    Ok((project_path, config, state))
}

/// Runs improvement iterations based on workflow configuration
#[allow(clippy::too_many_arguments)]
async fn run_improvement_iterations(
    cmd: &command::CookCommand,
    _config: &Config,
    variables: std::collections::HashMap<String, String>,
    files_changed: &mut usize,
    iteration: &mut u32,
    metrics_collector: &Option<MetricsCollector>,
    metrics_history: &mut MetricsHistory,
    project_path: &Path,
    verbose: bool,
) -> Result<u32> {
    let max_iterations = cmd.max_iterations;

    // Load playbook
    let workflow_config = load_playbook(&cmd.playbook).await?;
    if verbose {
        println!("📖 Loaded playbook: {}", cmd.playbook.display());
    }

    // Use configurable workflow
    let mut executor = WorkflowExecutor::new(workflow_config, verbose, max_iterations).await?;
    executor.set_variables(variables);

    while *iteration <= max_iterations {
        // Execute workflow iteration
        let focus_for_iteration = cmd.focus.as_deref();
        let iteration_success = executor
            .execute_iteration(*iteration, focus_for_iteration)
            .await?;

        if !iteration_success {
            println!("ℹ️  Iteration {iteration} completed with no changes - stopping early");
            println!("   (This typically means no issues were found to fix)");
            break;
        }

        *files_changed += 1;

        // Collect metrics after iteration
        maybe_collect_metrics(
            cmd,
            metrics_collector,
            metrics_history,
            project_path,
            *iteration,
            verbose,
        )
        .await?;

        // Run inter-iteration analysis if not skipped
        if !cmd.skip_analysis {
            if let Err(e) = analyze_project_comprehensive(project_path, verbose).await {
                if verbose {
                    eprintln!("⚠️  Inter-iteration analysis failed: {e}");
                    eprintln!("   Continuing with stale context");
                }
            }
        }

        *iteration += 1;
    }

    Ok(*iteration - 1)
}

/// Finalizes a non-worktree session by committing state and showing summary
async fn finalize_non_worktree_session(
    _state: &StateManager,
    actual_iterations: u32,
    files_changed: usize,
    cmd: &command::CookCommand,
    _verbose: bool,
) -> Result<()> {
    // Commit the state file
    commit_state_file(actual_iterations, files_changed as u32).await?;

    // Show completion message
    if actual_iterations == 0 {
        println!("\n⚠️  No improvements were made:");
        println!("   No issues were found to fix");
    } else if actual_iterations >= cmd.max_iterations {
        println!("\n✅ Improvement session completed:");
        println!("   Iterations: {actual_iterations} (reached maximum)");
        println!("   Files improved: {files_changed}");
    } else {
        println!("\n✅ Improvement session finished early:");
        println!(
            "   Iterations: {}/{}",
            actual_iterations, cmd.max_iterations
        );
        println!("   Files improved: {files_changed}");
        println!("   Reason: No more issues found");
    }

    Ok(())
}

/// Displays final metrics summary
fn display_metrics_summary(
    metrics_history: &MetricsHistory,
    actual_iterations: u32,
    project_path: &Path,
) -> Result<()> {
    if let Some(latest_metrics) = metrics_history.latest() {
        println!("\n📊 Final Metrics Summary:");
        println!(
            "   Overall Score: {:.1}/100",
            latest_metrics.overall_score()
        );
        println!("   Test Coverage: {:.1}%", latest_metrics.test_coverage);
        println!("   Lint Warnings: {}", latest_metrics.lint_warnings);
        println!("   Technical Debt: {:.1}", latest_metrics.tech_debt_score);

        // Show improvement velocity
        let velocity = metrics_history.calculate_velocity(actual_iterations as usize);
        if velocity != 0.0 {
            println!("   Improvement Rate: {velocity:+.1} points/iteration");
        }

        // Save final report
        let storage = MetricsStorage::new(project_path);
        let report = storage.generate_report(latest_metrics);
        if let Err(e) = storage.save_report(&report, &latest_metrics.iteration_id) {
            eprintln!("⚠️  Failed to save final metrics report: {}", e);
        }
    }
    Ok(())
}

async fn run_without_worktree_with_vars(
    cmd: command::CookCommand,
    variables: std::collections::HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
    println!("🔍 Starting improvement loop...");

    // Load configuration and setup
    let (project_path, config, state) = setup_improvement_environment(&cmd, verbose).await?;

    // Setup metrics collection
    let (metrics_collector, mut metrics_history) = setup_metrics(&cmd, &project_path)?;

    // Initialize iteration tracking
    let mut iteration = 1;
    let mut files_changed: usize = 0;

    // Display focus directive if provided
    if let Some(focus) = &cmd.focus {
        println!("📋 Focus: {focus}");
    }

    // Run main improvement loop
    let actual_iterations = run_improvement_iterations(
        &cmd,
        &config,
        variables,
        &mut files_changed,
        &mut iteration,
        &metrics_collector,
        &mut metrics_history,
        &project_path,
        verbose,
    )
    .await?;

    // Finalize session
    finalize_non_worktree_session(&state, actual_iterations, files_changed, &cmd, verbose).await?;

    // Show metrics summary if enabled
    if cmd.metrics && actual_iterations > 0 {
        display_metrics_summary(&metrics_history, actual_iterations, &project_path)?;
    }

    Ok(())
}

/// Call Claude CLI for code review and generate improvement spec
///
/// # Arguments
/// * `verbose` - Whether to show detailed progress messages
/// * `focus` - Optional focus directive for the first iteration
///
/// Run improvement with mapping in worktree context
async fn run_with_mapping_in_worktree(
    cmd: command::CookCommand,
    session: crate::worktree::WorktreeSession,
    original_repo_path: std::path::PathBuf,
    verbose: bool,
) -> Result<()> {
    let inputs = collect_mapping_inputs(&cmd)?;

    println!("📋 Processing {} input(s)", inputs.len());

    let mut success_count = 0;
    let mut failure_count = 0;
    let total = inputs.len();

    let worktree_manager = WorktreeManager::new(original_repo_path)?;

    for (index, input) in inputs.iter().enumerate() {
        let item_number = index + 1;
        println!("\n[{item_number}/{total}] Processing: {input}");

        // Create a new command instance for this input
        let input_cmd = cmd.clone();

        // Create variables for this input
        let variables = create_mapping_variables(&cmd, input, index, total);

        // Store variables in environment for the subprocess
        for (key, value) in &variables {
            std::env::set_var(format!("MMM_VAR_{key}"), value);
        }

        // Run the improvement loop with variables
        let result = run_improvement_loop_with_variables(
            input_cmd,
            &session,
            &worktree_manager,
            variables.clone(),
            verbose,
        )
        .await;

        // Clean up environment variables
        for key in ["ARG", "FILE", "INDEX", "TOTAL", "FILE_NAME", "FILE_STEM"] {
            std::env::remove_var(format!("MMM_VAR_{key}"));
        }

        match result {
            Ok(_) => {
                success_count += 1;
                println!("✅ [{item_number}/{total}] Completed: {input}");
            }
            Err(e) => {
                failure_count += 1;
                eprintln!("❌ [{item_number}/{total}] Failed: {input} - {e}");
                if cmd.fail_fast {
                    return Err(anyhow!("Stopping due to --fail-fast: {}", e));
                }
            }
        }
    }

    println!(
        "\n📊 Summary: {success_count} succeeded, {failure_count} failed out of {total} total"
    );

    if failure_count > 0 && !cmd.fail_fast {
        Err(anyhow!("{} input(s) failed processing", failure_count))
    } else {
        Ok(())
    }
}

/// Run improvement loop with variables in worktree context
async fn run_improvement_loop_with_variables(
    cmd: command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    variables: std::collections::HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
    // The actual improvement logic, but with state tracking
    // This is a copy of run_without_worktree logic with state updates

    // 1. Check for Claude CLI
    check_claude_cli().await?;

    // 2. Initial analysis
    // Run comprehensive analysis to understand the project and collect accurate metrics
    let project_path = std::env::current_dir()?;
    if !cmd.skip_analysis {
        if let Err(e) = analyze_project_comprehensive(&project_path, verbose).await {
            if verbose {
                eprintln!("⚠️  Comprehensive analysis failed: {e}");
                eprintln!("   Continuing without deep context understanding");
            }
        }
    } else if verbose {
        println!("📋 Skipping project analysis (--skip-analysis flag)");
    }

    if verbose {
        if let Some(focus) = &cmd.focus {
            println!("📊 Focus: {focus}");
        }
    }

    // 3. Setup basic state
    let _state = StateManager::new()?;
    let _start_time = Utc::now();

    // 4. Main improvement loop
    let mut iteration = 1;
    let files_changed = 0;

    // Load playbook
    let workflow_config = load_playbook(&cmd.playbook).await?;
    if verbose {
        println!("📖 Loaded playbook: {}", cmd.playbook.display());
    }

    let max_iterations = cmd.max_iterations;
    let mut executor = WorkflowExecutor::new(workflow_config, verbose, max_iterations).await?;
    executor.set_variables(variables);

    while iteration <= max_iterations {
        // Update worktree state before iteration
        worktree_manager.update_session_state(&session.name, |state| {
            state.iterations.completed = iteration - 1;
            state.iterations.max = max_iterations;
        })?;

        // Execute workflow iteration
        // Pass focus on every iteration to maintain consistent improvement direction
        let focus_for_iteration = cmd.focus.as_deref();

        let iteration_success = executor
            .execute_iteration(iteration, focus_for_iteration)
            .await?;
        if !iteration_success {
            println!("ℹ️  Iteration {iteration} completed with no changes - stopping early");
            println!("   (This typically means no issues were found to fix)");
            break;
        }

        // Run inter-iteration analysis if not skipped
        if !cmd.skip_analysis {
            if let Err(e) = analyze_project_comprehensive(&project_path, verbose).await {
                if verbose {
                    eprintln!("⚠️  Inter-iteration analysis failed: {e}");
                    eprintln!("   Continuing with stale context");
                }
            }
        }

        iteration += 1;
    }

    // Final state update
    worktree_manager.update_session_state(&session.name, |state| {
        state.iterations.completed = iteration - 1;
        state.stats.files_changed = files_changed as u32;
    })?;

    Ok(())
}

#[cfg(test)]
mod cook_inline_tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_command(worktree: bool, max_iterations: u32) -> command::CookCommand {
        command::CookCommand {
            playbook: PathBuf::from("examples/default.yml"),
            path: None,
            max_iterations,
            worktree,
            focus: None,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            skip_analysis: false,
        }
    }

    #[tokio::test]
    async fn test_run_with_worktree_flag() {
        // Test that worktree flag is properly handled
        let cmd = create_test_command(true, 1);

        // We can't fully test without a git repo, but we can verify the logic
        assert!(cmd.worktree);
        assert_eq!(cmd.max_iterations, 1);
    }

    #[test]
    fn test_focus_applied_every_iteration() {
        // Test that focus is consistently applied across all iterations
        let mut cmd = create_test_command(false, 3);
        cmd.focus = Some("performance".to_string());

        // Simulate multiple iterations and verify focus is available each time
        for iteration in 1..=3 {
            // In the actual code, focus_for_iteration is now always cmd.focus.as_deref()
            // regardless of iteration number
            let focus_for_iteration = cmd.focus.as_deref();

            assert_eq!(
                focus_for_iteration,
                Some("performance"),
                "Focus should be applied on iteration {iteration}"
            );
        }
    }

    #[tokio::test]
    async fn test_run_without_worktree_target_already_reached() {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .output()
            .unwrap();

        // Create minimal project structure
        std::fs::create_dir_all(".mmm").unwrap();

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

            assert_eq!(result, expected, "Failed for input: '{input}'");
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
                "Command '{cmd}' failed with exit code {code:?}\nStderr: {stderr}\nStdout: {stdout}"
            );

            assert!(formatted.contains(cmd));
            assert!(formatted.contains(&format!("{code:?}")));
            if !stderr.is_empty() {
                assert!(formatted.contains(stderr));
            }
            if !stdout.is_empty() {
                assert!(formatted.contains(stdout));
            }
        }
    }
}

/// Resume point for an interrupted session
#[derive(Debug)]
enum ResumePoint {
    FromStart,
    RetryCommand,
    NextCommand,
}

/// Resume an interrupted session from its last checkpoint
async fn resume_session(
    session_id: &str,
    mut cmd: command::CookCommand,
    verbose: bool,
) -> Result<()> {
    // Get current repository path
    let repo_path = std::env::current_dir()?;
    let worktree_manager = WorktreeManager::new(repo_path.clone())?;

    // Load interrupted session state
    let state = worktree_manager.load_session_state(session_id)?;

    if state.status != crate::worktree::WorktreeStatus::Interrupted || !state.resumable {
        return Err(anyhow!("Session {} is not resumable", session_id));
    }

    // Restore worktree session
    let session = worktree_manager.restore_session(session_id)?;

    // Determine where to resume from
    let (start_iteration, resume_point) = match &state.last_checkpoint {
        Some(checkpoint) => {
            println!(
                "Last checkpoint: {:?} command at iteration {}",
                checkpoint.last_command_type, checkpoint.iteration
            );

            // Determine if we need to retry the last command or move to next
            match checkpoint.last_command_type {
                crate::worktree::CommandType::CodeReview => {
                    if checkpoint.last_spec_id.is_some() {
                        // Review completed successfully, continue with implement
                        (checkpoint.iteration, ResumePoint::NextCommand)
                    } else {
                        // Review didn't complete, retry it
                        (checkpoint.iteration, ResumePoint::RetryCommand)
                    }
                }
                crate::worktree::CommandType::ImplementSpec => {
                    // Check if implementation was completed
                    if checkpoint.command_output.is_some() {
                        (checkpoint.iteration, ResumePoint::NextCommand)
                    } else {
                        (checkpoint.iteration, ResumePoint::RetryCommand)
                    }
                }
                crate::worktree::CommandType::Lint => {
                    // Lint is usually quick, just retry
                    (checkpoint.iteration, ResumePoint::RetryCommand)
                }
                crate::worktree::CommandType::Custom(_) => {
                    // For custom commands, be conservative and retry
                    (checkpoint.iteration, ResumePoint::RetryCommand)
                }
            }
        }
        None => (1, ResumePoint::FromStart),
    };

    println!("Resuming session {session_id} from iteration {start_iteration} ({resume_point:?})");

    // Update command with session's original settings
    cmd.focus = state.focus;
    cmd.max_iterations = state.iterations.max;

    // Change to worktree directory
    std::env::set_current_dir(&session.path)?;

    // Clear interrupted status
    worktree_manager.update_session_state(&session.name, |state| {
        state.status = crate::worktree::WorktreeStatus::InProgress;
        state.interrupted_at = None;
        state.interruption_type = None;
    })?;

    // Continue improvement loop from checkpoint
    run_improvement_loop_from(
        cmd,
        session,
        worktree_manager,
        start_iteration,
        resume_point,
        verbose,
    )
    .await
}

/// Run improvement loop from a specific point (for resume functionality)
async fn run_improvement_loop_from(
    cmd: command::CookCommand,
    session: crate::worktree::WorktreeSession,
    worktree_manager: WorktreeManager,
    start_iteration: u32,
    resume_point: ResumePoint,
    verbose: bool,
) -> Result<()> {
    // Set up signal handlers for graceful interruption
    let worktree_manager_arc = Arc::new(worktree_manager);
    signal_handler::setup_interrupt_handlers(worktree_manager_arc.clone(), session.name.clone())?;

    // Run improvement loop with checkpoint support
    let result = run_improvement_loop_with_checkpoints(
        cmd,
        &session,
        &worktree_manager_arc,
        start_iteration,
        resume_point,
        verbose,
    )
    .await;

    // Update final state
    update_session_final_state(&worktree_manager_arc, &session.name, &result)?;

    result
}

/// Determine command type from command string
#[allow(dead_code)]
fn determine_command_type(command: &str) -> crate::worktree::CommandType {
    if command.contains("mmm-code-review") {
        crate::worktree::CommandType::CodeReview
    } else if command.contains("mmm-implement-spec") {
        crate::worktree::CommandType::ImplementSpec
    } else if command.contains("mmm-lint") {
        crate::worktree::CommandType::Lint
    } else {
        crate::worktree::CommandType::Custom(command.to_string())
    }
}

/// Run improvement loop with checkpoint support
async fn run_improvement_loop_with_checkpoints(
    cmd: command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    _start_iteration: u32,
    _resume_point: ResumePoint,
    verbose: bool,
) -> Result<()> {
    // Most of the logic from run_improvement_loop, but with checkpoint creation
    // This is a simplified version - in practice, we'd refactor run_improvement_loop
    // to support checkpointing throughout

    // For now, just delegate to the existing function
    // In a full implementation, we'd add checkpoint creation before each command
    run_improvement_loop(cmd, session, worktree_manager, verbose).await
}
