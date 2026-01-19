//! Pure functions for extracting and transforming session information
//!
//! This module contains pure functions that extract data from JSON values
//! and transform session information. All functions are side-effect free
//! and can be tested in isolation.
//!
//! # Architecture
//!
//! Following the "pure core, imperative shell" pattern:
//! - This module contains the pure core logic for data extraction
//! - The `manager.rs` `list_detailed` method handles I/O operations
//! - This separation enables comprehensive unit testing

use serde_json::Value;
use std::path::PathBuf;

use super::display::{EnhancedSessionInfo, WorktreeSummary};
use super::WorktreeStatus;

/// Extracted workflow information from session state JSON
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkflowInfo {
    pub workflow_path: Option<PathBuf>,
    pub workflow_args: Vec<String>,
    pub current_step: usize,
    pub total_steps: Option<usize>,
}

/// Extracted MapReduce progress information
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapReduceProgress {
    pub items_processed: Option<u32>,
    pub total_items: Option<u32>,
}

/// Extract workflow information from session state JSON
///
/// # Arguments
/// * `session_state` - The parsed JSON value from session_state.json
///
/// # Returns
/// * `WorkflowInfo` - Extracted workflow information, with defaults for missing fields
///
/// # Example
/// ```ignore
/// let json: Value = serde_json::from_str(r#"{"workflow_state": {"workflow_path": "test.yaml"}}"#)?;
/// let info = extract_workflow_info(&json);
/// assert_eq!(info.workflow_path, Some(PathBuf::from("test.yaml")));
/// ```
#[must_use]
pub fn extract_workflow_info(session_state: &Value) -> WorkflowInfo {
    let mut info = WorkflowInfo::default();

    let Some(workflow_state) = session_state.get("workflow_state") else {
        return info;
    };

    // Extract workflow path
    info.workflow_path = workflow_state
        .get("workflow_path")
        .and_then(Value::as_str)
        .map(PathBuf::from);

    // Extract workflow arguments
    info.workflow_args = workflow_state
        .get("input_args")
        .and_then(Value::as_array)
        .map(|args| {
            args.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Extract current step
    info.current_step = workflow_state
        .get("current_step")
        .and_then(Value::as_u64)
        .map(|s| s as usize)
        .unwrap_or(0);

    // Extract total steps from completed_steps array length
    info.total_steps = workflow_state
        .get("completed_steps")
        .and_then(Value::as_array)
        .map(Vec::len);

    info
}

/// Extract MapReduce progress from session state JSON
///
/// # Arguments
/// * `session_state` - The parsed JSON value from session_state.json
///
/// # Returns
/// * `MapReduceProgress` - Extracted progress information
#[must_use]
pub fn extract_mapreduce_progress(session_state: &Value) -> MapReduceProgress {
    let Some(mapreduce_state) = session_state.get("mapreduce_state") else {
        return MapReduceProgress::default();
    };

    MapReduceProgress {
        items_processed: mapreduce_state
            .get("items_processed")
            .and_then(Value::as_u64)
            .map(|p| p as u32),
        total_items: mapreduce_state
            .get("total_items")
            .and_then(Value::as_u64)
            .map(|t| t as u32),
    }
}

/// Apply workflow information to an enhanced session info
///
/// # Arguments
/// * `info` - The enhanced session info to update
/// * `workflow` - The workflow information to apply
pub fn apply_workflow_info(info: &mut EnhancedSessionInfo, workflow: &WorkflowInfo) {
    info.workflow_path = workflow.workflow_path.clone();
    info.workflow_args = workflow.workflow_args.clone();
    info.current_step = workflow.current_step;
    info.total_steps = workflow.total_steps;
}

/// Apply MapReduce progress to an enhanced session info
///
/// # Arguments
/// * `info` - The enhanced session info to update
/// * `progress` - The MapReduce progress to apply
pub fn apply_mapreduce_progress(info: &mut EnhancedSessionInfo, progress: &MapReduceProgress) {
    info.items_processed = progress.items_processed;
    info.total_items = progress.total_items;
}

/// Calculate summary statistics from a list of enhanced session infos
///
/// # Arguments
/// * `sessions` - Slice of enhanced session information
///
/// # Returns
/// * `WorktreeSummary` - Aggregated summary statistics
#[must_use]
pub fn calculate_summary(sessions: &[EnhancedSessionInfo]) -> WorktreeSummary {
    sessions.iter().fold(WorktreeSummary::default(), |mut summary, session| {
        summary.total += 1;
        match session.status {
            WorktreeStatus::InProgress => summary.in_progress += 1,
            WorktreeStatus::Interrupted => summary.interrupted += 1,
            WorktreeStatus::Failed => summary.failed += 1,
            WorktreeStatus::Completed | WorktreeStatus::Merged => summary.completed += 1,
            WorktreeStatus::CleanedUp | WorktreeStatus::Abandoned => {}
        }
        summary
    })
}

/// Sort sessions by last activity (most recent first)
///
/// # Arguments
/// * `sessions` - Mutable slice of sessions to sort in place
pub fn sort_by_last_activity(sessions: &mut [EnhancedSessionInfo]) {
    sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn test_extract_workflow_info_complete() {
        let session_state = json!({
            "workflow_state": {
                "workflow_path": "/path/to/workflow.yaml",
                "input_args": ["arg1", "arg2", "arg3"],
                "current_step": 5,
                "completed_steps": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
            }
        });

        let info = extract_workflow_info(&session_state);

        assert_eq!(info.workflow_path, Some(PathBuf::from("/path/to/workflow.yaml")));
        assert_eq!(info.workflow_args, vec!["arg1", "arg2", "arg3"]);
        assert_eq!(info.current_step, 5);
        assert_eq!(info.total_steps, Some(10));
    }

    #[test]
    fn test_extract_workflow_info_missing_workflow_state() {
        let session_state = json!({
            "other_data": "value"
        });

        let info = extract_workflow_info(&session_state);

        assert_eq!(info, WorkflowInfo::default());
    }

    #[test]
    fn test_extract_workflow_info_partial() {
        let session_state = json!({
            "workflow_state": {
                "workflow_path": "test.yaml"
            }
        });

        let info = extract_workflow_info(&session_state);

        assert_eq!(info.workflow_path, Some(PathBuf::from("test.yaml")));
        assert!(info.workflow_args.is_empty());
        assert_eq!(info.current_step, 0);
        assert_eq!(info.total_steps, None);
    }

    #[test]
    fn test_extract_workflow_info_empty_args() {
        let session_state = json!({
            "workflow_state": {
                "input_args": []
            }
        });

        let info = extract_workflow_info(&session_state);

        assert!(info.workflow_args.is_empty());
    }

    #[test]
    fn test_extract_workflow_info_filters_non_string_args() {
        let session_state = json!({
            "workflow_state": {
                "input_args": ["valid", 123, "also_valid", null]
            }
        });

        let info = extract_workflow_info(&session_state);

        assert_eq!(info.workflow_args, vec!["valid", "also_valid"]);
    }

    #[test]
    fn test_extract_mapreduce_progress_complete() {
        let session_state = json!({
            "mapreduce_state": {
                "items_processed": 75,
                "total_items": 100
            }
        });

        let progress = extract_mapreduce_progress(&session_state);

        assert_eq!(progress.items_processed, Some(75));
        assert_eq!(progress.total_items, Some(100));
    }

    #[test]
    fn test_extract_mapreduce_progress_missing() {
        let session_state = json!({
            "other_data": "value"
        });

        let progress = extract_mapreduce_progress(&session_state);

        assert_eq!(progress, MapReduceProgress::default());
    }

    #[test]
    fn test_extract_mapreduce_progress_partial() {
        let session_state = json!({
            "mapreduce_state": {
                "items_processed": 50
            }
        });

        let progress = extract_mapreduce_progress(&session_state);

        assert_eq!(progress.items_processed, Some(50));
        assert_eq!(progress.total_items, None);
    }

    #[test]
    fn test_apply_workflow_info() {
        let mut info = create_test_session_info();
        let workflow = WorkflowInfo {
            workflow_path: Some(PathBuf::from("test.yaml")),
            workflow_args: vec!["arg1".to_string()],
            current_step: 3,
            total_steps: Some(10),
        };

        apply_workflow_info(&mut info, &workflow);

        assert_eq!(info.workflow_path, Some(PathBuf::from("test.yaml")));
        assert_eq!(info.workflow_args, vec!["arg1"]);
        assert_eq!(info.current_step, 3);
        assert_eq!(info.total_steps, Some(10));
    }

    #[test]
    fn test_apply_mapreduce_progress() {
        let mut info = create_test_session_info();
        let progress = MapReduceProgress {
            items_processed: Some(25),
            total_items: Some(100),
        };

        apply_mapreduce_progress(&mut info, &progress);

        assert_eq!(info.items_processed, Some(25));
        assert_eq!(info.total_items, Some(100));
    }

    #[test]
    fn test_calculate_summary_empty() {
        let sessions: Vec<EnhancedSessionInfo> = vec![];

        let summary = calculate_summary(&sessions);

        assert_eq!(summary.total, 0);
        assert_eq!(summary.in_progress, 0);
        assert_eq!(summary.interrupted, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.completed, 0);
    }

    #[test]
    fn test_calculate_summary_mixed_statuses() {
        let sessions = vec![
            create_session_with_status(WorktreeStatus::InProgress),
            create_session_with_status(WorktreeStatus::InProgress),
            create_session_with_status(WorktreeStatus::Interrupted),
            create_session_with_status(WorktreeStatus::Failed),
            create_session_with_status(WorktreeStatus::Completed),
            create_session_with_status(WorktreeStatus::Merged),
            create_session_with_status(WorktreeStatus::CleanedUp),
            create_session_with_status(WorktreeStatus::Abandoned),
        ];

        let summary = calculate_summary(&sessions);

        assert_eq!(summary.total, 8);
        assert_eq!(summary.in_progress, 2);
        assert_eq!(summary.interrupted, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.completed, 2); // Completed + Merged
    }

    #[test]
    fn test_sort_by_last_activity() {
        let now = Utc::now();
        let mut sessions = vec![
            create_session_with_last_activity(now - chrono::Duration::hours(2)),
            create_session_with_last_activity(now - chrono::Duration::minutes(30)),
            create_session_with_last_activity(now - chrono::Duration::hours(1)),
        ];

        sort_by_last_activity(&mut sessions);

        // Most recent should be first
        assert!(sessions[0].last_activity > sessions[1].last_activity);
        assert!(sessions[1].last_activity > sessions[2].last_activity);
    }

    // Helper functions for tests

    fn create_test_session_info() -> EnhancedSessionInfo {
        EnhancedSessionInfo {
            session_id: "test-session".to_string(),
            status: WorktreeStatus::InProgress,
            workflow_path: None,
            workflow_args: vec![],
            started_at: Utc::now(),
            last_activity: Utc::now(),
            current_step: 0,
            total_steps: None,
            error_summary: None,
            branch_name: "test-branch".to_string(),
            parent_branch: None,
            worktree_path: PathBuf::new(),
            files_changed: 0,
            commits: 0,
            items_processed: None,
            total_items: None,
        }
    }

    fn create_session_with_status(status: WorktreeStatus) -> EnhancedSessionInfo {
        let mut info = create_test_session_info();
        info.status = status;
        info
    }

    fn create_session_with_last_activity(
        last_activity: chrono::DateTime<Utc>,
    ) -> EnhancedSessionInfo {
        let mut info = create_test_session_info();
        info.last_activity = last_activity;
        info
    }
}
