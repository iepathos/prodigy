//! Work item validation for MapReduce workflows (Spec 176)
//!
//! This module provides comprehensive validation for work items before MapReduce
//! execution, using Stillwater's `Validation` applicative functor for error accumulation.
//!
//! ## Key Features
//!
//! - **Error Accumulation**: All validation errors are collected and reported together
//! - **Schema-Based Validation**: Optional schema for field types and constraints
//! - **DLQ Integration**: Validation failures can be added to the Dead Letter Queue
//! - **Pure Functions**: All validation logic is pure and testable
//!
//! ## Usage
//!
//! ```rust
//! use prodigy::cook::execution::mapreduce::validation::{
//!     validate_work_items, WorkItemSchema, FieldType,
//! };
//! use serde_json::json;
//! use stillwater::Validation;
//!
//! let items = vec![
//!     json!({"id": "item-1", "count": 5}),
//!     json!({"id": "item-2", "count": "invalid"}),  // Type error
//! ];
//!
//! let schema = WorkItemSchema::new()
//!     .require_field("id")
//!     .field_type("count", FieldType::Number);
//!
//! match validate_work_items(&items, Some(&schema)) {
//!     Validation::Success(valid_items) => {
//!         // All items valid
//!     }
//!     Validation::Failure(errors) => {
//!         // ALL errors reported at once
//!         for error in errors {
//!             eprintln!("{}", error);
//!         }
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use stillwater::Validation;

/// Validation error for work items
///
/// Provides detailed context about validation failures including
/// item index and field path for easy debugging.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkItemValidationError {
    /// Required field is missing from work item
    MissingRequiredField { item_index: usize, field: String },

    /// Field type does not match expected type
    InvalidFieldType {
        item_index: usize,
        field: String,
        expected: String,
        got: String,
    },

    /// Field value violates a constraint
    ConstraintViolation {
        item_index: usize,
        field: String,
        constraint: String,
        value: String,
    },

    /// Work item is not a JSON object
    NotAnObject { item_index: usize },

    /// Work item is null
    NullItem { item_index: usize },

    /// Duplicate item ID detected
    DuplicateId {
        item_index: usize,
        id: String,
        first_seen_at: usize,
    },

    /// Work item ID is invalid (empty, too long, etc.)
    InvalidId { item_index: usize, reason: String },
}

impl std::fmt::Display for WorkItemValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequiredField { item_index, field } => {
                write!(
                    f,
                    "Work item #{}: missing required field '{}'",
                    item_index, field
                )
            }
            Self::InvalidFieldType {
                item_index,
                field,
                expected,
                got,
            } => {
                write!(
                    f,
                    "Work item #{}, field '{}': expected {}, got {}",
                    item_index, field, expected, got
                )
            }
            Self::ConstraintViolation {
                item_index,
                field,
                constraint,
                value,
            } => {
                write!(
                    f,
                    "Work item #{}, field '{}': value '{}' violates constraint: {}",
                    item_index, field, value, constraint
                )
            }
            Self::NotAnObject { item_index } => {
                write!(f, "Work item #{}: must be a JSON object", item_index)
            }
            Self::NullItem { item_index } => {
                write!(f, "Work item #{}: cannot be null", item_index)
            }
            Self::DuplicateId {
                item_index,
                id,
                first_seen_at,
            } => {
                write!(
                    f,
                    "Work item #{}: duplicate ID '{}' (first seen at item #{})",
                    item_index, id, first_seen_at
                )
            }
            Self::InvalidId { item_index, reason } => {
                write!(f, "Work item #{}: invalid ID: {}", item_index, reason)
            }
        }
    }
}

impl std::error::Error for WorkItemValidationError {}

/// Field type for schema validation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    /// String value
    String,
    /// Numeric value (integer or float)
    Number,
    /// Boolean value
    Bool,
    /// Array of values
    Array,
    /// Nested object
    Object,
    /// Any type (no type checking)
    Any,
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Bool => write!(f, "boolean"),
            Self::Array => write!(f, "array"),
            Self::Object => write!(f, "object"),
            Self::Any => write!(f, "any"),
        }
    }
}

/// Constraint for field validation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    /// Numeric range (min, max)
    Range { min: f64, max: f64 },
    /// Minimum string length
    MinLength(usize),
    /// Maximum string length
    MaxLength(usize),
    /// Value must be one of these options
    OneOf(Vec<Value>),
    /// Custom regex pattern (stored as string)
    Pattern(String),
}

impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Range { min, max } => write!(f, "range [{}, {}]", min, max),
            Self::MinLength(len) => write!(f, "minimum length {}", len),
            Self::MaxLength(len) => write!(f, "maximum length {}", len),
            Self::OneOf(values) => {
                let opts: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                write!(f, "one of [{}]", opts.join(", "))
            }
            Self::Pattern(p) => write!(f, "pattern '{}'", p),
        }
    }
}

/// Schema for work item validation
///
/// Defines required fields, field types, and constraints for work items.
#[derive(Debug, Clone, Default)]
pub struct WorkItemSchema {
    /// Required field names
    pub required_fields: HashSet<String>,
    /// Field type expectations
    pub field_types: HashMap<String, FieldType>,
    /// Field constraints
    pub constraints: HashMap<String, Vec<Constraint>>,
    /// Field used as unique ID (default: "id")
    pub id_field: Option<String>,
}

impl WorkItemSchema {
    /// Create a new empty schema
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a field as required
    pub fn require_field(mut self, field: &str) -> Self {
        self.required_fields.insert(field.to_string());
        self
    }

    /// Set expected type for a field
    pub fn field_type(mut self, field: &str, field_type: FieldType) -> Self {
        self.field_types.insert(field.to_string(), field_type);
        self
    }

    /// Add a constraint to a field
    pub fn add_constraint(mut self, field: &str, constraint: Constraint) -> Self {
        self.constraints
            .entry(field.to_string())
            .or_default()
            .push(constraint);
        self
    }

    /// Set the ID field name
    pub fn id_field(mut self, field: &str) -> Self {
        self.id_field = Some(field.to_string());
        self
    }
}

/// A validated work item
#[derive(Debug, Clone)]
pub struct ValidatedWorkItem {
    /// Item index in original list
    pub index: usize,
    /// Validated data
    pub data: Value,
    /// Extracted ID (if available)
    pub id: Option<String>,
}

/// Validate all work items with error accumulation
///
/// This is the main entry point for work item validation. It validates
/// all items and accumulates ALL errors before returning.
///
/// # Arguments
///
/// * `items` - The work items to validate
/// * `schema` - Optional schema for type and constraint validation
///
/// # Returns
///
/// * `Validation::Success` - All items are valid
/// * `Validation::Failure` - One or more items failed, with ALL errors
pub fn validate_work_items(
    items: &[Value],
    schema: Option<&WorkItemSchema>,
) -> Validation<Vec<ValidatedWorkItem>, Vec<WorkItemValidationError>> {
    let mut all_errors = Vec::new();
    let mut valid_items = Vec::new();
    let mut seen_ids: HashMap<String, usize> = HashMap::new();

    let id_field = schema.and_then(|s| s.id_field.as_deref()).unwrap_or("id");

    for (idx, item) in items.iter().enumerate() {
        // Validate single item and accumulate errors
        let item_errors = validate_single_item(idx, item, schema);

        if item_errors.is_empty() {
            // Check for duplicate IDs
            if let Some(id) = extract_id(item, id_field) {
                if let Some(&first_idx) = seen_ids.get(&id) {
                    all_errors.push(WorkItemValidationError::DuplicateId {
                        item_index: idx,
                        id: id.clone(),
                        first_seen_at: first_idx,
                    });
                } else {
                    seen_ids.insert(id.clone(), idx);
                    valid_items.push(ValidatedWorkItem {
                        index: idx,
                        data: item.clone(),
                        id: Some(id),
                    });
                }
            } else {
                // No ID field, still valid
                valid_items.push(ValidatedWorkItem {
                    index: idx,
                    data: item.clone(),
                    id: None,
                });
            }
        } else {
            all_errors.extend(item_errors);
        }
    }

    if all_errors.is_empty() {
        Validation::success(valid_items)
    } else {
        Validation::failure(all_errors)
    }
}

/// Validate a single work item
fn validate_single_item(
    idx: usize,
    item: &Value,
    schema: Option<&WorkItemSchema>,
) -> Vec<WorkItemValidationError> {
    let mut errors = Vec::new();

    // Check for null
    if item.is_null() {
        errors.push(WorkItemValidationError::NullItem { item_index: idx });
        return errors;
    }

    // Check it's an object
    if !item.is_object() {
        errors.push(WorkItemValidationError::NotAnObject { item_index: idx });
        return errors;
    }

    // Apply schema validation if provided
    if let Some(schema) = schema {
        errors.extend(validate_required_fields(idx, item, schema));
        errors.extend(validate_field_types(idx, item, schema));
        errors.extend(validate_constraints(idx, item, schema));
    }

    errors
}

