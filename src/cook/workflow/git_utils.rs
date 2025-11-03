//! Pure utility functions for git change tracking
//!
//! This module contains stateless, side-effect-free functions for:
//! - File status classification
//! - List operations (deduplication, normalization)
//! - Git delta processing
//!
//! These functions have no dependencies on git2 I/O operations and are
//! independently testable.

use git2::Status;

use super::git_context::StepChanges;

/// Type of file change detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileChangeType {
    Added,
    Modified,
    Deleted,
    Unknown,
}

/// Classify a git status into a file change type
pub(crate) fn classify_file_status(status: Status) -> FileChangeType {
    if should_track_as_added(status) {
        FileChangeType::Added
    } else if should_track_as_modified(status) {
        FileChangeType::Modified
    } else if should_track_as_deleted(status) {
        FileChangeType::Deleted
    } else {
        FileChangeType::Unknown
    }
}

/// Check if a status should be tracked as an addition
pub(crate) fn should_track_as_added(status: Status) -> bool {
    status.contains(Status::WT_NEW) || status.contains(Status::INDEX_NEW)
}

/// Check if a status should be tracked as a modification
pub(crate) fn should_track_as_modified(status: Status) -> bool {
    status.contains(Status::WT_MODIFIED) || status.contains(Status::INDEX_MODIFIED)
}

/// Check if a status should be tracked as a deletion
pub(crate) fn should_track_as_deleted(status: Status) -> bool {
    status.contains(Status::WT_DELETED) || status.contains(Status::INDEX_DELETED)
}

/// Classify a git delta status into a file change type
pub(crate) fn classify_delta_status(delta: git2::Delta) -> FileChangeType {
    match delta {
        git2::Delta::Added => FileChangeType::Added,
        git2::Delta::Modified => FileChangeType::Modified,
        git2::Delta::Deleted => FileChangeType::Deleted,
        _ => FileChangeType::Unknown,
    }
}

/// Extract file path from a diff delta
pub(crate) fn extract_file_path(delta: &git2::DiffDelta) -> Option<String> {
    delta
        .new_file()
        .path()
        .map(|p| p.to_string_lossy().to_string())
}

/// Check if a path should be added to the list (i.e., not already present)
pub(crate) fn should_add_to_list(list: &[String], path: &str) -> bool {
    !list.contains(&path.to_string())
}

/// Add a file path to the list if it's not already present
pub(crate) fn add_unique_file(list: &mut Vec<String>, path: String) {
    if should_add_to_list(list, &path) {
        list.push(path);
    }
}

/// Sort and deduplicate a file list
pub(crate) fn normalize_file_list(list: &mut Vec<String>) {
    list.sort();
    list.dedup();
}

