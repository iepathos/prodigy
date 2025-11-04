//! Sorting configuration and logic for data pipeline
//!
//! Provides multi-field sorting with support for ascending/descending order,
//! null value positioning, and nested field access.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;

/// Sorting configuration
#[derive(Debug, Clone)]
pub struct Sorter {
    /// Fields to sort by
    pub fields: Vec<SortField>,
}

impl Sorter {
    /// Parse a sort specification string
    pub fn parse(spec: &str) -> Result<Self> {
        let mut fields = Vec::new();

        // Handle multiple sort fields separated by commas
        // Format: "field1 DESC, field2 ASC NULLS FIRST" or just "field1"
        for field_spec in spec.split(',') {
            let field_spec = field_spec.trim();
            let parts: Vec<&str> = field_spec.split_whitespace().collect();

            if parts.is_empty() {
                continue;
            }

            let path = parts[0].to_string();
            let mut order = SortOrder::Ascending;
            let mut null_position = NullPosition::Last;
            let mut i = 1;

            // Parse sort order
            if i < parts.len() {
                match parts[i].to_uppercase().as_str() {
                    "DESC" | "DESCENDING" => {
                        order = SortOrder::Descending;
                        i += 1;
                    }
                    "ASC" | "ASCENDING" => {
                        order = SortOrder::Ascending;
                        i += 1;
                    }
                    _ => {}
                }
            }

            // Parse null position
            if i < parts.len() && parts[i].to_uppercase() == "NULLS" {
                i += 1;
                if i < parts.len() {
                    match parts[i].to_uppercase().as_str() {
                        "FIRST" => null_position = NullPosition::First,
                        "LAST" => null_position = NullPosition::Last,
                        _ => {
                            return Err(anyhow!(
                                "Invalid null position: {}. Use NULLS FIRST or NULLS LAST",
                                parts[i]
                            ))
                        }
                    }
                }
            }

            fields.push(SortField {
                path,
                order,
                null_position,
            });
        }

        if fields.is_empty() {
            return Err(anyhow!("No sort fields specified"));
        }

        Ok(Self { fields })
    }

    /// Sort an array of JSON values
    #[allow(clippy::ptr_arg)]
    pub fn sort(&self, items: &mut Vec<Value>) {
        items.sort_by(|a, b| self.compare_items(a, b));
    }

    /// Compare two items according to the sort fields
    fn compare_items(&self, a: &Value, b: &Value) -> Ordering {
        for field in &self.fields {
            // Support nested field access for sorting
            let a_value = Self::get_nested_field_value(a, &field.path);
            let b_value = Self::get_nested_field_value(b, &field.path);

            let ordering =
                self.compare_values(a_value, b_value, &field.null_position, &field.order);

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        Ordering::Equal
    }

    /// Get a nested field value from a JSON object for sorting
    fn get_nested_field_value<'a>(item: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = item;

        for part in parts {
            match current.get(part) {
                Some(v) => current = v,
                None => return None,
            }
        }

        Some(current)
    }

    /// Compare two JSON values for sorting
    ///
    /// CRITICAL: null_position is independent of sort order (ASC/DESC)
    /// DESC reverses ONLY the value comparisons, NOT the null positioning
    fn compare_values(
        &self,
        a: Option<&Value>,
        b: Option<&Value>,
        null_position: &NullPosition,
        order: &SortOrder,
    ) -> Ordering {
        match (a, b) {
            (None, None) | (Some(Value::Null), Some(Value::Null)) => Ordering::Equal,
            // Null vs non-null: position is independent of ASC/DESC
            (None, Some(v)) | (Some(Value::Null), Some(v)) if !v.is_null() => match null_position {
                NullPosition::First => Ordering::Less,
                NullPosition::Last => Ordering::Greater,
            },
            (Some(v), None) | (Some(v), Some(Value::Null)) if !v.is_null() => match null_position {
                NullPosition::First => Ordering::Greater,
                NullPosition::Last => Ordering::Less,
            },
            // Both values present: apply ASC/DESC to the comparison
            (Some(a), Some(b)) => {
                let value_cmp = self.compare_json_values(a, b);
                match order {
                    SortOrder::Ascending => value_cmp,
                    SortOrder::Descending => value_cmp.reverse(),
                }
            }
            _ => Ordering::Equal,
        }
    }

    /// Compare two JSON values (handles both null and non-null)
    fn compare_json_values(&self, a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Greater, // Null is "greater" so it sorts last by default
            (_, Value::Null) => Ordering::Less, // Non-null is "less" so it sorts first by default
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Number(a), Value::Number(b)) => {
                let a_f64 = a.as_f64().unwrap_or(0.0);
                let b_f64 = b.as_f64().unwrap_or(0.0);
                a_f64.partial_cmp(&b_f64).unwrap_or(Ordering::Equal)
            }
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Array(a), Value::Array(b)) => a.len().cmp(&b.len()),
            (Value::Object(a), Value::Object(b)) => a.len().cmp(&b.len()),
            // Different types - use type ordering
            _ => {
                let type_order = |v: &Value| match v {
                    Value::Null => 0,
                    Value::Bool(_) => 1,
                    Value::Number(_) => 2,
                    Value::String(_) => 3,
                    Value::Array(_) => 4,
                    Value::Object(_) => 5,
                };
                type_order(a).cmp(&type_order(b))
            }
        }
    }
}

/// Sort field configuration
#[derive(Debug, Clone)]
pub struct SortField {
    /// Path to the field to sort by
    pub path: String,
    /// Sort order
    pub order: SortOrder,
    /// Position of null values
    pub null_position: NullPosition,
}

/// Sort order
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Position of null values in sorted output
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NullPosition {
    First,
    Last,
}

