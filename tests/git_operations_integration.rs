use mmm::cook::git_ops::*;
use tempfile::TempDir;
use tokio::process::Command;

/// Test helper: Create a temporary git repository
async fn create_temp_git_repo() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .await
        .unwrap();

    // Configure git user for commits
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(temp_dir.path())
        .output()
        .await
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(temp_dir.path())
        .output()
        .await
        .unwrap();

    temp_dir
}

#[tokio::test]
async fn test_git_operations_integration() {
    // Test complete git workflow
    let temp_repo = create_temp_git_repo().await;
    let original_dir = std::env::current_dir().unwrap();

    std::env::set_current_dir(temp_repo.path()).unwrap();

    // Verify repo status
    assert!(is_git_repo().await);

    // Create initial commit
    std::fs::write(temp_repo.path().join("initial.rs"), "pub fn initial() {}").unwrap();
    stage_all_changes().await.unwrap();
    create_commit("Initial commit").await.unwrap();

    // Create and stage changes
    std::fs::write(temp_repo.path().join("feature.rs"), "pub fn feature() {}").unwrap();
    stage_all_changes().await.unwrap();

    // Create commit
    create_commit("feat: Add new feature").await.unwrap();

    // Verify commit
    let message = get_last_commit_message().await.unwrap();
    assert_eq!(message, "feat: Add new feature");

    // Check clean status
    let status = check_git_status().await.unwrap();
    // Git status output varies by version and configuration
    // An empty status can also indicate a clean working tree
    assert!(
        status.is_empty() // git status --porcelain returns empty for clean tree
        || status.contains("nothing to commit") 
        || status.contains("working tree clean")
        || status.contains("nothing added to commit"),
        "Expected clean status, got: '{status}'"
    );

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[tokio::test]
async fn test_git_operations_with_git_reader_writer() -> Result<()> {
    use mmm::git::{GitCommandRunner, GitReader, GitWriter};
    use mmm::subprocess::SubprocessManager;
    use anyhow::Result;
    
    let temp_dir = TempDir::new()?;
    let subprocess = SubprocessManager::production();
    let git = GitCommandRunner::new(subprocess.runner());
    
    // Initialize repo
    git.init(temp_dir.path()).await?;
    
    // Test status
    let status = git.get_status(temp_dir.path()).await?;
    assert!(status.untracked.is_empty());
    assert!(status.modified.is_empty());
    
    // Create and add file
    std::fs::write(temp_dir.path().join("test.txt"), "hello")?;
    git.add(temp_dir.path(), &["test.txt"]).await?;
    
    // Verify staged
    let status = git.get_status(temp_dir.path()).await?;
    assert_eq!(status.staged.len(), 1);
    
    Ok(())
}

#[tokio::test]
async fn test_git_worktree_operations_with_git_ops() -> Result<()> {
    use mmm::git::{GitCommandRunner, GitWorktreeOps, GitWriter};
    use mmm::subprocess::SubprocessManager;
    use anyhow::Result;
    
    let temp_dir = TempDir::new()?;
    let subprocess = SubprocessManager::production();
    let git = GitCommandRunner::new(subprocess.runner());
    
    // Initialize repo with initial commit
    git.init(temp_dir.path()).await?;
    std::fs::write(temp_dir.path().join("README.md"), "# Test")?;
    git.add(temp_dir.path(), &["README.md"]).await?;
    git.commit(temp_dir.path(), "Initial commit").await?;
    
    // Create worktree
    let worktree_path = temp_dir.path().join("worktree1");
    git.create_worktree(temp_dir.path(), &worktree_path, "feature-branch").await?;
    
    // List worktrees
    let worktrees = git.list_worktrees(temp_dir.path()).await?;
    assert_eq!(worktrees.len(), 2); // main + worktree1
    
    Ok(())
}
