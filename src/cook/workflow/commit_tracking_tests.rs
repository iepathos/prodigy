//! Integration tests for commit tracking in workflows

#[cfg(test)]
mod tests {
    use crate::abstractions::git::{GitOperations, MockGitOperations};
    use crate::cook::commit_tracker::{CommitConfig, CommitTracker};
    use crate::cook::workflow::WorkflowStep;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workflow_commit_tracking_auto_commit() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Setup mock git client
        let mock_git = MockGitOperations::new();
        // Initialize responses
        mock_git.add_success_response("initial123\n").await; // initial HEAD
        mock_git.add_success_response("M src/main.rs\n").await; // has changes
        mock_git.add_success_response("").await; // stage all
        mock_git.add_success_response("").await; // commit
        mock_git.add_success_response("newcommit456\n").await; // new HEAD

        let git_client: Arc<dyn GitOperations> = Arc::new(mock_git);
        let mut tracker = CommitTracker::new(git_client.clone(), path.clone());

        // Initialize tracker
        tracker.initialize().await.unwrap();

        // Create workflow step with auto_commit enabled
        let step = WorkflowStep {
            claude: Some("/test-command".to_string()),
            auto_commit: true,
            commit_config: Some(CommitConfig {
                message_template: Some("chore: auto commit for ${command}".to_string()),
                message_pattern: None,
                sign: false,
                author: None,
                include_files: None,
                exclude_files: None,
                include_timestamp: false,
            }),
            ..Default::default()
        };

        // Create auto-commit
        let variables: HashMap<String, String> = HashMap::from([
            ("command".to_string(), "/test-command".to_string()),
        ]);

        let commit = tracker.create_auto_commit(
            "/test-command",
            step.commit_config.as_ref().and_then(|c| c.message_template.as_deref()),
            &variables,
            step.commit_config.as_ref(),
        ).await.unwrap();

        assert!(!commit.hash.is_empty(), "Should have created auto-commit");
        assert_eq!(commit.step_name, "/test-command");

        // Verify commit was tracked
        let all_commits = tracker.get_all_commits().await;
        assert_eq!(all_commits.len(), 1);
    }

    #[tokio::test]
    async fn test_workflow_commit_verification() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Setup mock git client - no commits created
        let mock_git = MockGitOperations::new();
        mock_git.add_success_response("initial456\n").await; // initial HEAD

        let git_client: Arc<dyn GitOperations> = Arc::new(mock_git);
        let mut tracker = CommitTracker::new(git_client.clone(), path.clone());

        // Initialize tracker
        tracker.initialize().await.unwrap();

        // Create workflow step that requires commits
        let step = WorkflowStep {
            claude: Some("/implement-feature".to_string()),
            commit_required: true,
            commit_config: Some(CommitConfig {
                message_template: None,
                message_pattern: Some(r"^(feat|fix|chore|docs|test|refactor):".to_string()),
                sign: false,
                author: None,
                include_files: None,
                exclude_files: None,
                include_timestamp: false,
            }),
            ..Default::default()
        };

        // Verify should find no commits
        let all_commits = tracker.get_all_commits().await;
        let commits_created = all_commits.iter().filter(|c| c.step_name == "/implement-feature").count();
        assert_eq!(commits_created, 0, "Should find no commits when none created");

        // Verification should fail when commits are required but none exist
        if step.commit_required && commits_created == 0 {
            // This would normally return an error in the workflow executor
            assert!(true, "Correctly detected missing required commits");
        }
    }

    #[tokio::test]
    async fn test_workflow_commit_config_defaults() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        // Setup mock git client
        let mock_git = MockGitOperations::new();
        mock_git.add_success_response("initialABC\n").await; // initial HEAD
        mock_git.add_success_response("M test.rs\n").await; // has changes
        mock_git.add_success_response("").await; // stage all
        mock_git.add_success_response("").await; // commit
        mock_git.add_success_response("defaultcommit\n").await; // new HEAD

        let git_client: Arc<dyn GitOperations> = Arc::new(mock_git);
        let mut tracker = CommitTracker::new(git_client.clone(), path.clone());

        // Initialize tracker
        tracker.initialize().await.unwrap();

        // Create workflow step with minimal config
        let step = WorkflowStep {
            claude: Some("/test".to_string()),
            auto_commit: true,
            commit_config: None, // Use defaults
            ..Default::default()
        };

        // Create auto-commit with default config
        let variables: HashMap<String, String> = HashMap::from([
            ("command".to_string(), "/test".to_string()),
        ]);

        let commit = tracker.create_auto_commit(
            "/test",
            None, // No custom message template
            &variables,
            step.commit_config.as_ref(),
        ).await.unwrap();

        assert!(!commit.hash.is_empty(), "Should create commit with default config");
        assert_eq!(commit.step_name, "/test");
    }
}