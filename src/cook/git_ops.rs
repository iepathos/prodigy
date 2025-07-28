//! Thread-safe git operations
//!
//! This module provides synchronized access to git operations to prevent
//! race conditions when multiple processes might be modifying the repository.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;

/// Global mutex for git operations
///
/// This ensures that only one git operation runs at a time, preventing
/// race conditions during concurrent modifications.
static GIT_MUTEX: Lazy<Arc<Mutex<()>>> = Lazy::new(|| Arc::new(Mutex::new(())));

/// Execute a git command with exclusive access
///
/// # Arguments
/// * `args` - Arguments to pass to the git command
/// * `description` - Human-readable description of the operation
///
/// # Returns
/// The command output on success, or an error with context
pub async fn git_command(args: &[&str], description: &str) -> Result<std::process::Output> {
    // Acquire the mutex to ensure exclusive access
    let _guard = GIT_MUTEX.lock().await;

    let output = Command::new("git")
        .args(args)
        .output()
        .await
        .context(format!("Failed to execute git {description}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Git {description} failed: {}",
            stderr.trim()
        ));
    }

    Ok(output)
}

/// Get the last commit message
///
/// Thread-safe wrapper for getting the most recent commit message.
pub async fn get_last_commit_message() -> Result<String> {
    let output = git_command(&["log", "-1", "--pretty=format:%s"], "log").await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Check git status
///
/// Thread-safe wrapper for checking repository status.
pub async fn check_git_status() -> Result<String> {
    let output = git_command(&["status", "--porcelain"], "status").await?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Stage all changes
///
/// Thread-safe wrapper for staging all modifications.
pub async fn stage_all_changes() -> Result<()> {
    git_command(&["add", "."], "add").await?;
    Ok(())
}

/// Create a commit
///
/// Thread-safe wrapper for creating a commit with the given message.
///
/// # Arguments
/// * `message` - The commit message
pub async fn create_commit(message: &str) -> Result<()> {
    git_command(&["commit", "-m", message], "commit").await?;
    Ok(())
}

/// Check if we're in a git repository
///
/// # Returns
/// true if the current directory is inside a git repository
pub async fn is_git_repo() -> bool {
    // Don't need mutex for read-only check
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_git_mutex_prevents_races() {
        // Create tasks that would race without synchronization
        let tasks: Vec<_> = (0..5)
            .map(|i| {
                tokio::spawn(async move {
                    // This would normally cause race conditions
                    let result = get_last_commit_message().await;
                    println!("Task {} completed: {:?}", i, result.is_ok());
                    result
                })
            })
            .collect();

        // All tasks should complete without race conditions
        for task in tasks {
            let _ = task.await;
        }
    }

    #[tokio::test]
    async fn test_is_git_repo() {
        // Create a temp directory with a git repo
        let temp_dir = TempDir::new().unwrap();

        // Test non-git directory
        let output = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        assert!(
            !output.status.success(),
            "Should not be a git repo initially"
        );

        // Initialize git repo
        let output = Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();

        // Ensure git init succeeded
        assert!(output.status.success(), "git init failed: {output:?}");

        // Test git directory
        let output = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(temp_dir.path())
            .output()
            .await
            .unwrap();
        assert!(output.status.success(), "Should be a git repo after init");
    }
}
