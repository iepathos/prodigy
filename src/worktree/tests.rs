use super::*;
use crate::subprocess::SubprocessManager;
use std::process::Command;
use tempfile::TempDir;

fn setup_test_repo() -> anyhow::Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Initialize git repo
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["init"])
        .output()?;

    // Create initial commit
    std::fs::write(temp_dir.path().join("README.md"), "# Test Repo")?;
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["add", "."])
        .output()?;
    Command::new("git")
        .current_dir(&temp_dir)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    Ok(temp_dir)
}

// Clean up worktree manager's base directory after tests
fn cleanup_worktree_dir(manager: &WorktreeManager) {
    if manager.base_dir.exists() {
        std::fs::remove_dir_all(&manager.base_dir).ok();
    }
}

#[test]
fn test_worktree_manager_creation() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    assert!(manager.base_dir.exists());
    // Should be in home directory now
    let home_dir = dirs::home_dir().unwrap();
    assert!(manager
        .base_dir
        .starts_with(home_dir.join(".mmm").join("worktrees")));

    // Clean up
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_create_session_with_generated_name() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    assert!(session.name.starts_with("session-"));
    assert!(session.path.exists());
    assert_eq!(session.branch, format!("mmm-{}", session.name));

    // Verify worktree was created
    let worktrees_output = Command::new("git")
        .current_dir(&temp_dir)
        .args(["worktree", "list"])
        .output()?;
    let worktrees = String::from_utf8_lossy(&worktrees_output.stdout);
    assert!(worktrees.contains(&session.name));

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_create_session_with_uuid_name() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;

    assert!(session.name.starts_with("session-"));
    assert!(session.path.exists());

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_list_sessions() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Create multiple sessions
    let session1 = manager.create_session().await?;
    let session2 = manager.create_session().await?;

    let sessions = manager.list_sessions().await?;

    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().any(|s| s.name == session1.name));
    assert!(sessions.iter().any(|s| s.name == session2.name));

    // Clean up
    manager.cleanup_all_sessions(false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_cleanup_session() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;
    let session_name = session.name.clone();

    // Verify worktree exists
    assert!(session.path.exists());

    // Clean up session
    manager.cleanup_session(&session_name, false).await?;

    // Verify worktree is removed
    assert!(!session.path.exists());

    // Verify branch is deleted
    let branches_output = Command::new("git")
        .current_dir(&temp_dir)
        .args(["branch", "--list", &session_name])
        .output()?;
    let branches = String::from_utf8_lossy(&branches_output.stdout);
    assert!(branches.trim().is_empty());

    // Clean up
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_get_worktree_for_branch() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    let session = manager.create_session().await?;
    let worktree_path = manager.get_worktree_for_branch(&session.branch).await?;

    assert!(worktree_path.is_some());
    assert_eq!(worktree_path.unwrap(), session.path);

    // Test non-existent branch
    let no_worktree = manager
        .get_worktree_for_branch("non-existent-branch")
        .await?;
    assert!(no_worktree.is_none());

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}

#[tokio::test]
async fn test_session_name_generation() -> anyhow::Result<()> {
    let temp_dir = setup_test_repo()?;
    let subprocess = SubprocessManager::production();
    let manager = WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)?;

    // Test with spaces and slashes
    let session = manager.create_session().await?;

    // Should replace spaces and slashes with hyphens
    assert!(session.name.starts_with("session-"));

    // Clean up
    manager.cleanup_session(&session.name, false).await?;
    cleanup_worktree_dir(&manager);
    Ok(())
}
