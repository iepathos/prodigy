//! Pure validation functions for work items using Stillwater's Validation type
//!
//! This module provides error accumulation for work item validation, allowing users
//! to see all validation errors in a single pass rather than fixing them iteratively.
//!
//! ## Architecture
//!
//! - **Pure Functions**: All validation logic is pure (no I/O, deterministic)
//! - **Error Accumulation**: Uses Stillwater's `Validation<T, E>` to collect all errors
//! - **Composable**: Small functions combine via `Validation::all()`
//! - **Testable**: Can test without file system or JSON parsing
//!
//! ## Usage
//!
//! ```rust
//! use crate::cook::execution::data_pipeline::validation::validate_all_items;
//!
//! let items = vec![/* work items from JSON */];
//! let result = validate_all_items(&items);
//!
//! match result {
//!     stillwater::Validation::Success(valid_items) => {
//!         // All items are valid
//!     }
//!     stillwater::Validation::Failure(errors) => {
//!         // See ALL validation errors at once
//!         for error in errors {
//!             println!("Validation error: {}", error);
//!         }
//!     }
//! }
//! ```

use serde_json::Value;
use stillwater::Validation;

/// Validation error for work items
#[derive(Debug, Clone, PartialEq)]
pub enum WorkItemValidationError {
    /// Work item ID is empty
    EmptyId { item_index: usize },

    /// Work item ID is too long (> 255 characters)
    IdTooLong { item_index: usize, length: usize },

    /// Work item ID contains invalid characters
    InvalidIdCharacters {
        item_index: usize,
        id: String,
        reason: String,
    },

    /// Work item data is not a valid JSON object or is null
    InvalidData { item_index: usize, reason: String },

    /// Required field is missing from work item data
    MissingRequiredField { item_index: usize, field: String },

    /// Duplicate work item ID detected
    DuplicateId {
        item_index: usize,
        id: String,
        first_seen_at: usize,
    },
}

impl std::fmt::Display for WorkItemValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyId { item_index } => {
                write!(f, "Item {}: Work item ID cannot be empty", item_index)
            }
            Self::IdTooLong { item_index, length } => {
                write!(
                    f,
                    "Item {}: Work item ID too long ({} characters, max 255)",
                    item_index, length
                )
            }
            Self::InvalidIdCharacters {
                item_index,
                id,
                reason,
            } => {
                write!(
                    f,
                    "Item {}: Invalid characters in ID '{}': {}",
                    item_index, id, reason
                )
            }
            Self::InvalidData { item_index, reason } => {
                write!(f, "Item {}: Invalid data: {}", item_index, reason)
            }
            Self::MissingRequiredField { item_index, field } => {
                write!(f, "Item {}: Missing required field '{}'", item_index, field)
            }
            Self::DuplicateId {
                item_index,
                id,
                first_seen_at,
            } => {
                write!(
                    f,
                    "Item {}: Duplicate ID '{}' (first seen at item {})",
                    item_index, id, first_seen_at
                )
            }
        }
    }
}

impl std::error::Error for WorkItemValidationError {}

/// A validated work item (newtype for type safety)
#[derive(Debug, Clone)]
pub struct ValidWorkItem {
    pub id: String,
    pub data: Value,
}

/// Validate work item ID (pure function)
///
/// Checks that:
/// - ID is not empty
/// - ID is not too long (max 255 characters)
/// - ID contains only valid characters
pub fn validate_item_id(id: &str) -> Validation<String, Vec<WorkItemValidationError>> {
    if id.is_empty() {
        return Validation::failure(vec![WorkItemValidationError::EmptyId {
            item_index: 0, // Will be updated with correct index later
        }]);
    }

    if id.len() > 255 {
        return Validation::failure(vec![WorkItemValidationError::IdTooLong {
            item_index: 0,
            length: id.len(),
        }]);
    }

    // Check for control characters or other problematic characters
    if id.chars().any(|c| c.is_control()) {
        return Validation::failure(vec![WorkItemValidationError::InvalidIdCharacters {
            item_index: 0,
            id: id.to_string(),
            reason: "ID contains control characters".to_string(),
        }]);
    }

    Validation::success(id.to_string())
}

