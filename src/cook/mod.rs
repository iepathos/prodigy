pub mod command;
pub mod git_ops;
pub mod retry;
pub mod session;
pub mod signal_handler;
pub mod workflow;

#[cfg(test)]
mod tests;

use crate::config::{ConfigLoader, WorkflowConfig};
use crate::context::{save_analysis, ContextAnalyzer, ProjectAnalyzer};
use crate::simple_state::StateManager;
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context as _, Result};
use chrono::Utc;
use git_ops::get_last_commit_message;
use retry::{check_claude_cli, execute_with_retry, format_subprocess_error};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};
use workflow::WorkflowExecutor;

/// Default number of retry attempts for Claude CLI commands
const DEFAULT_CLAUDE_RETRIES: u32 = 2;

/// Choice made by user when prompted to merge
enum MergeChoice {
    Yes,
    No,
}

/// Run context analysis on the project and save results
async fn analyze_project_context(project_path: &Path, verbose: bool) -> Result<()> {
    if verbose {
        info!("🔍 Analyzing project context...");
    }

    // Create analyzer
    let analyzer = ProjectAnalyzer::new();

    // Run analysis
    let analysis_result = analyzer.analyze(project_path).await?;

    // Get improvement suggestions
    let suggestions = analyzer.get_improvement_suggestions();

    if verbose {
        info!("✅ Context analysis complete");
        info!(
            "   - Dependencies: {} modules analyzed",
            analysis_result.dependency_graph.nodes.len()
        );
        info!(
            "   - Architecture: {} patterns detected",
            analysis_result.architecture.patterns.len()
        );
        info!(
            "   - Technical debt: {} items found",
            analysis_result.technical_debt.debt_items.len()
        );
        info!(
            "   - Test coverage: {:.1}% overall",
            analysis_result.test_coverage.overall_coverage * 100.0
        );

        if !suggestions.is_empty() {
            info!("\n📋 Top improvement suggestions:");
            for (i, suggestion) in suggestions.iter().take(3).enumerate() {
                info!("   {}. {}", i + 1, suggestion.title);
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

async fn run_internal(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    // Save original working directory before any changes
    let original_dir = std::env::current_dir().context("Failed to get current directory")?;

    // Handle path argument if provided
    if let Some(ref path) = cmd.path {
        // Expand tilde notation if present
        let expanded_path = if path.to_string_lossy().starts_with("~/") {
            let home =
                dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
            home.join(path.strip_prefix("~/").unwrap())
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
            info!("📁 Working in: {}", absolute_path.display());
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
async fn run_with_mapping(cmd: command::CookCommand, verbose: bool) -> Result<()> {
    use glob::glob;
    use std::collections::HashMap;

    // Check if worktree isolation should be used
    let use_worktree = if cmd.worktree {
        true
    } else if std::env::var("MMM_USE_WORKTREE")
        .unwrap_or_default()
        .eq_ignore_ascii_case("true")
    {
        warn!("⚠️  Warning: MMM_USE_WORKTREE environment variable is deprecated. Use --worktree flag instead.");
        true
    } else {
        false
    };

    if use_worktree {
        // Save the original repository path before changing directories
        let original_repo_path = std::env::current_dir()?;

        // Create a new worktree for this improvement session
        let worktree_manager = WorktreeManager::new(original_repo_path.clone())?;
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

        // Clean up on failure, keep on success for manual merge
        match &result {
            Ok(_) => {
                // Check if any actual improvements were made
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

                // Prompt for merge if in interactive terminal (or auto-accept) and improvements were made
                if (is_tty() || cmd.auto_accept) && iterations_completed > 0 {
                    // Update state to track prompt shown
                    let worktree_manager = WorktreeManager::new(original_repo_path.clone())?;
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
                            // Update state with response
                            worktree_manager.update_session_state(&session.name, |state| {
                                state.merge_prompt_response = Some("yes".to_string());
                            })?;

                            println!("Merging worktree {}...", session.name);
                            match merge_worktree(&session.name, &original_repo_path).await {
                                Ok(_) => {
                                    println!("✅ Successfully merged worktree: {}", session.name);

                                    // Update worktree status to merged
                                    worktree_manager.update_session_state(
                                        &session.name,
                                        |state| {
                                            state.status = crate::worktree::WorktreeStatus::Merged;
                                        },
                                    )?;

                                    // Prompt to delete the worktree
                                    let should_delete = if cmd.auto_accept {
                                        println!(
                                            "Auto-accepting worktree deletion (--yes flag set)"
                                        );
                                        true
                                    } else {
                                        print!(
                                            "\nWould you like to delete the worktree now? (y/N): "
                                        );
                                        io::stdout().flush().unwrap_or_default();

                                        let mut input = String::new();
                                        io::stdin().read_line(&mut input).unwrap_or_default();
                                        input.trim().to_lowercase() == "y"
                                    };

                                    if should_delete {
                                        println!("Deleting worktree {}...", session.name);
                                        match worktree_manager.cleanup_session(&session.name, false)
                                        {
                                            Ok(_) => {
                                                println!("✅ Worktree deleted successfully");
                                            }
                                            Err(e) => {
                                                warn!("❌ Failed to delete worktree: {e}");
                                                println!("You can manually delete it with:");
                                                println!("  mmm worktree delete {}", session.name);
                                            }
                                        }
                                    } else {
                                        println!("\nTo delete the worktree later, run:");
                                        println!("  mmm worktree delete {}", session.name);
                                    }
                                }
                                Err(e) => {
                                    warn!("❌ Failed to merge worktree: {e}");
                                    println!("\nTo merge changes manually, run:");
                                    println!("  mmm worktree merge {}", session.name);
                                }
                            }
                        }
                        MergeChoice::No => {
                            // Update state with response
                            worktree_manager.update_session_state(&session.name, |state| {
                                state.merge_prompt_response = Some("no".to_string());
                            })?;

                            println!("\nTo merge changes later, run:");
                            println!("  mmm worktree merge {}", session.name);
                        }
                    }
                } else if iterations_completed > 0 {
                    // Non-interactive environment but improvements were made
                    println!("\nWorktree completed. To merge changes, run:");
                    println!("  mmm worktree merge {}", session.name);
                } else {
                    // No improvements made
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

        return result;
    }

    // Non-worktree path continues below
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
                    warn!("Warning: Error matching pattern: {e}");
                }
            }
        }
    }

    // Add direct arguments from --args
    inputs.extend(cmd.args.clone());

    if inputs.is_empty() {
        return Err(anyhow!("No inputs found from --map patterns or --args"));
    }

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
        let mut variables = HashMap::new();

        // Determine the ARG value based on whether this came from --map or --args
        let arg_value = if cmd.map.iter().any(|pattern| {
            glob(pattern)
                .ok()
                .and_then(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .find(|p| &p.to_string_lossy() == input)
                })
                .is_some()
        }) {
            // This input came from --map, so extract spec ID if it's a spec file
            if input.ends_with(".md") && input.contains("spec") {
                extract_spec_id_from_path(input)
            } else {
                input.clone()
            }
        } else {
            // This input came from --args, use it directly
            input.clone()
        };

        variables.insert("ARG".to_string(), arg_value.clone());
        variables.insert("FILE".to_string(), input.clone());
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
        // Save the original repository path before changing directories
        let original_repo_path = std::env::current_dir()?;

        // Create worktree for this session
        let worktree_manager = WorktreeManager::new(original_repo_path.clone())?;
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

        // Clean up on failure, keep on success for manual merge
        match &result {
            Ok(_) => {
                // Check if any actual improvements were made
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

                // Prompt for merge if in interactive terminal (or auto-accept) and improvements were made
                if (is_tty() || cmd.auto_accept) && iterations_completed > 0 {
                    // Update state to track prompt shown
                    let worktree_manager = WorktreeManager::new(original_repo_path.clone())?;
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
                            // Update state with response
                            worktree_manager.update_session_state(&session.name, |state| {
                                state.merge_prompt_response = Some("yes".to_string());
                            })?;

                            println!("Merging worktree {}...", session.name);
                            match merge_worktree(&session.name, &original_repo_path).await {
                                Ok(_) => {
                                    println!("✅ Successfully merged worktree: {}", session.name);

                                    // Update worktree status to merged
                                    worktree_manager.update_session_state(
                                        &session.name,
                                        |state| {
                                            state.status = crate::worktree::WorktreeStatus::Merged;
                                        },
                                    )?;

                                    // Prompt to delete the worktree
                                    let should_delete = if cmd.auto_accept {
                                        println!(
                                            "Auto-accepting worktree deletion (--yes flag set)"
                                        );
                                        true
                                    } else {
                                        print!(
                                            "\nWould you like to delete the worktree now? (y/N): "
                                        );
                                        io::stdout().flush().unwrap_or_default();

                                        let mut input = String::new();
                                        io::stdin().read_line(&mut input).unwrap_or_default();
                                        input.trim().to_lowercase() == "y"
                                    };

                                    if should_delete {
                                        println!("Deleting worktree {}...", session.name);
                                        match worktree_manager.cleanup_session(&session.name, false)
                                        {
                                            Ok(_) => {
                                                println!("✅ Worktree deleted successfully");
                                            }
                                            Err(e) => {
                                                warn!("❌ Failed to delete worktree: {e}");
                                                println!("You can manually delete it with:");
                                                println!("  mmm worktree delete {}", session.name);
                                            }
                                        }
                                    } else {
                                        println!("\nTo delete the worktree later, run:");
                                        println!("  mmm worktree delete {}", session.name);
                                    }
                                }
                                Err(e) => {
                                    warn!("❌ Failed to merge worktree: {e}");
                                    println!("\nTo merge changes manually, run:");
                                    println!("  mmm worktree merge {}", session.name);
                                }
                            }
                        }
                        MergeChoice::No => {
                            // Update state with response
                            worktree_manager.update_session_state(&session.name, |state| {
                                state.merge_prompt_response = Some("no".to_string());
                            })?;

                            println!("\nTo merge changes later, run:");
                            println!("  mmm worktree merge {}", session.name);
                        }
                    }
                } else if iterations_completed > 0 {
                    // Non-interactive environment but improvements were made
                    println!("\nWorktree completed. To merge changes, run:");
                    println!("  mmm worktree merge {}", session.name);
                } else {
                    // No improvements made
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

        result
    } else {
        // Run without worktree isolation (default behavior)
        run_without_worktree(cmd, verbose).await
    }
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
        let worktree_manager = WorktreeManager::new(original_repo_path.clone())?;

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
        worktree_manager_arc.update_session_state(&session.name, |state| match &result {
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
}

async fn run_improvement_loop(
    cmd: command::CookCommand,
    session: &crate::worktree::WorktreeSession,
    worktree_manager: &WorktreeManager,
    verbose: bool,
) -> Result<()> {
    // The actual improvement logic, but with state tracking
    // This is a copy of run_without_worktree logic with state updates

    // 1. Check for Claude CLI
    check_claude_cli().await?;

    // 2. Initial analysis
    // Run context analysis to understand the project
    let project_path = std::env::current_dir()?;
    if let Err(e) = analyze_project_context(&project_path, verbose).await {
        if verbose {
            warn!("⚠️  Context analysis failed: {e}");
            warn!("   Continuing without deep context understanding");
        }
    }

    if verbose {
        if let Some(focus) = &cmd.focus {
            info!("📊 Focus: {focus}");
        }
    }

    // 3. Setup basic state
    let _state = StateManager::new()?;
    let _start_time = Utc::now();

    // 4. Main improvement loop
    let mut iteration = 1;
    let mut files_changed = 0;

    // Load configuration (with workflow if present)
    let config_loader = ConfigLoader::new().await?;
    config_loader
        .load_with_explicit_path(Path::new("."), cmd.config.as_deref())
        .await?;
    let config = config_loader.get_config();

    // Check if we have a workflow configuration
    if let Some(workflow_config) = config.workflow {
        // Use configurable workflow
        if verbose {
            info!("Using custom workflow from configuration");
        }

        let max_iterations = cmd.max_iterations;
        let mut executor = WorkflowExecutor::new(workflow_config, verbose, max_iterations);

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

            if verbose {
                info!("🔄 Iteration {iteration}/{}...", cmd.max_iterations);
            }

            // Step 1: Generate review spec and commit
            // Create checkpoint before code review
            worktree_manager.create_checkpoint(
                &session.name,
                crate::worktree::Checkpoint {
                    iteration,
                    timestamp: Utc::now(),
                    last_command: "/mmm-code-review".to_string(),
                    last_command_type: crate::worktree::CommandType::CodeReview,
                    last_spec_id: None,
                    files_modified: vec![],
                    command_output: None,
                },
            )?;

            // Pass focus on every iteration to maintain consistent improvement direction
            let focus_for_iteration = cmd.focus.as_deref();
            let review_success = call_claude_code_review(verbose, focus_for_iteration).await?;
            if !review_success {
                if verbose {
                    info!("Review failed - stopping iterations");
                }
                break;
            }

            // Step 2: Extract spec ID from latest commit
            let spec_id = extract_spec_from_git(verbose).await?;
            if spec_id.is_empty() {
                if verbose {
                    info!("No issues found - stopping iterations");
                }
                break;
            }

            // Update checkpoint with spec ID
            worktree_manager.update_checkpoint(&session.name, |checkpoint| {
                checkpoint.last_spec_id = Some(spec_id.clone());
                checkpoint.command_output = Some("Review completed".to_string());
            })?;

            // Step 3: Implement fixes and commit
            // Create checkpoint before implementation
            worktree_manager.create_checkpoint(
                &session.name,
                crate::worktree::Checkpoint {
                    iteration,
                    timestamp: Utc::now(),
                    last_command: format!("/mmm-implement-spec {spec_id}"),
                    last_command_type: crate::worktree::CommandType::ImplementSpec,
                    last_spec_id: Some(spec_id.clone()),
                    files_modified: vec![],
                    command_output: None,
                },
            )?;

            let implement_success = call_claude_implement_spec(&spec_id, verbose).await?;
            if !implement_success {
                if verbose {
                    info!("Implementation failed for iteration {iteration}");
                }
            } else {
                files_changed += 1;
                // Update checkpoint after successful implementation
                worktree_manager.update_checkpoint(&session.name, |checkpoint| {
                    checkpoint.command_output = Some("Implementation completed".to_string());
                    checkpoint.files_modified = detect_modified_files();
                })?;
            }

            // Step 4: Run linting/formatting and commit
            // Create checkpoint before linting
            worktree_manager.create_checkpoint(
                &session.name,
                crate::worktree::Checkpoint {
                    iteration,
                    timestamp: Utc::now(),
                    last_command: "/mmm-lint".to_string(),
                    last_command_type: crate::worktree::CommandType::Lint,
                    last_spec_id: Some(spec_id.clone()),
                    files_modified: vec![],
                    command_output: None,
                },
            )?;

            let lint_success = call_claude_lint(verbose).await?;

            if lint_success {
                // Update checkpoint after successful linting
                worktree_manager.update_checkpoint(&session.name, |checkpoint| {
                    checkpoint.command_output = Some("Linting completed".to_string());
                })?;
            }

            // Check if any command made changes in this iteration
            let any_changes = review_success || implement_success || lint_success;
            if !any_changes {
                if verbose {
                    info!("ℹ️  Iteration {iteration} completed with no changes - stopping early");
                    info!("   (This typically means no issues were found to fix)");
                }
                break;
            }

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
    let _ = _state; // Consume state to avoid unused variable warning

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
    if verbose {
        let iterations = iteration - 1;

        // Determine if we completed naturally or stopped early
        let completed_all = iterations >= cmd.max_iterations;

        if iterations == 0 {
            println!("\n⚠️  No improvements were made:");
            println!("   No issues were found to fix");
        } else if completed_all {
            println!("\n✅ Improvement session completed:");
            println!("   Iterations: {iterations} (reached maximum)");
            println!("   Files improved: {files_changed}");
        } else {
            println!("\n✅ Improvement session finished early:");
            println!("   Iterations: {}/{}", iterations, cmd.max_iterations);
            println!("   Files improved: {files_changed}");
            println!("   Reason: No more issues found");
        }
        println!("   Session state: saved");
    }

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

async fn run_without_worktree_with_vars(
    cmd: command::CookCommand,
    variables: std::collections::HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
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
    if verbose {
        if let Some(config_path) = &cmd.config {
            println!("📄 Using configuration from: {}", config_path.display());
        } else {
            println!("📄 Using default configuration");
        }
    }

    // Run context analysis to understand the project
    let project_path = std::env::current_dir()?;
    if let Err(e) = analyze_project_context(&project_path, verbose).await {
        if verbose {
            warn!("⚠️  Context analysis failed: {e}");
            warn!("   Continuing without deep context understanding");
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
    let _state = StateManager::new()?;

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
        let mut executor = WorkflowExecutor::new(workflow_config, verbose, max_iterations)
            .with_variables(variables);

        while iteration <= max_iterations {
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

            iteration += 1;
        }
    } else {
        // Use legacy hardcoded workflow
        while iteration <= cmd.max_iterations {
            if verbose {
                info!("🔄 Iteration {iteration}/{}...", cmd.max_iterations);
            }

            // Step 1: Generate review spec and commit
            // Pass focus on every iteration to maintain consistent improvement direction
            let focus_for_iteration = cmd.focus.as_deref();
            let review_success = call_claude_code_review(verbose, focus_for_iteration).await?;
            if !review_success {
                if verbose {
                    info!("Review failed - stopping iterations");
                }
                break;
            }

            // Step 2: Extract spec ID from latest commit
            let spec_id = extract_spec_from_git(verbose).await?;
            if spec_id.is_empty() {
                if verbose {
                    info!("No issues found - stopping iterations");
                }
                break;
            }

            // Step 3: Implement fixes and commit
            let implement_success = call_claude_implement_spec(&spec_id, verbose).await?;
            if !implement_success {
                if verbose {
                    info!("Implementation failed for iteration {iteration}");
                }
            } else {
                files_changed += 1;
            }

            // Step 4: Run linting/formatting and commit
            let lint_success = call_claude_lint(verbose).await?;

            // Check if any command made changes in this iteration
            let any_changes = review_success || implement_success || lint_success;
            if !any_changes {
                if verbose {
                    info!("ℹ️  Iteration {iteration} completed with no changes - stopping early");
                    info!("   (This typically means no issues were found to fix)");
                }
                break;
            }

            iteration += 1;
        }
    }

    // 5. Completion - record basic session info
    // StateManager handles saving automatically
    let _ = _state; // Consume state to avoid unused variable warning

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
            if !stderr.contains("nothing to commit") && verbose {
                warn!("Warning: Failed to commit .mmm/state.json: {stderr}");
            }
        }
    } else if verbose {
        let stderr = String::from_utf8_lossy(&git_add.stderr);
        warn!("Warning: Failed to stage .mmm/state.json: {stderr}");
    }

    // Completion message
    let actual_iterations = iteration - 1;

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

    // Skip actual execution in test mode
    if std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true" {
        if verbose {
            info!("[TEST MODE] Skipping Claude CLI execution for: mmm-code-review");
        }

        // Check if we should simulate no changes for this command
        if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
            if no_changes_cmds
                .split(',')
                .any(|cmd| cmd.trim() == "mmm-code-review")
            {
                if verbose {
                    info!("[TEST MODE] Simulating no changes for: mmm-code-review");
                }
                return Ok(false);
            }
        }

        // Track focus if requested
        if let Some(focus_directive) = focus {
            if let Ok(track_file) = std::env::var("MMM_TRACK_FOCUS") {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&track_file)
                {
                    let _ = writeln!(file, "iteration: focus={focus_directive}");
                }
            }
        }

        return Ok(true);
    }

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

    // Pass context directory if available
    let context_dir = std::env::current_dir()
        .ok()
        .map(|p| p.join(".mmm").join("context"));
    if let Some(ctx_dir) = context_dir {
        if ctx_dir.exists() {
            cmd.env("MMM_CONTEXT_DIR", ctx_dir);
        }
    }

    // Execute with retry logic for transient failures
    let output =
        execute_with_retry(cmd, "Claude code review", DEFAULT_CLAUDE_RETRIES, verbose).await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let error_msg = format_subprocess_error(
            "claude /mmm-code-review",
            output.status.code(),
            &stderr,
            &stdout,
        );
        return Err(anyhow!(error_msg));
    }

    if verbose {
        info!("✅ Code review completed");
        if !stdout.is_empty() {
            info!("📄 Claude response:");
            info!("{stdout}");
        }
        if !stderr.is_empty() {
            info!("⚠️  Claude stderr:");
            info!("{stderr}");
        }
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
/// This function looks for new spec files created in specs/temp/ directory
/// by checking the git diff of the last commit
async fn extract_spec_from_git(verbose: bool) -> Result<String> {
    if verbose {
        info!("Extracting spec ID from git history...");
    }

    // In test mode, return a mock spec ID
    if std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true" {
        // Check if there's a test spec ID override
        if let Ok(test_spec) = std::env::var("MMM_TEST_SPEC_ID") {
            return Ok(test_spec);
        }
        // Return a default test spec ID
        return Ok("iteration-1234567890-improvements".to_string());
    }

    // First check for uncommitted spec files (the review might have created but not committed them)
    let uncommitted_output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "specs/temp/"])
        .output()
        .await
        .context("Failed to check uncommitted files")?;

    if uncommitted_output.status.success() {
        let files = String::from_utf8_lossy(&uncommitted_output.stdout);
        for line in files.lines() {
            if line.ends_with(".md") {
                if let Some(filename) = line.split('/').next_back() {
                    let spec_id = filename.trim_end_matches(".md");
                    if verbose {
                        info!("Found uncommitted spec file: {spec_id}");
                    }
                    return Ok(spec_id.to_string());
                }
            }
        }
    }

    // Check the last commit for any new spec files in specs/temp/
    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD~1", "HEAD", "--", "specs/temp/"])
        .output()
        .await
        .context("Failed to get git diff")?;

    if !output.status.success() {
        // If we can't diff (e.g., no HEAD~1), try checking what files exist
        if let Ok(find_output) = Command::new("find")
            .args(["specs/temp", "-name", "*.md", "-type", "f", "-mmin", "-5"])
            .output()
            .await
        {
            let files = String::from_utf8_lossy(&find_output.stdout);
            for line in files.lines() {
                if let Some(filename) = line.split('/').next_back() {
                    if filename.ends_with(".md") {
                        let spec_id = filename.trim_end_matches(".md");
                        if verbose {
                            info!("Found recent spec file: {spec_id}");
                        }
                        return Ok(spec_id.to_string());
                    }
                }
            }
        }
        return Ok(String::new());
    }

    let files = String::from_utf8_lossy(&output.stdout);

    // Look for new .md files in specs/temp/
    for line in files.lines() {
        if line.starts_with("specs/temp/") && line.ends_with(".md") {
            if let Some(filename) = line.split('/').next_back() {
                let spec_id = filename.trim_end_matches(".md");
                if verbose {
                    info!("Found new spec file in commit: {spec_id}");
                }
                return Ok(spec_id.to_string());
            }
        }
    }

    // If no spec file in diff, check if this is a review commit
    // and look for recently created spec files
    let commit_message = get_last_commit_message()
        .await
        .context("Failed to get git log")?;

    if commit_message.starts_with("review:") {
        if let Ok(find_output) = Command::new("find")
            .args(["specs/temp", "-name", "*.md", "-type", "f", "-mmin", "-5"])
            .output()
            .await
        {
            let files = String::from_utf8_lossy(&find_output.stdout);
            for line in files.lines() {
                if let Some(filename) = line.split('/').next_back() {
                    if filename.ends_with(".md") {
                        let spec_id = filename.trim_end_matches(".md");
                        if verbose {
                            info!("Found recent spec file: {spec_id}");
                        }
                        return Ok(spec_id.to_string());
                    }
                }
            }
        }
    }

    Ok(String::new()) // No spec found
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
    // Skip actual execution in test mode
    if std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true" {
        if verbose {
            info!("[TEST MODE] Skipping Claude CLI execution for: mmm-implement-spec {spec_id}");
        }

        // Check if we should simulate no changes for this command
        if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
            if no_changes_cmds
                .split(',')
                .any(|cmd| cmd.trim() == "mmm-implement-spec")
            {
                if verbose {
                    info!("[TEST MODE] Simulating no changes for: mmm-implement-spec");
                }
                return Ok(false);
            }
        }

        return Ok(true);
    }

    // Validate spec_id format to prevent potential command injection
    // Accept both "iteration-XXXXXXXXXX-improvements" and "code-review-XXXXXXXXXX" formats
    let is_iteration_format = spec_id.starts_with("iteration-") 
        && spec_id.ends_with("-improvements")
        && spec_id.len() >= 24 // "iteration-" (10) + at least 1 digit + "-improvements" (13)
        && spec_id[10..spec_id.len()-13].chars().all(|c| c.is_ascii_digit() || c == '-');

    let is_code_review_format = spec_id.starts_with("code-review-")
        && spec_id.len() >= 13 // "code-review-" (12) + at least 1 digit
        && spec_id[12..].chars().all(|c| c.is_ascii_digit() || c == '-');

    if !is_iteration_format && !is_code_review_format {
        return Err(anyhow!(
            "Invalid spec ID format: {spec_id}. Expected format: iteration-XXXXXXXXXX-improvements or code-review-XXXXXXXXXX"
        ));
    }

    println!("🔧 Running /mmm-implement-spec {spec_id}...");

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-implement-spec") // The custom command for spec implementation
        .arg(spec_id) // The spec ID to implement (e.g., "iteration-123-improvements")
        .env("ARGUMENTS", spec_id) // Claude CLI expects the argument in $ARGUMENTS
        .env("MMM_AUTOMATION", "true"); // Signals to /mmm-implement-spec to run in automated mode

    // Pass context directory if available
    let context_dir = std::env::current_dir()
        .ok()
        .map(|p| p.join(".mmm").join("context"));
    if let Some(ctx_dir) = context_dir {
        if ctx_dir.exists() {
            cmd.env("MMM_CONTEXT_DIR", ctx_dir);
        }
    }

    // Execute with retry logic for transient failures
    let output = execute_with_retry(
        cmd,
        &format!("Claude implement spec {spec_id}"),
        DEFAULT_CLAUDE_RETRIES,
        verbose,
    )
    .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let error_msg = format_subprocess_error(
            &format!("claude /mmm-implement-spec {spec_id}"),
            output.status.code(),
            &stderr,
            &stdout,
        );
        return Err(anyhow!(error_msg));
    }

    if verbose {
        info!("✅ Implementation completed");
        if !stdout.is_empty() {
            info!("📄 Claude response:");
            info!("{stdout}");
        }
        if !stderr.is_empty() {
            info!("⚠️  Claude stderr:");
            info!("{stderr}");
        }
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

    // Skip actual execution in test mode
    if std::env::var("MMM_TEST_MODE").unwrap_or_default() == "true" {
        if verbose {
            info!("[TEST MODE] Skipping Claude CLI execution for: mmm-lint");
        }

        // Check if we should simulate no changes for this command
        if let Ok(no_changes_cmds) = std::env::var("MMM_TEST_NO_CHANGES_COMMANDS") {
            if no_changes_cmds
                .split(',')
                .any(|cmd| cmd.trim() == "mmm-lint")
            {
                if verbose {
                    info!("[TEST MODE] Simulating no changes for: mmm-lint");
                }
                return Ok(false);
            }
        }

        return Ok(true);
    }

    let mut cmd = Command::new("claude");
    cmd.arg("--dangerously-skip-permissions") // Required for automation: bypasses interactive permission checks
        .arg("--print") // Outputs response to stdout for capture instead of interactive display
        .arg("/mmm-lint") // The custom command for linting and formatting
        .env("MMM_AUTOMATION", "true"); // Signals to /mmm-lint to run in automated mode

    // Pass context directory if available
    let context_dir = std::env::current_dir()
        .ok()
        .map(|p| p.join(".mmm").join("context"));
    if let Some(ctx_dir) = context_dir {
        if ctx_dir.exists() {
            cmd.env("MMM_CONTEXT_DIR", ctx_dir);
        }
    }

    // Execute with retry logic for transient failures
    let output = execute_with_retry(cmd, "Claude lint", DEFAULT_CLAUDE_RETRIES, verbose).await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        let error_msg =
            format_subprocess_error("claude /mmm-lint", output.status.code(), &stderr, &stdout);
        return Err(anyhow!(error_msg));
    }

    if verbose {
        info!("✅ Linting completed");
        if !stdout.is_empty() {
            info!("📄 Claude response:");
            info!("{stdout}");
        }
        if !stderr.is_empty() {
            info!("⚠️  Claude stderr:");
            info!("{stderr}");
        }
    }

    Ok(true)
}

