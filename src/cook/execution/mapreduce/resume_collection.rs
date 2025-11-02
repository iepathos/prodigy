//! Pure functions for collecting work items from various sources in MapReduce resume.
//!
//! This module provides functional, side-effect-free helpers to collect work items
//! from pending items, failed items, and other sources. These functions operate on
//! immutable state and return new collections.

use crate::cook::execution::state::MapReduceJobState;
use serde_json::Value;

/// Collect pending items from job state.
///
/// Pure function that extracts pending work items without mutating state.
/// Items are returned in the order they appear in `pending_items`.
///
/// # Arguments
/// * `state` - Job state containing pending items
///
/// # Returns
/// Vector of work items that are still pending
///
/// # Examples
/// ```ignore
/// let pending = collect_pending_items(&state);
/// assert_eq!(pending.len(), 5);
/// ```
pub fn collect_pending_items(state: &MapReduceJobState) -> Vec<Value> {
    state
        .pending_items
        .iter()
        .filter_map(|item_id| state.find_work_item(item_id))
        .collect()
}

/// Collect failed items eligible for retry.
///
/// Pure function that extracts failed items based on retry limit.
/// Only returns items where `attempts < max_retries`.
///
/// # Arguments
/// * `state` - Job state containing failed agents
/// * `max_retries` - Maximum retry attempts allowed
///
/// # Returns
/// Vector of work items that failed but are eligible for retry
///
/// # Examples
/// ```ignore
/// let failed = collect_failed_items(&state, 3);
/// // Returns only items with < 3 retry attempts
/// ```
pub fn collect_failed_items(state: &MapReduceJobState, max_retries: u32) -> Vec<Value> {
    state
        .failed_agents
        .iter()
        .filter(|(_, failure)| failure.attempts < max_retries)
        .filter_map(|(item_id, _)| state.find_work_item(item_id))
        .collect()
}

/// Combine items from multiple sources in priority order.
///
/// Pure function using functional composition. Chains iterators to create
/// a single combined list while preserving source priority.
///
/// **Priority order**: pending → failed → dlq
///
/// # Arguments
/// * `pending` - Items not yet started
/// * `failed` - Items that failed and are eligible for retry
/// * `dlq` - Items from Dead Letter Queue
///
/// # Returns
/// Combined vector with all items in priority order
///
/// # Examples
/// ```ignore
/// let combined = combine_work_items(pending, failed, dlq);
/// // pending items appear first, then failed, then dlq
/// ```
pub fn combine_work_items(pending: Vec<Value>, failed: Vec<Value>, dlq: Vec<Value>) -> Vec<Value> {
    pending.into_iter().chain(failed).chain(dlq).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;
    use crate::cook::execution::state::FailureRecord;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_state() -> MapReduceJobState {
        let config = MapReduceConfig {
            input: "test.json".to_string(),
            json_path: "$.items[*]".to_string(),
            max_parallel: 5,
            agent_timeout_secs: None,
            continue_on_failure: false,
            batch_size: None,
            enable_checkpoints: true,
            max_items: None,
            offset: None,
        };

        MapReduceJobState {
            job_id: "test-job".to_string(),
            config,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            work_items: vec![
                json!({"id": "item-1"}),
                json!({"id": "item-2"}),
                json!({"id": "item-3"}),
            ],
            agent_results: HashMap::new(),
            completed_agents: Default::default(),
            failed_agents: HashMap::new(),
            pending_items: vec!["item_0".to_string(), "item_1".to_string()],
            checkpoint_version: 1,
            checkpoint_format_version: 1,
            parent_worktree: None,
            reduce_phase_state: None,
            total_items: 3,
            successful_count: 0,
            failed_count: 0,
            is_complete: false,
            agent_template: vec![],
            reduce_commands: None,
            variables: HashMap::new(),
            setup_output: None,
            setup_completed: false,
        }
    }

    #[test]
    fn test_collect_pending_items() {
        let state = create_test_state();
        let pending = collect_pending_items(&state);

        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0]["id"], "item-1");
        assert_eq!(pending[1]["id"], "item-2");
    }

    #[test]
    fn test_collect_pending_items_empty() {
        let mut state = create_test_state();
        state.pending_items.clear();

        let pending = collect_pending_items(&state);
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_collect_failed_items_respects_max_retries() {
        let mut state = create_test_state();

        // Add failed items with different retry counts
        state.failed_agents.insert(
            "item_0".to_string(),
            FailureRecord {
                item_id: "item_0".to_string(),
                attempts: 1,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        state.failed_agents.insert(
            "item_1".to_string(),
            FailureRecord {
                item_id: "item_1".to_string(),
                attempts: 5,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        let failed = collect_failed_items(&state, 3);

        // Only item_0 (attempts=1 < 3) should be returned
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0]["id"], "item-1"); // item_0 maps to work_items[0]
    }

    #[test]
    fn test_collect_failed_items_empty() {
        let state = create_test_state();
        let failed = collect_failed_items(&state, 3);

        assert_eq!(failed.len(), 0);
    }

    #[test]
    fn test_combine_work_items_preserves_priority() {
        let pending = vec![json!({"id": "p1"})];
        let failed = vec![json!({"id": "f1"})];
        let dlq = vec![json!({"id": "d1"})];

        let combined = combine_work_items(pending, failed, dlq);

        assert_eq!(combined.len(), 3);
        assert_eq!(combined[0]["id"], "p1"); // Pending first
        assert_eq!(combined[1]["id"], "f1"); // Failed second
        assert_eq!(combined[2]["id"], "d1"); // DLQ last
    }

    #[test]
    fn test_combine_work_items_with_empty_sources() {
        let pending = vec![json!({"id": "p1"}), json!({"id": "p2"})];
        let failed = vec![];
        let dlq = vec![json!({"id": "d1"})];

        let combined = combine_work_items(pending, failed, dlq);

        assert_eq!(combined.len(), 3);
        assert_eq!(combined[0]["id"], "p1");
        assert_eq!(combined[1]["id"], "p2");
        assert_eq!(combined[2]["id"], "d1");
    }

    #[test]
    fn test_combine_work_items_all_empty() {
        let pending: Vec<Value> = vec![];
        let failed: Vec<Value> = vec![];
        let dlq: Vec<Value> = vec![];

        let combined = combine_work_items(pending, failed, dlq);
        assert_eq!(combined.len(), 0);
    }
}