/// Normalize all file lists in step changes (sort and deduplicate)
pub(crate) fn normalize_file_lists(changes: &mut StepChanges) {
    normalize_file_list(&mut changes.files_added);
    normalize_file_list(&mut changes.files_modified);
    normalize_file_list(&mut changes.files_deleted);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 4 Tests: Pure Function Tests for Status Detection

    #[test]
    fn test_should_track_as_added_with_wt_new() {
        assert!(should_track_as_added(Status::WT_NEW));
    }

    #[test]
    fn test_should_track_as_added_with_index_new() {
        assert!(should_track_as_added(Status::INDEX_NEW));
    }

    #[test]
    fn test_should_track_as_added_with_combined_new() {
        let status = Status::WT_NEW | Status::INDEX_NEW;
        assert!(should_track_as_added(status));
    }

    #[test]
    fn test_should_track_as_modified_with_wt_modified() {
        assert!(should_track_as_modified(Status::WT_MODIFIED));
    }

    #[test]
    fn test_should_track_as_modified_with_index_modified() {
        assert!(should_track_as_modified(Status::INDEX_MODIFIED));
    }

    #[test]
    fn test_should_track_as_modified_with_combined_modified() {
        let status = Status::WT_MODIFIED | Status::INDEX_MODIFIED;
        assert!(should_track_as_modified(status));
    }

    #[test]
    fn test_should_track_as_deleted_with_wt_deleted() {
        assert!(should_track_as_deleted(Status::WT_DELETED));
    }

    #[test]
    fn test_should_track_as_deleted_with_index_deleted() {
        assert!(should_track_as_deleted(Status::INDEX_DELETED));
    }

    #[test]
    fn test_should_track_as_deleted_with_combined_deleted() {
        let status = Status::WT_DELETED | Status::INDEX_DELETED;
        assert!(should_track_as_deleted(status));
    }

    #[test]
    fn test_classify_file_status_added() {
        assert_eq!(classify_file_status(Status::WT_NEW), FileChangeType::Added);
        assert_eq!(
            classify_file_status(Status::INDEX_NEW),
            FileChangeType::Added
        );
    }

    #[test]
    fn test_classify_file_status_modified() {
        assert_eq!(
            classify_file_status(Status::WT_MODIFIED),
            FileChangeType::Modified
        );
        assert_eq!(
            classify_file_status(Status::INDEX_MODIFIED),
            FileChangeType::Modified
        );
    }

    #[test]
    fn test_classify_file_status_deleted() {
        assert_eq!(
            classify_file_status(Status::WT_DELETED),
            FileChangeType::Deleted
        );
        assert_eq!(
            classify_file_status(Status::INDEX_DELETED),
            FileChangeType::Deleted
        );
    }

    // Phase 5 Tests: Pure Function Tests for Diff Processing

    #[test]
    fn test_classify_delta_status_added() {
        assert_eq!(
            classify_delta_status(git2::Delta::Added),
            FileChangeType::Added
        );
    }

    #[test]
    fn test_classify_delta_status_modified() {
        assert_eq!(
            classify_delta_status(git2::Delta::Modified),
            FileChangeType::Modified
        );
    }

    #[test]
    fn test_classify_delta_status_deleted() {
        assert_eq!(
            classify_delta_status(git2::Delta::Deleted),
            FileChangeType::Deleted
        );
    }

    #[test]
    fn test_classify_delta_status_unknown() {
        assert_eq!(
            classify_delta_status(git2::Delta::Unmodified),
            FileChangeType::Unknown
        );
        assert_eq!(
            classify_delta_status(git2::Delta::Renamed),
            FileChangeType::Unknown
        );
        assert_eq!(
            classify_delta_status(git2::Delta::Copied),
            FileChangeType::Unknown
        );
    }

    #[test]
    fn test_should_add_to_list_empty() {
        let list: Vec<String> = vec![];
        assert!(should_add_to_list(&list, "test.txt"));
    }

    #[test]
    fn test_should_add_to_list_not_present() {
        let list = vec!["file1.txt".to_string(), "file2.txt".to_string()];
        assert!(should_add_to_list(&list, "file3.txt"));
    }

    #[test]
    fn test_should_add_to_list_already_present() {
        let list = vec!["file1.txt".to_string(), "file2.txt".to_string()];
        assert!(!should_add_to_list(&list, "file1.txt"));
    }

    #[test]
    fn test_add_unique_file_to_empty_list() {
        let mut list = vec![];
        add_unique_file(&mut list, "test.txt".to_string());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0], "test.txt");
    }

    #[test]
    fn test_add_unique_file_new_file() {
        let mut list = vec!["file1.txt".to_string()];
        add_unique_file(&mut list, "file2.txt".to_string());
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"file2.txt".to_string()));
    }

    #[test]
    fn test_add_unique_file_duplicate() {
        let mut list = vec!["file1.txt".to_string()];
        add_unique_file(&mut list, "file1.txt".to_string());
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_add_unique_file_multiple_duplicates() {
        let mut list = vec!["file1.txt".to_string()];
        add_unique_file(&mut list, "file1.txt".to_string());
        add_unique_file(&mut list, "file2.txt".to_string());
        add_unique_file(&mut list, "file1.txt".to_string());
        assert_eq!(list.len(), 2);
    }

    // Phase 6 Tests: Pure Function Tests for Normalization

    #[test]
    fn test_normalize_file_list_empty() {
        let mut list: Vec<String> = vec![];
        normalize_file_list(&mut list);
        assert!(list.is_empty());
    }

    #[test]
    fn test_normalize_file_list_sorts() {
        let mut list = vec![
            "zebra.txt".to_string(),
            "apple.txt".to_string(),
            "middle.txt".to_string(),
        ];
        normalize_file_list(&mut list);
        assert_eq!(list[0], "apple.txt");
        assert_eq!(list[1], "middle.txt");
        assert_eq!(list[2], "zebra.txt");
    }

    #[test]
    fn test_normalize_file_list_deduplicates() {
        let mut list = vec![
            "file1.txt".to_string(),
            "file2.txt".to_string(),
            "file1.txt".to_string(),
        ];
        normalize_file_list(&mut list);
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"file1.txt".to_string()));
        assert!(list.contains(&"file2.txt".to_string()));
    }

    #[test]
    fn test_normalize_file_list_sorts_and_deduplicates() {
        let mut list = vec![
            "zebra.txt".to_string(),
            "apple.txt".to_string(),
            "zebra.txt".to_string(),
            "middle.txt".to_string(),
            "apple.txt".to_string(),
        ];
        normalize_file_list(&mut list);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "apple.txt");
        assert_eq!(list[1], "middle.txt");
        assert_eq!(list[2], "zebra.txt");
    }

    #[test]
    fn test_normalize_file_lists_normalizes_all() {
        let mut changes = StepChanges {
            files_added: vec![
                "z.txt".to_string(),
                "a.txt".to_string(),
                "a.txt".to_string(),
            ],
            files_modified: vec![
                "y.txt".to_string(),
                "b.txt".to_string(),
                "b.txt".to_string(),
            ],
            files_deleted: vec![
                "x.txt".to_string(),
                "c.txt".to_string(),
                "c.txt".to_string(),
            ],
            ..Default::default()
        };

        normalize_file_lists(&mut changes);

        // Check all lists are sorted
        assert_eq!(changes.files_added[0], "a.txt");
        assert_eq!(changes.files_added[1], "z.txt");
        assert_eq!(changes.files_modified[0], "b.txt");
        assert_eq!(changes.files_modified[1], "y.txt");
        assert_eq!(changes.files_deleted[0], "c.txt");
        assert_eq!(changes.files_deleted[1], "x.txt");

        // Check all lists are deduplicated
        assert_eq!(changes.files_added.len(), 2);
        assert_eq!(changes.files_modified.len(), 2);
        assert_eq!(changes.files_deleted.len(), 2);
    }

    #[test]
    fn test_normalize_file_lists_handles_empty() {
        let mut changes = StepChanges::default();
        normalize_file_lists(&mut changes);
        assert!(changes.files_added.is_empty());
        assert!(changes.files_modified.is_empty());
        assert!(changes.files_deleted.is_empty());
    }
}
