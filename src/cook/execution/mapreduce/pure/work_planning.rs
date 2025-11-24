//! Pure work assignment planning for MapReduce execution
//!
//! This module provides pure functions for planning work distribution
//! without any I/O operations, enabling testability and composition.

use serde_json::Value;
use std::collections::HashMap;

/// Configuration for work planning
#[derive(Debug, Clone)]
pub struct WorkPlanConfig {
    /// Optional filter expression
    pub filter: Option<FilterExpression>,
    /// Number of items to skip
    pub offset: usize,
    /// Maximum number of items to process
    pub max_items: Option<usize>,
}

/// Filter expression for work items
#[derive(Debug, Clone)]
pub enum FilterExpression {
    /// Equality check
    Equals { field: String, value: Value },
    /// Greater than check
    GreaterThan { field: String, value: Value },
    /// Less than check
    LessThan { field: String, value: Value },
    /// Contains check for arrays/strings
    Contains { field: String, value: Value },
    /// Logical AND of multiple filters
    And(Vec<FilterExpression>),
    /// Logical OR of multiple filters
    Or(Vec<FilterExpression>),
}

/// A work assignment ready for execution
#[derive(Debug, Clone, PartialEq)]
pub struct WorkAssignment {
    /// Unique ID for this assignment
    pub id: usize,
    /// The work item data
    pub item: Value,
    /// Name of the worktree for this assignment
    pub worktree_name: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Pure: Plan work assignments from input items
///
/// Takes a list of work items and configuration, applies filtering and limiting,
/// and produces a list of work assignments ready for execution.
///
/// This function is pure and deterministic - same inputs always produce same outputs.
pub fn plan_work_assignments(items: Vec<Value>, config: &WorkPlanConfig) -> Vec<WorkAssignment> {
    // Apply filter, offset, and limit in sequence
    let filtered = apply_filter(items, &config.filter);
    let limited = apply_limits(filtered, config.offset, config.max_items);

    // Convert to work assignments with worktree names
    limited
        .into_iter()
        .enumerate()
        .map(|(idx, item)| WorkAssignment {
            id: idx,
            item,
            worktree_name: format!("agent-{}", idx),
            metadata: HashMap::new(),
        })
        .collect()
}

/// Pure: Apply filter predicates to work items
fn apply_filter(items: Vec<Value>, filter: &Option<FilterExpression>) -> Vec<Value> {
    match filter {
        Some(expr) => items
            .into_iter()
            .filter(|item| evaluate_filter(item, expr))
            .collect(),
        None => items,
    }
}

/// Pure: Evaluate filter expression against a work item
fn evaluate_filter(item: &Value, expr: &FilterExpression) -> bool {
    match expr {
        FilterExpression::Equals { field, value } => get_field_value(item, field) == Some(value),
        FilterExpression::GreaterThan { field, value } => {
            match (get_field_value(item, field), value) {
                (Some(Value::Number(item_val)), Value::Number(filter_val)) => {
                    item_val.as_f64().unwrap_or(0.0) > filter_val.as_f64().unwrap_or(0.0)
                }
                _ => false,
            }
        }
        FilterExpression::LessThan { field, value } => {
            match (get_field_value(item, field), value) {
                (Some(Value::Number(item_val)), Value::Number(filter_val)) => {
                    item_val.as_f64().unwrap_or(0.0) < filter_val.as_f64().unwrap_or(0.0)
                }
                _ => false,
            }
        }
        FilterExpression::Contains { field, value } => match get_field_value(item, field) {
            Some(Value::Array(arr)) => arr.contains(value),
            Some(Value::String(s)) => {
                if let Value::String(substr) = value {
                    s.contains(substr)
                } else {
                    false
                }
            }
            _ => false,
        },
        FilterExpression::And(exprs) => exprs.iter().all(|e| evaluate_filter(item, e)),
        FilterExpression::Or(exprs) => exprs.iter().any(|e| evaluate_filter(item, e)),
    }
}

/// Pure: Extract field value from JSON using dot notation
fn get_field_value<'a>(item: &'a Value, field: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = field.split('.').collect();
    let mut current = item;

    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(part)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

/// Pure: Apply offset and max_items limits
fn apply_limits(items: Vec<Value>, offset: usize, max_items: Option<usize>) -> Vec<Value> {
    let iter = items.into_iter().skip(offset);

    match max_items {
        Some(limit) => iter.take(limit).collect(),
        None => iter.collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_plan_work_assignments_no_filter() {
        let items = vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})];

        let config = WorkPlanConfig {
            filter: None,
            offset: 0,
            max_items: None,
        };

        let assignments = plan_work_assignments(items.clone(), &config);

        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[0].id, 0);
        assert_eq!(assignments[0].item, items[0]);
        assert_eq!(assignments[0].worktree_name, "agent-0");
    }

    #[test]
    fn test_plan_work_assignments_with_offset_and_limit() {
        let items = vec![
            json!({"id": 1}),
            json!({"id": 2}),
            json!({"id": 3}),
            json!({"id": 4}),
            json!({"id": 5}),
        ];

        let config = WorkPlanConfig {
            filter: None,
            offset: 1,
            max_items: Some(2),
        };

        let assignments = plan_work_assignments(items, &config);

        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].item["id"], 2);
        assert_eq!(assignments[1].item["id"], 3);
    }

    #[test]
    fn test_filter_equals() {
        let items = vec![
            json!({"type": "a", "value": 1}),
            json!({"type": "b", "value": 2}),
            json!({"type": "a", "value": 3}),
        ];

        let config = WorkPlanConfig {
            filter: Some(FilterExpression::Equals {
                field: "type".to_string(),
                value: json!("a"),
            }),
            offset: 0,
            max_items: None,
        };

        let assignments = plan_work_assignments(items, &config);

        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].item["value"], 1);
        assert_eq!(assignments[1].item["value"], 3);
    }

    #[test]
    fn test_filter_greater_than() {
        let items = vec![
            json!({"value": 5}),
            json!({"value": 15}),
            json!({"value": 10}),
        ];

        let config = WorkPlanConfig {
            filter: Some(FilterExpression::GreaterThan {
                field: "value".to_string(),
                value: json!(10),
            }),
            offset: 0,
            max_items: None,
        };

        let assignments = plan_work_assignments(items, &config);

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].item["value"], 15);
    }

    #[test]
    fn test_filter_and() {
        let items = vec![
            json!({"type": "a", "value": 5}),
            json!({"type": "a", "value": 15}),
            json!({"type": "b", "value": 15}),
        ];

        let config = WorkPlanConfig {
            filter: Some(FilterExpression::And(vec![
                FilterExpression::Equals {
                    field: "type".to_string(),
                    value: json!("a"),
                },
                FilterExpression::GreaterThan {
                    field: "value".to_string(),
                    value: json!(10),
                },
            ])),
            offset: 0,
            max_items: None,
        };

        let assignments = plan_work_assignments(items, &config);

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].item["value"], 15);
        assert_eq!(assignments[0].item["type"], "a");
    }

    #[test]
    fn test_nested_field_access() {
        let items = vec![
            json!({"metadata": {"priority": 1}}),
            json!({"metadata": {"priority": 5}}),
        ];

        let config = WorkPlanConfig {
            filter: Some(FilterExpression::GreaterThan {
                field: "metadata.priority".to_string(),
                value: json!(2),
            }),
            offset: 0,
            max_items: None,
        };

        let assignments = plan_work_assignments(items, &config);

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].item["metadata"]["priority"], 5);
    }
}
