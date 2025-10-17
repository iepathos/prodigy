//! Clean command implementation for managing Prodigy storage
//!
//! Provides comprehensive cleanup capabilities for all Prodigy storage types.

use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;

use crate::cli::args::CleanCommands;
use crate::storage::{
    extract_repo_name, parse_duration, CleanupConfig, GlobalStorage, StorageCleanupManager,
    StorageStats,
};

/// Execute the clean command
pub async fn execute(command: CleanCommands, repo_path: &Path) -> Result<()> {
    let storage = GlobalStorage::new()?;
    let repo_name = extract_repo_name(repo_path)?;

    match command {
        CleanCommands::All {
            older_than,
            all_repos,
            dry_run,
            force,
        } => {
            if all_repos {
                clean_all_repos(&storage, older_than, dry_run, force).await
            } else {
                clean_all_storage(&storage, &repo_name, older_than, dry_run, force).await
            }
        }
        CleanCommands::Worktrees {
            older_than,
            mapreduce,
            dry_run,
            force,
        } => clean_worktrees(&storage, &repo_name, older_than, mapreduce, dry_run, force).await,
        CleanCommands::Sessions {
            older_than,
            dry_run,
            force,
        } => clean_sessions(&storage, &repo_name, older_than, dry_run, force).await,
        CleanCommands::Logs {
            older_than,
            dry_run,
            force,
        } => clean_logs(&storage, &repo_name, older_than, dry_run, force).await,
        CleanCommands::State {
            older_than,
            dry_run,
            force,
        } => clean_state(&storage, &repo_name, older_than, dry_run, force).await,
        CleanCommands::Events {
            older_than,
            dry_run,
            force,
        } => clean_events(&storage, &repo_name, older_than, dry_run, force).await,
        CleanCommands::Dlq {
            older_than,
            dry_run,
            force,
        } => clean_dlq(&storage, &repo_name, older_than, dry_run, force).await,
    }
}

/// Clean all storage for a single repository
async fn clean_all_storage(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    println!("Analyzing storage for repository: {}", repo_name);
    println!();

    // Show storage statistics before cleanup
    let stats_before = manager.get_storage_stats().await?;
    print_storage_stats(&stats_before, "Before cleanup");

    if !force && !dry_run && !confirm_cleanup("all storage types")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    // Perform cleanup
    let results = manager.clean_all(&config).await?;

    // Show results
    println!();
    if dry_run {
        println!("=== Dry Run Results ===");
    } else {
        println!("=== Cleanup Results ===");
    }
    println!();

    let mut total_removed = 0;
    let mut total_bytes = 0u64;
    let mut all_errors = Vec::new();

    for (storage_type, stats) in &results {
        println!(
            "{}: {} items removed, {} reclaimed",
            storage_type,
            stats.items_removed,
            StorageStats::format_bytes(stats.bytes_reclaimed)
        );
        total_removed += stats.items_removed;
        total_bytes += stats.bytes_reclaimed;
        all_errors.extend(stats.errors.clone());
    }

    println!();
    println!(
        "Total: {} items, {}",
        total_removed,
        StorageStats::format_bytes(total_bytes)
    );

    if !all_errors.is_empty() {
        println!();
        println!("Errors encountered:");
        for error in &all_errors {
            println!("  - {}", error);
        }
    }

    Ok(())
}

