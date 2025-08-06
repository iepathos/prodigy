//! Unit tests for environment coordinator

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::config::ConfigLoader;
    use crate::subprocess::SubprocessManager;
    use crate::worktree::WorktreeManager;
    use std::sync::Arc;
    use crate::testing::mocks::MockGitOperations;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_verify_git_repository_success() {
        // Setup
        let git_ops = MockGitOperations::builder()
            .is_repo(true)
            .build();
        
        let config_loader = Arc::new(ConfigLoader::new().await.unwrap());
        let subprocess = SubprocessManager::production();
        let temp_dir = TempDir::new().unwrap();
        let worktree_manager = Arc::new(WorktreeManager::new(
            temp_dir.path().to_path_buf(),
            subprocess,
        ).unwrap());
        
        let coordinator = DefaultEnvironmentCoordinator::new(
            config_loader,
            worktree_manager,
            Arc::new(git_ops),
        );

        // Test
        let result = coordinator.verify_git_repository(temp_dir.path()).await;

        // Verify
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_git_repository_not_a_repo() {
        // Setup
        let git_ops = MockGitOperations::builder()
            .is_repo(false)
            .build();
        
        let config_loader = Arc::new(ConfigLoader::new().await.unwrap());
        let subprocess = SubprocessManager::production();
        let temp_dir = TempDir::new().unwrap();
        let worktree_manager = Arc::new(WorktreeManager::new(
            temp_dir.path().to_path_buf(),
            subprocess,
        ).unwrap());
        
        let coordinator = DefaultEnvironmentCoordinator::new(
            config_loader,
            worktree_manager,
            Arc::new(git_ops),
        );

        // Test
        let result = coordinator.verify_git_repository(temp_dir.path()).await;

        // Verify
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Not a git repository");
    }
}