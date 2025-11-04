//! MapReduce-specific worktree cleanup logic
//!
//! This module handles cleanup of MapReduce agent worktrees,
//! separating pure logic from I/O operations.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;

use crate::cook::execution::mapreduce::cleanup::{
    WorktreeCleanupConfig, WorktreeCleanupCoordinator,
};

/// Pure function to resolve worktree base path
///
/// Constructs the worktree base path from home directory and repository name.
/// This is a pure function that can be tested without I/O.
pub fn resolve_worktree_base_path(home_dir: &str, repo_name: &str) -> PathBuf {
    PathBuf::from(home_dir)
        .join(".prodigy")
        .join("worktrees")
        .join(repo_name)
}

/// Pure function to build cleanup configuration
///
/// Creates appropriate cleanup config based on force flag.
pub fn build_cleanup_config(force: bool) -> WorktreeCleanupConfig {
    if force {
        WorktreeCleanupConfig::aggressive()
    } else {
        WorktreeCleanupConfig::default()
    }
}

/// Get home directory with fallback
fn get_home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
}

/// Get repository name from current directory
fn get_repo_name() -> Result<String> {
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let repo_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(repo_name)
}

/// Run MapReduce-specific cleanup operation
///
/// This is the main orchestrator that coordinates cleanup operations.
/// It handles three scenarios:
/// 1. Clean specific job by ID
/// 2. Clean worktrees older than specified duration
/// 3. Clean all orphaned worktrees (default 1 hour)
pub async fn run_mapreduce_cleanup(
    job_id: Option<String>,
    older_than: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    // Resolve paths
    let home_dir = get_home_dir();
    let repo_name = get_repo_name()?;
    let worktree_base_path = resolve_worktree_base_path(&home_dir, &repo_name);

    // Build configuration
    let config = build_cleanup_config(force);
    let coordinator = WorktreeCleanupCoordinator::new(config, worktree_base_path);

    // Route to appropriate cleanup operation
    if let Some(job_id) = job_id {
        cleanup_job(&coordinator, &job_id, dry_run).await
    } else if let Some(duration_str) = older_than {
        cleanup_by_age(&coordinator, &duration_str, dry_run).await
    } else {
        cleanup_all_orphaned(&coordinator, dry_run).await
    }
}

/// Clean worktrees for a specific job
async fn cleanup_job(
    coordinator: &WorktreeCleanupCoordinator,
    job_id: &str,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("DRY RUN: Would clean all worktrees for job {}", job_id);
    } else {
        println!("Cleaning worktrees for job {}...", job_id);
        let count = coordinator.cleanup_job(job_id).await?;
        println!("Cleaned {} worktrees for job {}", count, job_id);
    }
    Ok(())
}

/// Clean worktrees older than specified duration
async fn cleanup_by_age(
    coordinator: &WorktreeCleanupCoordinator,
    duration_str: &str,
    dry_run: bool,
) -> Result<()> {
    use super::utils::parse_duration;

    let duration = parse_duration(duration_str)?;
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
    Ok(())
}

/// Clean all orphaned worktrees (default: 1 hour old)
async fn cleanup_all_orphaned(
    coordinator: &WorktreeCleanupCoordinator,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("DRY RUN: Would clean all orphaned MapReduce worktrees");
    } else {
        println!("Cleaning all orphaned MapReduce worktrees...");
        let count = coordinator
            .cleanup_orphaned_worktrees(Duration::from_secs(3600))
            .await?;
        println!("Cleaned {} orphaned MapReduce worktrees", count);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_worktree_base_path() {
        let path = resolve_worktree_base_path("/home/user", "myrepo");
        assert_eq!(path, PathBuf::from("/home/user/.prodigy/worktrees/myrepo"));
    }

    #[test]
    fn test_build_cleanup_config_default() {
        let config = build_cleanup_config(false);
        // Config should be default (not aggressive)
        assert_eq!(config.max_worktrees_per_job, 50);
        assert_eq!(config.cleanup_delay_secs, 30);
    }

    #[test]
    fn test_build_cleanup_config_aggressive() {
        let config = build_cleanup_config(true);
        // Config should be aggressive
        assert_eq!(config.max_worktrees_per_job, 20);
        assert_eq!(config.cleanup_delay_secs, 5);
    }
}
