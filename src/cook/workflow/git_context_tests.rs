//! Tests for git context tracking

#[cfg(test)]
mod tests {
    use super::super::git_context::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_test_repo() -> anyhow::Result<TempDir> {
        let dir = TempDir::new()?;
        {
            let repo = git2::Repository::init(dir.path())?;

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
    fn test_tracker_in_git_repo() {
        let dir = init_test_repo().unwrap();
        let tracker = GitChangeTracker::new(dir.path()).unwrap();
        assert!(tracker.is_active());
    }

    #[test]
    fn test_tracker_not_in_git_repo() {
        let dir = TempDir::new().unwrap();
        let tracker = GitChangeTracker::new(dir.path()).unwrap();
        assert!(!tracker.is_active());
    }

    #[test]
    fn test_step_tracking() {
        let dir = init_test_repo().unwrap();
        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();

        // Begin tracking a step
        tracker.begin_step("step_1").unwrap();

        // Create a new file
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        // Add file to git
        {
            let repo = git2::Repository::open(dir.path()).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("test.txt")).unwrap();
            index.write().unwrap();
        }

        // Complete the step
        let changes = tracker.complete_step().unwrap();

        // Check that the file was tracked as added
        assert!(changes.files_added.contains(&"test.txt".to_string()));
    }

    #[test]
    fn test_workflow_changes_aggregation() {
        let dir = init_test_repo().unwrap();
        let mut tracker = GitChangeTracker::new(dir.path()).unwrap();

        // Track first step
        tracker.begin_step("step_1").unwrap();
        fs::write(dir.path().join("file1.txt"), "content1").unwrap();
        {
            let repo = git2::Repository::open(dir.path()).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("file1.txt")).unwrap();
            index.write().unwrap();
        }
        tracker.complete_step().unwrap();

        // Track second step
        tracker.begin_step("step_2").unwrap();
        fs::write(dir.path().join("file2.txt"), "content2").unwrap();
        {
            let repo = git2::Repository::open(dir.path()).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("file2.txt")).unwrap();
            index.write().unwrap();
        }
        tracker.complete_step().unwrap();

        // Get workflow changes
        let workflow_changes = tracker.get_workflow_changes();

        // Both files should be in workflow changes
        assert_eq!(workflow_changes.files_added.len(), 2);
        assert!(workflow_changes
            .files_added
            .contains(&"file1.txt".to_string()));
        assert!(workflow_changes
            .files_added
            .contains(&"file2.txt".to_string()));
    }

    #[test]
    fn test_variable_resolution() {
        let dir = init_test_repo().unwrap();
        let tracker = GitChangeTracker::new(dir.path()).unwrap();

        // Test various variable paths
        let result = tracker.resolve_variable("step.files_added");
        assert!(result.is_ok());

        let result = tracker.resolve_variable("workflow.commit_count");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "0"); // No commits yet
    }

    #[test]
    fn test_file_filtering() {
        let changes = StepChanges {
            files_added: vec![
                "src/main.rs".to_string(),
                "src/lib.rs".to_string(),
                "README.md".to_string(),
                "docs/guide.md".to_string(),
            ],
            ..Default::default()
        };

        let filtered = changes.filter_files(&changes.files_added, "*.md");
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains(&"README.md".to_string()));
        assert!(filtered.contains(&"docs/guide.md".to_string()));
    }

    #[test]
    fn test_variable_formatting() {
        let files = vec!["a.txt".to_string(), "b.txt".to_string()];

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::SpaceSeparated),
            "a.txt b.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::NewlineSeparated),
            "a.txt\nb.txt"
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::JsonArray),
            r#"["a.txt","b.txt"]"#
        );

        assert_eq!(
            GitChangeTracker::format_file_list(&files, VariableFormat::CommaSeparated),
            "a.txt,b.txt"
        );
    }
}