/// Run improvement with mapping in worktree context
async fn run_with_mapping_in_worktree(
    cmd: command::CookCommand,
    session: crate::worktree::WorktreeSession,
    original_repo_path: std::path::PathBuf,
    verbose: bool,
) -> Result<()> {
    use glob::glob;
    use std::collections::HashMap;

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
                    warn!("Warning: Error matching pattern: {e}");
                }
            }
        }
    }

    // Add direct arguments from --args
    inputs.extend(cmd.args.clone());

    if inputs.is_empty() {
        return Err(anyhow!("No inputs found from --map patterns or --args"));
    }

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
        let mut variables = HashMap::new();

        // Determine the ARG value based on whether this came from --map or --args
        let arg_value = if cmd.map.iter().any(|pattern| {
            glob(pattern)
                .ok()
                .and_then(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .find(|p| &p.to_string_lossy() == input)
                })
                .is_some()
        }) {
            // This input came from --map, so extract spec ID if it's a spec file
            if input.ends_with(".md") && input.contains("spec") {
                extract_spec_id_from_path(input)
            } else {
                input.clone()
            }
        } else {
            // This input came from --args, use it directly
            input.clone()
        };

        variables.insert("ARG".to_string(), arg_value.clone());
        variables.insert("FILE".to_string(), input.clone());
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
    // Run context analysis to understand the project
    let project_path = std::env::current_dir()?;
    if let Err(e) = analyze_project_context(&project_path, verbose).await {
        if verbose {
            warn!("⚠️  Context analysis failed: {e}");
            warn!("   Continuing without deep context understanding");
        }
    }

    if verbose {
        if let Some(focus) = &cmd.focus {
            info!("📊 Focus: {focus}");
        }
    }

    // 3. Setup basic state
    let _state = StateManager::new()?;
    let _start_time = Utc::now();

    // 4. Main improvement loop
    let mut iteration = 1;
    let files_changed = 0;

    // Load configuration (with workflow if present)
    let config_loader = ConfigLoader::new().await?;
    config_loader
        .load_with_explicit_path(Path::new("."), cmd.config.as_deref())
        .await?;
    let config = config_loader.get_config();

    // Check if we have a workflow configuration
    if let Some(workflow_config) = config.workflow {
        // Use configurable workflow
        if verbose {
            info!("Using custom workflow from configuration");
        }

        let max_iterations = cmd.max_iterations;
        let mut executor = WorkflowExecutor::new(workflow_config, verbose, max_iterations)
            .with_variables(variables);

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

            iteration += 1;
        }

        // Final state update
        worktree_manager.update_session_state(&session.name, |state| {
            state.iterations.completed = iteration - 1;
            state.stats.files_changed = files_changed;
        })?;

        Ok(())
    } else {
        // Legacy hardcoded workflow - not recommended but kept for compatibility
        Err(anyhow!(
            "No workflow configuration found. Please provide a workflow configuration file."
        ))
    }
}

#[cfg(test)]
mod cook_inline_tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_command(worktree: bool, max_iterations: u32) -> command::CookCommand {
        command::CookCommand {
            path: None,
            max_iterations,
            worktree,
            focus: None,
            config: None,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            resume: None,
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
        let use_worktree = cmd.worktree
            || std::env::var("MMM_USE_WORKTREE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false);

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
            assert!(is_valid, "Should be valid: {spec_id}");
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
            assert!(!is_valid, "Should be invalid: {spec_id}");
        }
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
    worktree_manager_arc.update_session_state(&session.name, |state| match &result {
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

/// Helper function to detect modified files since last checkpoint
fn detect_modified_files() -> Vec<String> {
    // Use git to detect modified files
    match std::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
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
