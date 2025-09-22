//! Pure MapReduce business logic functions
//!
//! These functions handle MapReduce computations without performing any I/O operations.

use serde_json::Value;
use std::collections::HashMap;

/// Work item filtering result
#[derive(Debug, Clone)]
pub struct FilteredWorkItem {
    pub item: Value,
    pub passed: bool,
    pub reason: Option<String>,
}

/// Filter work items based on a filter expression
pub fn filter_work_items(items: Vec<Value>, filter_expr: Option<&str>) -> Vec<FilteredWorkItem> {
    if filter_expr.is_none() {
        return items
            .into_iter()
            .map(|item| FilteredWorkItem {
                item,
                passed: true,
                reason: None,
            })
            .collect();
    }

    let filter = filter_expr.unwrap();

    items
        .into_iter()
        .map(|item| {
            let passed = evaluate_filter(&item, filter);
            FilteredWorkItem {
                item,
                passed,
                reason: if !passed {
                    Some(format!("Failed filter: {}", filter))
                } else {
                    None
                },
            }
        })
        .collect()
}

/// Simple filter evaluation (pure function)
fn evaluate_filter(item: &Value, filter: &str) -> bool {
    // Basic implementation - can be enhanced with expression parser
    if filter.contains(">=") {
        let parts: Vec<&str> = filter.split(">=").collect();
        if parts.len() == 2 {
            let field = parts[0].trim();
            let value = parts[1].trim();

            if let Some(field_value) = get_field(item, field) {
                if let (Ok(field_num), Ok(value_num)) =
                    (field_value.as_f64().ok_or(()), value.parse::<f64>())
                {
                    return field_num >= value_num;
                }
            }
        }
    }

    true // Default to passing if we can't evaluate
}

/// Extract field value from JSON item
fn get_field<'a>(item: &'a Value, field_path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = field_path.split('.').collect();
    let mut current = item;

    for part in parts {
        match current.get(part) {
            Some(value) => current = value,
            None => return None,
        }
    }

    Some(current)
}

