//! Orphaned worktree cleanup logic
//!
//! This module handles cleanup of orphaned worktrees that failed during cleanup,
//! separating pure logic from I/O operations.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::cook::execution::mapreduce::coordination::executor::OrphanedWorktree;

/// Pure function to resolve registry base path
///
/// Constructs the registry path for orphaned worktrees.
pub fn resolve_registry_base_path(home_dir: &Path, repo_name: &str) -> PathBuf {
    home_dir
        .join(".prodigy")
        .join("orphaned_worktrees")
        .join(repo_name)
}

/// Pure function to find registry files
///
/// Reads directory and filters for JSON files.
pub fn find_registry_files(registry_path: &Path) -> Result<Vec<PathBuf>> {
    if !registry_path.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(registry_path)
        .context("Failed to read orphaned worktrees registry directory")?;

    let files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();

    Ok(files)
}

/// Pure function to format orphaned worktree display
///
/// Creates a display string for an orphaned worktree entry.
pub fn format_orphaned_worktree(orphaned: &OrphanedWorktree) -> String {
    format!(
        "  - {} (agent: {}, item: {}, error: {})",
        orphaned.path.display(),
        orphaned.agent_id,
        orphaned.item_id,
        orphaned.error
    )
}

/// Clean orphaned worktrees from cleanup failures
///
/// This is the main orchestrator for cleaning orphaned worktrees.
/// It handles registry file discovery, user confirmation, and cleanup.
pub async fn run_worktree_clean_orphaned(
    job_id: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<()> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;
    let repo_path = std::env::current_dir()?;
    let repo_name = repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Resolve registry path
    let registry_path = resolve_registry_base_path(&home_dir, repo_name);

    if !registry_path.exists() {
        println!("No orphaned worktrees registry found.");
        return Ok(());
    }

    // Find registry file(s)
    let registry_file = find_target_registry_file(&registry_path, job_id, force)?;

    if !registry_file.exists() {
        println!("No orphaned worktrees found for the specified job.");
        return Ok(());
    }

    // Load orphaned worktrees
    let orphaned_worktrees = load_orphaned_worktrees(&registry_file)?;

    if orphaned_worktrees.is_empty() {
        println!("No orphaned worktrees in registry.");
        return Ok(());
    }

    // Display orphaned worktrees
    display_orphaned_worktrees(&orphaned_worktrees);

    // Handle dry run
    if dry_run {
        println!(
            "\nDry run: would clean {} worktree(s)",
            orphaned_worktrees.len()
        );
        return Ok(());
    }

    // Confirm cleanup
    if !force && !confirm_cleanup()? {
        println!("Cleanup cancelled.");
        return Ok(());
    }

    // Perform cleanup
    let (cleaned, failed) = cleanup_orphaned_worktrees(&orphaned_worktrees).await;

    // Remove registry file if all cleaned successfully
    if failed == 0 {
        std::fs::remove_file(&registry_file)?;
        println!("\n✅ Cleaned {} orphaned worktree(s)", cleaned);
    } else {
        println!("\n⚠️  Cleaned {} worktree(s), {} failed", cleaned, failed);
    }

    Ok(())
}

/// Find the target registry file based on job_id and force flag
fn find_target_registry_file(
    registry_path: &Path,
    job_id: Option<String>,
    force: bool,
) -> Result<PathBuf> {
    if let Some(ref jid) = job_id {
        // Specific job registry
        Ok(registry_path.join(format!("{}.json", jid)))
    } else {
        // Find all registry files
        let mut files = find_registry_files(registry_path)?;

        if files.is_empty() {
            println!("No orphaned worktrees found.");
            return Ok(PathBuf::new());
        }

        if files.len() > 1 && !force {
            println!("Multiple job registries found. Please specify a job_id:");
            for file in &files {
                if let Some(name) = file.file_stem().and_then(|s| s.to_str()) {
                    println!("  - {}", name);
                }
            }
            return Ok(PathBuf::new());
        }

        Ok(files.remove(0))
    }
}

/// Load orphaned worktrees from registry file
fn load_orphaned_worktrees(registry_file: &Path) -> Result<Vec<OrphanedWorktree>> {
    let content = std::fs::read_to_string(registry_file)
        .context("Failed to read orphaned worktrees registry")?;
    let orphaned_worktrees: Vec<OrphanedWorktree> =
        serde_json::from_str(&content).context("Failed to parse orphaned worktrees registry")?;
    Ok(orphaned_worktrees)
}

/// Display orphaned worktrees
fn display_orphaned_worktrees(orphaned_worktrees: &[OrphanedWorktree]) {
    println!("Found {} orphaned worktree(s):", orphaned_worktrees.len());
    for orphaned in orphaned_worktrees {
        println!("{}", format_orphaned_worktree(orphaned));
    }
}

/// Confirm cleanup with user
fn confirm_cleanup() -> Result<bool> {
    println!("\nProceed with cleanup? [y/N]");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Failed to read user input")?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// Clean orphaned worktrees
async fn cleanup_orphaned_worktrees(orphaned_worktrees: &[OrphanedWorktree]) -> (usize, usize) {
    let mut cleaned = 0;
    let mut failed = 0;

    for orphaned in orphaned_worktrees {
        if orphaned.path.exists() {
            match std::fs::remove_dir_all(&orphaned.path) {
                Ok(_) => {
                    println!("✅ Cleaned: {}", orphaned.path.display());
                    cleaned += 1;
                }
                Err(e) => {
                    eprintln!("❌ Failed to clean {}: {}", orphaned.path.display(), e);
                    failed += 1;
                }
            }
        } else {
            println!("⚠️  Already removed: {}", orphaned.path.display());
            cleaned += 1;
        }
    }

    (cleaned, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_registry_base_path() {
        let home = PathBuf::from("/home/user");
        let path = resolve_registry_base_path(&home, "myrepo");
        assert_eq!(
            path,
            PathBuf::from("/home/user/.prodigy/orphaned_worktrees/myrepo")
        );
    }

    #[test]
    fn test_format_orphaned_worktree() {
        let orphaned = OrphanedWorktree {
            path: PathBuf::from("/tmp/worktree"),
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            failed_at: chrono::Utc::now(),
            error: "permission denied".to_string(),
        };

        let formatted = format_orphaned_worktree(&orphaned);
        assert!(formatted.contains("agent-1"));
        assert!(formatted.contains("item-1"));
        assert!(formatted.contains("permission denied"));
    }

    #[test]
    fn test_find_registry_files_empty_dir() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("test_registry_empty");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let files = find_registry_files(&temp_dir).unwrap();
        assert_eq!(files.len(), 0);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_find_registry_files_nonexistent() {
        let nonexistent = PathBuf::from("/nonexistent/path/registry");
        let files = find_registry_files(&nonexistent).unwrap();
        assert_eq!(files.len(), 0);
    }
}
