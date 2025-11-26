//! DLQ Integration for MapReduce Agent Failures (Spec 176)
//!
//! This module provides pure functions to convert AgentResult failures and
//! validation errors into DeadLetteredItems for the Dead Letter Queue (DLQ).
//! All functions are pure (no I/O) and fully testable.
//!
//! ## Validation Error Integration (Spec 176)
//!
//! When work items fail validation before MapReduce execution, they are
//! converted to DLQ items with full error context preserved:
//!
//! ```rust
//! use prodigy::cook::execution::mapreduce::dlq_integration::validation_errors_to_dlq_items;
//! use prodigy::cook::execution::mapreduce::validation::WorkItemValidationError;
//! use serde_json::json;
//!
//! let errors = vec![
//!     WorkItemValidationError::MissingRequiredField {
//!         item_index: 0,
//!         field: "name".to_string(),
//!     },
//! ];
//! let items = vec![json!({"id": "item-1"})];
//!
//! let dlq_items = validation_errors_to_dlq_items(&errors, &items);
//! assert_eq!(dlq_items.len(), 1);
//! ```

use crate::cook::execution::dlq::{DeadLetteredItem, ErrorType, FailureDetail, WorktreeArtifacts};
use crate::cook::execution::mapreduce::agent::types::{AgentResult, AgentStatus};
use crate::cook::execution::mapreduce::validation::WorkItemValidationError;
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

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
        error_context: extract_error_context(error_msg),
        stack_trace: result.error.clone(),
        agent_id: format!("agent-{}", result.item_id),
        step_failed: "agent_execution".to_string(),
        duration_ms: result.duration.as_millis() as u64,
        json_log_location: result.json_log_location.clone(),
    }
}

