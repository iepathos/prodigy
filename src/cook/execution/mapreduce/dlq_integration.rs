//! DLQ Integration for MapReduce Agent Failures
//!
//! This module provides pure functions to convert AgentResult failures into
//! DeadLetteredItems for the Dead Letter Queue (DLQ). All functions are pure
//! (no I/O) and fully testable.

use crate::cook::execution::dlq::{DeadLetteredItem, ErrorType, FailureDetail, WorktreeArtifacts};
use crate::cook::execution::mapreduce::agent::types::{AgentResult, AgentStatus};
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Convert an AgentResult to a DeadLetteredItem for DLQ insertion
///
/// Returns Some(DeadLetteredItem) for failed/timeout agents, None for successful agents.
///
/// # Arguments
///
/// * `result` - The agent execution result
/// * `work_item` - The original work item data
/// * `attempt_number` - The attempt number for this execution
///
/// # Example
///
/// ```
/// use prodigy::cook::execution::mapreduce::dlq_integration::agent_result_to_dlq_item;
/// use prodigy::cook::execution::mapreduce::agent::types::{AgentResult, AgentStatus};
/// use serde_json::json;
/// use std::time::Duration;
///
/// let result = AgentResult {
///     item_id: "item-1".to_string(),
///     status: AgentStatus::Failed("test error".to_string()),
///     output: None,
///     commits: vec![],
///     files_modified: vec![],
///     duration: Duration::from_secs(10),
///     error: Some("test error".to_string()),
///     worktree_path: None,
///     branch_name: None,
///     worktree_session_id: None,
///     json_log_location: Some("/path/to/log.json".to_string()),
///     cleanup_status: None,
/// };
/// let work_item = json!({"id": 1, "data": "test"});
///
/// let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);
/// assert!(dlq_item.is_some());
/// ```
pub fn agent_result_to_dlq_item(
    result: &AgentResult,
    work_item: &Value,
    attempt_number: u32,
) -> Option<DeadLetteredItem> {
    match &result.status {
        AgentStatus::Failed(error_msg) => Some(create_dlq_item_from_failure(
            result,
            work_item,
            error_msg,
            attempt_number,
        )),
        AgentStatus::Timeout => {
            let error_msg = "Agent execution timed out";
            Some(create_dlq_item_from_failure(
                result,
                work_item,
                error_msg,
                attempt_number,
            ))
        }
        _ => None, // Success, Running, Pending, Retrying - don't add to DLQ
    }
}

/// Create a DeadLetteredItem from a failed AgentResult
fn create_dlq_item_from_failure(
    result: &AgentResult,
    work_item: &Value,
    error_msg: &str,
    attempt_number: u32,
) -> DeadLetteredItem {
    DeadLetteredItem {
        item_id: result.item_id.clone(),
        item_data: work_item.clone(),
        first_attempt: Utc::now(),
        last_attempt: Utc::now(),
        failure_count: 1,
        failure_history: vec![create_failure_detail(result, error_msg, attempt_number)],
        error_signature: create_error_signature(error_msg),
        worktree_artifacts: extract_worktree_artifacts(result),
        reprocess_eligible: is_reprocessable(&result.status, error_msg),
        manual_review_required: requires_manual_review(error_msg),
    }
}

/// Create a FailureDetail from an AgentResult
fn create_failure_detail(result: &AgentResult, error_msg: &str, attempt: u32) -> FailureDetail {
    FailureDetail {
        attempt_number: attempt,
        timestamp: Utc::now(),
        error_type: classify_error(&result.status, error_msg),
        error_message: error_msg.to_string(),
        stack_trace: result.error.clone(),
        agent_id: format!("agent-{}", result.item_id),
        step_failed: "agent_execution".to_string(),
        duration_ms: result.duration.as_millis() as u64,
        json_log_location: result.json_log_location.clone(),
    }
}

/// Create a consistent error signature for pattern analysis
///
/// Uses SHA256 hash of error message, truncated to 16 characters for brevity
fn create_error_signature(error_msg: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(error_msg.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}", hash)[..16].to_string()
}

/// Classify error type based on status and message content
fn classify_error(status: &AgentStatus, error_msg: &str) -> ErrorType {
    match status {
        AgentStatus::Timeout => ErrorType::Timeout,
        AgentStatus::Failed(_) => {
            let msg_lower = error_msg.to_lowercase();
            if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
                ErrorType::Timeout
            } else if msg_lower.contains("merge") || msg_lower.contains("conflict") {
                ErrorType::MergeConflict
            } else if msg_lower.contains("worktree") {
                ErrorType::WorktreeError
            } else if msg_lower.contains("validation") || msg_lower.contains("invalid") {
                ErrorType::ValidationFailed
            } else if msg_lower.contains("resource") || msg_lower.contains("out of memory") {
                ErrorType::ResourceExhausted
            } else {
                // Extract exit code if present in error message
                if let Some(exit_code) = extract_exit_code(error_msg) {
                    ErrorType::CommandFailed { exit_code }
                } else {
                    ErrorType::Unknown
                }
            }
        }
        _ => ErrorType::Unknown,
    }
}

