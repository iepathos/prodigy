//! Phase 1 Tests: Uncommitted Changes Detection
//!
//! Tests for detecting uncommitted changes in git repositories,
//! including new files, modified files, deleted files, and staged changes.

#[cfg(test)]
mod tests {
    use crate::cook::workflow::git_context::*;
    use crate::cook::workflow::git_context_test_utils::init_test_repo;
    use anyhow::Result;
    use git2::Repository;
    use std::path::Path;

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
}
