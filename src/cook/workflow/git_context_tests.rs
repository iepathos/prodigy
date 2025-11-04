//! Basic unit tests for git context tracking
//!
//! This module contains basic unit tests for StepChanges struct and GitChangeTracker.
//! Additional comprehensive tests are organized in separate modules:
//!
//! - `git_context_uncommitted_tests` - Phase 1: Uncommitted Changes Detection
//! - `git_context_commit_tests` - Phase 2: Commit History Walking
//! - `git_context_diff_tests` - Phase 3: Diff Statistics and File Changes
//! - `git_context_test_utils` - Shared test utilities

#[cfg(test)]
mod tests {
    use crate::cook::workflow::git_context::*;
    use crate::cook::workflow::git_context_test_utils::init_test_repo;
    use anyhow::Result;
    use tempfile::TempDir;

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

}
