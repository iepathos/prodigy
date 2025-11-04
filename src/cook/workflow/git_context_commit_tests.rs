//! Phase 2 Tests: Commit History Walking
//!
//! Tests for tracking commit history and changes across multiple commits,
//! including commit counts, file tracking, and diff statistics.

#[cfg(test)]
mod tests {
    use crate::cook::workflow::git_context::*;
    use crate::cook::workflow::git_context_test_utils::init_test_repo;
    use anyhow::Result;
    use git2::Repository;
    use std::path::Path;

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
}
