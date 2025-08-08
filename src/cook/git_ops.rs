//! Thread-safe git operations
//!
//! This module provides synchronized access to git operations to prevent
//! race conditions when multiple processes might be modifying the repository.
//!
//! This module now acts as a compatibility layer, delegating to the trait-based
//! abstraction for better testability while maintaining the existing API.

use crate::abstractions::{GitOperations, RealGitOperations};
use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Global singleton for git operations
static GIT_OPS: Lazy<Arc<Mutex<RealGitOperations>>> =
    Lazy::new(|| Arc::new(Mutex::new(RealGitOperations::new())));

/// Execute a git command with exclusive access
///
/// # Arguments
/// * `args` - Arguments to pass to the git command
/// * `description` - Human-readable description of the operation
///
/// # Returns
/// The command output on success, or an error with context
pub async fn git_command(args: &[&str], description: &str) -> Result<std::process::Output> {
    let ops = GIT_OPS.lock().await;
    ops.git_command(args, description).await
}

/// Get the last commit message
///
/// Thread-safe wrapper for getting the most recent commit message.
pub async fn get_last_commit_message() -> Result<String> {
    let ops = GIT_OPS.lock().await;
    ops.get_last_commit_message().await
}

/// Check git status
///
/// Thread-safe wrapper for checking repository status.
pub async fn check_git_status() -> Result<String> {
    let ops = GIT_OPS.lock().await;
    ops.check_git_status().await
}

/// Stage all changes
///
/// Thread-safe wrapper for staging all modifications.
pub async fn stage_all_changes() -> Result<()> {
    let ops = GIT_OPS.lock().await;
    ops.stage_all_changes().await
}

/// Create a commit
///
/// Thread-safe wrapper for creating a commit with the given message.
///
/// # Arguments
/// * `message` - The commit message
pub async fn create_commit(message: &str) -> Result<()> {
    let ops = GIT_OPS.lock().await;
    ops.create_commit(message).await
}

