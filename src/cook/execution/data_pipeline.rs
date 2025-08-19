//! Data pipeline for MapReduce workflows
//!
//! Provides JSON path extraction, filtering, sorting, and data transformation
//! capabilities for processing work items in MapReduce workflows.

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Read;
use tracing::{debug, warn};

/// Data pipeline configuration from MapReduce config
#[derive(Debug, Clone, Default)]
pub struct DataPipeline {
    /// JSON path expression for extracting items
    pub json_path: Option<JsonPath>,
    /// Filter expression for selecting items
    pub filter: Option<FilterExpression>,
    /// Sorting configuration
    pub sorter: Option<Sorter>,
    /// Maximum number of items to process
    pub limit: Option<usize>,
    /// Number of items to skip
    pub offset: Option<usize>,
    /// Field mapping for transformations
    pub field_mapping: Option<HashMap<String, String>>,
}

impl DataPipeline {
    /// Create a new data pipeline from configuration
    pub fn from_config(
        json_path: Option<String>,
        filter: Option<String>,
        sort_by: Option<String>,
        max_items: Option<usize>,
    ) -> Result<Self> {
        let json_path = if let Some(path) = json_path {
            if !path.is_empty() {
                Some(JsonPath::compile(&path)?)
            } else {
                None
            }
        } else {
            None
        };

        let filter = if let Some(expr) = filter {
            Some(FilterExpression::parse(&expr)?)
        } else {
            None
        };

        let sorter = if let Some(sort_spec) = sort_by {
            Some(Sorter::parse(&sort_spec)?)
        } else {
            None
        };

        Ok(Self {
            json_path,
            filter,
            sorter,
            limit: max_items,
            offset: None,
            field_mapping: None,
        })
    }

    /// Process input data through the pipeline
    pub fn process(&self, input: &Value) -> Result<Vec<Value>> {
        debug!("Processing data through pipeline");

        // Step 1: Extract items using JSON path
        let mut items = if let Some(ref json_path) = self.json_path {
            json_path.select(input)?
        } else {
            // No JSON path specified, treat input as array or single item
            match input {
                Value::Array(arr) => arr.clone(),
                other => vec![other.clone()],
            }
        };

        debug!("Extracted {} items from JSON path", items.len());

        // Step 2: Apply filter
        if let Some(ref filter) = self.filter {
            items.retain(|item| filter.evaluate(item));
            debug!("After filtering: {} items", items.len());
        }

        // Step 3: Sort items
        if let Some(ref sorter) = self.sorter {
            sorter.sort(&mut items);
            debug!("Sorted {} items", items.len());
        }

        // Step 4: Apply offset
        if let Some(offset) = self.offset {
            if offset < items.len() {
                items = items[offset..].to_vec();
                debug!("Applied offset {}, {} items remaining", offset, items.len());
            } else {
                items.clear();
            }
        }

        // Step 5: Apply limit
        if let Some(limit) = self.limit {
            items.truncate(limit);
            debug!("Limited to {} items", items.len());
        }

        // Step 6: Apply field mapping
        if let Some(ref mapping) = self.field_mapping {
            items = items
                .into_iter()
                .map(|item| self.apply_field_mapping(item, mapping))
                .collect();
        }

        Ok(items)
    }

    /// Process streaming JSON input
    ///
    /// Note: Streaming JSON processing for very large files is planned for a future release.
    /// For now, use the regular process() method which handles reasonably sized files efficiently.
    pub fn process_streaming<R: Read>(&self, _reader: R) -> Result<Vec<Value>> {
        Err(anyhow!(
            "Streaming JSON processing not yet implemented. Use regular process() for now."
        ))
    }

    /// Apply field mapping to transform an item
    fn apply_field_mapping(&self, item: Value, mapping: &HashMap<String, String>) -> Value {
        let mut result = item.clone();
        if let Value::Object(ref mut obj) = result {
            for (target_field, source_path) in mapping {
                if let Some(value) = self.extract_field_value(&item, source_path) {
                    obj.insert(target_field.clone(), value);
                }
            }
        }
        result
    }