/// Extract exit code from error message if present
fn extract_exit_code(error_msg: &str) -> Option<i32> {
    // Try to find patterns like "exit code: 1" or "exited with code 127"
    let patterns = [
        regex::Regex::new(r"exit code:?\s*(\d+)").ok()?,
        regex::Regex::new(r"exited with code\s*(\d+)").ok()?,
        regex::Regex::new(r"status code:?\s*(\d+)").ok()?,
    ];

    for pattern in &patterns {
        if let Some(captures) = pattern.captures(error_msg) {
            if let Some(code_str) = captures.get(1) {
                if let Ok(code) = code_str.as_str().parse::<i32>() {
                    return Some(code);
                }
            }
        }
    }

    None
}

/// Extract worktree artifacts from AgentResult
fn extract_worktree_artifacts(result: &AgentResult) -> Option<WorktreeArtifacts> {
    result.worktree_path.as_ref().map(|path| WorktreeArtifacts {
        worktree_path: path.clone(),
        branch_name: result.branch_name.clone().unwrap_or_default(),
        uncommitted_changes: None, // Would require filesystem I/O to detect
        error_logs: None,          // Would require filesystem I/O to read
    })
}

/// Determine if the failure is eligible for reprocessing
fn is_reprocessable(status: &AgentStatus, error_msg: &str) -> bool {
    match status {
        // Timeouts are generally reprocessable
        AgentStatus::Timeout => true,
        // Failed agents might be reprocessable depending on error type
        AgentStatus::Failed(_) => {
            let msg_lower = error_msg.to_lowercase();
            // Transient errors are reprocessable
            msg_lower.contains("timeout")
                || msg_lower.contains("network")
                || msg_lower.contains("temporary")
                || msg_lower.contains("rate limit")
                || msg_lower.contains("resource")
        }
        // Other statuses are not in DLQ
        _ => false,
    }
}