/// Sort work items based on a sort expression
pub fn sort_work_items(mut items: Vec<Value>, sort_expr: Option<&str>) -> Vec<Value> {
    if let Some(expr) = sort_expr {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if !parts.is_empty() {
            let field = parts[0];
            let descending = parts.len() > 1 && parts[1].to_uppercase() == "DESC";

            items.sort_by(|a, b| {
                let a_val = get_field(a, field);
                let b_val = get_field(b, field);

                match (a_val, b_val) {
                    (Some(a), Some(b)) => {
                        let cmp = if let (Some(a_num), Some(b_num)) = (a.as_f64(), b.as_f64()) {
                            a_num
                                .partial_cmp(&b_num)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        } else if let (Some(a_str), Some(b_str)) = (a.as_str(), b.as_str()) {
                            a_str.cmp(b_str)
                        } else {
                            std::cmp::Ordering::Equal
                        };

                        if descending {
                            cmp.reverse()
                        } else {
                            cmp
                        }
                    }
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }
    }

    items
}

/// Calculate work distribution across agents
#[derive(Debug, Clone)]
pub struct WorkDistribution {
    pub agent_id: String,
    pub items: Vec<Value>,
    pub start_index: usize,
    pub end_index: usize,
}

pub fn distribute_work(
    items: Vec<Value>,
    max_parallel: usize,
    offset: Option<usize>,
    max_items: Option<usize>,
) -> Vec<WorkDistribution> {
    let start = offset.unwrap_or(0);
    let end = if let Some(max) = max_items {
        (start + max).min(items.len())
    } else {
        items.len()
    };

    if start >= items.len() {
        return Vec::new();
    }

    let work_slice = &items[start..end];
    let total_items = work_slice.len();
    let actual_agents = max_parallel.min(total_items);

    if actual_agents == 0 {
        return Vec::new();
    }

    let items_per_agent = total_items / actual_agents;
    let remainder = total_items % actual_agents;

    let mut distributions = Vec::new();
    let mut current_start = 0;

    for i in 0..actual_agents {
        let agent_items = items_per_agent + if i < remainder { 1 } else { 0 };
        let agent_end = current_start + agent_items;

        distributions.push(WorkDistribution {
            agent_id: format!("agent-{}", i + 1),
            items: work_slice[current_start..agent_end].to_vec(),
            start_index: start + current_start,
            end_index: start + agent_end,
        });

        current_start = agent_end;
    }

    distributions
}

/// Aggregate map results
#[derive(Debug, Clone)]
pub struct MapResultSummary {
    pub total_items: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub average_time_ms: f64,
}

pub fn aggregate_map_results(results: Vec<HashMap<String, Value>>) -> MapResultSummary {
    let total_items = results.len();
    let mut successful = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut total_time_ms = 0.0;
    let mut time_count = 0;

    for result in &results {
        if let Some(status) = result.get("status").and_then(|v| v.as_str()) {
            match status {
                "success" => successful += 1,
                "failed" => failed += 1,
                "skipped" => skipped += 1,
                _ => {}
            }
        }

        if let Some(time) = result.get("duration_ms").and_then(|v| v.as_f64()) {
            total_time_ms += time;
            time_count += 1;
        }
    }

    MapResultSummary {
        total_items,
        successful,
        failed,
        skipped,
        average_time_ms: if time_count > 0 {
            total_time_ms / time_count as f64
        } else {
            0.0
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_filter_work_items() {
        let items = vec![
            json!({"score": 10, "name": "item1"}),
            json!({"score": 5, "name": "item2"}),
            json!({"score": 8, "name": "item3"}),
        ];

        // No filter
        let result = filter_work_items(items.clone(), None);
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|r| r.passed));

        // With filter
        let result = filter_work_items(items, Some("score >= 7"));
        assert_eq!(result.iter().filter(|r| r.passed).count(), 2);
    }

    #[test]
    fn test_sort_work_items() {
        let items = vec![
            json!({"score": 10, "name": "item1"}),
            json!({"score": 5, "name": "item2"}),
            json!({"score": 8, "name": "item3"}),
        ];

        // Ascending
        let sorted = sort_work_items(items.clone(), Some("score"));
        assert_eq!(sorted[0]["score"], 5);
        assert_eq!(sorted[2]["score"], 10);

        // Descending
        let sorted = sort_work_items(items, Some("score DESC"));
        assert_eq!(sorted[0]["score"], 10);
        assert_eq!(sorted[2]["score"], 5);
    }

    #[test]
    fn test_distribute_work() {
        let items = vec![
            json!({"id": 1}),
            json!({"id": 2}),
            json!({"id": 3}),
            json!({"id": 4}),
            json!({"id": 5}),
        ];

        let distribution = distribute_work(items.clone(), 2, None, None);
        assert_eq!(distribution.len(), 2);
        assert_eq!(distribution[0].items.len(), 3); // First agent gets 3
        assert_eq!(distribution[1].items.len(), 2); // Second agent gets 2

        // With offset and limit
        let distribution = distribute_work(items, 2, Some(1), Some(3));
        assert_eq!(distribution.len(), 2);
        let total_items: usize = distribution.iter().map(|d| d.items.len()).sum();
        assert_eq!(total_items, 3);
    }

    #[test]
    fn test_aggregate_map_results() {
        let results = vec![
            HashMap::from([
                ("status".to_string(), json!("success")),
                ("duration_ms".to_string(), json!(100.0)),
            ]),
            HashMap::from([
                ("status".to_string(), json!("failed")),
                ("duration_ms".to_string(), json!(200.0)),
            ]),
            HashMap::from([
                ("status".to_string(), json!("success")),
                ("duration_ms".to_string(), json!(150.0)),
            ]),
        ];

        let summary = aggregate_map_results(results);
        assert_eq!(summary.total_items, 3);
        assert_eq!(summary.successful, 2);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.average_time_ms, 150.0);
    }
}
