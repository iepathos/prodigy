//! Worktree command implementation
//!
//! This module handles git worktree management for parallel sessions.

use crate::cli::args::WorktreeCommands;
use anyhow::Result;

use super::age_cleanup::cleanup_old_worktrees;
use super::mapreduce_cleanup::run_mapreduce_cleanup;
use super::operations::{
    list_sessions_operation, merge_all_sessions_operation, merge_session_operation,
};
use super::orphaned_cleanup::run_worktree_clean_orphaned;
use super::presentation::{
    format_batch_merge_summary, format_merge_result, format_sessions_table,
};
use super::utils::parse_duration;

/// Execute worktree-related commands
pub async fn run_worktree_command(command: WorktreeCommands) -> Result<()> {
    match command {
        WorktreeCommands::Ls { json, detailed } => run_worktree_ls(json, detailed).await,
        WorktreeCommands::Merge { name, all } => run_worktree_merge(name, all).await,
        WorktreeCommands::Clean {
            all,
            name,
            force,
            merged_only,
            mapreduce,
            older_than,
            dry_run,
            job_id,
        } => {
            run_worktree_clean(
                all,
                name,
                force,
                merged_only,
                mapreduce,
                older_than,
                dry_run,
                job_id,
            )
            .await
        }
        WorktreeCommands::CleanOrphaned {
            job_id,
            dry_run,
            force,
        } => run_worktree_clean_orphaned(job_id, dry_run, force).await,
    }
}

/// List active worktrees
async fn run_worktree_ls(_json: bool, _detailed: bool) -> Result<()> {
    use crate::subprocess::SubprocessManager;
    use crate::worktree::manager::WorktreeManager;

    // Initialize dependencies
    let repo_path = std::env::current_dir()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(repo_path, subprocess)?;

    // Execute operation
    let result = list_sessions_operation(&manager).await?;

    // Display results using presentation layer
    let output = format_sessions_table(&result.sessions);
    print!("{}", output);

    Ok(())
}

/// Merge worktree changes
async fn run_worktree_merge(name: Option<String>, all: bool) -> Result<()> {
    use crate::subprocess::SubprocessManager;
    use crate::worktree::manager::WorktreeManager;

    // Initialize dependencies
    let repo_path = std::env::current_dir()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(repo_path, subprocess)?;

    if all {
        // Merge all active worktrees
        println!("Merging all worktrees...");
        let result = merge_all_sessions_operation(&manager).await?;

        // Display results using presentation layer
        for merge_result in &result.results {
            let message = format_merge_result(merge_result);
            if merge_result.success {
                println!("{}", message);
            } else {
                eprintln!("{}", message);
            }
        }

        let summary = format_batch_merge_summary(&result);
        if !summary.is_empty() {
            println!("{}", summary);
        }
        Ok(())
    } else if let Some(name) = name {
        println!("Merging worktree '{}'...", name);
        let result = merge_session_operation(&manager, &name).await;

        if result.success {
            println!("{}", format_merge_result(&result));
            Ok(())
        } else {
            let error_msg = result
                .error
                .unwrap_or_else(|| "Unknown error".to_string());
            Err(anyhow::anyhow!(
                "Failed to merge worktree '{}': {}",
                name,
                error_msg
            ))
        }
    } else {
        println!("Please specify a worktree name or use --all");
        Ok(())
    }
}

/// Clean up worktrees
#[allow(clippy::too_many_arguments)]
async fn run_worktree_clean(
    all: bool,
    name: Option<String>,
    force: bool,
    _merged_only: bool,
    mapreduce: bool,
    older_than: Option<String>,
    dry_run: bool,
    job_id: Option<String>,
) -> Result<()> {
    use crate::subprocess::SubprocessManager;
    use crate::worktree::manager::WorktreeManager;

    // Handle MapReduce-specific cleanup
    if mapreduce {
        return run_mapreduce_cleanup(job_id, older_than, dry_run, force).await;
    }

    // Get current repository path
    let repo_path = std::env::current_dir()?;
    let subprocess = SubprocessManager::production();

    let manager = WorktreeManager::new(repo_path, subprocess)?;

    // Handle older_than option
    if let Some(duration_str) = older_than {
        let duration = parse_duration(&duration_str)?;
        return cleanup_old_worktrees(&manager, duration, force, dry_run).await;
    }

    if all {
        // Clean all inactive worktrees
        if dry_run {
            println!("DRY RUN: Would clean all worktrees");
            let sessions = manager.list_sessions().await?;
            for session in sessions {
                println!("  - Would remove: {}", session.name);
            }
        } else {
            println!(
                "Cleaning all worktrees{}",
                if force { " (forced)" } else { "" }
            );
            manager.cleanup_all_sessions(force).await?;
            println!("All worktrees cleaned successfully");
        }
    } else if let Some(name) = name {
        if dry_run {
            println!("DRY RUN: Would remove worktree: {}", name);
        } else {
            println!("Removing worktree: {}", name);
            manager.cleanup_session(&name, force).await?;
            println!("Worktree '{}' removed successfully", name);
        }
    } else {
        println!("No worktrees specified for cleanup. Use --all or specify a worktree name.");
    }

    Ok(())
}



