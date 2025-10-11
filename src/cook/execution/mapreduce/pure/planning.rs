//! Pure functions for execution planning
//!
//! These functions calculate parallelism, distribute work items,
//! and determine optimal execution strategies.

use serde_json::Value;

/// Execution phase
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Setup,
    Map,
    Reduce,
}

/// Calculate optimal parallelism level
///
/// # Arguments
///
/// * `total_items` - Total number of items to process
/// * `max_parallel` - Maximum parallel workers allowed
///
/// # Returns
///
/// Optimal parallelism level (0 if no items)
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::pure::planning::calculate_parallelism;
///
/// assert_eq!(calculate_parallelism(100, 10), 10);
/// assert_eq!(calculate_parallelism(5, 10), 5);
/// assert_eq!(calculate_parallelism(0, 10), 0);
/// ```
pub fn calculate_parallelism(total_items: usize, max_parallel: usize) -> usize {
    if total_items == 0 {
        return 0;
    }
    std::cmp::min(total_items, max_parallel)
}

/// Determine execution phases
///
/// # Arguments
///
/// * `has_setup` - Whether setup phase exists
/// * `has_reduce` - Whether reduce phase exists
///
/// # Returns
///
/// Vector of phases in execution order
pub fn plan_execution_phases(has_setup: bool, has_reduce: bool) -> Vec<Phase> {
    let mut phases = Vec::new();

    if has_setup {
        phases.push(Phase::Setup);
    }
    phases.push(Phase::Map);
    if has_reduce {
        phases.push(Phase::Reduce);
    }

    phases
}

/// Distribute work items across agents
///
/// # Arguments
///
/// * `items` - Work items to distribute
/// * `parallelism` - Number of agents to distribute across
///
/// # Returns
///
/// Vector of item chunks, one per agent
pub fn distribute_work(items: Vec<Value>, parallelism: usize) -> Vec<Vec<Value>> {
    if items.is_empty() || parallelism == 0 {
        return vec![];
    }

    let chunk_size = items.len().div_ceil(parallelism);
    items
        .chunks(chunk_size)
        .map(|chunk| chunk.to_vec())
        .collect()
}

/// Calculate batch size for processing
///
/// # Arguments
///
/// * `total_items` - Total number of items
/// * `available_resources` - Number of available resources
/// * `max_batch` - Maximum batch size allowed
///
/// # Returns
///
/// Optimal batch size
pub fn calculate_batch_size(
    total_items: usize,
    available_resources: usize,
    max_batch: usize,
) -> usize {
    if available_resources == 0 {
        return 0;
    }
    let ideal_batch = total_items.div_ceil(available_resources);
    std::cmp::min(ideal_batch, max_batch)
}

/// Determine if work should be batched
///
/// # Arguments
///
/// * `item_count` - Number of items to process
/// * `threshold` - Threshold for batching
///
/// # Returns
///
/// True if batching is recommended
pub fn should_batch(item_count: usize, threshold: usize) -> bool {
    item_count > threshold
}

/// Sort items by priority
///
/// # Arguments
///
/// * `items` - Items to sort
/// * `priority_field` - Field name containing priority value
/// * `descending` - Whether to sort in descending order
///
/// # Returns
///
/// Sorted vector of items
pub fn sort_by_priority(
    mut items: Vec<Value>,
    priority_field: &str,
    descending: bool,
) -> Vec<Value> {
    items.sort_by(|a, b| {
        let a_priority = extract_priority(a, priority_field);
        let b_priority = extract_priority(b, priority_field);

        if descending {
            b_priority.cmp(&a_priority)
        } else {
            a_priority.cmp(&b_priority)
        }
    });
    items
}

