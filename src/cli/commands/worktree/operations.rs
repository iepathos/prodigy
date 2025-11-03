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

#[cfg(test)]
mod tests {
    use super::*;

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
}
