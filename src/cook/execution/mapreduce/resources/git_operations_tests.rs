//! Unit tests for GitOperationsService
//!
//! This module tests the git operations functionality including:
//! - Commit retrieval with various filters
//! - File modification tracking
//! - Error handling scenarios
//! - Edge cases and boundary conditions

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::cook::execution::errors::MapReduceError;
    use chrono::Utc;
    use git2::{Oid, Repository, Signature};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Helper to create a test repository with initial commits
    fn create_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("Failed to init repository");

        // Configure git user for commits
        let mut config = repo.config().expect("Failed to get config");
        config
            .set_str("user.name", "Test User")
            .expect("Failed to set user name");
        config
            .set_str("user.email", "test@example.com")
            .expect("Failed to set user email");

        (temp_dir, repo)
    }

    /// Helper to create a commit in the repository
    fn create_commit(
        repo: &Repository,
        message: &str,
        files: Vec<(&str, &str)>,
    ) -> Result<Oid, git2::Error> {
        let sig = Signature::now("Test User", "test@example.com")?;

        // Create or update files
        for (filename, content) in files {
            let file_path = repo.workdir().unwrap().join(filename);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::write(&file_path, content).expect("Failed to write file");
        }

        // Stage all changes
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        // Get parent commit if exists
        let parent_commit = if let Ok(head) = repo.head() {
            Some(head.peel_to_commit()?)
        } else {
            None
        };

        let parent_commits: Vec<&git2::Commit> = parent_commit
            .as_ref()
            .map(|c| vec![c])
            .unwrap_or_else(Vec::new);

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            message,
            &tree,
            &parent_commits,
        )
    }

    #[tokio::test]
    async fn test_new_service_with_default_config() {
        let config = GitOperationsConfig::default();
        let service = GitOperationsService::new(config.clone());

        // Verify default configuration values
        assert_eq!(service.config.enable_caching, true);
        assert_eq!(service.config.cache_ttl_secs, 300);
        assert_eq!(service.config.max_commits, 1000);
        assert_eq!(service.config.max_files, 5000);
        assert_eq!(service.config.include_diffs, false);
        assert_eq!(service.config.operation_timeout_secs, 30);
    }

    #[tokio::test]
    async fn test_new_service_with_custom_config() {
        let config = GitOperationsConfig {
            enable_caching: false,
            cache_ttl_secs: 600,
            max_commits: 500,
            max_files: 1000,
            include_diffs: true,
            operation_timeout_secs: 60,
        };
        let service = GitOperationsService::new(config.clone());

        assert_eq!(service.config.enable_caching, false);
        assert_eq!(service.config.cache_ttl_secs, 600);
        assert_eq!(service.config.max_commits, 500);
        assert_eq!(service.config.max_files, 1000);
        assert_eq!(service.config.include_diffs, true);
        assert_eq!(service.config.operation_timeout_secs, 60);
    }

    #[tokio::test]
    async fn test_get_worktree_commits_empty_repo() {
        let (temp_dir, _repo) = create_test_repo();
        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let result = service
            .get_worktree_commits(temp_dir.path(), None, None)
            .await;

        // Should return error for empty repository (no HEAD)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_worktree_commits_single_commit() {
        let (temp_dir, repo) = create_test_repo();
        create_commit(&repo, "Initial commit", vec![("test.txt", "Hello World")])
            .expect("Failed to create commit");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let commits = service
            .get_worktree_commits(temp_dir.path(), None, None)
            .await
            .expect("Failed to get commits");

        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Initial commit");
        assert_eq!(commits[0].files_changed.len(), 1);
        assert!(commits[0].files_changed[0].ends_with("test.txt"));
    }

    #[tokio::test]
    async fn test_get_worktree_commits_multiple() {
        let (temp_dir, repo) = create_test_repo();

        create_commit(&repo, "First commit", vec![("file1.txt", "Content 1")])
            .expect("Failed to create first commit");
        create_commit(&repo, "Second commit", vec![("file2.txt", "Content 2")])
            .expect("Failed to create second commit");
        create_commit(
            &repo,
            "Third commit",
            vec![("file3.txt", "Content 3"), ("file1.txt", "Modified 1")],
        )
        .expect("Failed to create third commit");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let commits = service
            .get_worktree_commits(temp_dir.path(), None, None)
            .await
            .expect("Failed to get commits");

        assert_eq!(commits.len(), 3);
        // Commits should be in reverse chronological order (newest first)
        assert_eq!(commits[0].message, "Third commit");
        assert_eq!(commits[1].message, "Second commit");
        assert_eq!(commits[2].message, "First commit");

        // Check commit statistics
        let third_commit = &commits[0];
        assert!(third_commit.stats.is_some());
        let stats = third_commit.stats.as_ref().unwrap();
        assert_eq!(stats.files_changed, 2); // file3.txt added, file1.txt modified
    }

    #[tokio::test]
    async fn test_get_worktree_commits_with_time_filter() {
        let (temp_dir, repo) = create_test_repo();

        // Create commits with time gaps
        create_commit(&repo, "Old commit", vec![("old.txt", "Old content")])
            .expect("Failed to create old commit");

        std::thread::sleep(std::time::Duration::from_millis(100));
        let since_time = Utc::now();
        std::thread::sleep(std::time::Duration::from_millis(100));

        create_commit(&repo, "Recent commit", vec![("recent.txt", "Recent content")])
            .expect("Failed to create recent commit");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        // Get commits since the middle timestamp
        let commits = service
            .get_worktree_commits(temp_dir.path(), Some(since_time), None)
            .await
            .expect("Failed to get commits");

        // Should only get the recent commit
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "Recent commit");
    }

    #[tokio::test]
    async fn test_get_worktree_commits_max_limit() {
        let (temp_dir, repo) = create_test_repo();

        // Create more commits than the limit
        for i in 0..10 {
            create_commit(
                &repo,
                &format!("Commit {}", i),
                vec![(&format!("file{}.txt", i), &format!("Content {}", i))],
            )
            .expect("Failed to create commit");
        }

        let config = GitOperationsConfig {
            max_commits: 5,
            ..Default::default()
        };
        let mut service = GitOperationsService::new(config);

        let commits = service
            .get_worktree_commits(temp_dir.path(), None, None)
            .await
            .expect("Failed to get commits");

        // Should be limited to 5 commits
        assert_eq!(commits.len(), 5);
        // Should get the 5 most recent commits
        assert_eq!(commits[0].message, "Commit 9");
        assert_eq!(commits[4].message, "Commit 5");
    }

    #[tokio::test]
    async fn test_get_worktree_modified_files_empty_repo() {
        let (temp_dir, _repo) = create_test_repo();
        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let result = service
            .get_worktree_modified_files(temp_dir.path(), None)
            .await;

        // Should return error for empty repository
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_worktree_modified_files_with_changes() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial commits
        let first_commit = create_commit(&repo, "First", vec![("file1.txt", "Content 1")])
            .expect("Failed to create first commit");
        create_commit(
            &repo,
            "Second",
            vec![
                ("file2.txt", "Content 2"),
                ("file1.txt", "Modified 1"), // Modify existing file
            ],
        )
        .expect("Failed to create second commit");

        // Add untracked file
        fs::write(temp_dir.path().join("untracked.txt"), "Untracked content")
            .expect("Failed to create untracked file");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        // Get all modifications
        let files = service
            .get_worktree_modified_files(temp_dir.path(), None)
            .await
            .expect("Failed to get modified files");

        // Should include working directory changes and recent commits
        assert!(files.len() >= 1); // At least the untracked file
        let untracked = files
            .iter()
            .find(|f| f.path.to_string_lossy().contains("untracked.txt"));
        assert!(untracked.is_some());
        if let Some(untracked_file) = untracked {
            assert!(matches!(
                untracked_file.modification_type,
                ModificationType::Added
            ));
        }

        // Get modifications since first commit
        let files_since = service
            .get_worktree_modified_files(temp_dir.path(), Some(&first_commit.to_string()))
            .await
            .expect("Failed to get modified files since commit");

        // Should include file2.txt (added) and file1.txt (modified) from second commit
        // Plus the untracked file
        assert!(files_since.len() >= 2);
    }

    #[tokio::test]
    async fn test_get_worktree_modified_files_max_limit() {
        let (temp_dir, repo) = create_test_repo();

        // Create many files
        let mut files = vec![];
        for i in 0..20 {
            files.push((
                format!("file{}.txt", i).as_str().to_string(),
                format!("Content {}", i).as_str().to_string(),
            ));
        }

        create_commit(
            &repo,
            "Many files",
            files
                .iter()
                .map(|(name, content)| (name.as_str(), content.as_str()))
                .collect(),
        )
        .expect("Failed to create commit");

        let config = GitOperationsConfig {
            max_files: 10,
            ..Default::default()
        };
        let mut service = GitOperationsService::new(config);

        let modified_files = service
            .get_worktree_modified_files(temp_dir.path(), None)
            .await
            .expect("Failed to get modified files");

        // Should be limited to 10 files
        assert_eq!(modified_files.len(), 10);
    }

    #[tokio::test]
    async fn test_file_modification_types() {
        let (temp_dir, repo) = create_test_repo();

        // Create initial files
        create_commit(
            &repo,
            "Initial",
            vec![("file1.txt", "Content 1"), ("file2.txt", "Content 2")],
        )
        .expect("Failed to create initial commit");

        // Rename file (simulate via delete and add)
        fs::remove_file(temp_dir.path().join("file1.txt")).expect("Failed to delete file");
        fs::write(temp_dir.path().join("file1_renamed.txt"), "Content 1")
            .expect("Failed to create renamed file");

        // Modify file
        fs::write(temp_dir.path().join("file2.txt"), "Modified content 2")
            .expect("Failed to modify file");

        // Add new file
        fs::write(temp_dir.path().join("file3.txt"), "Content 3")
            .expect("Failed to create new file");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let files = service
            .get_worktree_modified_files(temp_dir.path(), None)
            .await
            .expect("Failed to get modified files");

        // Check for different modification types
        let added = files
            .iter()
            .filter(|f| matches!(f.modification_type, ModificationType::Added))
            .count();
        let modified = files
            .iter()
            .filter(|f| matches!(f.modification_type, ModificationType::Modified))
            .count();
        let deleted = files
            .iter()
            .filter(|f| matches!(f.modification_type, ModificationType::Deleted))
            .count();

        assert!(added >= 1); // At least file3.txt
        assert!(modified >= 1); // file2.txt
        assert!(deleted >= 1); // file1.txt
    }

    #[tokio::test]
    async fn test_get_merge_git_info() {
        let (temp_dir, repo) = create_test_repo();

        // Create some commits
        create_commit(&repo, "First commit", vec![("file1.txt", "Content 1")])
            .expect("Failed to create first commit");
        create_commit(&repo, "Second commit", vec![("file2.txt", "Content 2")])
            .expect("Failed to create second commit");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let merge_info = service
            .get_merge_git_info(temp_dir.path(), "main")
            .await
            .expect("Failed to get merge git info");

        assert_eq!(merge_info.target_branch, "main");
        assert_eq!(merge_info.worktree_path, temp_dir.path());
        assert_eq!(merge_info.commits.len(), 2);
        assert!(merge_info.modified_files.len() >= 2);
        assert!(merge_info.generated_at <= Utc::now());
    }

    #[tokio::test]
    async fn test_commit_info_details() {
        let (temp_dir, repo) = create_test_repo();

        let commit_id = create_commit(
            &repo,
            "Test commit message\n\nDetailed description here",
            vec![
                ("src/main.rs", "fn main() {}"),
                ("Cargo.toml", "[package]\nname = \"test\""),
            ],
        )
        .expect("Failed to create commit");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let commits = service
            .get_worktree_commits(temp_dir.path(), None, None)
            .await
            .expect("Failed to get commits");

        assert_eq!(commits.len(), 1);
        let commit = &commits[0];

        // Check commit details
        assert_eq!(commit.id, commit_id.to_string());
        assert_eq!(commit.short_id, format!("{:.7}", commit_id.to_string()));
        assert!(commit
            .message
            .starts_with("Test commit message\n\nDetailed description"));
        assert_eq!(commit.author.name, "Test User");
        assert_eq!(commit.author.email, "test@example.com");
        assert_eq!(commit.parent_ids.len(), 0); // First commit has no parents

        // Check stats
        assert!(commit.stats.is_some());
        let stats = commit.stats.as_ref().unwrap();
        assert_eq!(stats.files_changed, 2);
        assert!(stats.insertions > 0);

        // Check files changed
        assert_eq!(commit.files_changed.len(), 2);
        assert!(commit.files_changed.iter().any(|f| f.contains("main.rs")));
        assert!(commit.files_changed.iter().any(|f| f.contains("Cargo.toml")));
    }

    #[tokio::test]
    async fn test_error_handling_invalid_path() {
        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let invalid_path = PathBuf::from("/nonexistent/path");
        let result = service
            .get_worktree_commits(&invalid_path, None, None)
            .await;

        assert!(result.is_err());
        if let Err(MapReduceError::General { message, .. }) = result {
            assert!(message.contains("Invalid repository path"));
        } else {
            panic!("Expected General error with invalid path message");
        }
    }

    #[tokio::test]
    async fn test_error_handling_not_a_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let result = service
            .get_worktree_commits(temp_dir.path(), None, None)
            .await;

        assert!(result.is_err());
        if let Err(MapReduceError::General { message, .. }) = result {
            assert!(message.contains("Failed to open repository"));
        } else {
            panic!("Expected General error with repository open failure");
        }
    }

    #[tokio::test]
    async fn test_deduplicate_modifications() {
        let (temp_dir, repo) = create_test_repo();

        // Create commits that modify the same file multiple times
        create_commit(&repo, "First", vec![("file.txt", "Version 1")])
            .expect("Failed to create first commit");
        create_commit(&repo, "Second", vec![("file.txt", "Version 2")])
            .expect("Failed to create second commit");
        create_commit(&repo, "Third", vec![("file.txt", "Version 3")])
            .expect("Failed to create third commit");

        let config = GitOperationsConfig::default();
        let mut service = GitOperationsService::new(config);

        let files = service
            .get_worktree_modified_files(temp_dir.path(), None)
            .await
            .expect("Failed to get modified files");

        // Should deduplicate to show file.txt only once
        let file_txt_count = files
            .iter()
            .filter(|f| f.path.to_string_lossy().contains("file.txt"))
            .count();
        assert_eq!(file_txt_count, 1);
    }

    #[tokio::test]
    async fn test_helper_trait_to_string_list() {
        let commits = vec![
            CommitInfo {
                id: "abc123".to_string(),
                short_id: "abc1234".to_string(),
                author: AuthorInfo {
                    name: "Test".to_string(),
                    email: "test@example.com".to_string(),
                    timestamp: Utc::now(),
                },
                committer: AuthorInfo {
                    name: "Test".to_string(),
                    email: "test@example.com".to_string(),
                    timestamp: Utc::now(),
                },
                message: "Test commit".to_string(),
                timestamp: Utc::now(),
                parent_ids: vec![],
                tree_id: "tree123".to_string(),
                stats: None,
                files_changed: vec![],
            },
            CommitInfo {
                id: "def456".to_string(),
                short_id: "def4567".to_string(),
                author: AuthorInfo {
                    name: "Test".to_string(),
                    email: "test@example.com".to_string(),
                    timestamp: Utc::now(),
                },
                committer: AuthorInfo {
                    name: "Test".to_string(),
                    email: "test@example.com".to_string(),
                    timestamp: Utc::now(),
                },
                message: "Another commit".to_string(),
                timestamp: Utc::now(),
                parent_ids: vec![],
                tree_id: "tree456".to_string(),
                stats: None,
                files_changed: vec![],
            },
        ];

        let string_list = commits.to_string_list();
        assert_eq!(string_list.len(), 2);
        assert_eq!(string_list[0], "abc123");
        assert_eq!(string_list[1], "def456");

        let files = vec![
            FileModificationInfo {
                path: PathBuf::from("file1.txt"),
                modification_type: ModificationType::Added,
                size_before: None,
                size_after: Some(100),
                last_modified: Utc::now(),
                commit_id: None,
                diff_stats: None,
                content_diff: None,
            },
            FileModificationInfo {
                path: PathBuf::from("dir/file2.rs"),
                modification_type: ModificationType::Modified,
                size_before: Some(50),
                size_after: Some(75),
                last_modified: Utc::now(),
                commit_id: None,
                diff_stats: None,
                content_diff: None,
            },
        ];

        let file_list = files.to_string_list();
        assert_eq!(file_list.len(), 2);
        assert_eq!(file_list[0], "file1.txt");
        assert_eq!(file_list[1], "dir/file2.rs");
    }
}