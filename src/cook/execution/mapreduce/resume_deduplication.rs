//! Pure functions for deduplicating work items in MapReduce resume operations.
//!
//! This module provides functional, testable deduplication logic to ensure each
//! work item is processed exactly once when resuming from multiple sources
//! (pending items, failed items, and DLQ items).

use serde_json::Value;
use std::collections::HashSet;

/// Deduplicate work items by ID, keeping first occurrence of each unique ID.
///
/// Uses a HashSet for O(n) time complexity. Preserves order of first occurrences.
/// This is a pure function with no side effects, making it fully testable.
///
/// # Arguments
/// * `items` - Work items to deduplicate
///
/// # Returns
/// Deduplicated list with first occurrence of each unique item ID
///
/// # Examples
/// ```
/// use serde_json::json;
/// use prodigy::cook::execution::mapreduce::resume_deduplication::deduplicate_work_items;
///
/// let items = vec![
///     json!({"id": "1", "data": "a"}),
///     json!({"id": "2", "data": "b"}),
///     json!({"id": "1", "data": "c"}),  // Duplicate of first
/// ];
/// let deduped = deduplicate_work_items(items);
/// assert_eq!(deduped.len(), 2);
/// ```
pub fn deduplicate_work_items(items: Vec<Value>) -> Vec<Value> {
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut deduped = Vec::new();

    for item in items {
        let item_id = extract_item_id(&item);

        // Skip items without ID or with duplicate ID
        if !item_id.is_empty() && seen_ids.insert(item_id) {
            deduped.push(item);
        }
    }

    deduped
}

/// Extract item ID from work item JSON.
///
/// Tries multiple field names for compatibility:
/// - "id"
/// - "item_id"
/// - "_id"
///
/// Supports both string and numeric IDs (converts numbers to strings).
///
/// # Arguments
/// * `item` - Work item JSON
///
/// # Returns
/// Item ID string, or empty string if not found
fn extract_item_id(item: &Value) -> String {
    // Try common ID field names
    let id_value = item
        .get("id")
        .or_else(|| item.get("item_id"))
        .or_else(|| item.get("_id"));

    match id_value {
        Some(v) => {
            // Try as string first
            if let Some(s) = v.as_str() {
                s.to_string()
            }
            // Then try as integer
            else if let Some(n) = v.as_i64() {
                n.to_string()
            }
            // Then try as unsigned integer
            else if let Some(n) = v.as_u64() {
                n.to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    }
}

/// Count duplicate items in a list.
///
/// Pure function for observability metrics. Useful for logging and monitoring.
///
/// # Arguments
/// * `items` - Work items to analyze
///
/// # Returns
/// Number of duplicate items found
///
/// # Examples
/// ```
/// use serde_json::json;
/// use prodigy::cook::execution::mapreduce::resume_deduplication::count_duplicates;
///
/// let items = vec![
///     json!({"id": "1"}),
///     json!({"id": "2"}),
///     json!({"id": "1"}),  // Duplicate
/// ];
/// assert_eq!(count_duplicates(&items), 1);
/// ```
pub fn count_duplicates(items: &[Value]) -> usize {
    let total = items.len();
    let unique = deduplicate_work_items(items.to_vec()).len();
    total.saturating_sub(unique)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deduplicate_empty_list() {
        let items: Vec<Value> = vec![];
        let result = deduplicate_work_items(items);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_deduplicate_no_duplicates() {
        let items = vec![
            json!({"id": "1", "data": "a"}),
            json!({"id": "2", "data": "b"}),
            json!({"id": "3", "data": "c"}),
        ];
        let result = deduplicate_work_items(items.clone());
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], json!({"id": "1", "data": "a"}));
        assert_eq!(result[1], json!({"id": "2", "data": "b"}));
        assert_eq!(result[2], json!({"id": "3", "data": "c"}));
    }

    #[test]
    fn test_deduplicate_with_duplicates() {
        let items = vec![
            json!({"id": "1", "data": "first"}),
            json!({"id": "2", "data": "second"}),
            json!({"id": "1", "data": "duplicate"}), // Should be removed
            json!({"id": "3", "data": "third"}),
        ];
        let result = deduplicate_work_items(items);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["id"], "1");
        assert_eq!(result[0]["data"], "first"); // First occurrence kept
        assert_eq!(result[1]["id"], "2");
        assert_eq!(result[2]["id"], "3");
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let items = vec![
            json!({"id": "3", "data": "third"}),
            json!({"id": "1", "data": "first"}),
            json!({"id": "2", "data": "second"}),
        ];
        let result = deduplicate_work_items(items.clone());

        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["id"], "3");
        assert_eq!(result[1]["id"], "1");
        assert_eq!(result[2]["id"], "2");
    }

    #[test]
    fn test_deduplicate_missing_ids_skipped() {
        let items = vec![
            json!({"id": "1", "data": "a"}),
            json!({"data": "no_id"}), // No ID field
            json!({"id": "2", "data": "b"}),
        ];
        let result = deduplicate_work_items(items);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["id"], "1");
        assert_eq!(result[1]["id"], "2");
    }

    #[test]
    fn test_deduplicate_large_dataset() {
        use std::time::Instant;

        // Create 10,000 items with 50% duplicates
        let mut items = Vec::new();
        for i in 0..5000 {
            items.push(json!({"id": i.to_string(), "data": "test"}));
            items.push(json!({"id": i.to_string(), "data": "duplicate"}));
        }

        let start = Instant::now();
        let result = deduplicate_work_items(items);
        let duration = start.elapsed();

        assert_eq!(result.len(), 5000);
        assert!(
            duration.as_millis() < 10,
            "Should complete in <10ms, took {}ms",
            duration.as_millis()
        );
    }

    #[test]
    fn test_count_duplicates() {
        let items = vec![
            json!({"id": "1"}),
            json!({"id": "2"}),
            json!({"id": "1"}), // Duplicate
            json!({"id": "3"}),
            json!({"id": "2"}), // Duplicate
        ];

        assert_eq!(count_duplicates(&items), 2);
    }

    #[test]
    fn test_count_duplicates_no_duplicates() {
        let items = vec![json!({"id": "1"}), json!({"id": "2"}), json!({"id": "3"})];

        assert_eq!(count_duplicates(&items), 0);
    }

    #[test]
    fn test_extract_item_id_variants() {
        // Test "id" field
        let item1 = json!({"id": "test-1"});
        assert_eq!(extract_item_id(&item1), "test-1");

        // Test "item_id" field
        let item2 = json!({"item_id": "test-2"});
        assert_eq!(extract_item_id(&item2), "test-2");

        // Test "_id" field
        let item3 = json!({"_id": "test-3"});
        assert_eq!(extract_item_id(&item3), "test-3");

        // Test no ID field
        let item4 = json!({"data": "no-id"});
        assert_eq!(extract_item_id(&item4), "");

        // Test numeric ID (integer)
        let item5 = json!({"id": 42});
        assert_eq!(extract_item_id(&item5), "42");

        // Test numeric ID (unsigned)
        let item6 = json!({"id": 123u64});
        assert_eq!(extract_item_id(&item6), "123");
    }
}
