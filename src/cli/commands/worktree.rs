//! Worktree command implementation
//!
//! This module handles git worktree management for parallel sessions.

use crate::cli::args::WorktreeCommands;
use anyhow::Result;

/// Execute worktree-related commands
pub async fn run_worktree_command(command: WorktreeCommands) -> Result<()> {
    match command {
        WorktreeCommands::Ls { json, detailed } => {
            run_worktree_ls(json, detailed).await
        }
        WorktreeCommands::Merge { name, all } => {
            run_worktree_merge(name, all).await
        }
        WorktreeCommands::Clean { all, name, force, merged_only } => {
            run_worktree_clean(all, name, force, merged_only).await
        }
    }
}

/// List active worktrees
async fn run_worktree_ls(_json: bool, _detailed: bool) -> Result<()> {
    use crate::worktree::manager::WorktreeManager;
    use crate::subprocess::SubprocessManager;

    // Get current repository path
    let repo_path = std::env::current_dir()?;
    let subprocess = SubprocessManager::production();

    let manager = WorktreeManager::new(repo_path, subprocess)?;
    let sessions = manager.list_sessions().await?;

    if sessions.is_empty() {
        println!("No active Prodigy worktrees found.");
    } else {
        // Output as table
        println!("Active Prodigy worktrees:");
        println!("{:<40} {:<30} {:<20}", "Name", "Branch", "Created");
        println!("{}", "-".repeat(90));

        for session in sessions {
            println!("{:<40} {:<30} {:<20}",
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
    if all {
        println!("Merging all worktrees...");
        Ok(())
    } else if let Some(name) = name {
        println!("Merging worktree '{}'...", name);
        Ok(())
    } else {
        println!("Please specify a worktree name or use --all");
        Ok(())
    }
}

/// Clean up worktrees
async fn run_worktree_clean(all: bool, name: Option<String>, _force: bool, _merged_only: bool) -> Result<()> {
    use crate::worktree::manager::WorktreeManager;
    use crate::subprocess::SubprocessManager;

    // Get current repository path
    let repo_path = std::env::current_dir()?;
    let subprocess = SubprocessManager::production();

    let manager = WorktreeManager::new(repo_path, subprocess)?;

    if all {
        // Clean all inactive worktrees
        let sessions = manager.list_sessions().await?;

        if sessions.is_empty() {
            println!("No worktrees to clean");
        } else {
            for session in sessions {
                println!("Would remove: {}", session.name);
                // TODO: Actually remove the worktree
            }
        }
    } else if let Some(name) = name {
        println!("Removing worktree: {}", name);
        // TODO: Remove specific worktree
    } else {
        println!("No worktrees specified for cleanup. Use --all or specify a worktree name.");
    }

    Ok(())
}