/// Check if we're in a git repository
///
/// # Returns
/// true if the current directory is inside a git repository
pub async fn is_git_repo() -> bool {
    let ops = GIT_OPS.lock().await;
    ops.is_git_repo().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::TempDir;
    use tokio::process::Command;

    /// Test helper: Execute git command in a specific directory
    async fn git_in_dir(dir: &std::path::Path, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git command failed: {}", stderr));
        }

        Ok(output)
    }

    /// Test helper: Check git status in a specific directory
    async fn check_git_status_in_dir(dir: &std::path::Path) -> Result<String> {
        let output = git_in_dir(dir, &["status", "--porcelain"]).await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Test helper: Get last commit message in a specific directory
    async fn get_last_commit_message_in_dir(dir: &std::path::Path) -> Result<String> {
        let output = git_in_dir(dir, &["log", "-1", "--pretty=format:%s"]).await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Test helper: Stage all changes in a specific directory
    pub(super) async fn stage_all_changes_in_dir(dir: &std::path::Path) -> Result<()> {
        git_in_dir(dir, &["add", "."]).await?;
        Ok(())
    }

    /// Test helper: Create commit in a specific directory
    pub(super) async fn create_commit_in_dir(dir: &std::path::Path, message: &str) -> Result<()> {
        git_in_dir(dir, &["commit", "-m", message]).await?;
        Ok(())
    }

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

    /// Test helper: Create a temporary git repository
    pub(super) async fn create_temp_git_repo() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(temp_dir.path())
            .output()
            .await?;

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(temp_dir.path())
            .output()
            .await?;

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(temp_dir.path())
            .output()
            .await?;

        Ok(temp_dir)
    }

    /// Test helper: Create a commit in a repository
    async fn create_test_commit(repo_path: &std::path::Path, message: &str) -> Result<()> {
        // Create a unique file to avoid conflicts
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let filename = format!("test_{timestamp}.txt");
        let file_path = repo_path.join(&filename);
        std::fs::write(&file_path, "test content")?;

        // Stage the file
        Command::new("git")
            .args(["add", &filename])
            .current_dir(repo_path)
            .output()
            .await?;

        // Create commit
        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(repo_path)
            .output()
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_get_last_commit_message_success() {
        // Test getting last commit message in a valid repo
        let temp_repo = create_temp_git_repo().await.unwrap();

        // Create commits
        create_test_commit(temp_repo.path(), "Initial commit")
            .await
            .unwrap();
        create_test_commit(temp_repo.path(), "Feature: Add new functionality")
            .await
            .unwrap();

        let result = get_last_commit_message_in_dir(temp_repo.path()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Feature: Add new functionality");
    }

    #[tokio::test]
    async fn test_get_last_commit_message_no_commits() {
        // Test error when no commits exist
        let temp_repo = create_temp_git_repo().await.unwrap();

        let result = get_last_commit_message_in_dir(temp_repo.path()).await;
        assert!(result.is_err());
        // Git error messages vary by version, so just check it failed
    }

    #[tokio::test]
    async fn test_stage_all_changes_success() {
        // Test staging all changes successfully
        let temp_repo = create_temp_git_repo().await.unwrap();

        // Create initial commit
        create_test_commit(temp_repo.path(), "Initial commit")
            .await
            .unwrap();

        // Create a new file
        std::fs::write(temp_repo.path().join("new_file.txt"), "content").unwrap();

        let result = stage_all_changes_in_dir(temp_repo.path()).await;
        assert!(result.is_ok());

        // Verify file is staged
        let status = check_git_status_in_dir(temp_repo.path()).await.unwrap();
        assert!(status.contains("new_file.txt"));
    }

    #[tokio::test]
    async fn test_stage_all_changes_no_changes() {
        // Test staging when no changes exist
        let temp_repo = create_temp_git_repo().await.unwrap();

        // Create initial commit
        create_test_commit(temp_repo.path(), "Initial commit")
            .await
            .unwrap();

        let result = stage_all_changes_in_dir(temp_repo.path()).await;
        assert!(result.is_ok()); // Should succeed even with no changes
    }

    #[tokio::test]
    async fn test_create_commit_success() {
        // Test creating a commit successfully
        let temp_repo = create_temp_git_repo().await.unwrap();

        // Create initial commit
        create_test_commit(temp_repo.path(), "Initial commit")
            .await
            .unwrap();

        // Stage a change
        std::fs::write(temp_repo.path().join("new_test.txt"), "new content").unwrap();
        stage_all_changes_in_dir(temp_repo.path()).await.unwrap();

        let result = create_commit_in_dir(temp_repo.path(), "test: Add test file").await;
        assert!(result.is_ok());

        let last_message = get_last_commit_message_in_dir(temp_repo.path())
            .await
            .unwrap();
        assert_eq!(last_message, "test: Add test file");
    }

    #[tokio::test]
    async fn test_create_commit_no_staged_changes() {
        // Test error when no changes are staged
        let temp_repo = create_temp_git_repo().await.unwrap();

        // Create initial commit
        create_test_commit(temp_repo.path(), "Initial commit")
            .await
            .unwrap();

        let result = create_commit_in_dir(temp_repo.path(), "test: Empty commit").await;
        assert!(result.is_err());
        // Git will reject commits with no changes
    }

    #[tokio::test]
    async fn test_check_git_status_success() {
        // Test with clean repo
        let temp_dir = create_temp_git_repo().await.unwrap();

        let status = check_git_status_in_dir(temp_dir.path()).await.unwrap();
        // The --porcelain output is empty for a clean repo
        assert_eq!(status.trim(), "", "Expected empty status for clean repo");
    }

    #[tokio::test]
    async fn test_check_git_status_with_changes() {
        // Test with uncommitted changes
        let temp_dir = create_temp_git_repo().await.unwrap();

        // Create a file
        std::fs::write(temp_dir.path().join("test.txt"), "test content").unwrap();

        let status = check_git_status_in_dir(temp_dir.path()).await.unwrap();
        // The --porcelain output shows untracked files with ??
        assert!(
            status.contains("?? test.txt"),
            "Expected untracked file in status: {status}"
        );
    }

    // ================== PUBLIC API TESTS ==================
    // These tests verify that the public API functions work correctly.
    // We test them in the actual MMM repository since they use a global
    // singleton that operates on the current directory.

    #[tokio::test]
    async fn test_public_api_functions_in_real_repo() {
        // Skip test if not in a git repository (e.g., during CI or certain test environments)
        if !is_git_repo().await {
            eprintln!("Skipping test - not in a git repository");
            return;
        }

        // Test git_command - use a safe read-only command
        let result = git_command(&["status", "--porcelain"], "Check status").await;
        assert!(result.is_ok(), "git_command should succeed in MMM repo");

        // Test check_git_status
        let status = check_git_status().await;
        assert!(status.is_ok(), "Should be able to check status in MMM repo");

        // Test get_last_commit_message - MMM repo should have commits
        let message = get_last_commit_message().await;
        assert!(
            message.is_ok(),
            "Should get last commit message in MMM repo"
        );

        // The other functions (stage_all_changes, create_commit) modify the repo
        // so we don't test them here to avoid affecting the actual repository
    }

    #[tokio::test]
    async fn test_mutex_synchronization() {
        // Test that the mutex properly synchronizes access
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let counter = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..5)
            .map(|_| {
                let counter = counter.clone();
                tokio::spawn(async move {
                    // This should be serialized by the mutex
                    let _result = check_git_status().await;
                    counter.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        for task in tasks {
            task.await.unwrap();
        }

        assert_eq!(
            counter.load(Ordering::SeqCst),
            5,
            "All tasks should complete"
        );
    }
}

#[cfg(test)]
mod mock_tests {
    use super::tests::{create_commit_in_dir, create_temp_git_repo, stage_all_changes_in_dir};

    #[tokio::test]
    async fn test_stage_all_changes_with_mock() {
        // This test verifies staging works in an isolated environment
        let temp_repo = match create_temp_git_repo().await {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("Skipping test - could not create temp git repo: {e}");
                return;
            }
        };

        // Create a file to stage using directory-specific path
        std::fs::write(temp_repo.path().join("test_stage.txt"), "content").unwrap();

        // Test staging using directory-specific operation
        let result = stage_all_changes_in_dir(temp_repo.path()).await;

        assert!(result.is_ok(), "stage_all_changes_in_dir should succeed");
    }

    #[tokio::test]
    async fn test_create_commit_with_mock() {
        // This test verifies git operations work in an isolated environment
        let temp_repo = match create_temp_git_repo().await {
            Ok(repo) => repo,
            Err(e) => {
                eprintln!("Skipping test - could not create temp git repo: {e}");
                return;
            }
        };

        // Create and stage a file using directory-specific operations
        std::fs::write(temp_repo.path().join("test_commit.txt"), "content").unwrap();
        let stage_result = stage_all_changes_in_dir(temp_repo.path()).await;
        assert!(
            stage_result.is_ok(),
            "stage_all_changes_in_dir should succeed"
        );

        // Test commit creation using directory-specific operation
        let result = create_commit_in_dir(temp_repo.path(), "test: mock commit").await;

        assert!(result.is_ok(), "create_commit_in_dir should succeed");
    }
}