/// Validate required fields are present
fn validate_required_fields(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Vec<WorkItemValidationError> {
    schema
        .required_fields
        .iter()
        .filter(|field| item.get(field.as_str()).is_none())
        .map(|field| WorkItemValidationError::MissingRequiredField {
            item_index: idx,
            field: field.clone(),
        })
        .collect()
}

/// Validate field types match schema
fn validate_field_types(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Vec<WorkItemValidationError> {
    schema
        .field_types
        .iter()
        .filter_map(|(field, expected_type)| {
            item.get(field.as_str()).and_then(|value| {
                if matches_type(value, expected_type) {
                    None
                } else {
                    Some(WorkItemValidationError::InvalidFieldType {
                        item_index: idx,
                        field: field.clone(),
                        expected: expected_type.to_string(),
                        got: json_type_name(value),
                    })
                }
            })
        })
        .collect()
}

/// Validate field constraints
fn validate_constraints(
    idx: usize,
    item: &Value,
    schema: &WorkItemSchema,
) -> Vec<WorkItemValidationError> {
    let mut errors = Vec::new();

    for (field, constraints) in &schema.constraints {
        if let Some(value) = item.get(field.as_str()) {
            for constraint in constraints {
                if !satisfies_constraint(value, constraint) {
                    errors.push(WorkItemValidationError::ConstraintViolation {
                        item_index: idx,
                        field: field.clone(),
                        constraint: constraint.to_string(),
                        value: value.to_string(),
                    });
                }
            }
        }
    }

    errors
}

/// Check if a JSON value matches an expected type
fn matches_type(value: &Value, expected: &FieldType) -> bool {
    match expected {
        FieldType::String => value.is_string(),
        FieldType::Number => value.is_number(),
        FieldType::Bool => value.is_boolean(),
        FieldType::Array => value.is_array(),
        FieldType::Object => value.is_object(),
        FieldType::Any => true,
    }
}

/// Get the type name of a JSON value
fn json_type_name(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

/// Check if a value satisfies a constraint
fn satisfies_constraint(value: &Value, constraint: &Constraint) -> bool {
    match constraint {
        Constraint::Range { min, max } => {
            if let Some(n) = value.as_f64() {
                n >= *min && n <= *max
            } else {
                false
            }
        }
        Constraint::MinLength(min) => {
            if let Some(s) = value.as_str() {
                s.len() >= *min
            } else {
                false
            }
        }
        Constraint::MaxLength(max) => {
            if let Some(s) = value.as_str() {
                s.len() <= *max
            } else {
                false
            }
        }
        Constraint::OneOf(options) => options.contains(value),
        Constraint::Pattern(pattern) => {
            if let Some(s) = value.as_str() {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(s))
                    .unwrap_or(false)
            } else {
                false
            }
        }
    }
}

/// Extract ID from a work item
fn extract_id(item: &Value, id_field: &str) -> Option<String> {
    item.get(id_field).and_then(|v| match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// Format validation errors for user display
///
/// Groups errors by item and provides a clear summary.
pub fn format_work_item_errors(errors: &[WorkItemValidationError]) -> String {
    if errors.is_empty() {
        return "No validation errors".to_string();
    }

    let mut output = format!(
        "Work item validation failed with {} error(s):\n",
        errors.len()
    );

    for (i, error) in errors.iter().enumerate() {
        output.push_str(&format!("  {}. {}\n", i + 1, error));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validate_work_items_success() {
        let items = vec![
            json!({"id": "item-1", "name": "First"}),
            json!({"id": "item-2", "name": "Second"}),
        ];

        let result = validate_work_items(&items, None);
        assert!(matches!(result, Validation::Success(_)));

        if let Validation::Success(valid) = result {
            assert_eq!(valid.len(), 2);
            assert_eq!(valid[0].id, Some("item-1".to_string()));
            assert_eq!(valid[1].id, Some("item-2".to_string()));
        }
    }

    #[test]
    fn test_validate_work_items_null_item() {
        let items = vec![json!({"id": "1"}), Value::Null, json!({"id": "3"})];

        let result = validate_work_items(&items, None);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::NullItem { item_index: 1 }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_work_items_not_object() {
        let items = vec![
            json!({"id": "1"}),
            json!("not an object"),
            json!({"id": "3"}),
        ];

        let result = validate_work_items(&items, None);
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::NotAnObject { item_index: 1 }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_required_fields() {
        let items = vec![
            json!({"id": "1", "name": "Valid"}),
            json!({"id": "2"}), // Missing "name"
        ];

        let schema = WorkItemSchema::new().require_field("name");

        let result = validate_work_items(&items, Some(&schema));
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::MissingRequiredField { item_index: 1, .. }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_field_types() {
        let items = vec![
            json!({"id": "1", "count": 5}),
            json!({"id": "2", "count": "not a number"}), // Wrong type
        ];

        let schema = WorkItemSchema::new().field_type("count", FieldType::Number);

        let result = validate_work_items(&items, Some(&schema));
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                match &errors[0] {
                    WorkItemValidationError::InvalidFieldType {
                        item_index,
                        expected,
                        got,
                        ..
                    } => {
                        assert_eq!(*item_index, 1);
                        assert_eq!(expected, "number");
                        assert_eq!(got, "string");
                    }
                    _ => panic!("Expected InvalidFieldType error"),
                }
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_constraints_range() {
        let items = vec![
            json!({"id": "1", "score": 50}),
            json!({"id": "2", "score": 150}), // Out of range
        ];

        let schema = WorkItemSchema::new().add_constraint(
            "score",
            Constraint::Range {
                min: 0.0,
                max: 100.0,
            },
        );

        let result = validate_work_items(&items, Some(&schema));
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::ConstraintViolation { item_index: 1, .. }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }

    #[test]
    fn test_validate_duplicate_ids() {
        let items = vec![
            json!({"id": "item-1"}),
            json!({"id": "item-2"}),
            json!({"id": "item-1"}), // Duplicate
        ];

        let result = validate_work_items(&items, None);
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
    fn test_accumulates_multiple_errors() {
        let items = vec![
            Value::Null,                                 // Error 1: null
            json!({"id": "1", "count": "not a number"}), // Error 2: wrong type
            json!({"id": "2"}),                          // Error 3: missing required field
            json!({"id": "3", "score": 200}),            // Error 4: constraint violation
        ];

        let schema = WorkItemSchema::new()
            .require_field("count")
            .field_type("count", FieldType::Number)
            .add_constraint(
                "score",
                Constraint::Range {
                    min: 0.0,
                    max: 100.0,
                },
            );

        let result = validate_work_items(&items, Some(&schema));
        match result {
            Validation::Failure(errors) => {
                // Should have accumulated multiple errors
                assert!(
                    errors.len() >= 4,
                    "Expected at least 4 errors, got {}",
                    errors.len()
                );

                // Verify different error types present
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::NullItem { .. })));
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::InvalidFieldType { .. })));
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::MissingRequiredField { .. })));
                assert!(errors
                    .iter()
                    .any(|e| matches!(e, WorkItemValidationError::ConstraintViolation { .. })));
            }
            _ => panic!("Expected validation failure with multiple errors"),
        }
    }

    #[test]
    fn test_format_work_item_errors() {
        let errors = vec![
            WorkItemValidationError::NullItem { item_index: 0 },
            WorkItemValidationError::MissingRequiredField {
                item_index: 1,
                field: "name".to_string(),
            },
        ];

        let formatted = format_work_item_errors(&errors);
        assert!(formatted.contains("2 error(s)"));
        assert!(formatted.contains("1."));
        assert!(formatted.contains("2."));
    }

    #[test]
    fn test_custom_id_field() {
        let items = vec![
            json!({"custom_id": "a1"}),
            json!({"custom_id": "a2"}),
            json!({"custom_id": "a1"}), // Duplicate
        ];

        let schema = WorkItemSchema::new().id_field("custom_id");

        let result = validate_work_items(&items, Some(&schema));
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    &errors[0],
                    WorkItemValidationError::DuplicateId { id, .. } if id == "a1"
                ));
            }
            _ => panic!("Expected validation failure for duplicate custom ID"),
        }
    }

    #[test]
    fn test_constraint_one_of() {
        let items = vec![
            json!({"id": "1", "status": "active"}),
            json!({"id": "2", "status": "invalid"}), // Not in allowed values
        ];

        let schema = WorkItemSchema::new().add_constraint(
            "status",
            Constraint::OneOf(vec![json!("active"), json!("inactive"), json!("pending")]),
        );

        let result = validate_work_items(&items, Some(&schema));
        match result {
            Validation::Failure(errors) => {
                assert_eq!(errors.len(), 1);
                assert!(matches!(
                    errors[0],
                    WorkItemValidationError::ConstraintViolation { item_index: 1, .. }
                ));
            }
            _ => panic!("Expected validation failure"),
        }
    }
}
