//! JSON path parsing and evaluation for data pipeline
//!
//! Provides JSONPath-like syntax for extracting values from JSON data.
//! Supports field access, array indexing, wildcards, recursive descent, and filters.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use tracing::debug;

/// JSON path expression for extracting values from JSON data
#[derive(Debug, Clone)]
pub struct JsonPath {
    /// Original expression
    pub expression: String,
    /// Parsed path components
    components: Vec<PathComponent>,
}

/// Components that make up a JSON path
#[derive(Debug, Clone)]
enum PathComponent {
    Root,
    Field(String),
    Index(usize),
    ArrayAll,
    RecursiveDescent(String),
    Filter(String),
}

impl JsonPath {
    /// Compile a JSON path expression
    pub fn compile(expr: &str) -> Result<Self> {
        let mut components = Vec::new();
        let mut current = expr;

        // Handle root $
        if current.starts_with('$') {
            components.push(PathComponent::Root);
            current = &current[1..];
            if current.starts_with('.') {
                current = &current[1..];
            }
        }

        // Parse path components
        while !current.is_empty() {
            if current.starts_with("..") {
                // Recursive descent
                current = &current[2..];
                let field = Self::parse_field(&mut current)?;
                components.push(PathComponent::RecursiveDescent(field));
            } else if current.starts_with('[') {
                // Array access or filter
                current = &current[1..];
                if current.starts_with('*') {
                    components.push(PathComponent::ArrayAll);
                    current = &current[1..];
                    if current.starts_with(']') {
                        current = &current[1..];
                    }
                } else if current.starts_with("?(") {
                    // Filter expression
                    let end = current
                        .find(")]")
                        .ok_or_else(|| anyhow!("Unclosed filter expression"))?;
                    let filter = current[2..end].to_string();
                    components.push(PathComponent::Filter(filter));
                    current = &current[end + 2..];
                } else if let Some(end) = current.find(']') {
                    // Index
                    let index_str = &current[..end];
                    let index = index_str.parse::<usize>().context("Invalid array index")?;
                    components.push(PathComponent::Index(index));
                    current = &current[end + 1..];
                }
            } else {
                // Field access
                let field = Self::parse_field(&mut current)?;
                if !field.is_empty() {
                    // Check if it ends with [*]
                    if field.ends_with("[*]") {
                        let field_name = &field[..field.len() - 3];
                        components.push(PathComponent::Field(field_name.to_string()));
                        components.push(PathComponent::ArrayAll);
                    } else {
                        components.push(PathComponent::Field(field));
                    }
                }
            }

            // Skip dot separator
            if current.starts_with('.') && !current.starts_with("..") {
                current = &current[1..];
            }
        }

        Ok(Self {
            expression: expr.to_string(),
            components,
        })
    }

    /// Parse a field name from the path
    fn parse_field(current: &mut &str) -> Result<String> {
        let mut field = String::new();
        let chars = current.chars();

        for ch in chars {
            match ch {
                '.' | '[' => break,
                _ => field.push(ch),
            }
        }

        *current = &current[field.len()..];
        Ok(field)
    }

    /// Select values from JSON using the path
    pub fn select(&self, data: &Value) -> Result<Vec<Value>> {
        debug!("Selecting with JSON path: {}", self.expression);
        debug!("Path components: {:?}", self.components);

        let mut results = vec![data.clone()];

        for component in &self.components {
            let mut next_results = Vec::new();

            for value in results {
                match component {
                    PathComponent::Root => {
                        next_results.push(value);
                    }
                    PathComponent::Field(field) => {
                        if let Some(v) = value.get(field) {
                            next_results.push(v.clone());
                        }
                    }
                    PathComponent::Index(idx) => {
                        if let Value::Array(arr) = value {
                            if let Some(v) = arr.get(*idx) {
                                next_results.push(v.clone());
                            }
                        }
                    }
                    PathComponent::ArrayAll => {
                        if let Value::Array(arr) = value {
                            next_results.extend(arr.clone());
                        }
                    }
                    PathComponent::RecursiveDescent(field) => {
                        Self::recursive_descent(&value, field, &mut next_results);
                    }
                    PathComponent::Filter(filter_expr) => {
                        if let Value::Array(arr) = value {
                            for item in arr {
                                if Self::evaluate_filter(&item, filter_expr) {
                                    next_results.push(item.clone());
                                }
                            }
                        }
                    }
                }
            }

            results = next_results;
        }

        Ok(results)
    }

    /// Recursively find all values with a given field name
    fn recursive_descent(value: &Value, field: &str, results: &mut Vec<Value>) {
        if let Some(v) = value.get(field) {
            results.push(v.clone());
        }

        match value {
            Value::Object(obj) => {
                for (_, v) in obj {
                    Self::recursive_descent(v, field, results);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    Self::recursive_descent(v, field, results);
                }
            }
            _ => {}
        }
    }

    /// Evaluate a simple filter expression
    fn evaluate_filter(item: &Value, filter_expr: &str) -> bool {
        // Simple implementation for basic filters like @.field > value
        // Format: @.field operator value
        let parts: Vec<&str> = filter_expr.split_whitespace().collect();
        if parts.len() != 3 {
            return false;
        }

        let field_path = parts[0].trim_start_matches("@.");
        let operator = parts[1];
        let expected_value = parts[2].trim_matches('"').trim_matches('\'');

        let actual_value = item.get(field_path);

        match operator {
            "==" | "=" => {
                if let Some(Value::String(s)) = actual_value {
                    s == expected_value
                } else if let Some(Value::Number(n)) = actual_value {
                    if let Ok(expected_num) = expected_value.parse::<f64>() {
                        n.as_f64() == Some(expected_num)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "!=" => {
                if let Some(Value::String(s)) = actual_value {
                    s != expected_value
                } else {
                    true
                }
            }
            ">" => {
                if let Some(Value::Number(n)) = actual_value {
                    if let Ok(expected_num) = expected_value.parse::<f64>() {
                        n.as_f64().is_some_and(|v| v > expected_num)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "<" => {
                if let Some(Value::Number(n)) = actual_value {
                    if let Ok(expected_num) = expected_value.parse::<f64>() {
                        n.as_f64().is_some_and(|v| v < expected_num)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            ">=" => {
                if let Some(Value::Number(n)) = actual_value {
                    if let Ok(expected_num) = expected_value.parse::<f64>() {
                        n.as_f64().is_some_and(|v| v >= expected_num)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            "<=" => {
                if let Some(Value::Number(n)) = actual_value {
                    if let Ok(expected_num) = expected_value.parse::<f64>() {
                        n.as_f64().is_some_and(|v| v <= expected_num)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
