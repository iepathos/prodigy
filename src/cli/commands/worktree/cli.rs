//! Worktree command implementation
//!
//! This module handles git worktree management for parallel sessions.

use crate::cli::args::WorktreeCommands;
use anyhow::Result;

use super::mapreduce_cleanup::run_mapreduce_cleanup;
use super::operations::{
    list_sessions_operation, merge_all_sessions_operation, merge_session_operation,
};
use super::orphaned_cleanup::run_worktree_clean_orphaned;
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

    // Display results
    if result.sessions.is_empty() {
        println!("No active Prodigy worktrees found.");
    } else {
        println!("Active Prodigy worktrees:");
        println!("{:<40} {:<30} {:<20}", "Name", "Branch", "Created");
        println!("{}", "-".repeat(90));

        for session in result.sessions {
            println!(
                "{:<40} {:<30} {:<20}",
                session.name,
                session.branch,
                session.created_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
    }

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

        // Display results
        for merge_result in &result.results {
            if merge_result.success {
                println!(
                    "✅ Successfully merged worktree '{}'",
                    merge_result.session_name
                );
            } else {
                eprintln!(
                    "❌ Failed to merge worktree '{}': {}",
                    merge_result.session_name,
                    merge_result
                        .error
                        .as_ref()
                        .unwrap_or(&"Unknown error".to_string())
                );
            }
        }

        if result.merged_count > 0 {
            println!("Successfully merged {} worktree(s)", result.merged_count);
        }
        Ok(())
    } else if let Some(name) = name {
        println!("Merging worktree '{}'...", name);
        let result = merge_session_operation(&manager, &name).await;

        if result.success {
            println!("✅ Successfully merged worktree '{}'", name);
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to merge worktree '{}': {}",
                name,
                result.error.unwrap_or_else(|| "Unknown error".to_string())
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

/// Clean up old worktrees
async fn cleanup_old_worktrees(
    manager: &crate::worktree::manager::WorktreeManager,
    max_age: std::time::Duration,
    force: bool,
    dry_run: bool,
) -> Result<()> {
    let sessions = manager.list_sessions().await?;
    let now = chrono::Utc::now();
    let mut cleaned = 0;

    for session in sessions {
        let age = now.signed_duration_since(session.created_at);
        if age.num_seconds() as u64 > max_age.as_secs() {
            if dry_run {
                println!(
                    "DRY RUN: Would remove worktree '{}' (age: {} hours)",
                    session.name,
                    age.num_hours()
                );
            } else {
                println!(
                    "Removing old worktree '{}' (age: {} hours)",
                    session.name,
                    age.num_hours()
                );
                manager.cleanup_session(&session.name, force).await?;
                cleaned += 1;
            }
        }
    }

    if !dry_run {
        println!("Cleaned {} old worktrees", cleaned);
    }

    Ok(())
}