/// Validate work item data (pure function)
///
/// Checks that:
/// - Data is not null
/// - Data is a valid JSON value
pub fn validate_item_data(data: &Value) -> Validation<Value, Vec<WorkItemValidationError>> {
    if data.is_null() {
        return Validation::failure(vec![WorkItemValidationError::InvalidData {
            item_index: 0,
            reason: "Data cannot be null".to_string(),
        }]);
    }

    Validation::success(data.clone())
}

/// Validate a single work item (pure composition)
///
/// Combines ID and data validation using `Validation::all()` to accumulate errors.
pub fn validate_work_item(
    id: &str,
    data: &Value,
) -> Validation<ValidWorkItem, Vec<WorkItemValidationError>> {
    let id_validation = validate_item_id(id);
    let data_validation = validate_item_data(data);

    // Combine validations - accumulates all errors
    match (id_validation, data_validation) {
        (Validation::Success(id), Validation::Success(data)) => {
            Validation::success(ValidWorkItem { id, data })
        }
        (Validation::Failure(mut id_errors), Validation::Failure(mut data_errors)) => {
            id_errors.append(&mut data_errors);
            Validation::failure(id_errors)
        }
        (Validation::Failure(errors), _) | (_, Validation::Failure(errors)) => {
            Validation::failure(errors)
        }
    }
}

/// Validate all work items with error accumulation
///
/// This function validates all work items and collects ALL errors across all items.
/// If any item fails validation, all errors are returned with their item indices.
///
/// # Arguments
///
/// * `items` - Slice of (id, data) tuples representing work items
///
/// # Returns
///
/// * `Validation::Success` - All items are valid
/// * `Validation::Failure` - One or more items failed validation, with ALL errors
pub fn validate_all_items(
    items: &[(String, Value)],
) -> Validation<Vec<ValidWorkItem>, Vec<WorkItemValidationError>> {
    let mut seen_ids = std::collections::HashMap::new();
    let mut valid_items = Vec::new();
    let mut all_errors = Vec::new();

    for (idx, (id, data)) in items.iter().enumerate() {
        // Check for duplicate IDs
        if let Some(&first_idx) = seen_ids.get(id) {
            let duplicate_error = WorkItemValidationError::DuplicateId {
                item_index: idx,
                id: id.clone(),
                first_seen_at: first_idx,
            };
            all_errors.push(duplicate_error);
            continue;
        }
        seen_ids.insert(id.clone(), idx);

        // Validate the item and add index context to errors
        let validation = validate_work_item(id, data).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| update_error_index(error, idx))
                .collect::<Vec<_>>()
        });

        match validation {
            Validation::Success(item) => valid_items.push(item),
            Validation::Failure(errors) => all_errors.extend(errors),
        }
    }

    // Return success if no errors, failure if any errors accumulated
    if all_errors.is_empty() {
        Validation::success(valid_items)
    } else {
        Validation::failure(all_errors)
    }
}

