//! Unit tests for environment coordinator

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::config::ConfigLoader;
    use crate::subprocess::SubprocessManager;
    use crate::testing::mocks::MockGitOperations;
    use crate::worktree::WorktreeManager;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_verify_git_repository_success() {
        // Setup
        let git_ops = MockGitOperations::builder().is_repo(true).build();

        let config_loader = Arc::new(ConfigLoader::new().await.unwrap());
        let subprocess = SubprocessManager::production();
        // Use the home directory's .prodigy/worktrees as the base for the test
        let worktree_base = directories::BaseDirs::new()
            .expect("base dirs")
            .home_dir()
            .join(".prodigy")
            .join("worktrees");
        std::fs::create_dir_all(&worktree_base).ok();

        // Skip test if we can't create the worktree manager
        let worktree_manager = match WorktreeManager::new(worktree_base, subprocess) {
            Ok(manager) => Arc::new(manager),
            Err(_) => {
                eprintln!("Skipping test: Cannot create WorktreeManager in test environment");
                return;
            }
        };

        let coordinator =
            DefaultEnvironmentCoordinator::new(config_loader, worktree_manager, Arc::new(git_ops));

        // Test - just use current directory for testing
        let result = coordinator
            .verify_git_repository(std::path::Path::new("."))
            .await;

        // Verify
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_git_repository_not_a_repo() {
        // Setup
        let git_ops = MockGitOperations::builder().is_repo(false).build();

        let config_loader = Arc::new(ConfigLoader::new().await.unwrap());
        let subprocess = SubprocessManager::production();
        // Use the home directory's .prodigy/worktrees as the base for the test
        let worktree_base = directories::BaseDirs::new()
            .expect("base dirs")
            .home_dir()
            .join(".prodigy")
            .join("worktrees");
        std::fs::create_dir_all(&worktree_base).ok();

        // Skip test if we can't create the worktree manager
        let worktree_manager = match WorktreeManager::new(worktree_base, subprocess) {
            Ok(manager) => Arc::new(manager),
            Err(_) => {
                eprintln!("Skipping test: Cannot create WorktreeManager in test environment");
                return;
            }
        };

        let coordinator =
            DefaultEnvironmentCoordinator::new(config_loader, worktree_manager, Arc::new(git_ops));

        // Test - just use current directory for testing
        let result = coordinator
            .verify_git_repository(std::path::Path::new("."))
            .await;

        // Verify
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Not a git repository");
    }
}