/// Clean all repositories
async fn clean_all_repos(
    storage: &GlobalStorage,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let repos = storage.list_all_repositories().await?;

    if repos.is_empty() {
        println!("No repositories found with storage data");
        return Ok(());
    }

    println!("Found {} repositories with storage data", repos.len());
    println!();

    if !force && !dry_run
        && !confirm_cleanup(&format!("all storage for {} repositories", repos.len()))?
    {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let mut total_removed = 0;
    let mut total_bytes = 0u64;

    for repo_name in &repos {
        println!("Cleaning repository: {}", repo_name);
        let manager = StorageCleanupManager::new(storage.clone(), repo_name.clone());

        let results = manager.clean_all(&config).await?;

        for stats in results.values() {
            total_removed += stats.items_removed;
            total_bytes += stats.bytes_reclaimed;
        }

        println!();
    }

    println!("=== Total Results ===");
    println!(
        "Removed {} items, reclaimed {}",
        total_removed,
        StorageStats::format_bytes(total_bytes)
    );

    Ok(())
}

/// Clean worktrees
async fn clean_worktrees(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    _mapreduce: bool,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    if !force && !dry_run && !confirm_cleanup("worktrees")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let stats = manager.clean_worktrees(&config).await?;
    print_cleanup_stats("Worktrees", &stats, dry_run);

    Ok(())
}

/// Clean sessions
async fn clean_sessions(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    if !force && !dry_run && !confirm_cleanup("session state")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let stats = manager.clean_sessions(&config).await?;
    print_cleanup_stats("Sessions", &stats, dry_run);

    Ok(())
}

/// Clean logs
async fn clean_logs(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    if !force && !dry_run && !confirm_cleanup("Claude execution logs")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let stats = manager.clean_logs(&config).await?;
    print_cleanup_stats("Logs", &stats, dry_run);

    Ok(())
}

/// Clean state
async fn clean_state(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    if !force && !dry_run && !confirm_cleanup("MapReduce job state")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let stats = manager.clean_state(&config).await?;
    print_cleanup_stats("State", &stats, dry_run);

    Ok(())
}

/// Clean events
async fn clean_events(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    if !force && !dry_run && !confirm_cleanup("event logs")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let stats = manager.clean_events(&config).await?;
    print_cleanup_stats("Events", &stats, dry_run);

    Ok(())
}

/// Clean DLQ data
async fn clean_dlq(
    storage: &GlobalStorage,
    repo_name: &str,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let config = create_config(older_than, dry_run, force)?;
    let manager = StorageCleanupManager::new(storage.clone(), repo_name.to_string());

    if !force && !dry_run && !confirm_cleanup("Dead Letter Queue data")? {
        println!("Cleanup cancelled");
        return Ok(());
    }

    let stats = manager.clean_dlq(&config).await?;
    print_cleanup_stats("DLQ", &stats, dry_run);

    Ok(())
}

/// Create cleanup config from CLI options
fn create_config(older_than: Option<String>, dry_run: bool, force: bool) -> Result<CleanupConfig> {
    let duration = if let Some(duration_str) = older_than {
        Some(parse_duration(&duration_str).context("Invalid duration format")?)
    } else {
        None
    };

    Ok(CleanupConfig {
        older_than: duration,
        dry_run,
        force,
    })
}

/// Print storage statistics
fn print_storage_stats(stats: &StorageStats, title: &str) {
    println!("=== {} ===", title);
    println!(
        "Worktrees: {}",
        StorageStats::format_bytes(stats.worktrees_bytes)
    );
    println!(
        "Sessions:  {}",
        StorageStats::format_bytes(stats.sessions_bytes)
    );
    println!(
        "Logs:      {}",
        StorageStats::format_bytes(stats.logs_bytes)
    );
    println!(
        "State:     {}",
        StorageStats::format_bytes(stats.state_bytes)
    );
    println!(
        "Events:    {}",
        StorageStats::format_bytes(stats.events_bytes)
    );
    println!("DLQ:       {}", StorageStats::format_bytes(stats.dlq_bytes));
    println!(
        "Total:     {}",
        StorageStats::format_bytes(stats.total_bytes)
    );
    println!();
}

/// Print cleanup statistics
fn print_cleanup_stats(storage_type: &str, stats: &crate::storage::CleanupStats, dry_run: bool) {
    println!();
    if dry_run {
        println!("=== Dry Run Results: {} ===", storage_type);
        println!("Would remove {} items", stats.items_removed);
    } else {
        println!("=== Cleanup Results: {} ===", storage_type);
        println!("Removed {} items", stats.items_removed);
    }
    println!(
        "Space reclaimed: {}",
        StorageStats::format_bytes(stats.bytes_reclaimed)
    );

    if !stats.errors.is_empty() {
        println!();
        println!("Errors encountered:");
        for error in &stats.errors {
            println!("  - {}", error);
        }
    }
}

/// Confirm cleanup operation with user
fn confirm_cleanup(storage_type: &str) -> Result<bool> {
    print!("Clean {}? [y/N]: ", storage_type);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_lowercase() == "y")
}
