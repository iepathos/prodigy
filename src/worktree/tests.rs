#[cfg(test)]
mod tests {
    use super::super::*;
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
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

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

    #[test]
    fn test_create_session_without_focus() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        let session = manager.create_session(None)?;

        assert!(session.name.starts_with("mmm-session-"));
        assert_eq!(session.branch, session.name);
        assert!(session.path.exists());
        assert!(session.focus.is_none());

        // Cleanup
        manager.cleanup_session(&session.name)?;
        cleanup_worktree_dir(&manager);

        Ok(())
    }

    #[test]
    fn test_create_session_with_focus() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        let session = manager.create_session(Some("performance"))?;

        assert!(session.name.starts_with("mmm-performance-"));
        assert_eq!(session.branch, session.name);
        assert!(session.path.exists());
        assert_eq!(session.focus, Some("performance".to_string()));

        // Cleanup
        manager.cleanup_session(&session.name)?;
        cleanup_worktree_dir(&manager);

        Ok(())
    }

    #[test]
    fn test_list_sessions() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        // Create multiple sessions with a small delay to ensure different timestamps
        let session1 = manager.create_session(None)?;
        std::thread::sleep(std::time::Duration::from_millis(10));
        let session2 = manager.create_session(Some("security"))?;

        let sessions = manager.list_sessions()?;

        assert_eq!(sessions.len(), 2);

        // Verify sessions are found
        let names: Vec<String> = sessions.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&session1.name));
        assert!(names.contains(&session2.name));

        // Cleanup
        manager.cleanup_all_sessions()?;
        cleanup_worktree_dir(&manager);

        Ok(())
    }

    #[test]
    fn test_cleanup_session() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        let session = manager.create_session(None)?;
        let session_path = session.path.clone();

        // Verify worktree exists
        assert!(session_path.exists());

        // Cleanup
        manager.cleanup_session(&session.name)?;

        // Verify worktree is removed
        assert!(!session_path.exists());

        // Verify it's not in the list
        let sessions = manager.list_sessions()?;
        assert!(sessions.is_empty());

        // Final cleanup
        cleanup_worktree_dir(&manager);

        Ok(())
    }

    #[test]
    fn test_focus_sanitization() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        let session = manager.create_session(Some("user experience/testing"))?;

        // Verify slashes and spaces are sanitized
        assert!(session.name.contains("user-experience-testing"));

        // Cleanup
        manager.cleanup_session(&session.name)?;
        cleanup_worktree_dir(&manager);

        Ok(())
    }
}
