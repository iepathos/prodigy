//! Worktree command implementation
//!
//! This module handles git worktree management for parallel sessions.

use crate::cli::args::WorktreeCommands;
use anyhow::Result;

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
    }
}

/// List active worktrees
async fn run_worktree_ls(_json: bool, _detailed: bool) -> Result<()> {
    use crate::subprocess::SubprocessManager;
    use crate::worktree::manager::WorktreeManager;

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

/// Parse duration string (e.g., "1h", "24h", "7d")
fn parse_duration(s: &str) -> Result<std::time::Duration> {
    use std::time::Duration;

    let s = s.trim().to_lowercase();
    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
    } else if s.ends_with('s') {
        (&s[..s.len() - 1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], "h")
    } else if s.ends_with('d') {
        (&s[..s.len() - 1], "d")
    } else {
        return Err(anyhow::anyhow!(
            "Invalid duration format. Use format like '1h', '24h', '7d'"
        ));
    };

    let num: u64 = num_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid number in duration"))?;

    Ok(match unit {
        "ms" => Duration::from_millis(num),
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        "d" => Duration::from_secs(num * 86400),
        _ => unreachable!(),
    })
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

/// Run MapReduce-specific cleanup
async fn run_mapreduce_cleanup(
    job_id: Option<String>,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    use crate::cook::execution::mapreduce::cleanup::{
        WorktreeCleanupConfig, WorktreeCleanupCoordinator,
    };
    use std::path::PathBuf;

    // Get worktree base path
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let repo_name = std::env::current_dir()?
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let worktree_base_path = PathBuf::from(home)
        .join(".prodigy")
        .join("worktrees")
        .join(&repo_name);

    // Create cleanup coordinator
    let config = if force {
        WorktreeCleanupConfig::aggressive()
    } else {
        WorktreeCleanupConfig::default()
    };
    let coordinator = WorktreeCleanupCoordinator::new(config, worktree_base_path.clone());

    if let Some(job_id) = job_id {
        // Clean specific job
        if dry_run {
            println!("DRY RUN: Would clean all worktrees for job {}", job_id);
        } else {
            println!("Cleaning worktrees for job {}...", job_id);
            let count = coordinator.cleanup_job(&job_id).await?;
            println!("Cleaned {} worktrees for job {}", count, job_id);
        }
    } else if let Some(duration_str) = older_than {
        // Clean old MapReduce worktrees
        let duration = parse_duration(&duration_str)?;
        if dry_run {
            println!(
                "DRY RUN: Would clean MapReduce worktrees older than {}",
                duration_str
            );
        } else {
            println!(
                "Cleaning MapReduce worktrees older than {}...",
                duration_str
            );
            let count = coordinator.cleanup_orphaned_worktrees(duration).await?;
            println!("Cleaned {} orphaned MapReduce worktrees", count);
        }
    } else {
        // Clean all orphaned MapReduce worktrees
        if dry_run {
            println!("DRY RUN: Would clean all orphaned MapReduce worktrees");
        } else {
            println!("Cleaning all orphaned MapReduce worktrees...");
            // Default to 1 hour old
            let count = coordinator
                .cleanup_orphaned_worktrees(std::time::Duration::from_secs(3600))
                .await?;
            println!("Cleaned {} orphaned MapReduce worktrees", count);
        }
    }

    Ok(())
}
