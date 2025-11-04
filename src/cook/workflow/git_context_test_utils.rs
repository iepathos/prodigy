//! Shared test utilities for git context tests

use anyhow::Result;
use git2::Repository;
use tempfile::TempDir;

/// Initialize a test repository with an initial commit
pub fn init_test_repo() -> Result<TempDir> {
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