    /// Extract a field value using a path expression
    fn extract_field_value(&self, item: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = item.clone();

        for part in parts {
            current = current.get(part)?.clone();
        }

        Some(current)
    }
}

/// JSON path expression evaluator
#[derive(Debug, Clone)]
pub struct JsonPath {
    /// Original expression
    pub expression: String,
    /// Parsed path components
    components: Vec<PathComponent>,
}

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

/// Filter expression AST
#[derive(Debug, Clone)]
pub enum FilterExpression {
    /// Comparison expression
    Comparison {
        field: String,
        op: ComparisonOp,
        value: Value,
    },
    /// Logical expression
    Logical {
        op: LogicalOp,
        operands: Vec<FilterExpression>,
    },
    /// Function expression
    Function { name: String, args: Vec<String> },
    /// In expression for checking if a value is in a list
    In { field: String, values: Vec<Value> },
}

impl FilterExpression {
    /// Parse a filter expression string
    pub fn parse(expr: &str) -> Result<Self> {
        // Simple parser for expressions like:
        // "severity == 'high'"
        // "priority > 5"
        // "severity in ['high', 'critical']"
        // "severity == 'high' && priority > 5"

        let expr = expr.trim();

        // Check for logical operators (&&, ||)
        if let Some(and_pos) = expr.find("&&") {
            let left = Self::parse(&expr[..and_pos])?;
            let right = Self::parse(&expr[and_pos + 2..])?;
            return Ok(FilterExpression::Logical {
                op: LogicalOp::And,
                operands: vec![left, right],
            });
        }

        if let Some(or_pos) = expr.find("||") {
            let left = Self::parse(&expr[..or_pos])?;
            let right = Self::parse(&expr[or_pos + 2..])?;
            return Ok(FilterExpression::Logical {
                op: LogicalOp::Or,
                operands: vec![left, right],
            });
        }

        // Check for 'in' operator
        if let Some(in_pos) = expr.find(" in ") {
            let field = expr[..in_pos].trim().to_string();
            let values_str = expr[in_pos + 4..].trim();

            // Parse array of values
            let values = if values_str.starts_with('[') && values_str.ends_with(']') {
                let values_inner = &values_str[1..values_str.len() - 1];
                let mut parsed_values = Vec::new();

                for value_str in values_inner.split(',') {
                    let value_str = value_str.trim().trim_matches('\'').trim_matches('"');
                    parsed_values.push(Value::String(value_str.to_string()));
                }

                parsed_values
            } else {
                return Err(anyhow!("Invalid 'in' expression format"));
            };

            return Ok(FilterExpression::In { field, values });
        }

        // Check for function calls
        if expr.contains('(') && expr.contains(')') {
            let open_paren = expr.find('(').unwrap();
            let close_paren = expr.rfind(')').unwrap();
            let name = expr[..open_paren].trim().to_string();
            let args_str = &expr[open_paren + 1..close_paren];

            let args: Vec<String> = if args_str.is_empty() {
                Vec::new()
            } else {
                args_str.split(',').map(|s| s.trim().to_string()).collect()
            };

            return Ok(FilterExpression::Function { name, args });
        }

        // Parse comparison operators
        let operators = ["==", "!=", ">=", "<=", ">", "<", "="];
        for op_str in &operators {
            if let Some(op_pos) = expr.find(op_str) {
                let field = expr[..op_pos].trim().to_string();
                let value_str = expr[op_pos + op_str.len()..].trim();

                let value = Self::parse_value(value_str)?;

                let op = match *op_str {
                    "==" | "=" => ComparisonOp::Equal,
                    "!=" => ComparisonOp::NotEqual,
                    ">" => ComparisonOp::Greater,
                    "<" => ComparisonOp::Less,
                    ">=" => ComparisonOp::GreaterEqual,
                    "<=" => ComparisonOp::LessEqual,
                    _ => return Err(anyhow!("Unknown operator: {}", op_str)),
                };

                return Ok(FilterExpression::Comparison { field, op, value });
            }
        }

        Err(anyhow!("Invalid filter expression: {}", expr))
    }

