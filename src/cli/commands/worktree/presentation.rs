//! Presentation layer for worktree commands
//!
//! This module contains pure functions that format data for display.
//! All functions are pure: they take data and return formatted strings
//! without performing I/O directly.

use crate::worktree::WorktreeSession;

use super::operations::{BatchMergeResult, MergeResult};

/// Format a list of sessions as a table
#[allow(dead_code)] // Used in Phase 5
pub fn format_sessions_table(sessions: &[WorktreeSession]) -> String {
    if sessions.is_empty() {
        return "No active Prodigy worktrees found.".to_string();
    }

    let mut output = String::new();
    output.push_str("Active Prodigy worktrees:\n");
    output.push_str(&format!(
        "{:<40} {:<30} {:<20}\n",
        "Name", "Branch", "Created"
    ));
    output.push_str(&format!("{}\n", "-".repeat(90)));

    for session in sessions {
        output.push_str(&format!(
            "{:<40} {:<30} {:<20}\n",
            session.name,
            session.branch,
            session.created_at.format("%Y-%m-%d %H:%M:%S")
        ));
    }

    output
}

/// Format a merge result message
#[allow(dead_code)] // Used in Phase 5
pub fn format_merge_result(result: &MergeResult) -> String {
    if result.success {
        format!("✅ Successfully merged worktree '{}'", result.session_name)
    } else {
        format!(
            "❌ Failed to merge worktree '{}': {}",
            result.session_name,
            result
                .error
                .as_ref()
                .unwrap_or(&"Unknown error".to_string())
        )
    }
}

/// Format batch merge summary
#[allow(dead_code)] // Used in Phase 5
pub fn format_batch_merge_summary(result: &BatchMergeResult) -> String {
    if result.merged_count > 0 {
        format!("Successfully merged {} worktree(s)", result.merged_count)
    } else {
        "No worktrees were merged.".to_string()
    }
}

/// Format cleanup summary message
#[allow(dead_code)] // Used in Phase 5
pub fn format_cleanup_summary(cleaned_count: usize, dry_run: bool) -> String {
    if dry_run {
        format!("DRY RUN: Would clean {} worktree(s)", cleaned_count)
    } else {
        format!("Cleaned {} old worktrees", cleaned_count)
    }
}

/// Format session removal message
#[allow(dead_code)] // Used in Phase 5
pub fn format_session_removal(session_name: &str, dry_run: bool) -> String {
    if dry_run {
        format!("DRY RUN: Would remove worktree: {}", session_name)
    } else {
        format!("Removing worktree: {}", session_name)
    }
}

/// Format old worktree removal message
#[allow(dead_code)] // Used in Phase 5
pub fn format_old_worktree_removal(session_name: &str, age_hours: i64, dry_run: bool) -> String {
    if dry_run {
        format!(
            "DRY RUN: Would remove worktree '{}' (age: {} hours)",
            session_name, age_hours
        )
    } else {
        format!(
            "Removing old worktree '{}' (age: {} hours)",
            session_name, age_hours
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktree::WorktreeSession;
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn test_format_sessions_table_empty() {
        let sessions = vec![];
        let output = format_sessions_table(&sessions);
        assert_eq!(output, "No active Prodigy worktrees found.");
    }

    #[test]
    fn test_format_sessions_table_single() {
        let sessions = vec![WorktreeSession {
            name: "session-1".to_string(),
            branch: "main".to_string(),
            created_at: Utc::now(),
            path: PathBuf::from("/tmp/session-1"),
        }];

        let output = format_sessions_table(&sessions);
        assert!(output.contains("Active Prodigy worktrees"));
        assert!(output.contains("session-1"));
        assert!(output.contains("main"));
    }

    #[test]
    fn test_format_sessions_table_multiple() {
        let sessions = vec![
            WorktreeSession {
                name: "session-1".to_string(),
                branch: "main".to_string(),
                created_at: Utc::now(),
                path: PathBuf::from("/tmp/session-1"),
            },
            WorktreeSession {
                name: "session-2".to_string(),
                branch: "feature".to_string(),
                created_at: Utc::now(),
                path: PathBuf::from("/tmp/session-2"),
            },
        ];

        let output = format_sessions_table(&sessions);
        assert!(output.contains("session-1"));
        assert!(output.contains("session-2"));
        assert!(output.contains("main"));
        assert!(output.contains("feature"));
    }

    #[test]
    fn test_format_merge_result_success() {
        let result = MergeResult {
            session_name: "test-session".to_string(),
            success: true,
            error: None,
        };

        let output = format_merge_result(&result);
        assert!(output.contains("✅"));
        assert!(output.contains("Successfully merged"));
        assert!(output.contains("test-session"));
    }

    #[test]
    fn test_format_merge_result_failure() {
        let result = MergeResult {
            session_name: "test-session".to_string(),
            success: false,
            error: Some("merge conflict".to_string()),
        };

        let output = format_merge_result(&result);
        assert!(output.contains("❌"));
        assert!(output.contains("Failed to merge"));
        assert!(output.contains("test-session"));
        assert!(output.contains("merge conflict"));
    }

    #[test]
    fn test_format_batch_merge_summary_success() {
        use super::super::operations::BatchMergeResult;

        let result = BatchMergeResult {
            results: vec![],
            merged_count: 3,
            failed_count: 0,
        };

        let output = format_batch_merge_summary(&result);
        assert!(output.contains("Successfully merged 3"));
    }

    #[test]
    fn test_format_batch_merge_summary_none() {
        use super::super::operations::BatchMergeResult;

        let result = BatchMergeResult {
            results: vec![],
            merged_count: 0,
            failed_count: 2,
        };

        let output = format_batch_merge_summary(&result);
        assert!(output.contains("No worktrees were merged"));
    }

    #[test]
    fn test_format_cleanup_summary_dry_run() {
        let output = format_cleanup_summary(5, true);
        assert!(output.contains("DRY RUN"));
        assert!(output.contains("5"));
    }

    #[test]
    fn test_format_cleanup_summary_actual() {
        let output = format_cleanup_summary(3, false);
        assert!(output.contains("Cleaned 3"));
        assert!(!output.contains("DRY RUN"));
    }

    #[test]
    fn test_format_session_removal_dry_run() {
        let output = format_session_removal("my-session", true);
        assert!(output.contains("DRY RUN"));
        assert!(output.contains("my-session"));
    }

    #[test]
    fn test_format_session_removal_actual() {
        let output = format_session_removal("my-session", false);
        assert!(output.contains("Removing worktree"));
        assert!(output.contains("my-session"));
        assert!(!output.contains("DRY RUN"));
    }

    #[test]
    fn test_format_old_worktree_removal_dry_run() {
        let output = format_old_worktree_removal("old-session", 48, true);
        assert!(output.contains("DRY RUN"));
        assert!(output.contains("old-session"));
        assert!(output.contains("48 hours"));
    }

    #[test]
    fn test_format_old_worktree_removal_actual() {
        let output = format_old_worktree_removal("old-session", 24, false);
        assert!(output.contains("Removing old worktree"));
        assert!(output.contains("old-session"));
        assert!(output.contains("24 hours"));
        assert!(!output.contains("DRY RUN"));
    }
}
