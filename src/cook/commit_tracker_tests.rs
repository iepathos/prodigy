//! Tests for commit tracking functionality

#[cfg(test)]
mod tests {
    use crate::abstractions::MockGitOperations;
    use crate::cook::commit_tracker::{CommitConfig, CommitTracker, TrackedCommit};
    use crate::cook::workflow::executor::WorkflowStep;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_commit_tracker_initialization() {
        let mock_git = Arc::new(MockGitOperations::new());
        mock_git.add_success_response("abc123def456\n").await;

        let mut tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));
        tracker.initialize().await.unwrap();

        assert_eq!(tracker.initial_head, Some("abc123def456".to_string()));
    }

    #[tokio::test]
    async fn test_has_changes_detection() {
        let mock_git = Arc::new(MockGitOperations::new());

        // First check - has changes
        mock_git
            .add_success_response("M  src/main.rs\nA  src/new.rs\n")
            .await;
        let tracker = CommitTracker::new(mock_git.clone(), PathBuf::from("/test"));
        assert!(tracker.has_changes().await.unwrap());

        // Second check - no changes
        mock_git.add_success_response("").await;
        assert!(!tracker.has_changes().await.unwrap());
    }

    #[tokio::test]
    async fn test_get_commits_between() {
        let mock_git = Arc::new(MockGitOperations::new());

        // Mock the git log output
        let log_output = "hash1|feat: add feature|John Doe|2024-01-01T12:00:00Z\nfile1.rs\nfile2.rs\n\nhash2|fix: bug fix|Jane Smith|2024-01-02T12:00:00Z\nfile3.rs\n";
        mock_git.add_success_response(log_output).await;

        // Mock diff stats for first commit
        mock_git
            .add_success_response(" 2 files changed, 10 insertions(+), 3 deletions(-)\n")
            .await;
        // Mock diff stats for second commit
        mock_git
            .add_success_response(" 1 file changed, 5 insertions(+), 2 deletions(-)\n")
            .await;

        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));
        let commits = tracker.get_commits_between("HEAD~2", "HEAD").await.unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].hash, "hash1");
        assert_eq!(commits[0].message, "feat: add feature");
        assert_eq!(commits[0].author, "John Doe");
        assert_eq!(commits[0].files_changed.len(), 2);
        assert_eq!(commits[0].insertions, 10);
        assert_eq!(commits[0].deletions, 3);

        assert_eq!(commits[1].hash, "hash2");
        assert_eq!(commits[1].message, "fix: bug fix");
        assert_eq!(commits[1].files_changed.len(), 1);
    }

    #[tokio::test]
    async fn test_create_auto_commit() {
        let mock_git = Arc::new(MockGitOperations::new());

        // Mock has_changes
        mock_git.add_success_response("M  src/main.rs\n").await;
        // Mock git add
        mock_git.add_success_response("").await;
        // Mock git commit
        mock_git.add_success_response("").await;
        // Mock get HEAD
        mock_git.add_success_response("new_hash\n").await;
        // Mock get commits between
        let log_output =
            "new_hash|Auto-commit: test-step|Test User|2024-01-01T12:00:00Z\nsrc/main.rs\n";
        mock_git.add_success_response(log_output).await;
        // Mock diff stats
        mock_git
            .add_success_response(" 1 file changed, 5 insertions(+), 2 deletions(-)\n")
            .await;

        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));
        let variables = HashMap::new();
        let commit = tracker
            .create_auto_commit("test-step", None, &variables)
            .await
            .unwrap();

        assert_eq!(commit.hash, "new_hash");
        assert_eq!(commit.message, "Auto-commit: test-step");
        assert_eq!(commit.step_name, "test-step");
    }

    #[tokio::test]
    async fn test_message_template_interpolation() {
        let mock_git = Arc::new(MockGitOperations::new());
        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));

        let mut variables = HashMap::new();
        variables.insert("item".to_string(), "user.py".to_string());
        variables.insert("feature".to_string(), "authentication".to_string());

        let message = tracker
            .interpolate_template(
                "feat: modernize ${item} for ${feature} in ${step.name}",
                "refactor-step",
                &variables,
            )
            .unwrap();

        assert_eq!(
            message,
            "feat: modernize user.py for authentication in refactor-step"
        );
    }

    #[tokio::test]
    async fn test_commit_message_validation() {
        let mock_git = Arc::new(MockGitOperations::new());
        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));

        let pattern = r"^(feat|fix|docs|style|refactor|test|chore)(\([a-z]+\))?: .+$";

        // Valid messages
        assert!(tracker
            .validate_message("feat: add new feature", pattern)
            .is_ok());
        assert!(tracker
            .validate_message("fix(auth): resolve login issue", pattern)
            .is_ok());
        assert!(tracker
            .validate_message("docs: update README", pattern)
            .is_ok());

        // Invalid messages
        assert!(tracker.validate_message("bad message", pattern).is_err());
        assert!(tracker
            .validate_message("Feature: wrong case", pattern)
            .is_err());
    }

    #[tokio::test]
    async fn test_track_step_commits() {
        let mock_git = Arc::new(MockGitOperations::new());

        // Mock get commits between
        let log_output = "hash1|feat: step change|Dev|2024-01-01T12:00:00Z\nfile1.rs\n";
        mock_git.add_success_response(log_output).await;
        // Mock diff stats
        mock_git
            .add_success_response(" 1 file changed, 10 insertions(+), 0 deletions(-)\n")
            .await;

        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));
        let commits = tracker
            .track_step_commits("test-step", "old_hash", "new_hash")
            .await
            .unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].step_name, "test-step");

        // Check that commits were added to tracked commits
        let all_commits = tracker.get_all_commits().await;
        assert_eq!(all_commits.len(), 1);
        assert_eq!(all_commits[0].step_name, "test-step");
    }

    #[tokio::test]
    async fn test_squash_commits() {
        let mock_git = Arc::new(MockGitOperations::new());

        // Create test commits
        let commits = vec![
            TrackedCommit {
                hash: "hash1".to_string(),
                message: "commit 1".to_string(),
                author: "test".to_string(),
                timestamp: Utc::now(),
                files_changed: vec![PathBuf::from("file1.rs")],
                insertions: 10,
                deletions: 5,
                step_name: "step1".to_string(),
                agent_id: None,
            },
            TrackedCommit {
                hash: "hash2".to_string(),
                message: "commit 2".to_string(),
                author: "test".to_string(),
                timestamp: Utc::now(),
                files_changed: vec![PathBuf::from("file2.rs")],
                insertions: 20,
                deletions: 3,
                step_name: "step2".to_string(),
                agent_id: None,
            },
        ];

        // Mock get parent
        mock_git.add_success_response("parent_hash\n").await;
        // Mock reset
        mock_git.add_success_response("").await;
        // Mock commit
        mock_git.add_success_response("").await;
        // Mock get HEAD
        mock_git.add_success_response("squashed_hash\n").await;

        let tracker = CommitTracker::new(mock_git, PathBuf::from("/test"));
        let squashed = tracker
            .squash_commits(&commits, "feat: squashed changes")
            .await
            .unwrap();

        assert_eq!(squashed, "squashed_hash");
    }

    #[tokio::test]
    async fn test_commit_config_serialization() {
        let config = CommitConfig {
            message_template: Some("feat: ${item}".to_string()),
            message_pattern: Some(r"^feat:".to_string()),
            sign: true,
            author: Some("Test Author".to_string()),
            include_files: Some(vec!["*.rs".to_string()]),
            exclude_files: Some(vec!["*.tmp".to_string()]),
            squash: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: CommitConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.message_template, config.message_template);
        assert_eq!(deserialized.sign, config.sign);
        assert_eq!(deserialized.author, config.author);
    }

    #[tokio::test]
    async fn test_commit_tracking_result() {
        use crate::cook::commit_tracker::CommitTrackingResult;

        let commits = vec![
            TrackedCommit {
                hash: "hash1".to_string(),
                message: "commit 1".to_string(),
                author: "test".to_string(),
                timestamp: Utc::now(),
                files_changed: vec![PathBuf::from("file1.rs"), PathBuf::from("file2.rs")],
                insertions: 10,
                deletions: 5,
                step_name: "step1".to_string(),
                agent_id: None,
            },
            TrackedCommit {
                hash: "hash2".to_string(),
                message: "commit 2".to_string(),
                author: "test".to_string(),
                timestamp: Utc::now(),
                files_changed: vec![PathBuf::from("file2.rs"), PathBuf::from("file3.rs")],
                insertions: 20,
                deletions: 3,
                step_name: "step2".to_string(),
                agent_id: None,
            },
        ];

        let result = CommitTrackingResult::from_commits(commits);

        assert_eq!(result.commits.len(), 2);
        assert_eq!(result.total_files_changed, 3); // file1, file2, file3 (deduplicated)
        assert_eq!(result.total_insertions, 30);
        assert_eq!(result.total_deletions, 8);
    }
}