/// Extract priority value from item
fn extract_priority(item: &Value, field: &str) -> i64 {
    item.get(field).and_then(|v| v.as_i64()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_calculate_parallelism_normal() {
        assert_eq!(calculate_parallelism(100, 10), 10);
    }

    #[test]
    fn test_calculate_parallelism_fewer_items_than_max() {
        assert_eq!(calculate_parallelism(5, 10), 5);
    }

    #[test]
    fn test_calculate_parallelism_zero_items() {
        assert_eq!(calculate_parallelism(0, 10), 0);
    }

    #[test]
    fn test_plan_execution_phases_all() {
        let phases = plan_execution_phases(true, true);
        assert_eq!(phases.len(), 3);
        assert_eq!(phases[0], Phase::Setup);
        assert_eq!(phases[1], Phase::Map);
        assert_eq!(phases[2], Phase::Reduce);
    }

    #[test]
    fn test_plan_execution_phases_map_only() {
        let phases = plan_execution_phases(false, false);
        assert_eq!(phases.len(), 1);
        assert_eq!(phases[0], Phase::Map);
    }

    #[test]
    fn test_plan_execution_phases_setup_and_map() {
        let phases = plan_execution_phases(true, false);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0], Phase::Setup);
        assert_eq!(phases[1], Phase::Map);
    }

    #[test]
    fn test_distribute_work_even_distribution() {
        let items = vec![json!(1), json!(2), json!(3), json!(4)];
        let distributed = distribute_work(items, 2);
        assert_eq!(distributed.len(), 2);
        assert_eq!(distributed[0].len(), 2);
        assert_eq!(distributed[1].len(), 2);
    }

    #[test]
    fn test_distribute_work_uneven_distribution() {
        let items = vec![json!(1), json!(2), json!(3), json!(4), json!(5)];
        let distributed = distribute_work(items, 2);
        assert_eq!(distributed.len(), 2);
        assert_eq!(distributed[0].len(), 3);
        assert_eq!(distributed[1].len(), 2);
    }

    #[test]
    fn test_distribute_work_empty() {
        let distributed = distribute_work(vec![], 2);
        assert_eq!(distributed.len(), 0);
    }

    #[test]
    fn test_distribute_work_zero_parallelism() {
        let items = vec![json!(1), json!(2)];
        let distributed = distribute_work(items, 0);
        assert_eq!(distributed.len(), 0);
    }

    #[test]
    fn test_calculate_batch_size_normal() {
        assert_eq!(calculate_batch_size(100, 10, 20), 10);
    }

    #[test]
    fn test_calculate_batch_size_exceeds_max() {
        assert_eq!(calculate_batch_size(100, 2, 20), 20);
    }

    #[test]
    fn test_calculate_batch_size_zero_resources() {
        assert_eq!(calculate_batch_size(100, 0, 20), 0);
    }

    #[test]
    fn test_should_batch_above_threshold() {
        assert!(should_batch(100, 50));
    }

    #[test]
    fn test_should_batch_below_threshold() {
        assert!(!should_batch(30, 50));
    }

    #[test]
    fn test_should_batch_at_threshold() {
        assert!(!should_batch(50, 50));
    }

    #[test]
    fn test_sort_by_priority_descending() {
        let items = vec![
            json!({"name": "a", "priority": 1}),
            json!({"name": "b", "priority": 5}),
            json!({"name": "c", "priority": 3}),
        ];
        let sorted = sort_by_priority(items, "priority", true);
        assert_eq!(sorted[0].get("priority").unwrap().as_i64().unwrap(), 5);
        assert_eq!(sorted[1].get("priority").unwrap().as_i64().unwrap(), 3);
        assert_eq!(sorted[2].get("priority").unwrap().as_i64().unwrap(), 1);
    }

    #[test]
    fn test_sort_by_priority_ascending() {
        let items = vec![
            json!({"name": "a", "priority": 5}),
            json!({"name": "b", "priority": 1}),
            json!({"name": "c", "priority": 3}),
        ];
        let sorted = sort_by_priority(items, "priority", false);
        assert_eq!(sorted[0].get("priority").unwrap().as_i64().unwrap(), 1);
        assert_eq!(sorted[1].get("priority").unwrap().as_i64().unwrap(), 3);
        assert_eq!(sorted[2].get("priority").unwrap().as_i64().unwrap(), 5);
    }

    #[test]
    fn test_sort_by_priority_missing_field() {
        let items = vec![
            json!({"name": "a"}),
            json!({"name": "b", "priority": 5}),
            json!({"name": "c", "priority": 3}),
        ];
        let sorted = sort_by_priority(items, "priority", true);
        // Item without priority gets default 0
        assert_eq!(sorted[2].get("priority"), None);
    }
}
