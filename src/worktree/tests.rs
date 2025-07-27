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

    #[test]
    fn test_worktree_manager_creation() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        assert!(manager.base_dir.exists());
        assert!(manager.base_dir.ends_with(".mmm/worktrees"));

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

        Ok(())
    }

    #[test]
    fn test_list_sessions() -> anyhow::Result<()> {
        let temp_dir = setup_test_repo()?;
        let manager = WorktreeManager::new(temp_dir.path().to_path_buf())?;

        // Create multiple sessions
        let session1 = manager.create_session(None)?;
        let session2 = manager.create_session(Some("security"))?;

        let sessions = manager.list_sessions()?;
        assert_eq!(sessions.len(), 2);

        // Verify sessions are found
        let names: Vec<String> = sessions.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&session1.name));
        assert!(names.contains(&session2.name));

        // Cleanup
        manager.cleanup_all_sessions()?;

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

        Ok(())
    }
}