    /// Parse a value string into a JSON value
    fn parse_value(value_str: &str) -> Result<Value> {
        let trimmed = value_str.trim();

        // String values (quoted)
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            let unquoted = &trimmed[1..trimmed.len() - 1];
            return Ok(Value::String(unquoted.to_string()));
        }

        // Boolean values
        if trimmed == "true" {
            return Ok(Value::Bool(true));
        }
        if trimmed == "false" {
            return Ok(Value::Bool(false));
        }

        // Null value
        if trimmed == "null" {
            return Ok(Value::Null);
        }

        // Number values
        if let Ok(num) = trimmed.parse::<f64>() {
            return Ok(serde_json::Number::from_f64(num)
                .map(Value::Number)
                .unwrap_or(Value::Null));
        }

        // Default to string
        Ok(Value::String(trimmed.to_string()))
    }

    /// Evaluate the filter expression against a JSON value
    pub fn evaluate(&self, item: &Value) -> bool {
        match self {
            FilterExpression::Comparison { field, op, value } => {
                let actual = item.get(field);
                Self::compare(actual, op, value)
            }
            FilterExpression::Logical { op, operands } => match op {
                LogicalOp::And => operands.iter().all(|expr| expr.evaluate(item)),
                LogicalOp::Or => operands.iter().any(|expr| expr.evaluate(item)),
                LogicalOp::Not => !operands.first().is_some_and(|expr| expr.evaluate(item)),
            },
            FilterExpression::Function { name, args } => Self::evaluate_function(item, name, args),
            FilterExpression::In { field, values } => {
                if let Some(actual) = item.get(field) {
                    values.iter().any(|v| actual == v)
                } else {
                    false
                }
            }
        }
    }

    /// Compare two values using the given operator
    fn compare(actual: Option<&Value>, op: &ComparisonOp, expected: &Value) -> bool {
        match op {
            ComparisonOp::Equal => actual == Some(expected),
            ComparisonOp::NotEqual => actual != Some(expected),
            ComparisonOp::Greater => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() > e.as_f64()
                } else {
                    false
                }
            }
            ComparisonOp::Less => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() < e.as_f64()
                } else {
                    false
                }
            }
            ComparisonOp::GreaterEqual => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() >= e.as_f64()
                } else {
                    false
                }
            }
            ComparisonOp::LessEqual => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() <= e.as_f64()
                } else {
                    false
                }
            }
            ComparisonOp::Contains => {
                if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    a.contains(e.as_str())
                } else {
                    false
                }
            }
            ComparisonOp::StartsWith => {
                if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    a.starts_with(e.as_str())
                } else {
                    false
                }
            }
            ComparisonOp::EndsWith => {
                if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    a.ends_with(e.as_str())
                } else {
                    false
                }
            }
            ComparisonOp::Matches => {
                if let (Some(Value::String(a)), Value::String(pattern)) = (actual, expected) {
                    // Try to compile and match the regex
                    match Regex::new(pattern) {
                        Ok(re) => re.is_match(a),
                        Err(e) => {
                            warn!("Invalid regex pattern '{}': {}", pattern, e);
                            false
                        }
                    }
                } else {
                    false
                }
            }
        }
    }

    /// Evaluate a function expression
    fn evaluate_function(item: &Value, name: &str, args: &[String]) -> bool {
        match name {
            "contains" => {
                if args.len() == 2 {
                    if let Some(Value::String(s)) = item.get(&args[0]) {
                        return s.contains(&args[1]);
                    }
                }
                false
            }
            "starts_with" => {
                if args.len() == 2 {
                    if let Some(Value::String(s)) = item.get(&args[0]) {
                        return s.starts_with(&args[1]);
                    }
                }
                false
            }
            "ends_with" => {
                if args.len() == 2 {
                    if let Some(Value::String(s)) = item.get(&args[0]) {
                        return s.ends_with(&args[1]);
                    }
                }
                false
            }
            "is_null" => {
                if args.len() == 1 {
                    return item.get(&args[0]) == Some(&Value::Null);
                }
                false
            }
            "is_not_null" => {
                if args.len() == 1 {
                    return item.get(&args[0]) != Some(&Value::Null);
                }
                false
            }
            _ => {
                warn!("Unknown function in filter expression: {}", name);
                false
            }
        }
    }
}

