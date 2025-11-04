//! Tests for git context tracking
//!
//! This module contains comprehensive tests for the git change tracking functionality,
//! organized into three test phases:
//!
//! - Phase 1: Uncommitted Changes Detection
//! - Phase 2: Commit History Walking
//! - Phase 3: Diff Statistics and File Changes

#[cfg(test)]
mod tests {
    use crate::cook::workflow::git_context::*;
    use anyhow::Result;
    use git2::Repository;
    use std::path::Path;
    use tempfile::TempDir;

    fn init_test_repo() -> Result<TempDir> {
        let dir = TempDir::new()?;
        {
            let repo = Repository::init(dir.path())?;

            // Create initial commit
            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = {
                let mut index = repo.index()?;
                index.write_tree()?
            };
            let tree = repo.find_tree(tree_id)?;
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])?;
        }

        Ok(dir)
    }

    #[test]
    fn test_step_changes() {
        let changes = StepChanges {
            files_added: vec!["a.txt".into(), "b.txt".into()],
            files_modified: vec!["c.txt".into()],
            files_deleted: vec!["d.txt".into()],
            commits: vec!["abc123".into()],
            insertions: 10,
            deletions: 5,
        };

        assert_eq!(changes.files_changed().len(), 4);
        assert_eq!(changes.commit_count(), 1);
    }

    #[test]
    fn test_filter_files() {
        let changes = StepChanges {
            files_added: vec![
                "src/main.rs".into(),
                "src/lib.rs".into(),
                "README.md".into(),
                "docs/guide.md".into(),
            ],
            ..Default::default()
        };

        let filtered = changes.filter_files(&changes.files_added, "*.md");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"README.md".to_string()));
        assert!(filtered.contains(&"docs/guide.md".to_string()));
    }

    #[test]
    fn test_format_file_list() {
        let files = vec!["a.txt".into(), "b.txt".into()];

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::SpaceSeparated),
            "a.txt b.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::NewlineSeparated),
            "a.txt\nb.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::CommaSeparated),
            "a.txt,b.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::JsonArray),
            r#"["a.txt","b.txt"]"#
        );
    }

    #[test]
    fn test_tracker_initialization() -> Result<()> {
        let dir = init_test_repo()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        assert!(tracker.is_active());
        assert!(tracker.workflow_start_commit.is_some());

        Ok(())
    }

    #[test]
    fn test_non_git_directory() -> Result<()> {
        let dir = TempDir::new()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        assert!(!tracker.is_active());
        assert!(tracker.workflow_start_commit.is_none());

        Ok(())
    }

    // Phase 1 Tests: Uncommitted Changes Detection

    #[test]
    fn test_calculate_step_changes_with_new_file() -> Result<()> {
        let dir = init_test_repo()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        // Create a new file
        std::fs::write(dir.path().join("new_file.txt"), "content")?;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"new_file.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_modified_file() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("existing.txt"), "original content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("existing.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add existing file",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Modify the file
        std::fs::write(dir.path().join("existing.txt"), "modified content")?;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_modified.contains(&"existing.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_deleted_file() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_delete.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file to delete",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Delete the file
        std::fs::remove_file(dir.path().join("to_delete.txt"))?;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_deleted.contains(&"to_delete.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_staged_new_file() -> Result<()> {
        let dir = init_test_repo()?;
        let tracker = GitChangeTracker::new(dir.path())?;

        // Create and stage a new file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("staged_new.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("staged_new.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"staged_new.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_staged_modification() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "original")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file to modify",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Modify and stage the file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "modified")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes
            .files_modified
            .contains(&"to_modify.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_staged_deletion() -> Result<()> {
        let dir = init_test_repo()?;

        // Create and commit a file first
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("staged_delete.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("staged_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file for staged deletion",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Delete and stage the deletion
        {
            let repo = Repository::open(dir.path())?;
            std::fs::remove_file(dir.path().join("staged_delete.txt"))?;
            let mut index = repo.index()?;
            index.remove_path(Path::new("staged_delete.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes
            .files_deleted
            .contains(&"staged_delete.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_mixed_changes() -> Result<()> {
        let dir = init_test_repo()?;

        // Setup: create and commit initial files
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("existing1.txt"), "content1")?;
            std::fs::write(dir.path().join("existing2.txt"), "content2")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("existing1.txt"))?;
            index.add_path(Path::new("existing2.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add initial files",
                &tree,
                &[&parent],
            )?;
        }

        let tracker = GitChangeTracker::new(dir.path())?;

        // Create mixed changes: new file, modified file (unstaged), deleted file (staged)
        {
            let repo = Repository::open(dir.path())?;

            // New file (not staged)
            std::fs::write(dir.path().join("new.txt"), "new content")?;

            // Modified file (not staged)
            std::fs::write(dir.path().join("existing1.txt"), "modified content")?;

            // Deleted file (staged)
            std::fs::remove_file(dir.path().join("existing2.txt"))?;
            let mut index = repo.index()?;
            index.remove_path(Path::new("existing2.txt"))?;
            index.write()?;
        }

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"new.txt".to_string()));
        assert!(changes
            .files_modified
            .contains(&"existing1.txt".to_string()));
        assert!(changes.files_deleted.contains(&"existing2.txt".to_string()));

        Ok(())
    }

    // Phase 2 Tests: Commit History Walking

    #[test]
    fn test_calculate_step_changes_with_new_commit() -> Result<()> {
        let dir = init_test_repo()?;

        // Create initial tracker to capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a new commit
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("new_commit.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("new_commit.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "New commit for test",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit to simulate previous step
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 1);
        assert!(changes.files_added.contains(&"new_commit.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_multiple_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create multiple commits
        {
            let repo = Repository::open(dir.path())?;
            let sig = git2::Signature::now("Test", "test@example.com")?;

            // First commit
            std::fs::write(dir.path().join("file1.txt"), "content1")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("file1.txt"))?;
            index.write()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "First commit", &tree, &[&parent])?;

            // Second commit
            std::fs::write(dir.path().join("file2.txt"), "content2")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("file2.txt"))?;
            index.write()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])?;

            // Third commit
            std::fs::write(dir.path().join("file3.txt"), "content3")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("file3.txt"))?;
            index.write()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Third commit", &tree, &[&parent])?;
        }

        // Update tracker's last_commit to simulate previous step
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 3);
        assert!(changes.files_added.contains(&"file1.txt".to_string()));
        assert!(changes.files_added.contains(&"file2.txt".to_string()));
        assert!(changes.files_added.contains(&"file3.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_commit_stats() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a commit with known insertions
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(
                dir.path().join("stats_test.txt"),
                "line1\nline2\nline3\nline4\nline5\n",
            )?;
            let mut index = repo.index()?;
            index.add_path(Path::new("stats_test.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Commit with stats",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.insertions > 0);
        assert_eq!(changes.deletions, 0);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_with_no_new_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // Create tracker and immediately check again (no new commits)
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let current_commit = tracker.workflow_start_commit.clone();
        tracker.last_commit = current_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 0);
        assert_eq!(changes.insertions, 0);
        assert_eq!(changes.deletions, 0);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_tracks_commit_shas() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a commit
        let new_commit_sha = {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("sha_test.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("sha_test.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            let commit_oid = repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "SHA tracking test",
                &tree,
                &[&parent],
            )?;
            commit_oid.to_string()
        };

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert_eq!(changes.commits.len(), 1);
        assert_eq!(changes.commits[0], new_commit_sha);

        Ok(())
    }

    // Phase 3 Tests: Diff Statistics and File Changes

    #[test]
    fn test_calculate_step_changes_tracks_added_files_from_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a commit that adds files
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("added1.txt"), "content")?;
            std::fs::write(dir.path().join("added2.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("added1.txt"))?;
            index.add_path(Path::new("added2.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Add files", &tree, &[&parent])?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"added1.txt".to_string()));
        assert!(changes.files_added.contains(&"added2.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_tracks_modified_files_from_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // First, create and commit a file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "original")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Initial file", &tree, &[&parent])?;
        }

        // Capture commit after initial file
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Now modify and commit the file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_modify.txt"), "modified content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_modify.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Modify file", &tree, &[&parent])?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes
            .files_modified
            .contains(&"to_modify.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_tracks_deleted_files_from_commits() -> Result<()> {
        let dir = init_test_repo()?;

        // First, create and commit a file
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("to_delete.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("to_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add file to delete",
                &tree,
                &[&parent],
            )?;
        }

        // Capture commit after initial file
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Now delete and commit
        {
            let repo = Repository::open(dir.path())?;
            std::fs::remove_file(dir.path().join("to_delete.txt"))?;
            let mut index = repo.index()?;
            index.remove_path(Path::new("to_delete.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(Some("HEAD"), &sig, &sig, "Delete file", &tree, &[&parent])?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_deleted.contains(&"to_delete.txt".to_string()));

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_deduplicates_files() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a file, stage it (appears in index), then commit it
        // This will cause the file to appear in both uncommitted and committed changes
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("dup_test.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("dup_test.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Commit for dedup test",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        // File should appear only once in files_added (deduplication working)
        let count = changes
            .files_added
            .iter()
            .filter(|f| *f == "dup_test.txt")
            .count();
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_sorts_file_lists() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create files in non-alphabetical order
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("zebra.txt"), "z")?;
            std::fs::write(dir.path().join("apple.txt"), "a")?;
            std::fs::write(dir.path().join("middle.txt"), "m")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("zebra.txt"))?;
            index.add_path(Path::new("apple.txt"))?;
            index.add_path(Path::new("middle.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add unsorted files",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        // Verify files are sorted
        let mut sorted = changes.files_added.clone();
        sorted.sort();
        assert_eq!(changes.files_added, sorted);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_calculates_insertions_deletions() -> Result<()> {
        let dir = init_test_repo()?;

        // First, create a file with some content
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(dir.path().join("changes.txt"), "line1\nline2\nline3\n")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("changes.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Initial content",
                &tree,
                &[&parent],
            )?;
        }

        // Capture commit after initial file
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Now modify the file: remove 1 line, add 2 lines
        {
            let repo = Repository::open(dir.path())?;
            std::fs::write(
                dir.path().join("changes.txt"),
                "line2\nline3\nnew line 1\nnew line 2\n",
            )?;
            let mut index = repo.index()?;
            index.add_path(Path::new("changes.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Modify content",
                &tree,
                &[&parent],
            )?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.insertions > 0);
        assert!(changes.deletions > 0);

        Ok(())
    }

    #[test]
    fn test_calculate_step_changes_handles_mixed_commit_and_uncommitted() -> Result<()> {
        let dir = init_test_repo()?;

        // Capture starting commit
        let mut tracker = GitChangeTracker::new(dir.path())?;
        let start_commit = tracker.workflow_start_commit.clone();

        // Create a committed file and an uncommitted file
        {
            let repo = Repository::open(dir.path())?;

            // Committed file
            std::fs::write(dir.path().join("committed.txt"), "content")?;
            let mut index = repo.index()?;
            index.add_path(Path::new("committed.txt"))?;
            index.write()?;

            let sig = git2::Signature::now("Test", "test@example.com")?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &sig,
                &sig,
                "Add committed file",
                &tree,
                &[&parent],
            )?;

            // Uncommitted file
            std::fs::write(dir.path().join("uncommitted.txt"), "content")?;
        }

        // Update tracker's last_commit
        tracker.last_commit = start_commit;

        let changes = tracker.calculate_step_changes()?;
        assert!(changes.files_added.contains(&"committed.txt".to_string()));
        assert!(changes.files_added.contains(&"uncommitted.txt".to_string()));
        assert_eq!(changes.commits.len(), 1);

        Ok(())
    }
}
