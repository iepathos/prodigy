//! Business logic for worktree operations
//!
//! This module contains pure business logic functions that orchestrate
//! worktree operations. These functions take dependencies as parameters
//! and return structured results without performing I/O directly.

use crate::worktree::manager::WorktreeManager;
use crate::worktree::WorktreeSession;
use anyhow::Result;

/// Result of a session listing operation
#[derive(Debug, Clone)]
pub struct SessionListResult {
    pub sessions: Vec<WorktreeSession>,
}

/// Result of a single merge operation
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub session_name: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Result of a batch merge operation
#[derive(Debug, Clone)]
pub struct BatchMergeResult {
    pub results: Vec<MergeResult>,
    pub merged_count: usize,
    #[allow(dead_code)] // May be used in future phases
    pub failed_count: usize,
}

/// List all active worktree sessions
///
/// This is a pure orchestration function that wraps the manager's list_sessions
/// and returns a structured result.
pub async fn list_sessions_operation(manager: &WorktreeManager) -> Result<SessionListResult> {
    let sessions = manager.list_sessions().await?;
    Ok(SessionListResult { sessions })
}

/// Merge a single worktree session
///
/// Returns a MergeResult indicating success or failure.
pub async fn merge_session_operation(
    manager: &WorktreeManager,
    session_name: &str,
) -> MergeResult {
    match manager.merge_session(session_name).await {
        Ok(_) => MergeResult {
            session_name: session_name.to_string(),
            success: true,
            error: None,
        },
        Err(e) => MergeResult {
            session_name: session_name.to_string(),
            success: false,
            error: Some(e.to_string()),
        },
    }
}

/// Merge all active worktree sessions
///
/// Processes each session and returns aggregated results.
pub async fn merge_all_sessions_operation(manager: &WorktreeManager) -> Result<BatchMergeResult> {
    let sessions = manager.list_sessions().await?;
    let mut results = Vec::new();

    for session in sessions {
        let result = merge_session_operation(manager, &session.name).await;
        results.push(result);
    }

    let merged_count = results.iter().filter(|r| r.success).count();
    let failed_count = results.len() - merged_count;

    Ok(BatchMergeResult {
        results,
        merged_count,
        failed_count,
    })
}

/// Clean up a single worktree session
///
/// Returns a result indicating success or failure.
#[allow(dead_code)] // Used in Phase 3
pub async fn cleanup_session_operation(
    manager: &WorktreeManager,
    session_name: &str,
    force: bool,
) -> Result<()> {
    manager.cleanup_session(session_name, force).await
}

/// Clean up all worktree sessions
///
/// Returns the number of sessions cleaned.
#[allow(dead_code)] // Used in Phase 3
pub async fn cleanup_all_sessions_operation(
    manager: &WorktreeManager,
    force: bool,
) -> Result<usize> {
    let sessions = manager.list_sessions().await?;
    let count = sessions.len();
    manager.cleanup_all_sessions(force).await?;
    Ok(count)
}

/// Filter sessions older than the specified duration
///
/// Pure function that filters sessions based on age.
pub fn filter_old_sessions(
    sessions: Vec<WorktreeSession>,
    max_age: std::time::Duration,
) -> Vec<WorktreeSession> {
    let now = chrono::Utc::now();
    sessions
        .into_iter()
        .filter(|session| {
            let age = now.signed_duration_since(session.created_at);
            age.num_seconds() as u64 > max_age.as_secs()
        })
        .collect()
}

/// Result of a cleanup operation
#[derive(Debug, Clone)]
pub struct CleanupResult {
    #[allow(dead_code)] // Used in Phase 5
    pub cleaned_count: usize,
    #[allow(dead_code)] // Used in Phase 5
    pub session_names: Vec<String>,
}

