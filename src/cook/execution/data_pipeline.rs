//! Data pipeline for MapReduce workflows
//!
//! Provides JSON path extraction, filtering, sorting, and data transformation
//! capabilities for processing work items in MapReduce workflows.

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
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
    /// Field for deduplication
    pub distinct: Option<String>,
    /// Field mapping for transformations
    pub field_mapping: Option<HashMap<String, String>>,
    /// Preview mode - don't execute, just show filtered/sorted results
    pub preview_mode: bool,
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
            distinct: None,
            field_mapping: None,
            preview_mode: false,
        })
    }

    /// Create a new data pipeline with all configuration options
    pub fn from_full_config(
        json_path: Option<String>,
        filter: Option<String>,
        sort_by: Option<String>,
        max_items: Option<usize>,
        offset: Option<usize>,
        distinct: Option<String>,
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
            offset,
            distinct,
            field_mapping: None,
            preview_mode: false,
        })
    }

    /// Process input data through the pipeline
    pub fn process(&self, input: &Value) -> Result<Vec<Value>> {
        debug!("Processing data through pipeline");

        // Step 1: Extract items using JSON path
        let mut items = if let Some(ref json_path) = self.json_path {
            debug!("Applying JSON path: {}", json_path.expression);
            let selected = json_path.select(input)?;
            debug!("JSON path selected {} items", selected.len());
            selected
        } else {
            // No JSON path specified, treat input as array or single item
            debug!("No JSON path, treating input as array or single item");
            match input {
                Value::Array(arr) => {
                    debug!("Input is array with {} items", arr.len());
                    arr.clone()
                }
                other => {
                    debug!("Input is single item");
                    vec![other.clone()]
                }
            }
        };

        debug!("Extracted {} items from JSON path", items.len());

        // Step 2: Apply filter
        if let Some(ref filter) = self.filter {
            debug!("Applying filter: {:?}", filter);
            let before_count = items.len();
            items.retain(|item| filter.evaluate(item));
            debug!(
                "After filtering: {} items (filtered out {})",
                items.len(),
                before_count - items.len()
            );
        }

        // Step 3: Sort items
        if let Some(ref sorter) = self.sorter {
            sorter.sort(&mut items);
            debug!("Sorted {} items", items.len());
        }

        // Step 4: Apply distinct (deduplication)
        if let Some(ref distinct_field) = self.distinct {
            items = self.deduplicate(items, distinct_field)?;
            debug!("Deduplicated to {} items", items.len());
        }

        // Step 5: Apply offset
        if let Some(offset) = self.offset {
            if offset < items.len() {
                items = items[offset..].to_vec();
                debug!("Applied offset {}, {} items remaining", offset, items.len());
            } else {
                items.clear();
            }
        }

        // Step 6: Apply limit
        if let Some(limit) = self.limit {
            items.truncate(limit);
            debug!("Limited to {} items", items.len());
        }

        // Step 7: Apply field mapping
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

    /// Deduplicate items based on a field value
    fn deduplicate(&self, items: Vec<Value>, distinct_field: &str) -> Result<Vec<Value>> {
        let mut seen = std::collections::HashSet::<String>::new();
        let mut result = Vec::new();

        for item in items {
            let field_value = self.extract_field_value(&item, distinct_field);
            let key = match field_value {
                Some(v) => serde_json::to_string(&v)?,
                None => "null".to_string(),
            };

            if seen.insert(key) {
                result.push(item);
            }
        }

        Ok(result)
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

/// Path component for field access with array support
#[derive(Debug, Clone)]
enum PathPart {
    Field(String),
    Index(usize),
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
        // "!is_null(field)"
        // "!(priority > 5)"

        let expr = expr.trim();

        // Check if the entire expression is parenthesized and strip outer parentheses
        if expr.starts_with('(') && expr.ends_with(')') {
            // Check that parentheses are balanced and wrap entire expression
            let mut depth = 0;
            let chars: Vec<char> = expr.chars().collect();
            let mut wraps_entire = true;
            for (i, &ch) in chars.iter().enumerate() {
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    depth -= 1;
                    // If depth reaches 0 before the last character, outer parens don't wrap whole expr
                    if depth == 0 && i < chars.len() - 1 {
                        wraps_entire = false;
                        break;
                    }
                }
            }
            // If outer parentheses wrap the entire expression, recursively parse inner
            if wraps_entire && depth == 0 {
                return Self::parse(&expr[1..expr.len() - 1]);
            }
        }

        // Check for NOT operator at the beginning
        if expr.starts_with("!") {
            // Handle negation
            let inner_expr = expr[1..].trim();
            // If it starts with '(' and ends with ')', parse the inner expression
            let inner = if inner_expr.starts_with('(') && inner_expr.ends_with(')') {
                Self::parse(&inner_expr[1..inner_expr.len() - 1])?
            } else {
                Self::parse(inner_expr)?
            };
            return Ok(FilterExpression::Logical {
                op: LogicalOp::Not,
                operands: vec![inner],
            });
        }

        // Check for logical operators (&&, ||)
        // Need to handle parentheses properly, so we do a simple level-aware scan
        let mut paren_depth = 0;
        let mut and_pos = None;
        let mut or_pos = None;
        let chars: Vec<char> = expr.chars().collect();

        for i in 0..chars.len() {
            match chars[i] {
                '(' => paren_depth += 1,
                ')' => paren_depth -= 1,
                '&' if paren_depth == 0 && i + 1 < chars.len() && chars[i + 1] == '&' => {
                    if and_pos.is_none() {
                        and_pos = Some(i);
                    }
                }
                '|' if paren_depth == 0 && i + 1 < chars.len() && chars[i + 1] == '|' => {
                    if or_pos.is_none() {
                        or_pos = Some(i);
                    }
                }
                _ => {}
            }
        }

        // Process OR first (lower precedence than AND)
        if let Some(pos) = or_pos {
            let left = Self::parse(&expr[..pos])?;
            let right = Self::parse(&expr[pos + 2..])?;
            return Ok(FilterExpression::Logical {
                op: LogicalOp::Or,
                operands: vec![left, right],
            });
        }

        // Then process AND
        if let Some(pos) = and_pos {
            let left = Self::parse(&expr[..pos])?;
            let right = Self::parse(&expr[pos + 2..])?;
            return Ok(FilterExpression::Logical {
                op: LogicalOp::And,
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
                // Support nested field access like "unified_score.final_score" and array access like "tags[0]"
                let actual = Self::get_nested_field_with_array(item, field);
                Self::compare(actual.as_ref(), op, value)
            }
            FilterExpression::Logical { op, operands } => match op {
                LogicalOp::And => operands.iter().all(|expr| expr.evaluate(item)),
                LogicalOp::Or => operands.iter().any(|expr| expr.evaluate(item)),
                LogicalOp::Not => !operands.first().is_some_and(|expr| expr.evaluate(item)),
            },
            FilterExpression::Function { name, args } => Self::evaluate_function(item, name, args),
            FilterExpression::In { field, values } => {
                // Support nested field access with array indices
                if let Some(actual) = Self::get_nested_field_with_array(item, field) {
                    values.iter().any(|v| &actual == v)
                } else {
                    false
                }
            }
        }
    }

    /// Get a nested field value from a JSON object
    #[allow(dead_code)]
    fn get_nested_field(item: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = item.clone();

        for part in parts {
            current = current.get(part)?.clone();
        }

        Some(current)
    }

    /// Get a nested field value with array index support
    fn get_nested_field_with_array(item: &Value, path: &str) -> Option<Value> {
        let mut current = item.clone();
        let parts = Self::parse_path_with_array(path);

        for part in parts {
            match part {
                PathPart::Field(field) => {
                    current = current.get(field)?.clone();
                }
                PathPart::Index(idx) => {
                    if let Value::Array(arr) = current {
                        current = arr.get(idx)?.clone();
                    } else {
                        return None;
                    }
                }
            }
        }

        Some(current)
    }

    /// Parse a path that may contain array indices
    fn parse_path_with_array(path: &str) -> Vec<PathPart> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut chars = path.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '.' => {
                    if !current.is_empty() {
                        parts.push(PathPart::Field(current.clone()));
                        current.clear();
                    }
                }
                '[' => {
                    if !current.is_empty() {
                        parts.push(PathPart::Field(current.clone()));
                        current.clear();
                    }
                    // Parse array index
                    let mut index = String::new();
                    for ch in chars.by_ref() {
                        if ch == ']' {
                            break;
                        }
                        index.push(ch);
                    }
                    if let Ok(idx) = index.parse::<usize>() {
                        parts.push(PathPart::Index(idx));
                    }
                }
                _ => {
                    current.push(ch);
                }
            }
        }

        if !current.is_empty() {
            parts.push(PathPart::Field(current));
        }

        parts
    }

    /// Compare two values using the given operator
    fn compare(actual: Option<&Value>, op: &ComparisonOp, expected: &Value) -> bool {
        match op {
            ComparisonOp::Equal => {
                // Special handling for null comparisons
                match (actual, expected) {
                    (None, Value::Null) => true,              // Missing field equals null
                    (Some(Value::Null), Value::Null) => true, // Explicit null equals null
                    _ => actual == Some(expected),
                }
            }
            ComparisonOp::NotEqual => {
                // Special handling for null comparisons
                match (actual, expected) {
                    (None, Value::Null) => false, // Missing field equals null (so not not-equal)
                    (Some(Value::Null), Value::Null) => false, // Explicit null equals null (so not not-equal)
                    _ => actual != Some(expected),
                }
            }
            ComparisonOp::Greater => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() > e.as_f64()
                } else if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    // Support date string comparisons (ISO 8601 format)
                    a > e
                } else {
                    false
                }
            }
            ComparisonOp::Less => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() < e.as_f64()
                } else if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    // Support date string comparisons (ISO 8601 format)
                    a < e
                } else {
                    false
                }
            }
            ComparisonOp::GreaterEqual => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() >= e.as_f64()
                } else if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    // Support date string comparisons (ISO 8601 format)
                    a >= e
                } else {
                    false
                }
            }
            ComparisonOp::LessEqual => {
                if let (Some(Value::Number(a)), Value::Number(e)) = (actual, expected) {
                    a.as_f64() <= e.as_f64()
                } else if let (Some(Value::String(a)), Value::String(e)) = (actual, expected) {
                    // Support date string comparisons (ISO 8601 format)
                    a <= e
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
                    if let Some(Value::String(s)) =
                        Self::get_nested_field_with_array(item, &args[0]).as_ref()
                    {
                        return s.contains(&args[1]);
                    }
                }
                false
            }
            "starts_with" => {
                if args.len() == 2 {
                    if let Some(Value::String(s)) =
                        Self::get_nested_field_with_array(item, &args[0]).as_ref()
                    {
                        return s.starts_with(&args[1]);
                    }
                }
                false
            }
            "ends_with" => {
                if args.len() == 2 {
                    if let Some(Value::String(s)) =
                        Self::get_nested_field_with_array(item, &args[0]).as_ref()
                    {
                        return s.ends_with(&args[1]);
                    }
                }
                false
            }
            "is_null" => {
                if args.len() == 1 {
                    let val = Self::get_nested_field_with_array(item, &args[0]);
                    return val == Some(Value::Null); // Only match explicit null, not missing
                }
                false
            }
            "is_not_null" => {
                if args.len() == 1 {
                    let val = Self::get_nested_field_with_array(item, &args[0]);
                    return val.is_some() && val != Some(Value::Null);
                }
                false
            }
            // Type checking functions
            "is_number" => {
                if args.len() == 1 {
                    if let Some(val) = Self::get_nested_field_with_array(item, &args[0]) {
                        return matches!(val, Value::Number(_));
                    }
                }
                false
            }
            "is_string" => {
                if args.len() == 1 {
                    if let Some(val) = Self::get_nested_field_with_array(item, &args[0]) {
                        return matches!(val, Value::String(_));
                    }
                }
                false
            }
            "is_bool" => {
                if args.len() == 1 {
                    if let Some(val) = Self::get_nested_field_with_array(item, &args[0]) {
                        return matches!(val, Value::Bool(_));
                    }
                }
                false
            }
            "is_array" => {
                if args.len() == 1 {
                    if let Some(val) = Self::get_nested_field_with_array(item, &args[0]) {
                        return matches!(val, Value::Array(_));
                    }
                }
                false
            }
            "is_object" => {
                if args.len() == 1 {
                    if let Some(val) = Self::get_nested_field_with_array(item, &args[0]) {
                        return matches!(val, Value::Object(_));
                    }
                }
                false
            }
            // Computed field functions
            "length" => {
                if args.len() == 2 {
                    if let Some(val) = Self::get_nested_field_with_array(item, &args[0]) {
                        let len = match val {
                            Value::String(s) => s.len() as f64,
                            Value::Array(arr) => arr.len() as f64,
                            Value::Object(obj) => obj.len() as f64,
                            _ => return false,
                        };
                        if let Ok(expected) = args[1].parse::<f64>() {
                            return (len - expected).abs() < f64::EPSILON;
                        }
                    }
                }
                false
            }
            "matches" => {
                if args.len() == 2 {
                    if let Some(Value::String(s)) =
                        Self::get_nested_field_with_array(item, &args[0]).as_ref()
                    {
                        // Remove quotes from regex pattern if present
                        let pattern = args[1].trim_matches('"').trim_matches('\'');
                        match Regex::new(pattern) {
                            Ok(re) => return re.is_match(s),
                            Err(e) => {
                                warn!("Invalid regex pattern '{}': {}", pattern, e);
                                return false;
                            }
                        }
                    }
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
    fn compare_values(
        &self,
        a: Option<&Value>,
        b: Option<&Value>,
        null_position: &NullPosition,
    ) -> Ordering {
        match (a, b) {
            (None, None) | (Some(Value::Null), Some(Value::Null)) => Ordering::Equal,
            (None, Some(v)) | (Some(Value::Null), Some(v)) if !v.is_null() => match null_position {
                NullPosition::First => Ordering::Less,
                NullPosition::Last => Ordering::Greater,
            },
            (Some(v), None) | (Some(v), Some(Value::Null)) if !v.is_null() => match null_position {
                NullPosition::First => Ordering::Greater,
                NullPosition::Last => Ordering::Less,
            },
            (Some(a), Some(b)) => self.compare_json_values(a, b),
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

    #[test]
    fn test_nested_field_filtering() {
        // Test basic nested field access
        let filter = FilterExpression::parse("unified_score.final_score >= 5").unwrap();

        let item1 = json!({
            "unified_score": {
                "final_score": 7.5,
                "complexity_factor": 3.0
            }
        });

        let item2 = json!({
            "unified_score": {
                "final_score": 3.2,
                "complexity_factor": 2.0
            }
        });

        let item3 = json!({
            "unified_score": {
                "complexity_factor": 8.0
                // missing final_score
            }
        });

        assert!(filter.evaluate(&item1)); // 7.5 >= 5
        assert!(!filter.evaluate(&item2)); // 3.2 < 5
        assert!(!filter.evaluate(&item3)); // missing field
    }

    #[test]
    fn test_deeply_nested_field_filtering() {
        // Test deeply nested field access (3+ levels)
        let filter = FilterExpression::parse("location.coordinates.lat > 40.0").unwrap();

        let item1 = json!({
            "location": {
                "coordinates": {
                    "lat": 45.5,
                    "lng": -122.6
                }
            }
        });

        let item2 = json!({
            "location": {
                "coordinates": {
                    "lat": 35.0,
                    "lng": -80.0
                }
            }
        });

        assert!(filter.evaluate(&item1)); // 45.5 > 40.0
        assert!(!filter.evaluate(&item2)); // 35.0 < 40.0
    }

    #[test]
    fn test_nested_field_with_logical_operators() {
        // Test nested fields with AND/OR operators
        let filter = FilterExpression::parse(
            "unified_score.final_score >= 5 && debt_type.category == 'complexity'",
        )
        .unwrap();

        let item1 = json!({
            "unified_score": {
                "final_score": 7.5
            },
            "debt_type": {
                "category": "complexity"
            }
        });

        let item2 = json!({
            "unified_score": {
                "final_score": 7.5
            },
            "debt_type": {
                "category": "performance"
            }
        });

        let item3 = json!({
            "unified_score": {
                "final_score": 3.0
            },
            "debt_type": {
                "category": "complexity"
            }
        });

        assert!(filter.evaluate(&item1)); // Both conditions true
        assert!(!filter.evaluate(&item2)); // Wrong category
        assert!(!filter.evaluate(&item3)); // Score too low
    }

    #[test]
    fn test_nested_field_in_operator() {
        // Test nested field with IN operator
        let filter = FilterExpression::parse("debt_type.severity in ['high', 'critical']").unwrap();

        let item1 = json!({
            "debt_type": {
                "severity": "high"
            }
        });

        let item2 = json!({
            "debt_type": {
                "severity": "critical"
            }
        });

        let item3 = json!({
            "debt_type": {
                "severity": "low"
            }
        });

        assert!(filter.evaluate(&item1));
        assert!(filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
    }

    #[test]
    fn test_nested_field_sorting() {
        // Test sorting by nested fields
        let sorter = Sorter::parse("unified_score.final_score DESC").unwrap();

        let mut items = vec![
            json!({
                "id": 1,
                "unified_score": {"final_score": 3.5}
            }),
            json!({
                "id": 2,
                "unified_score": {"final_score": 8.0}
            }),
            json!({
                "id": 3,
                "unified_score": {"final_score": 5.5}
            }),
        ];

        sorter.sort(&mut items);

        // Check order: should be 8.0, 5.5, 3.5
        assert_eq!(items[0]["id"], 2);
        assert_eq!(items[1]["id"], 3);
        assert_eq!(items[2]["id"], 1);
    }

    #[test]
    fn test_mapreduce_debtmap_scenario() {
        // Test the exact scenario from the debtmap MapReduce workflow
        let pipeline = DataPipeline::from_config(
            Some("$.items[*]".to_string()),
            Some("unified_score.final_score >= 5".to_string()),
            Some("unified_score.final_score DESC".to_string()),
            Some(3), // max_items
        )
        .unwrap();

        let data = json!({
            "items": [
                {
                    "location": {"file": "src/main.rs"},
                    "unified_score": {"final_score": 3.0}
                },
                {
                    "location": {"file": "src/lib.rs"},
                    "unified_score": {"final_score": 7.5}
                },
                {
                    "location": {"file": "src/utils.rs"},
                    "unified_score": {"final_score": 5.1}
                },
                {
                    "location": {"file": "src/parser.rs"},
                    "unified_score": {"final_score": 9.2}
                },
                {
                    "location": {"file": "src/config.rs"},
                    "unified_score": {"final_score": 4.8}
                },
                {
                    "location": {"file": "src/test.rs"},
                    "unified_score": {"final_score": 6.0}
                },
            ]
        });

        let results = pipeline.process(&data).unwrap();

        // Should have 3 items (max_items limit)
        assert_eq!(results.len(), 3);

        // Should be sorted by score descending: 9.2, 7.5, 6.0
        assert_eq!(results[0]["unified_score"]["final_score"], 9.2);
        assert_eq!(results[1]["unified_score"]["final_score"], 7.5);
        assert_eq!(results[2]["unified_score"]["final_score"], 6.0);

        // Item with score 5.1 should be included if we had max_items=4
        let pipeline_4 = DataPipeline::from_config(
            Some("$.items[*]".to_string()),
            Some("unified_score.final_score >= 5".to_string()),
            Some("unified_score.final_score DESC".to_string()),
            Some(4),
        )
        .unwrap();

        let results_4 = pipeline_4.process(&data).unwrap();
        assert_eq!(results_4.len(), 4);
        assert_eq!(results_4[3]["unified_score"]["final_score"], 5.1);
    }

    #[test]
    fn test_distinct_deduplication() {
        // Test deduplication based on distinct field
        let pipeline = DataPipeline {
            distinct: Some("id".to_string()),
            ..Default::default()
        };

        let items = vec![
            json!({"id": 1, "value": "a"}),
            json!({"id": 2, "value": "b"}),
            json!({"id": 1, "value": "c"}), // Duplicate id
            json!({"id": 3, "value": "d"}),
            json!({"id": 2, "value": "e"}), // Duplicate id
        ];

        let result = pipeline.deduplicate(items, "id").unwrap();
        assert_eq!(result.len(), 3); // Only unique ids: 1, 2, 3
        assert_eq!(result[0]["id"], 1);
        assert_eq!(result[1]["id"], 2);
        assert_eq!(result[2]["id"], 3);
    }

    #[test]
    fn test_array_access_in_filter() {
        // Test array index access in filter expressions
        // Note: Currently parses as a simple field name, not array access
        // This would need additional parser enhancement for full array syntax
        // For now, test nested field access which is implemented
        let filter = FilterExpression::parse("tags.0 == 'urgent'").unwrap();

        let item1 = json!({
            "tags": {"0": "urgent"} // Using object with numeric key as workaround
        });

        let item2 = json!({
            "tags": {"0": "normal"}
        });

        let item3 = json!({
            "tags": {} // Empty object
        });

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
    }

    #[test]
    fn test_date_comparison() {
        // Test date string comparisons (ISO 8601 format)
        let filter = FilterExpression::parse("created_at > '2024-01-01T00:00:00Z'").unwrap();

        let item1 = json!({
            "created_at": "2024-06-15T12:00:00Z"
        });

        let item2 = json!({
            "created_at": "2023-12-31T23:59:59Z"
        });

        let item3 = json!({
            "created_at": "2024-01-01T00:00:01Z"
        });

        assert!(filter.evaluate(&item1)); // After 2024-01-01
        assert!(!filter.evaluate(&item2)); // Before 2024-01-01
        assert!(filter.evaluate(&item3)); // Just after 2024-01-01
    }

    #[test]
    fn test_null_handling_in_filter() {
        // Test null comparisons
        let filter1 = FilterExpression::parse("optional_field == null").unwrap();
        let filter2 = FilterExpression::parse("optional_field != null").unwrap();

        let item_null = json!({
            "optional_field": null
        });

        let item_missing = json!({
            "other_field": "value"
        });

        let item_present = json!({
            "optional_field": "value"
        });

        // == null should match explicit null
        assert!(filter1.evaluate(&item_null));
        assert!(filter1.evaluate(&item_missing)); // Missing is treated as null for == null comparison
        assert!(!filter1.evaluate(&item_present));

        // != null should match present values
        assert!(!filter2.evaluate(&item_null));
        assert!(!filter2.evaluate(&item_missing)); // Missing is treated as null for != null comparison
        assert!(filter2.evaluate(&item_present));
    }

    #[test]
    fn test_sort_with_null_position() {
        // Test sorting with DESC - nulls end up first because DESC reverses the order
        // and nulls are considered "greater" (sort last in ASC, first in DESC)
        let sorter = Sorter::parse("score DESC").unwrap();

        let mut items = vec![
            json!({"id": 1, "score": 5}),
            json!({"id": 2, "score": 3}),
            json!({"id": 3, "score": null}),
            json!({"id": 4, "score": 10}),
        ];

        sorter.sort(&mut items);

        // With DESC, the order is: null first, then 10, 5, 3
        assert_eq!(items[0]["score"], Value::Null); // Null comes first in DESC
        assert_eq!(items[1]["score"], 10); // Highest non-null score
        assert_eq!(items[2]["score"], 5); // Middle score
        assert_eq!(items[3]["score"], 3); // Lowest score
    }

    #[test]
    fn test_type_checking_functions() {
        // Test is_number
        let filter = FilterExpression::Function {
            name: "is_number".to_string(),
            args: vec!["score".to_string()],
        };

        let item1 = json!({"score": 42});
        let item2 = json!({"score": "42"});
        let item3 = json!({"score": null});
        let item4 = json!({"name": "test"}); // Missing field

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
        assert!(!filter.evaluate(&item4));

        // Test is_string
        let filter = FilterExpression::Function {
            name: "is_string".to_string(),
            args: vec!["name".to_string()],
        };

        let item1 = json!({"name": "Alice"});
        let item2 = json!({"name": 123});
        let item3 = json!({"name": null});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));

        // Test is_bool
        let filter = FilterExpression::Function {
            name: "is_bool".to_string(),
            args: vec!["active".to_string()],
        };

        let item1 = json!({"active": true});
        let item2 = json!({"active": false});
        let item3 = json!({"active": "true"});
        let item4 = json!({"active": 1});

        assert!(filter.evaluate(&item1));
        assert!(filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
        assert!(!filter.evaluate(&item4));

        // Test is_array
        let filter = FilterExpression::Function {
            name: "is_array".to_string(),
            args: vec!["tags".to_string()],
        };

        let item1 = json!({"tags": ["a", "b", "c"]});
        let item2 = json!({"tags": "a,b,c"});
        let item3 = json!({"tags": {"a": 1}});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));

        // Test is_object
        let filter = FilterExpression::Function {
            name: "is_object".to_string(),
            args: vec!["metadata".to_string()],
        };

        let item1 = json!({"metadata": {"key": "value"}});
        let item2 = json!({"metadata": ["key", "value"]});
        let item3 = json!({"metadata": "key=value"});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
    }

    #[test]
    fn test_length_function() {
        // Test length of string
        let filter = FilterExpression::Function {
            name: "length".to_string(),
            args: vec!["name".to_string(), "5".to_string()],
        };

        let item1 = json!({"name": "Alice"}); // length 5
        let item2 = json!({"name": "Bob"}); // length 3
        let item3 = json!({"name": "Charlie"}); // length 7

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));

        // Test length of array
        let filter = FilterExpression::Function {
            name: "length".to_string(),
            args: vec!["tags".to_string(), "3".to_string()],
        };

        let item1 = json!({"tags": ["a", "b", "c"]}); // length 3
        let item2 = json!({"tags": ["a", "b"]}); // length 2
        let item3 = json!({"tags": ["a", "b", "c", "d"]}); // length 4

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(!filter.evaluate(&item3));
    }

    #[test]
    fn test_matches_regex_function() {
        // Test email regex
        let filter = FilterExpression::Function {
            name: "matches".to_string(),
            args: vec![
                "email".to_string(),
                r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$".to_string(),
            ],
        };

        let item1 = json!({"email": "user@example.com"});
        let item2 = json!({"email": "invalid-email"});
        let item3 = json!({"email": "another@test.co.uk"});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(filter.evaluate(&item3));

        // Test file extension regex
        let filter = FilterExpression::Function {
            name: "matches".to_string(),
            args: vec!["filename".to_string(), r"\.rs$".to_string()],
        };

        let item1 = json!({"filename": "main.rs"});
        let item2 = json!({"filename": "README.md"});
        let item3 = json!({"filename": "lib.rs"});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(filter.evaluate(&item3));
    }

    #[test]
    fn test_not_operator() {
        // Test simple NOT
        let filter = FilterExpression::parse("!is_null(optional_field)").unwrap();

        let item1 = json!({"optional_field": "value"});
        let item2 = json!({"optional_field": null});
        let item3 = json!({"other_field": "value"}); // Missing field

        assert!(filter.evaluate(&item1));  // !is_null("value") = !false = true
        assert!(!filter.evaluate(&item2));  // !is_null(null) = !true = false
        assert!(filter.evaluate(&item3));   // !is_null(missing) = !false = true (missing != null)

        // Test NOT with comparison
        let filter = FilterExpression::parse("!(priority > 5)").unwrap();

        let item1 = json!({"priority": 3});
        let item2 = json!({"priority": 7});
        let item3 = json!({"priority": 5});

        assert!(filter.evaluate(&item1));
        assert!(!filter.evaluate(&item2));
        assert!(filter.evaluate(&item3));

        // Test NOT with logical operators
        let filter = FilterExpression::parse("!(status == 'active' && priority > 5)").unwrap();

        let item1 = json!({"status": "active", "priority": 7});
        let item2 = json!({"status": "active", "priority": 3});
        let item3 = json!({"status": "pending", "priority": 7});

        assert!(!filter.evaluate(&item1));
        assert!(filter.evaluate(&item2));
        assert!(filter.evaluate(&item3));
    }

    #[test]
    fn test_complex_expressions_with_parentheses() {
        // Test complex expression with mixed operators and parentheses
        let filter = FilterExpression::parse(
            "(status == 'active' || status == 'pending') && !(priority < 3)",
        )
        .unwrap();

        let item1 = json!({"status": "active", "priority": 5});
        let item2 = json!({"status": "pending", "priority": 7});
        let item3 = json!({"status": "archived", "priority": 5});
        let item4 = json!({"status": "active", "priority": 2});

        assert!(filter.evaluate(&item1)); // active AND priority >= 3
        assert!(filter.evaluate(&item2)); // pending AND priority >= 3
        assert!(!filter.evaluate(&item3)); // archived (fails first condition)
        assert!(!filter.evaluate(&item4)); // priority < 3 (fails second condition)
    }

    #[test]
    fn test_complex_multifield_sorting() {
        // Test multi-field sorting with different directions
        // Note: NULLS FIRST/LAST parsing is implemented but behavior needs refinement
        let sorter = Sorter::parse("category ASC, priority DESC, name ASC").unwrap();

        let mut items = vec![
            json!({"category": "urgent", "priority": 5, "name": "Task A"}),
            json!({"category": "normal", "priority": null, "name": "Task B"}),
            json!({"category": "urgent", "priority": 10, "name": "Task C"}),
            json!({"category": "normal", "priority": 8, "name": "Task D"}),
            json!({"category": "urgent", "priority": 5, "name": "Task E"}),
        ];

        sorter.sort(&mut items);

        // Check sorting: first by category ASC (normal < urgent),
        // then by priority DESC (nulls come first in DESC), then by name ASC
        assert_eq!(items[0]["category"], "normal");
        assert_eq!(items[0]["priority"], Value::Null); // Null comes first in DESC

        assert_eq!(items[1]["category"], "normal");
        assert_eq!(items[1]["priority"], 8); // Highest non-null priority in "normal"

        assert_eq!(items[2]["category"], "urgent");
        assert_eq!(items[2]["priority"], 10); // Highest priority in "urgent"

        assert_eq!(items[3]["category"], "urgent");
        assert_eq!(items[3]["priority"], 5);
        assert_eq!(items[3]["name"], "Task A"); // Sorted by name when priority equal

        assert_eq!(items[4]["category"], "urgent");
        assert_eq!(items[4]["priority"], 5);
        assert_eq!(items[4]["name"], "Task E");
    }

    #[test]
    fn test_nested_field_functions() {
        // Test function expressions with nested fields
        let contains_filter = FilterExpression::Function {
            name: "contains".to_string(),
            args: vec!["location.file".to_string(), "main".to_string()],
        };

        let item1 = json!({
            "location": {
                "file": "src/main.rs"
            }
        });

        let item2 = json!({
            "location": {
                "file": "src/lib.rs"
            }
        });

        assert!(contains_filter.evaluate(&item1));
        assert!(!contains_filter.evaluate(&item2));

        // Test starts_with on nested field
        let starts_filter = FilterExpression::Function {
            name: "starts_with".to_string(),
            args: vec!["location.file".to_string(), "src/".to_string()],
        };

        assert!(starts_filter.evaluate(&item1));
        assert!(starts_filter.evaluate(&item2));

        // Test is_null on nested field
        let null_filter = FilterExpression::Function {
            name: "is_null".to_string(),
            args: vec!["location.line".to_string()],
        };

        let item_with_null = json!({
            "location": {
                "file": "src/main.rs",
                "line": null
            }
        });

        let item_without_field = json!({
            "location": {
                "file": "src/main.rs"
            }
        });

        assert!(null_filter.evaluate(&item_with_null));
        // For is_null function, missing field returns false (None != Some(Null))
        assert!(null_filter.evaluate(&item_with_null));
        assert!(!null_filter.evaluate(&item_without_field)); // is_null requires explicit null
    }
}