/// Extract error context from error message formatted by ContextError's Display impl
///
/// Stillwater's ContextError Display format includes context trails like:
/// ```text
/// Error: base error message
/// Context:
///   -> Context message 1
///   -> Context message 2
///   -> Context message 3
/// ```
///
/// This function parses that format to extract the context trail.
fn extract_error_context(error_msg: &str) -> Option<Vec<String>> {
    // Check if the error message contains a "Context:" section
    if !error_msg.contains("Context:") {
        return None;
    }

    let mut context_lines = Vec::new();
    let mut in_context_section = false;

    for line in error_msg.lines() {
        let trimmed = line.trim();

        // Start of context section
        if trimmed == "Context:" {
            in_context_section = true;
            continue;
        }

        // Context line (starts with "->")
        if in_context_section && trimmed.starts_with("->") {
            // Remove the "-> " prefix and collect the context message
            let context_msg = trimmed.strip_prefix("->").unwrap_or(trimmed).trim();
            if !context_msg.is_empty() {
                context_lines.push(context_msg.to_string());
            }
        } else if in_context_section && !trimmed.is_empty() {
            // End of context section (hit a non-empty, non-arrow line)
            break;
        }
    }

    if context_lines.is_empty() {
        None
    } else {
        Some(context_lines)
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
            // Check for commit validation failure first (most specific)
            if msg_lower.contains("commit required") || msg_lower.contains("commit validation") {
                ErrorType::CommitValidationFailed
            } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
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
    // Commit validation failures always require manual review as they indicate
    // a workflow configuration issue or agent implementation problem
    msg_lower.contains("permission")
        || msg_lower.contains("access denied")
        || msg_lower.contains("critical")
        || msg_lower.contains("fatal")
        || msg_lower.contains("corrupted")
        || msg_lower.contains("validation")
        || msg_lower.contains("commit required")
        || msg_lower.contains("commit validation")
}

// ============================================================================
// Validation Error DLQ Integration (Spec 176)
// ============================================================================

/// Convert validation errors to DLQ items
///
/// Groups validation errors by item index and creates a DeadLetteredItem for
/// each unique item that failed validation. All errors for each item are
/// preserved in the error_context field.
///
/// # Arguments
///
/// * `errors` - The validation errors to convert
/// * `items` - The original work items (used to get item data)
///
/// # Returns
///
/// A list of DeadLetteredItems, one per unique failed item
pub fn validation_errors_to_dlq_items(
    errors: &[WorkItemValidationError],
    items: &[Value],
) -> Vec<DeadLetteredItem> {
    // Group errors by item index
    let mut errors_by_index: HashMap<usize, Vec<&WorkItemValidationError>> = HashMap::new();

    for error in errors {
        let idx = get_error_item_index(error);
        errors_by_index.entry(idx).or_default().push(error);
    }

    // Convert each group to a DLQ item
    errors_by_index
        .into_iter()
        .map(|(idx, item_errors)| {
            create_validation_dlq_item(idx, &item_errors, items.get(idx).cloned())
        })
        .collect()
}

/// Get the item index from a validation error
fn get_error_item_index(error: &WorkItemValidationError) -> usize {
    match error {
        WorkItemValidationError::MissingRequiredField { item_index, .. } => *item_index,
        WorkItemValidationError::InvalidFieldType { item_index, .. } => *item_index,
        WorkItemValidationError::ConstraintViolation { item_index, .. } => *item_index,
        WorkItemValidationError::NotAnObject { item_index } => *item_index,
        WorkItemValidationError::NullItem { item_index } => *item_index,
        WorkItemValidationError::DuplicateId { item_index, .. } => *item_index,
        WorkItemValidationError::InvalidId { item_index, .. } => *item_index,
    }
}

/// Create a DLQ item from grouped validation errors
fn create_validation_dlq_item(
    idx: usize,
    errors: &[&WorkItemValidationError],
    item_data: Option<Value>,
) -> DeadLetteredItem {
    let now = Utc::now();

    // Create error context from all errors for this item
    let error_context: Vec<String> = errors.iter().map(|e| e.to_string()).collect();

    // Create a combined error message
    let error_message = if errors.len() == 1 {
        errors[0].to_string()
    } else {
        format!(
            "{} validation error(s) for work item #{}",
            errors.len(),
            idx
        )
    };

    // Create error signature from first error (for grouping)
    let error_signature = create_error_signature(&error_message);

    let failure_detail = FailureDetail {
        attempt_number: 1,
        timestamp: now,
        error_type: ErrorType::ValidationFailed,
        error_message: error_message.clone(),
        error_context: Some(error_context),
        stack_trace: None,
        agent_id: format!("validation-{}", idx),
        step_failed: "work_item_validation".to_string(),
        duration_ms: 0,
        json_log_location: None,
    };

    DeadLetteredItem {
        item_id: format!("item-{}", idx),
        item_data: item_data.unwrap_or(Value::Null),
        first_attempt: now,
        last_attempt: now,
        failure_count: 1,
        failure_history: vec![failure_detail],
        error_signature,
        worktree_artifacts: None,     // No worktree for validation failures
        reprocess_eligible: false,    // Validation failures need data fixes
        manual_review_required: true, // Always requires review to fix data
    }
}

/// Create a single DLQ item from a validation error
///
/// Convenience function for creating a DLQ item from a single error.
pub fn validation_error_to_dlq_item(
    error: &WorkItemValidationError,
    item_data: &Value,
) -> DeadLetteredItem {
    let idx = get_error_item_index(error);
    create_validation_dlq_item(idx, &[error], Some(item_data.clone()))
}

/// Extract ID from work item for DLQ (best effort)
#[allow(dead_code)]
fn extract_item_id(item: &Value, idx: usize) -> String {
    // Try common ID fields
    for field in &["id", "item_id", "_id", "key"] {
        if let Some(Value::String(s)) = item.get(*field) {
            return s.clone();
        }
        if let Some(Value::Number(n)) = item.get(*field) {
            return n.to_string();
        }
    }
    // Fallback to index-based ID
    format!("item-{}", idx)
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

    #[test]
    fn test_extract_error_context_with_context() {
        let error_msg = r#"Error: Command failed
Context:
  -> Executing command 2/5
  -> Processing work item item-123
  -> Running MapReduce job job-456"#;

        let context = extract_error_context(error_msg);

        assert!(context.is_some());
        let context = context.unwrap();
        assert_eq!(context.len(), 3);
        assert_eq!(context[0], "Executing command 2/5");
        assert_eq!(context[1], "Processing work item item-123");
        assert_eq!(context[2], "Running MapReduce job job-456");
    }

    #[test]
    fn test_extract_error_context_without_context() {
        let error_msg = "Simple error message without context";
        let context = extract_error_context(error_msg);
        assert!(context.is_none());
    }

    #[test]
    fn test_extract_error_context_empty_context_section() {
        let error_msg = r#"Error: Command failed
Context:
"#;
        let context = extract_error_context(error_msg);
        assert!(context.is_none());
    }

    #[test]
    fn test_extract_error_context_with_extra_whitespace() {
        let error_msg = r#"Error: Command failed
Context:
  ->    Executing command with spaces
  ->  Processing item
"#;
        let context = extract_error_context(error_msg);
        assert!(context.is_some());
        let context = context.unwrap();
        assert_eq!(context.len(), 2);
        assert_eq!(context[0], "Executing command with spaces");
        assert_eq!(context[1], "Processing item");
    }

    #[test]
    fn test_dlq_item_with_context_preservation() {
        let error_msg = r#"Error: Command execution failed
Context:
  -> Executing agent command
  -> Processing work item item-1
  -> MapReduce map phase"#;

        let result = create_failed_agent_result(error_msg);
        let work_item = serde_json::json!({"id": 1});

        let dlq_item = agent_result_to_dlq_item(&result, &work_item, 1).unwrap();

        assert!(dlq_item.failure_history[0].error_context.is_some());
        let context = dlq_item.failure_history[0].error_context.as_ref().unwrap();
        assert_eq!(context.len(), 3);
        assert_eq!(context[0], "Executing agent command");
        assert_eq!(context[1], "Processing work item item-1");
        assert_eq!(context[2], "MapReduce map phase");
    }

    // ============================================================================
    // Validation DLQ Integration Tests (Spec 176)
    // ============================================================================

    #[test]
    fn test_validation_errors_to_dlq_items_single_error() {
        let errors = vec![WorkItemValidationError::MissingRequiredField {
            item_index: 0,
            field: "name".to_string(),
        }];
        let items = vec![serde_json::json!({"id": "item-1"})];

        let dlq_items = validation_errors_to_dlq_items(&errors, &items);

        assert_eq!(dlq_items.len(), 1);
        let dlq_item = &dlq_items[0];
        assert_eq!(dlq_item.item_id, "item-0");
        assert_eq!(dlq_item.failure_count, 1);
        assert_eq!(
            dlq_item.failure_history[0].error_type,
            ErrorType::ValidationFailed
        );
        assert!(dlq_item.manual_review_required);
        assert!(!dlq_item.reprocess_eligible);
    }

    #[test]
    fn test_validation_errors_to_dlq_items_multiple_errors_same_item() {
        let errors = vec![
            WorkItemValidationError::MissingRequiredField {
                item_index: 0,
                field: "name".to_string(),
            },
            WorkItemValidationError::InvalidFieldType {
                item_index: 0,
                field: "count".to_string(),
                expected: "number".to_string(),
                got: "string".to_string(),
            },
        ];
        let items = vec![serde_json::json!({"id": "item-1", "count": "bad"})];

        let dlq_items = validation_errors_to_dlq_items(&errors, &items);

        // Should be grouped into one DLQ item
        assert_eq!(dlq_items.len(), 1);
        let dlq_item = &dlq_items[0];

        // Error context should contain both errors
        let context = dlq_item.failure_history[0].error_context.as_ref().unwrap();
        assert_eq!(context.len(), 2);
    }

    #[test]
    fn test_validation_errors_to_dlq_items_multiple_items() {
        let errors = vec![
            WorkItemValidationError::NullItem { item_index: 0 },
            WorkItemValidationError::MissingRequiredField {
                item_index: 2,
                field: "name".to_string(),
            },
        ];
        let items = vec![
            Value::Null,
            serde_json::json!({"id": "item-2"}),
            serde_json::json!({"id": "item-3"}),
        ];

        let dlq_items = validation_errors_to_dlq_items(&errors, &items);

        // Should create two DLQ items
        assert_eq!(dlq_items.len(), 2);

        // Verify different item indices
        let indices: Vec<_> = dlq_items.iter().map(|i| &i.item_id).collect();
        assert!(indices.contains(&&"item-0".to_string()));
        assert!(indices.contains(&&"item-2".to_string()));
    }

    #[test]
    fn test_validation_error_to_dlq_item() {
        let error = WorkItemValidationError::ConstraintViolation {
            item_index: 5,
            field: "score".to_string(),
            constraint: "range [0, 100]".to_string(),
            value: "150".to_string(),
        };
        let item_data = serde_json::json!({"id": "item-5", "score": 150});

        let dlq_item = validation_error_to_dlq_item(&error, &item_data);

        assert_eq!(dlq_item.item_id, "item-5");
        assert_eq!(dlq_item.item_data, item_data);
        assert_eq!(
            dlq_item.failure_history[0].step_failed,
            "work_item_validation"
        );
    }

    #[test]
    fn test_get_error_item_index() {
        assert_eq!(
            get_error_item_index(&WorkItemValidationError::NullItem { item_index: 3 }),
            3
        );
        assert_eq!(
            get_error_item_index(&WorkItemValidationError::NotAnObject { item_index: 7 }),
            7
        );
        assert_eq!(
            get_error_item_index(&WorkItemValidationError::DuplicateId {
                item_index: 10,
                id: "dup".to_string(),
                first_seen_at: 2,
            }),
            10
        );
    }

    #[test]
    fn test_extract_item_id() {
        // Test with "id" field
        let item = serde_json::json!({"id": "my-id"});
        assert_eq!(extract_item_id(&item, 0), "my-id");

        // Test with numeric id
        let item = serde_json::json!({"id": 123});
        assert_eq!(extract_item_id(&item, 0), "123");

        // Test with alternative field
        let item = serde_json::json!({"item_id": "alt-id"});
        assert_eq!(extract_item_id(&item, 0), "alt-id");

        // Test fallback to index
        let item = serde_json::json!({"name": "no-id"});
        assert_eq!(extract_item_id(&item, 5), "item-5");
    }
}