/// Clean up old worktree sessions
///
/// Returns the list of cleaned sessions.
#[allow(dead_code)] // Used in Phase 5
pub async fn cleanup_old_sessions_operation(
    manager: &WorktreeManager,
    max_age: std::time::Duration,
    force: bool,
) -> Result<CleanupResult> {
    let sessions = manager.list_sessions().await?;
    let old_sessions = filter_old_sessions(sessions, max_age);
    let session_names: Vec<String> = old_sessions.iter().map(|s| s.name.clone()).collect();

    for session in &old_sessions {
        manager.cleanup_session(&session.name, force).await?;
    }

    Ok(CleanupResult {
        cleaned_count: old_sessions.len(),
        session_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    // Note: Full integration tests would require mocking WorktreeManager
    // For now, we test the data structures and ensure the functions compile

    #[test]
    fn test_session_list_result_creation() {
        let result = SessionListResult {
            sessions: vec![],
        };
        assert_eq!(result.sessions.len(), 0);
    }

    #[test]
    fn test_merge_result_success() {
        let result = MergeResult {
            session_name: "test-session".to_string(),
            success: true,
            error: None,
        };
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_merge_result_failure() {
        let result = MergeResult {
            session_name: "test-session".to_string(),
            success: false,
            error: Some("merge failed".to_string()),
        };
        assert!(!result.success);
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap(), "merge failed");
    }

    #[test]
    fn test_batch_merge_result_aggregation() {
        let results = vec![
            MergeResult {
                session_name: "session1".to_string(),
                success: true,
                error: None,
            },
            MergeResult {
                session_name: "session2".to_string(),
                success: false,
                error: Some("error".to_string()),
            },
            MergeResult {
                session_name: "session3".to_string(),
                success: true,
                error: None,
            },
        ];

        let batch_result = BatchMergeResult {
            results: results.clone(),
            merged_count: 2,
            failed_count: 1,
        };

        assert_eq!(batch_result.merged_count, 2);
        assert_eq!(batch_result.failed_count, 1);
        assert_eq!(batch_result.results.len(), 3);
    }

    #[test]
    fn test_filter_old_sessions_empty() {
        let sessions = vec![];
        let max_age = std::time::Duration::from_secs(3600); // 1 hour
        let filtered = filter_old_sessions(sessions, max_age);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_old_sessions_all_new() {
        use crate::worktree::WorktreeSession;

        let sessions = vec![
            WorktreeSession {
                name: "session1".to_string(),
                branch: "branch1".to_string(),
                created_at: Utc::now(),
                path: std::path::PathBuf::from("/tmp/session1"),
            },
            WorktreeSession {
                name: "session2".to_string(),
                branch: "branch2".to_string(),
                created_at: Utc::now(),
                path: std::path::PathBuf::from("/tmp/session2"),
            },
        ];

        let max_age = std::time::Duration::from_secs(3600); // 1 hour
        let filtered = filter_old_sessions(sessions, max_age);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_old_sessions_all_old() {
        use crate::worktree::WorktreeSession;

        let old_time = Utc::now() - Duration::hours(2);
        let sessions = vec![
            WorktreeSession {
                name: "session1".to_string(),
                branch: "branch1".to_string(),
                created_at: old_time,
                path: std::path::PathBuf::from("/tmp/session1"),
            },
            WorktreeSession {
                name: "session2".to_string(),
                branch: "branch2".to_string(),
                created_at: old_time,
                path: std::path::PathBuf::from("/tmp/session2"),
            },
        ];

        let max_age = std::time::Duration::from_secs(3600); // 1 hour
        let filtered = filter_old_sessions(sessions, max_age);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_old_sessions_mixed() {
        use crate::worktree::WorktreeSession;

        let old_time = Utc::now() - Duration::hours(2);
        let new_time = Utc::now();
        let sessions = vec![
            WorktreeSession {
                name: "old_session".to_string(),
                branch: "branch1".to_string(),
                created_at: old_time,
                path: std::path::PathBuf::from("/tmp/old"),
            },
            WorktreeSession {
                name: "new_session".to_string(),
                branch: "branch2".to_string(),
                created_at: new_time,
                path: std::path::PathBuf::from("/tmp/new"),
            },
        ];

        let max_age = std::time::Duration::from_secs(3600); // 1 hour
        let filtered = filter_old_sessions(sessions, max_age);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "old_session");
    }

    #[test]
    fn test_cleanup_result_creation() {
        let result = CleanupResult {
            cleaned_count: 3,
            session_names: vec![
                "session1".to_string(),
                "session2".to_string(),
                "session3".to_string(),
            ],
        };
        assert_eq!(result.cleaned_count, 3);
        assert_eq!(result.session_names.len(), 3);
    }
}