/// Update error with correct item index
fn update_error_index(error: WorkItemValidationError, index: usize) -> WorkItemValidationError {
    match error {
        WorkItemValidationError::EmptyId { .. } => {
            WorkItemValidationError::EmptyId { item_index: index }
        }
        WorkItemValidationError::IdTooLong { length, .. } => WorkItemValidationError::IdTooLong {
            item_index: index,
            length,
        },
        WorkItemValidationError::InvalidIdCharacters { id, reason, .. } => {
            WorkItemValidationError::InvalidIdCharacters {
                item_index: index,
                id,
                reason,
            }
        }
        WorkItemValidationError::InvalidData { reason, .. } => {
            WorkItemValidationError::InvalidData {
                item_index: index,
                reason,
            }
        }
        WorkItemValidationError::MissingRequiredField { field, .. } => {
            WorkItemValidationError::MissingRequiredField {
                item_index: index,
                field,
            }
        }
        WorkItemValidationError::DuplicateId {
            id, first_seen_at, ..
        } => WorkItemValidationError::DuplicateId {
            item_index: index,
            id,
            first_seen_at,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_item_id_success() {
        let result = validate_item_id("valid-id-123");
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_item_id_empty() {
        let result = validate_item_id("");
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(errors[0], WorkItemValidationError::EmptyId { .. }));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_item_id_too_long() {
        let long_id = "x".repeat(300);
        let result = validate_item_id(&long_id);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::IdTooLong { length: 300, .. }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_item_id_control_characters() {
        let id_with_control = "test\x00id";
        let result = validate_item_id(id_with_control);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::InvalidIdCharacters { .. }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_item_data_success() {
        let data = json!({"key": "value"});
        let result = validate_item_data(&data);
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_item_data_null() {
        let data = Value::Null;
        let result = validate_item_data(&data);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::InvalidData { .. }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_work_item_success() {
        let result = validate_work_item("item-1", &json!({"data": "test"}));
        assert!(matches!(result, Validation::Success(_)));
    }

    #[test]
    fn test_validate_work_item_multiple_errors() {
        // Both ID and data are invalid
        let result = validate_work_item("", &Value::Null);
        match result {
            Validation::Failure(errors) => {
                // Should have errors for both ID and data
                assert_eq!(errors.len(), 2);
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::EmptyId { .. })));
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::InvalidData { .. })));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_all_items_success() {
        let items = vec![
            ("item-1".to_string(), json!({"data": "test1"})),
            ("item-2".to_string(), json!({"data": "test2"})),
            ("item-3".to_string(), json!({"data": "test3"})),
        ];

        let result = validate_all_items(&items);
        match result {
            Validation::Success(valid_items) => {
                assert_eq!(valid_items.len(), 3);
                assert_eq!(valid_items[0].id, "item-1");
                assert_eq!(valid_items[1].id, "item-2");
                assert_eq!(valid_items[2].id, "item-3");
            }
            _ => panic!("Expected validation success"),
        }
    }

    #[test]
    fn test_validate_all_items_accumulates_errors() {
        let items = vec![
            ("".to_string(), json!({"data": "test1"})), // Empty ID
            ("item-2".to_string(), json!({"data": "test2"})), // Valid
            ("x".repeat(300), Value::Null),             // ID too long, null data
        ];

        let result = validate_all_items(&items);
        match result {
            Validation::Failure(errors) => {
                // Should have errors from items 0 and 2
                assert!(errors.len() >= 3); // EmptyId, IdTooLong, InvalidData

                // Check that item indices are correctly set
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::EmptyId { item_index: 0 })));
                assert!(errors.iter().any(|e| matches!(
                    e,
                    WorkItemValidationError::IdTooLong { item_index: 2, .. }
                )));
                assert!(errors.iter().any(|e| matches!(
                    e,
                    WorkItemValidationError::InvalidData { item_index: 2, .. }
                )));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_all_items_duplicate_ids() {
        let items = vec![
            ("item-1".to_string(), json!({"data": "test1"})),
            ("item-2".to_string(), json!({"data": "test2"})),
            ("item-1".to_string(), json!({"data": "test3"})), // Duplicate
        ];

        let result = validate_all_items(&items);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                match &errors[0] {
                    WorkItemValidationError::DuplicateId {
                        item_index,
                        id,
                        first_seen_at,
                    } => {
                        assert_eq!(*item_index, 2);
                        assert_eq!(id, "item-1");
                        assert_eq!(*first_seen_at, 0);
                    }
                    _ => panic!("Expected DuplicateId error"),
                }
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_error_display_messages() {
        let empty_id_error = WorkItemValidationError::EmptyId { item_index: 5 };
        assert_eq!(
            empty_id_error.to_string(),
            "Item 5: Work item ID cannot be empty"
        );

        let id_too_long_error = WorkItemValidationError::IdTooLong {
            item_index: 3,
            length: 300,
        };
        assert_eq!(
            id_too_long_error.to_string(),
            "Item 3: Work item ID too long (300 characters, max 255)"
        );

        let duplicate_error = WorkItemValidationError::DuplicateId {
            item_index: 7,
            id: "test-id".to_string(),
            first_seen_at: 2,
        };
        assert_eq!(
            duplicate_error.to_string(),
            "Item 7: Duplicate ID 'test-id' (first seen at item 2)"
        );
    }
}