/// Determine if manual review is required
fn requires_manual_review(error_msg: &str) -> bool {
    let msg_lower = error_msg.to_lowercase();
    // Permission errors and critical failures need manual review
    msg_lower.contains("permission")
        || msg_lower.contains("access denied")
        || msg_lower.contains("critical")
        || msg_lower.contains("fatal")
        || msg_lower.contains("corrupted")
        || msg_lower.contains("validation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::types::CleanupStatus;
    use std::path::PathBuf;
    use std::time::Duration;

    fn create_failed_agent_result(error_msg: &str) -> AgentResult {
        AgentResult {
            item_id: "item-1".to_string(),
            status: AgentStatus::Failed(error_msg.to_string()),
            output: None,
            commits: vec![],
            files_modified: vec![],
            duration: Duration::from_secs(10),
            error: Some(error_msg.to_string()),
            worktree_path: Some(PathBuf::from("/tmp/worktree")),
            branch_name: Some("agent-branch".to_string()),
            worktree_session_id: Some("session-123".to_string()),
            json_log_location: Some("/path/to/log.json".to_string()),
            cleanup_status: Some(CleanupStatus::Success),
        }
    }

    fn create_timeout_agent_result() -> AgentResult {
        AgentResult {
            item_id: "item-2".to_string(),
            status: AgentStatus::Timeout,
            output: None,
            commits: vec![],
            files_modified: vec![],
            duration: Duration::from_secs(300),
            error: Some("Agent execution timed out".to_string()),
            worktree_path: Some(PathBuf::from("/tmp/worktree2")),
            branch_name: Some("agent-branch-2".to_string()),
            worktree_session_id: Some("session-456".to_string()),
            json_log_location: Some("/path/to/log2.json".to_string()),
            cleanup_status: Some(CleanupStatus::Success),
        }
    }

    fn create_successful_agent_result() -> AgentResult {
        AgentResult {
            item_id: "item-3".to_string(),
            status: AgentStatus::Success,
            output: Some("Success output".to_string()),
            commits: vec!["abc123".to_string()],
            files_modified: vec!["file.txt".to_string()],
            duration: Duration::from_secs(5),
            error: None,
            worktree_path: Some(PathBuf::from("/tmp/worktree3")),
            branch_name: Some("agent-branch-3".to_string()),
            worktree_session_id: Some("session-789".to_string()),
            json_log_location: Some("/path/to/log3.json".to_string()),
            cleanup_status: Some(CleanupStatus::Success),
        }
    }

    #[test]
    fn test_agent_result_to_dlq_item_failed() {
        let result = create_failed_agent_result("Test error");
        let work_item = serde_json::json!({"id": 1, "data": "test"});

        let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

        assert!(dlq_item.is_some());
        let item = dlq_item.unwrap();
        assert_eq!(item.item_id, "item-1");
        assert_eq!(item.failure_count, 1);
        assert_eq!(item.failure_history.len(), 1);
        assert_eq!(item.failure_history[0].attempt_number, 1);
        assert_eq!(item.failure_history[0].error_message, "Test error");
        assert_eq!(
            item.failure_history[0].json_log_location,
            Some("/path/to/log.json".to_string())
        );
    }

    #[test]
    fn test_agent_result_to_dlq_item_timeout() {
        let result = create_timeout_agent_result();
        let work_item = serde_json::json!({"id": 2, "data": "test"});

        let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

        assert!(dlq_item.is_some());
        let item = dlq_item.unwrap();
        assert_eq!(item.item_id, "item-2");
        assert!(item.reprocess_eligible);
        assert_eq!(item.error_signature.len(), 16);
        assert_eq!(item.failure_history[0].error_type, ErrorType::Timeout);
    }

    #[test]
    fn test_agent_result_to_dlq_item_success_returns_none() {
        let result = create_successful_agent_result();
        let work_item = serde_json::json!({"id": 3, "data": "test"});

        let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1);

        assert!(dlq_item.is_none());
    }

    #[test]
    fn test_error_signature_consistency() {
        let msg = "Test error message";
        let sig1 = create_error_signature(msg);
        let sig2 = create_error_signature(msg);

        assert_eq!(sig1, sig2);
        assert_eq!(sig1.len(), 16);
    }

    #[test]
    fn test_classify_error_timeout() {
        let status = AgentStatus::Timeout;
        let error = "Operation timed out after 30s";
        let error_type = classify_error(&status, error);

        assert_eq!(error_type, ErrorType::Timeout);
    }

    #[test]
    fn test_classify_error_merge_conflict() {
        let status = AgentStatus::Failed("merge failed".to_string());
        let error = "Merge conflict in file.txt";
        let error_type = classify_error(&status, error);

        assert_eq!(error_type, ErrorType::MergeConflict);
    }

    #[test]
    fn test_classify_error_worktree() {
        let status = AgentStatus::Failed("worktree error".to_string());
        let error = "Worktree creation failed";
        let error_type = classify_error(&status, error);

        assert_eq!(error_type, ErrorType::WorktreeError);
    }

    #[test]
    fn test_classify_error_validation() {
        let status = AgentStatus::Failed("validation error".to_string());
        let error = "Validation failed: invalid input";
        let error_type = classify_error(&status, error);

        assert_eq!(error_type, ErrorType::ValidationFailed);
    }

    #[test]
    fn test_classify_error_with_exit_code() {
        let status = AgentStatus::Failed("command failed".to_string());
        let error = "Command exited with code 127";
        let error_type = classify_error(&status, error);

        assert_eq!(error_type, ErrorType::CommandFailed { exit_code: 127 });
    }

    #[test]
    fn test_extract_exit_code_various_formats() {
        assert_eq!(extract_exit_code("exit code: 1"), Some(1));
        assert_eq!(extract_exit_code("exited with code 127"), Some(127));
        assert_eq!(extract_exit_code("status code: 42"), Some(42));
        assert_eq!(extract_exit_code("no code here"), None);
    }

    #[test]
    fn test_is_reprocessable_timeout() {
        let status = AgentStatus::Timeout;
        assert!(is_reprocessable(&status, "timeout"));
    }

    #[test]
    fn test_is_reprocessable_transient_errors() {
        let status = AgentStatus::Failed("network error".to_string());
        assert!(is_reprocessable(&status, "network timeout"));
        assert!(is_reprocessable(&status, "rate limit exceeded"));
        assert!(is_reprocessable(&status, "temporary unavailable"));
    }

    #[test]
    fn test_is_not_reprocessable_permanent_errors() {
        let status = AgentStatus::Failed("validation error".to_string());
        assert!(!is_reprocessable(&status, "validation failed"));
        assert!(!is_reprocessable(&status, "syntax error"));
    }

    #[test]
    fn test_requires_manual_review_permission_error() {
        assert!(requires_manual_review("Permission denied"));
        assert!(requires_manual_review("access denied"));
    }

    #[test]
    fn test_requires_manual_review_critical_errors() {
        assert!(requires_manual_review("Critical failure occurred"));
        assert!(requires_manual_review("Fatal error in processing"));
        assert!(requires_manual_review("Data corrupted"));
    }

    #[test]
    fn test_extract_worktree_artifacts() {
        let result = create_failed_agent_result("test");
        let artifacts = extract_worktree_artifacts(&result);

        assert!(artifacts.is_some());
        let artifacts = artifacts.unwrap();
        assert_eq!(artifacts.worktree_path, PathBuf::from("/tmp/worktree"));
        assert_eq!(artifacts.branch_name, "agent-branch");
    }

    #[test]
    fn test_dlq_item_includes_json_log_location() {
        let result = create_failed_agent_result("Test error");
        let work_item = serde_json::json!({"id": 1});

        let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1).unwrap();

        assert_eq!(
            dlq_item.failure_history[0].json_log_location,
            Some("/path/to/log.json".to_string())
        );
    }
}