/// Comparison operators
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
}

/// Logical operators
#[derive(Debug, Clone)]
pub enum LogicalOp {
    And,
    Or,
    Not,
}

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
        // Format: "field1 DESC, field2 ASC" or just "field1"
        for field_spec in spec.split(',') {
            let field_spec = field_spec.trim();
            let parts: Vec<&str> = field_spec.split_whitespace().collect();

            let (path, order) = if parts.len() == 2 {
                let order = match parts[1].to_uppercase().as_str() {
                    "DESC" | "DESCENDING" => SortOrder::Descending,
                    "ASC" | "ASCENDING" => SortOrder::Ascending,
                    _ => return Err(anyhow!("Invalid sort order: {}. Use ASC or DESC", parts[1])),
                };
                (parts[0].to_string(), order)
            } else if parts.len() == 1 {
                (parts[0].to_string(), SortOrder::Ascending)
            } else {
                return Err(anyhow!("Invalid sort specification: {}", field_spec));
            };

            fields.push(SortField {
                path,
                order,
                null_position: NullPosition::Last,
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
            let a_value = a.get(&field.path);
            let b_value = b.get(&field.path);

            let ordering = self.compare_values(a_value, b_value, &field.null_position);

            let ordering = match field.order {
                SortOrder::Ascending => ordering,
                SortOrder::Descending => ordering.reverse(),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        Ordering::Equal
    }

    /// Compare two JSON values for sorting
    fn compare_values(
        &self,
        a: Option<&Value>,
        b: Option<&Value>,
        null_position: &NullPosition,
    ) -> Ordering {
        match (a, b) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => match null_position {
                NullPosition::First => Ordering::Less,
                NullPosition::Last => Ordering::Greater,
            },
            (Some(_), None) => match null_position {
                NullPosition::First => Ordering::Greater,
                NullPosition::Last => Ordering::Less,
            },
            (Some(a), Some(b)) => self.compare_json_values(a, b),
        }
    }

    /// Compare two non-null JSON values
    fn compare_json_values(&self, a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
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
#[derive(Debug, Clone, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Position of null values in sorted output
#[derive(Debug, Clone)]
pub enum NullPosition {
    First,
    Last,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_path_basic() {
        let path = JsonPath::compile("$.items[*]").unwrap();
        let data = json!({
            "items": [
                {"id": 1, "name": "Item 1"},
                {"id": 2, "name": "Item 2"}
            ]
        });

        let results = path.select(&data).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["id"], 1);
        assert_eq!(results[1]["id"], 2);
    }

    #[test]
    fn test_json_path_nested() {
        let path = JsonPath::compile("$.data.items[*].name").unwrap();
        let data = json!({
            "data": {
                "items": [
                    {"id": 1, "name": "Item 1"},
                    {"id": 2, "name": "Item 2"}
                ]
            }
        });

        let results = path.select(&data).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "Item 1");
        assert_eq!(results[1], "Item 2");
    }

    #[test]
    fn test_filter_comparison() {
        let filter = FilterExpression::parse("priority > 5").unwrap();

        let item1 = json!({"priority": 3});
        let item2 = json!({"priority": 7});

        assert!(!filter.evaluate(&item1));
        assert!(filter.evaluate(&item2));
    }

    #[test]
    fn test_filter_logical() {
        let filter = FilterExpression::parse("severity == 'high' && priority > 5").unwrap();

        let item1 = json!({"severity": "high", "priority": 7});
        let item2 = json!({"severity": "high", "priority": 3});
        let item3 = json!({"severity": "low", "priority": 7});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
    }

    #[test]
    fn test_filter_in_operator() {
        let filter = FilterExpression::parse("severity in ['high', 'critical']").unwrap();

        let item1 = json!({"severity": "high"});
        let item2 = json!({"severity": "critical"});
        let item3 = json!({"severity": "low"});

        assert!(filter.evaluate(&item1));
        assert!(filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
    }

    #[test]
    fn test_sorter_single_field() {
        let sorter = Sorter::parse("priority DESC").unwrap();

        let mut items = vec![
            json!({"priority": 3}),
            json!({"priority": 1}),
            json!({"priority": 5}),
        ];

        sorter.sort(&mut items);

        assert_eq!(items[0]["priority"], 5);
        assert_eq!(items[1]["priority"], 3);
        assert_eq!(items[2]["priority"], 1);
    }

    #[test]
    fn test_sorter_multiple_fields() {
        let sorter = Sorter::parse("severity DESC, priority ASC").unwrap();

        let mut items = vec![
            json!({"severity": "high", "priority": 3}),
            json!({"severity": "high", "priority": 1}),
            json!({"severity": "critical", "priority": 5}),
        ];

        sorter.sort(&mut items);

        // First by severity DESC (alphabetically: "high" > "critical")
        assert_eq!(items[0]["severity"], "high");
        assert_eq!(items[1]["severity"], "high");
        assert_eq!(items[2]["severity"], "critical");
        // Then by priority ASC for same severity
        assert_eq!(items[0]["priority"], 1); // high with priority 1
        assert_eq!(items[1]["priority"], 3); // high with priority 3
        assert_eq!(items[2]["priority"], 5); // critical with priority 5
    }

    #[test]
    fn test_pipeline_complete() {
        let pipeline = DataPipeline::from_config(
            Some("$.items[*]".to_string()),
            Some("priority > 3".to_string()),
            Some("priority DESC".to_string()),
            Some(2),
        )
        .unwrap();

        let data = json!({
            "items": [
                {"id": 1, "priority": 5},
                {"id": 2, "priority": 2},
                {"id": 3, "priority": 8},
                {"id": 4, "priority": 4},
            ]
        });

        let results = pipeline.process(&data).unwrap();

        // Should filter (priority > 3), sort DESC, and limit to 2
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["priority"], 8);
        assert_eq!(results[1]["priority"], 5);
    }

    #[test]
    fn test_filter_regex_matching() {
        // Test the Matches operator with regex patterns
        let filter = FilterExpression::Comparison {
            field: "email".to_string(),
            op: ComparisonOp::Matches,
            value: json!(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"),
        };

        let valid_email = json!({"email": "user@example.com"});
        let invalid_email = json!({"email": "not-an-email"});
        let no_email = json!({"name": "John"});

        assert!(filter.evaluate(&valid_email));
        assert!(!filter.evaluate(&invalid_email));
        assert!(!filter.evaluate(&no_email));

        // Test pattern matching on file paths
        let path_filter = FilterExpression::Comparison {
            field: "path".to_string(),
            op: ComparisonOp::Matches,
            value: json!(r"\.rs$"),
        };

        let rust_file = json!({"path": "src/main.rs"});
        let other_file = json!({"path": "README.md"});

        assert!(path_filter.evaluate(&rust_file));
        assert!(!path_filter.evaluate(&other_file));
    }
}
