//! Phase 3 Tests: Diff Statistics and File Changes
//!
//! Tests for diff statistics, file change tracking, deduplication,
//! sorting, and insertions/deletions calculations.

#[cfg(test)]
mod tests {
    use crate::cook::workflow::git_context::*;
    use crate::cook::workflow::git_context_test_utils::init_test_repo;
    use anyhow::Result;
    use git2::Repository;
    use std::path::Path;

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
