//! Retry tracking for MapReduce work items.
//!
//! This module provides pure functions to track retry attempts across
//! MapReduce executions, enabling accurate DLQ failure history and
//! retry strategies.
//!
//! **Key Functions**:
//! - `get_item_attempt_number()` - Get current attempt number
//! - `increment_retry_count()` - Update retry count
//! - `merge_retry_counts()` - Combine counts from multiple sources

use crate::cook::execution::dlq::DeadLetteredItem;
use crate::cook::execution::state::FailureRecord;
use std::collections::HashMap;

/// Get the attempt number for a work item.
///
/// Returns the current attempt count + 1 (since this is the next attempt).
/// If item has never been attempted, returns 1.
///
/// # Arguments
/// * `item_id` - The work item identifier
/// * `retry_counts` - Map of item IDs to retry counts
///
/// # Returns
/// The attempt number for this execution (1-indexed)
///
/// # Examples
/// ```
/// use std::collections::HashMap;
/// use prodigy::cook::execution::mapreduce::retry_tracking::get_item_attempt_number;
///
/// let mut retry_counts = HashMap::new();
///
/// // First attempt
/// assert_eq!(get_item_attempt_number("item-1", &retry_counts), 1);
///
/// // After one retry
/// retry_counts.insert("item-1".to_string(), 1);
/// assert_eq!(get_item_attempt_number("item-1", &retry_counts), 2);
/// ```
pub fn get_item_attempt_number(item_id: &str, retry_counts: &HashMap<String, u32>) -> u32 {
    retry_counts
        .get(item_id)
        .map(|count| count + 1)
        .unwrap_or(1)
}

/// Increment retry count for an item.
///
/// Pure function - returns new HashMap with incremented count.
///
/// # Arguments
/// * `item_id` - The work item identifier
/// * `retry_counts` - Current retry counts map
///
/// # Returns
/// New HashMap with incremented count for the item
pub fn increment_retry_count(
    item_id: &str,
    mut retry_counts: HashMap<String, u32>,
) -> HashMap<String, u32> {
    *retry_counts.entry(item_id.to_string()).or_insert(0) += 1;
    retry_counts
}

/// Get retry counts from multiple sources (failed_agents, DLQ).
///
/// Pure function that merges retry information.
///
/// # Arguments
/// * `failed_items` - Items from failed_agents with their failure counts
/// * `dlq_items` - Items from DLQ with their failure counts
///
/// # Returns
/// Merged retry counts map
pub fn merge_retry_counts(
    failed_items: &HashMap<String, FailureRecord>,
    dlq_items: &[DeadLetteredItem],
) -> HashMap<String, u32> {
    let mut counts = HashMap::new();

    // Add counts from failed_agents
    for (item_id, failure) in failed_items {
        counts.insert(item_id.clone(), failure.attempts);
    }

    // Merge with DLQ counts (use max if both exist)
    for dlq_item in dlq_items {
        counts
            .entry(dlq_item.item_id.clone())
            .and_modify(|count| *count = (*count).max(dlq_item.failure_count))
            .or_insert(dlq_item.failure_count);
    }

    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn test_get_item_attempt_number_first_attempt() {
        let retry_counts = HashMap::new();
        assert_eq!(get_item_attempt_number("item-1", &retry_counts), 1);
    }

    #[test]
    fn test_get_item_attempt_number_after_retry() {
        let mut retry_counts = HashMap::new();
        retry_counts.insert("item-1".to_string(), 1);

        assert_eq!(get_item_attempt_number("item-1", &retry_counts), 2);
    }

    #[test]
    fn test_get_item_attempt_number_multiple_retries() {
        let mut retry_counts = HashMap::new();
        retry_counts.insert("item-1".to_string(), 5);

        assert_eq!(get_item_attempt_number("item-1", &retry_counts), 6);
    }

    #[test]
    fn test_increment_retry_count() {
        let counts = HashMap::new();
        let updated = increment_retry_count("item-1", counts);

        assert_eq!(updated.get("item-1"), Some(&1));

        let updated2 = increment_retry_count("item-1", updated);
        assert_eq!(updated2.get("item-1"), Some(&2));
    }

    #[test]
    fn test_increment_retry_count_multiple_items() {
        let mut counts = HashMap::new();
        counts.insert("item-1".to_string(), 2);

        let updated = increment_retry_count("item-2", counts);

        assert_eq!(updated.get("item-1"), Some(&2)); // unchanged
        assert_eq!(updated.get("item-2"), Some(&1)); // new item
    }

    #[test]
    fn test_merge_retry_counts_from_failed_and_dlq() {
        let mut failed_items = HashMap::new();
        failed_items.insert(
            "item-1".to_string(),
            FailureRecord {
                item_id: "item-1".to_string(),
                attempts: 2,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        let dlq_items = vec![DeadLetteredItem {
            item_id: "item-2".to_string(),
            item_data: json!({"id": 2}),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 3,
            failure_history: vec![],
            error_signature: "sig".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        }];

        let merged = merge_retry_counts(&failed_items, &dlq_items);

        assert_eq!(merged.get("item-1"), Some(&2));
        assert_eq!(merged.get("item-2"), Some(&3));
    }

    #[test]
    fn test_merge_retry_counts_uses_max() {
        // Item in both failed_agents (2 attempts) and DLQ (3 attempts)
        // Should use max (3)

        let mut failed_items = HashMap::new();
        failed_items.insert(
            "item-1".to_string(),
            FailureRecord {
                item_id: "item-1".to_string(),
                attempts: 2,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        let dlq_items = vec![DeadLetteredItem {
            item_id: "item-1".to_string(),
            item_data: json!({"id": 1}),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 3,
            failure_history: vec![],
            error_signature: "sig".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        }];

        let merged = merge_retry_counts(&failed_items, &dlq_items);
        assert_eq!(merged.get("item-1"), Some(&3), "Should use max count");
    }

    #[test]
    fn test_merge_retry_counts_empty_inputs() {
        let failed_items = HashMap::new();
        let dlq_items = vec![];

        let merged = merge_retry_counts(&failed_items, &dlq_items);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_merge_retry_counts_only_failed() {
        let mut failed_items = HashMap::new();
        failed_items.insert(
            "item-1".to_string(),
            FailureRecord {
                item_id: "item-1".to_string(),
                attempts: 5,
                last_error: "error".to_string(),
                last_attempt: Utc::now(),
                worktree_info: None,
            },
        );

        let dlq_items = vec![];
        let merged = merge_retry_counts(&failed_items, &dlq_items);

        assert_eq!(merged.get("item-1"), Some(&5));
    }

    #[test]
    fn test_merge_retry_counts_only_dlq() {
        let failed_items = HashMap::new();

        let dlq_items = vec![DeadLetteredItem {
            item_id: "item-1".to_string(),
            item_data: json!({"id": 1}),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 4,
            failure_history: vec![],
            error_signature: "sig".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        }];

        let merged = merge_retry_counts(&failed_items, &dlq_items);
        assert_eq!(merged.get("item-1"), Some(&4));
    }
}
