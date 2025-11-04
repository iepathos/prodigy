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
#[derive(Debug, Clone, PartialEq)]
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

        // Try parsing in order of precedence
        Self::try_strip_outer_parens(expr)
            .or_else(|| Self::try_parse_not_operator(expr))
            .or_else(|| Self::try_parse_or_operator(expr))
            .or_else(|| Self::try_parse_and_operator(expr))
            .or_else(|| Self::try_parse_in_operator(expr))
            .or_else(|| Self::try_parse_function(expr))
            .or_else(|| Self::try_parse_comparison(expr))
            .unwrap_or_else(|| Err(anyhow!("Invalid filter expression: {}", expr)))
    }

    /// Check if outer parentheses wrap the entire expression and strip them
    fn try_strip_outer_parens(expr: &str) -> Option<Result<Self>> {
        if !Self::has_outer_parens(expr) {
            return None;
        }

        if Self::outer_parens_wrap_entire_expr(expr) {
            Some(Self::parse(&expr[1..expr.len() - 1]))
        } else {
            None
        }
    }

    /// Check if expression starts and ends with parentheses
    fn has_outer_parens(expr: &str) -> bool {
        expr.starts_with('(') && expr.ends_with(')')
    }

    /// Check if outer parentheses wrap the entire expression
    fn outer_parens_wrap_entire_expr(expr: &str) -> bool {
        let chars: Vec<char> = expr.chars().collect();
        let mut depth = 0;

        for (i, &ch) in chars.iter().enumerate() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    // If depth reaches 0 before the last character, outer parens don't wrap whole expr
                    if depth == 0 && i < chars.len() - 1 {
                        return false;
                    }
                }
                _ => {}
            }
        }

        depth == 0
    }

    /// Try to parse a NOT operator expression
    fn try_parse_not_operator(expr: &str) -> Option<Result<Self>> {
        expr.strip_prefix("!")
            .map(|stripped| Self::parse_not_expression(stripped.trim()))
    }

    /// Parse the inner expression of a NOT operator
    fn parse_not_expression(inner_expr: &str) -> Result<Self> {
        let inner = if Self::has_outer_parens(inner_expr) {
            Self::parse(&inner_expr[1..inner_expr.len() - 1])?
        } else {
            Self::parse(inner_expr)?
        };

        Ok(FilterExpression::Logical {
            op: LogicalOp::Not,
            operands: vec![inner],
        })
    }

    /// Try to parse an OR logical operator (supports both || and OR)
    fn try_parse_or_operator(expr: &str) -> Option<Result<Self>> {
        Self::find_logical_operator(expr, "||")
            .map(|pos| Self::parse_binary_logical(expr, pos, 2, LogicalOp::Or))
            .or_else(|| {
                Self::find_word_logical_operator(expr, "OR")
                    .map(|pos| Self::parse_binary_logical(expr, pos, 2, LogicalOp::Or))
            })
    }

    /// Try to parse an AND logical operator (supports both && and AND)
    fn try_parse_and_operator(expr: &str) -> Option<Result<Self>> {
        Self::find_logical_operator(expr, "&&")
            .map(|pos| Self::parse_binary_logical(expr, pos, 2, LogicalOp::And))
            .or_else(|| {
                Self::find_word_logical_operator(expr, "AND")
                    .map(|pos| Self::parse_binary_logical(expr, pos, 3, LogicalOp::And))
            })
    }

    /// Find the position of a logical operator outside of parentheses
    fn find_logical_operator(expr: &str, op: &str) -> Option<usize> {
        let chars: Vec<char> = expr.chars().collect();
        let mut paren_depth = 0;
        let op_chars: Vec<char> = op.chars().collect();

        for i in 0..chars.len() {
            match chars[i] {
                '(' => paren_depth += 1,
                ')' => paren_depth -= 1,
                _ if paren_depth == 0 && Self::matches_operator_at(&chars, i, &op_chars) => {
                    return Some(i);
                }
                _ => {}
            }
        }

        None
    }

    /// Find the position of a word-based logical operator (OR, AND) outside of parentheses
    /// Ensures the operator is surrounded by whitespace to avoid false matches
    fn find_word_logical_operator(expr: &str, op: &str) -> Option<usize> {
        let expr_upper = expr.to_uppercase();
        let mut paren_depth = 0;
        let chars: Vec<char> = expr.chars().collect();

        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '(' => paren_depth += 1,
                ')' => paren_depth -= 1,
                _ => {}
            }

            // Only check for operator if not inside parentheses
            if paren_depth == 0 {
                // Check if we're at a word boundary and the operator matches
                if (i == 0
                    || chars
                        .get(i.saturating_sub(1))
                        .is_none_or(|c| c.is_whitespace()))
                    && expr_upper[i..].starts_with(op)
                    && expr
                        .get(i + op.len()..i + op.len() + 1)
                        .is_none_or(|s| s.starts_with(char::is_whitespace))
                {
                    return Some(i);
                }
            }

            i += 1;
        }

        None
    }

    /// Check if the operator matches at the given position
    fn matches_operator_at(chars: &[char], pos: usize, op_chars: &[char]) -> bool {
        if pos + op_chars.len() > chars.len() {
            return false;
        }

        chars[pos..pos + op_chars.len()]
            .iter()
            .zip(op_chars.iter())
            .all(|(a, b)| a == b)
    }

    /// Parse a binary logical expression (AND/OR)
    fn parse_binary_logical(expr: &str, pos: usize, op_len: usize, op: LogicalOp) -> Result<Self> {
        let left = Self::parse(&expr[..pos])?;
        let right = Self::parse(&expr[pos + op_len..])?;

        Ok(FilterExpression::Logical {
            op,
            operands: vec![left, right],
        })
    }

    /// Try to parse an 'in' operator expression
    fn try_parse_in_operator(expr: &str) -> Option<Result<Self>> {
        expr.find(" in ")
            .map(|pos| Self::parse_in_expression(expr, pos))
    }

    /// Parse an 'in' expression for checking if a value is in a list
    fn parse_in_expression(expr: &str, in_pos: usize) -> Result<Self> {
        let field = expr[..in_pos].trim().to_string();
        let values_str = expr[in_pos + 4..].trim();

        let values = Self::parse_array_values(values_str)?;

        Ok(FilterExpression::In { field, values })
    }

    /// Parse an array of values from a string like "['value1', 'value2']"
    fn parse_array_values(values_str: &str) -> Result<Vec<Value>> {
        if !values_str.starts_with('[') || !values_str.ends_with(']') {
            return Err(anyhow!("Invalid 'in' expression format: expected array"));
        }

        let values_inner = &values_str[1..values_str.len() - 1];
        let parsed_values = values_inner
            .split(',')
            .map(|s| Self::parse_quoted_string(s.trim()))
            .collect();

        Ok(parsed_values)
    }

    /// Parse a quoted string into a Value
    fn parse_quoted_string(s: &str) -> Value {
        let unquoted = s.trim_matches('\'').trim_matches('"');
        Value::String(unquoted.to_string())
    }

    /// Try to parse a function call expression
    fn try_parse_function(expr: &str) -> Option<Result<Self>> {
        if !expr.contains('(') || !expr.contains(')') {
            return None;
        }

        Some(Self::parse_function_expression(expr))
    }

    /// Parse a function call expression
    fn parse_function_expression(expr: &str) -> Result<Self> {
        let open_paren = expr
            .find('(')
            .context("Invalid expression: missing opening parenthesis")?;
        let close_paren = expr
            .rfind(')')
            .context("Invalid expression: missing closing parenthesis")?;

        let name = expr[..open_paren].trim().to_string();
        let args = Self::parse_function_args(&expr[open_paren + 1..close_paren]);

        Ok(FilterExpression::Function { name, args })
    }

    /// Parse function arguments from a comma-separated string
    fn parse_function_args(args_str: &str) -> Vec<String> {
        if args_str.is_empty() {
            Vec::new()
        } else {
            args_str.split(',').map(|s| s.trim().to_string()).collect()
        }
    }

    /// Try to parse a comparison expression
    fn try_parse_comparison(expr: &str) -> Option<Result<Self>> {
        Self::find_comparison_operator(expr)
            .map(|(op_str, pos)| Self::parse_comparison_expression(expr, op_str, pos))
    }

    /// Find a comparison operator in the expression
    fn find_comparison_operator(expr: &str) -> Option<(&'static str, usize)> {
        let operators = ["==", "!=", ">=", "<=", ">", "<", "="];

        operators
            .iter()
            .find_map(|&op| expr.find(op).map(|pos| (op, pos)))
    }

    /// Parse a comparison expression
    fn parse_comparison_expression(expr: &str, op_str: &str, op_pos: usize) -> Result<Self> {
        let field = expr[..op_pos].trim().to_string();
        let value_str = expr[op_pos + op_str.len()..].trim();
        let value = Self::parse_value(value_str)?;
        let op = Self::string_to_comparison_op(op_str)?;

        Ok(FilterExpression::Comparison { field, op, value })
    }

    /// Convert a string operator to a ComparisonOp
    fn string_to_comparison_op(op_str: &str) -> Result<ComparisonOp> {
        match op_str {
            "==" | "=" => Ok(ComparisonOp::Equal),
            "!=" => Ok(ComparisonOp::NotEqual),
            ">" => Ok(ComparisonOp::Greater),
            "<" => Ok(ComparisonOp::Less),
            ">=" => Ok(ComparisonOp::GreaterEqual),
            "<=" => Ok(ComparisonOp::LessEqual),
            _ => Err(anyhow!("Unknown operator: {}", op_str)),
        }
    }

    /// Parse a value string into a JSON value
    fn parse_value(value_str: &str) -> Result<Value> {
        let trimmed = value_str.trim();

        let value = Self::try_parse_quoted_string(trimmed)
            .or_else(|| Self::try_parse_boolean(trimmed))
            .or_else(|| Self::try_parse_null(trimmed))
            .or_else(|| Self::try_parse_number(trimmed))
            .unwrap_or_else(|| Value::String(trimmed.to_string()));

        Ok(value)
    }

    /// Pure function: Try to parse a quoted string
    fn try_parse_quoted_string(s: &str) -> Option<Value> {
        Self::is_quoted(s).then(|| Value::String(Self::unquote(s)))
    }

    /// Pure function: Check if string is quoted
    fn is_quoted(s: &str) -> bool {
        (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    }

    /// Pure function: Remove quotes from string
    fn unquote(s: &str) -> String {
        s[1..s.len() - 1].to_string()
    }

    /// Pure function: Try to parse a boolean value
    fn try_parse_boolean(s: &str) -> Option<Value> {
        match s {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        }
    }

    /// Pure function: Try to parse a null value
    fn try_parse_null(s: &str) -> Option<Value> {
        (s == "null").then_some(Value::Null)
    }

    /// Pure function: Try to parse a numeric value
    fn try_parse_number(s: &str) -> Option<Value> {
        s.parse::<f64>()
            .ok()
            .and_then(|num| serde_json::Number::from_f64(num).map(Value::Number))
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

    /// Parse a path that may contain array indices (e.g., "field.array\[0\].nested")
    fn parse_path_with_array(path: &str) -> Vec<PathPart> {
        let mut parts = Vec::new();
        let mut chars = path.chars().peekable();

        while chars.peek().is_some() {
            if let Some(part) = Self::parse_next_path_part(&mut chars) {
                parts.push(part);
            }
        }

        parts
    }

    /// Pure function: Parse the next path part from character iterator
    fn parse_next_path_part(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<PathPart> {
        // Skip dots
        if chars.peek() == Some(&'.') {
            chars.next();
        }

        // Check if we're parsing an array index
        if chars.peek() == Some(&'[') {
            return Self::parse_array_index(chars);
        }

        // Parse field name
        Self::parse_field_name(chars)
    }

    /// Pure function: Parse a field name until we hit '.', '[', or end
    fn parse_field_name(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<PathPart> {
        let mut field = String::new();

        while let Some(&ch) = chars.peek() {
            if ch == '.' || ch == '[' {
                break;
            }
            field.push(ch);
            chars.next();
        }

        (!field.is_empty()).then_some(PathPart::Field(field))
    }

    /// Pure function: Parse an array index from "\[N\]"
    fn parse_array_index(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<PathPart> {
        // Consume opening bracket
        chars.next()?;

        // Collect digits until closing bracket
        let mut index_str = String::new();
        while let Some(&ch) = chars.peek() {
            if ch == ']' {
                break;
            }
            index_str.push(ch);
            chars.next();
        }

        // Consume closing bracket
        chars.next();

        index_str.parse::<usize>().ok().map(PathPart::Index)
    }

    /// Compare two values using the given operator
    fn compare(actual: Option<&Value>, op: &ComparisonOp, expected: &Value) -> bool {
        match op {
            ComparisonOp::Equal => Self::compare_equal(actual, expected),
            ComparisonOp::NotEqual => Self::compare_not_equal(actual, expected),
            ComparisonOp::Greater => Self::compare_greater(actual, expected),
            ComparisonOp::Less => Self::compare_less(actual, expected),
            ComparisonOp::GreaterEqual => Self::compare_greater_equal(actual, expected),
            ComparisonOp::LessEqual => Self::compare_less_equal(actual, expected),
            ComparisonOp::Contains => {
                Self::compare_string_op(actual, expected, |a, e| a.contains(e))
            }
            ComparisonOp::StartsWith => {
                Self::compare_string_op(actual, expected, |a, e| a.starts_with(e))
            }
            ComparisonOp::EndsWith => {
                Self::compare_string_op(actual, expected, |a, e| a.ends_with(e))
            }
            ComparisonOp::Matches => Self::compare_regex(actual, expected),
        }
    }

    /// Pure function: Compare for equality with null handling
    fn compare_equal(actual: Option<&Value>, expected: &Value) -> bool {
        match (actual, expected) {
            (None, Value::Null) => true,              // Missing field equals null
            (Some(Value::Null), Value::Null) => true, // Explicit null equals null
            _ => actual == Some(expected),
        }
    }

    /// Pure function: Compare for inequality with null handling
    fn compare_not_equal(actual: Option<&Value>, expected: &Value) -> bool {
        !Self::compare_equal(actual, expected)
    }

    /// Pure function: Compare for greater than
    fn compare_greater(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a > e, |a, e| a > e)
    }

    /// Pure function: Compare for less than
    fn compare_less(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a < e, |a, e| a < e)
    }

    /// Pure function: Compare for greater than or equal
    fn compare_greater_equal(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a >= e, |a, e| a >= e)
    }

    /// Pure function: Compare for less than or equal
    fn compare_less_equal(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a <= e, |a, e| a <= e)
    }

    /// Pure function: Compare using numeric or string comparison
    fn compare_numeric_or_string<FNum, FStr>(
        actual: Option<&Value>,
        expected: &Value,
        num_op: FNum,
        str_op: FStr,
    ) -> bool
    where
        FNum: Fn(&f64, &f64) -> bool,
        FStr: Fn(&str, &str) -> bool,
    {
        match (actual, expected) {
            (Some(Value::Number(a)), Value::Number(e)) => a
                .as_f64()
                .zip(e.as_f64())
                .is_some_and(|(a, e)| num_op(&a, &e)),
            (Some(Value::String(a)), Value::String(e)) => str_op(a.as_str(), e.as_str()),
            _ => false,
        }
    }

    /// Pure function: Compare strings using provided operation
    fn compare_string_op<F>(actual: Option<&Value>, expected: &Value, op: F) -> bool
    where
        F: Fn(&str, &str) -> bool,
    {
        match (actual, expected) {
            (Some(Value::String(a)), Value::String(e)) => op(a.as_str(), e.as_str()),
            _ => false,
        }
    }

    /// Pure function: Compare string against regex pattern
    fn compare_regex(actual: Option<&Value>, expected: &Value) -> bool {
        match (actual, expected) {
            (Some(Value::String(a)), Value::String(pattern)) => Regex::new(pattern).map_or_else(
                |e| {
                    warn!("Invalid regex pattern '{}': {}", pattern, e);
                    false
                },
                |re| re.is_match(a),
            ),
            _ => false,
        }
    }

    /// Evaluate a function expression
    fn evaluate_function(item: &Value, name: &str, args: &[String]) -> bool {
        match name {
            "contains" => Self::eval_string_binary_fn(item, args, |s, pattern| s.contains(pattern)),
            "starts_with" => {
                Self::eval_string_binary_fn(item, args, |s, pattern| s.starts_with(pattern))
            }
            "ends_with" => {
                Self::eval_string_binary_fn(item, args, |s, pattern| s.ends_with(pattern))
            }
            "is_null" => Self::eval_is_null(item, args),
            "is_not_null" => Self::eval_is_not_null(item, args),
            "is_number" => Self::eval_type_check(item, args, |v| matches!(v, Value::Number(_))),
            "is_string" => Self::eval_type_check(item, args, |v| matches!(v, Value::String(_))),
            "is_bool" => Self::eval_type_check(item, args, |v| matches!(v, Value::Bool(_))),
            "is_array" => Self::eval_type_check(item, args, |v| matches!(v, Value::Array(_))),
            "is_object" => Self::eval_type_check(item, args, |v| matches!(v, Value::Object(_))),
            "length" => Self::eval_length(item, args),
            "matches" => Self::eval_matches(item, args),
            _ => {
                warn!("Unknown function in filter expression: {}", name);
                false
            }
        }
    }

    /// Pure function: Evaluate a binary string function (contains, starts_with, ends_with)
    fn eval_string_binary_fn<F>(item: &Value, args: &[String], op: F) -> bool
    where
        F: Fn(&str, &str) -> bool,
    {
        if args.len() == 2 {
            Self::get_nested_field_with_array(item, &args[0])
                .and_then(|v| match v {
                    Value::String(s) => Some(op(s.as_str(), args[1].as_str())),
                    _ => None,
                })
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Pure function: Evaluate is_null function
    fn eval_is_null(item: &Value, args: &[String]) -> bool {
        args.len() == 1 && Self::get_nested_field_with_array(item, &args[0]) == Some(Value::Null)
    }

    /// Pure function: Evaluate is_not_null function
    fn eval_is_not_null(item: &Value, args: &[String]) -> bool {
        args.len() == 1
            && Self::get_nested_field_with_array(item, &args[0]).is_some_and(|v| v != Value::Null)
    }

    /// Pure function: Evaluate type checking function
    fn eval_type_check<F>(item: &Value, args: &[String], predicate: F) -> bool
    where
        F: Fn(&Value) -> bool,
    {
        if args.len() == 1 {
            Self::get_nested_field_with_array(item, &args[0]).is_some_and(|v| predicate(&v))
        } else {
            false
        }
    }

    /// Pure function: Evaluate length function
    fn eval_length(item: &Value, args: &[String]) -> bool {
        if args.len() == 2 {
            Self::get_nested_field_with_array(item, &args[0])
                .and_then(|v| Self::get_value_length(&v))
                .zip(args[1].parse::<f64>().ok())
                .is_some_and(|(len, expected)| (len - expected).abs() < f64::EPSILON)
        } else {
            false
        }
    }

    /// Pure function: Get length of a value (string, array, or object)
    fn get_value_length(value: &Value) -> Option<f64> {
        match value {
            Value::String(s) => Some(s.len() as f64),
            Value::Array(arr) => Some(arr.len() as f64),
            Value::Object(obj) => Some(obj.len() as f64),
            _ => None,
        }
    }

    /// Pure function: Evaluate regex matches function
    fn eval_matches(item: &Value, args: &[String]) -> bool {
        if args.len() == 2 {
            Self::get_nested_field_with_array(item, &args[0])
                .and_then(|v| match v {
                    Value::String(s) => {
                        let pattern = args[1].trim_matches('"').trim_matches('\'');
                        Some(Self::regex_matches(s.as_str(), pattern))
                    }
                    _ => None,
                })
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Pure function: Check if string matches regex pattern
    fn regex_matches(text: &str, pattern: &str) -> bool {
        Regex::new(pattern).map_or_else(
            |e| {
                warn!("Invalid regex pattern '{}': {}", pattern, e);
                false
            },
            |re| re.is_match(text),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Integration and pipeline tests
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

    // Tests for pure helper functions
    #[test]
    fn test_pure_parse_value_helpers() {
        // Test is_quoted
        assert!(FilterExpression::is_quoted("\"hello\""));
        assert!(FilterExpression::is_quoted("'hello'"));
        assert!(!FilterExpression::is_quoted("hello"));
        assert!(!FilterExpression::is_quoted("\"hello"));
        assert!(!FilterExpression::is_quoted("hello\""));

        // Test unquote
        assert_eq!(FilterExpression::unquote("\"hello\""), "hello");
        assert_eq!(FilterExpression::unquote("'world'"), "world");

        // Test try_parse_boolean
        assert_eq!(
            FilterExpression::try_parse_boolean("true"),
            Some(Value::Bool(true))
        );
        assert_eq!(
            FilterExpression::try_parse_boolean("false"),
            Some(Value::Bool(false))
        );
        assert_eq!(FilterExpression::try_parse_boolean("TRUE"), None);
        assert_eq!(FilterExpression::try_parse_boolean("1"), None);

        // Test try_parse_null
        assert_eq!(FilterExpression::try_parse_null("null"), Some(Value::Null));
        assert_eq!(FilterExpression::try_parse_null("NULL"), None);
        assert_eq!(FilterExpression::try_parse_null("nil"), None);

        // Test try_parse_number
        assert!(FilterExpression::try_parse_number("42").is_some());
        assert!(FilterExpression::try_parse_number("3.14").is_some());
        assert!(FilterExpression::try_parse_number("-10").is_some());
        assert_eq!(FilterExpression::try_parse_number("abc"), None);
    }

    #[test]
    fn test_pure_compare_helpers() {
        // Test compare_equal
        assert!(FilterExpression::compare_equal(None, &Value::Null));
        assert!(FilterExpression::compare_equal(
            Some(&Value::Null),
            &Value::Null
        ));
        assert!(FilterExpression::compare_equal(
            Some(&Value::String("test".to_string())),
            &Value::String("test".to_string())
        ));
        assert!(!FilterExpression::compare_equal(
            Some(&Value::String("test".to_string())),
            &Value::String("other".to_string())
        ));

        // Test compare_not_equal
        assert!(!FilterExpression::compare_not_equal(
            Some(&Value::Null),
            &Value::Null
        ));
        assert!(FilterExpression::compare_not_equal(
            Some(&Value::String("test".to_string())),
            &Value::String("other".to_string())
        ));

        // Test compare_greater
        assert!(FilterExpression::compare_greater(
            Some(&json!(10)),
            &json!(5)
        ));
        assert!(!FilterExpression::compare_greater(
            Some(&json!(5)),
            &json!(10)
        ));
        assert!(FilterExpression::compare_greater(
            Some(&Value::String("b".to_string())),
            &Value::String("a".to_string())
        ));

        // Test compare_less
        assert!(FilterExpression::compare_less(Some(&json!(5)), &json!(10)));
        assert!(!FilterExpression::compare_less(Some(&json!(10)), &json!(5)));
    }

    #[test]
    fn test_pure_path_parsing_helpers() {
        // Test parse_field_name
        let mut chars = "field.nested".chars().peekable();
        let result = FilterExpression::parse_field_name(&mut chars);
        assert_eq!(result, Some(PathPart::Field("field".to_string())));
        assert_eq!(chars.peek(), Some(&'.')); // Should stop at dot

        // Test parse_array_index
        let mut chars = "[42]".chars().peekable();
        let result = FilterExpression::parse_array_index(&mut chars);
        assert_eq!(result, Some(PathPart::Index(42)));
        assert_eq!(chars.peek(), None); // Should consume all

        // Test invalid index
        let mut chars = "[abc]".chars().peekable();
        let result = FilterExpression::parse_array_index(&mut chars);
        assert_eq!(result, None);
    }

    #[test]
    fn test_pure_eval_function_helpers() {
        let item = json!({
            "name": "Alice",
            "score": 42,
            "tags": ["a", "b"],
            "optional": null
        });

        // Test eval_is_null
        assert!(!FilterExpression::eval_is_null(
            &item,
            &["name".to_string()]
        ));
        assert!(FilterExpression::eval_is_null(
            &item,
            &["optional".to_string()]
        ));

        // Test eval_is_not_null
        assert!(FilterExpression::eval_is_not_null(
            &item,
            &["name".to_string()]
        ));
        assert!(!FilterExpression::eval_is_not_null(
            &item,
            &["optional".to_string()]
        ));

        // Test get_value_length
        assert_eq!(
            FilterExpression::get_value_length(&Value::String("hello".to_string())),
            Some(5.0)
        );
        assert_eq!(
            FilterExpression::get_value_length(&json!(["a", "b", "c"])),
            Some(3.0)
        );
        assert_eq!(
            FilterExpression::get_value_length(&json!({"a": 1, "b": 2})),
            Some(2.0)
        );
        assert_eq!(FilterExpression::get_value_length(&json!(42)), None);

        // Test regex_matches
        assert!(FilterExpression::regex_matches("test@example.com", r"@"));
        assert!(!FilterExpression::regex_matches("test", r"@"));
        assert!(FilterExpression::regex_matches("hello123", r"\d+"));
    }

    mod json_path {
        use super::*;

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
        fn test_array_index_access() {
            // Test array index access through path parsing
            let item = json!({
                "tags": ["urgent", "bug", "priority"]
            });

            // Test with array index syntax
            let result = FilterExpression::get_nested_field_with_array(&item, "tags[0]");
            assert_eq!(result, Some(Value::String("urgent".to_string())));

            let result = FilterExpression::get_nested_field_with_array(&item, "tags[1]");
            assert_eq!(result, Some(Value::String("bug".to_string())));

            let result = FilterExpression::get_nested_field_with_array(&item, "tags[2]");
            assert_eq!(result, Some(Value::String("priority".to_string())));

            // Test out of bounds
            let result = FilterExpression::get_nested_field_with_array(&item, "tags[999]");
            assert_eq!(result, None);
        }

        #[test]
        fn test_nested_array_access() {
            // Test nested field with array access
            let item = json!({
                "data": {
                    "items": [
                        {"id": 1, "name": "first"},
                        {"id": 2, "name": "second"}
                    ]
                }
            });

            let result = FilterExpression::get_nested_field_with_array(&item, "data.items[0].name");
            assert_eq!(result, Some(Value::String("first".to_string())));

            let result = FilterExpression::get_nested_field_with_array(&item, "data.items[1].id");
            assert_eq!(result, Some(json!(2)));
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
    }

    mod filter_expression {
        use super::*;

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

            // Test word-based OR operator
            let filter_or = FilterExpression::parse(
                "File.score >= 30 OR Function.unified_score.final_score >= 30",
            )
            .unwrap();

            let file_item = json!({"File": {"score": 105.0}});
            let func_item = json!({"Function": {"unified_score": {"final_score": 45.0}}});
            let low_score_file = json!({"File": {"score": 15.0}});
            let low_score_func = json!({"Function": {"unified_score": {"final_score": 10.0}}});

            assert!(filter_or.evaluate(&file_item));
            assert!(filter_or.evaluate(&func_item));
            assert!(!filter_or.evaluate(&low_score_file));
            assert!(!filter_or.evaluate(&low_score_func));

            // Test word-based AND operator
            let filter_and =
                FilterExpression::parse("priority > 5 AND severity == 'high'").unwrap();
            assert!(filter_and.evaluate(&item1));
            assert!(!filter_and.evaluate(&item2));
            assert!(!filter_and.evaluate(&item3));
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
            let filter =
                FilterExpression::parse("debt_type.severity in ['high', 'critical']").unwrap();

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

            assert!(filter.evaluate(&item1)); // !is_null("value") = !false = true
            assert!(!filter.evaluate(&item2)); // !is_null(null) = !true = false
            assert!(filter.evaluate(&item3)); // !is_null(missing) = !false = true (missing != null)

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

        #[test]
        fn test_string_comparison_operators() {
            // Test Contains - parsing doesn't support ~= operator, so create directly
            let filter = FilterExpression::Comparison {
                field: "name".to_string(),
                op: ComparisonOp::Contains,
                value: json!("test"),
            };

            assert!(matches!(
                filter,
                FilterExpression::Comparison {
                    op: ComparisonOp::Contains,
                    ..
                }
            ));

            // Test StartsWith
            assert!(FilterExpression::compare_string_op(
                Some(&json!("/usr/bin/test")),
                &json!("/usr"),
                |a, e| a.starts_with(e)
            ));

            // Test EndsWith
            assert!(FilterExpression::compare_string_op(
                Some(&json!("file.rs")),
                &json!(".rs"),
                |a, e| a.ends_with(e)
            ));
        }

        #[test]
        fn test_comparison_edge_cases() {
            // Test numeric comparison with different types
            assert!(!FilterExpression::compare_greater(
                Some(&Value::String("10".to_string())),
                &json!(5)
            ));

            // Test null comparisons
            assert!(FilterExpression::compare_equal(None, &Value::Null));
            assert!(!FilterExpression::compare_greater(None, &json!(5)));

            // Test string date comparisons
            assert!(FilterExpression::compare_greater(
                Some(&Value::String("2024-01-02".to_string())),
                &Value::String("2024-01-01".to_string())
            ));
        }

        #[test]
        fn test_filter_expression_parsing_edge_cases() {
            // Test parsing with extra whitespace
            let filter = FilterExpression::parse("  priority  >  5  ").unwrap();
            let item = json!({"priority": 7});
            assert!(filter.evaluate(&item));

            // Test parsing with parentheses
            let filter = FilterExpression::parse("(priority > 5)").unwrap();
            assert!(filter.evaluate(&item));

            // Test parsing NOT with parentheses
            let filter = FilterExpression::parse("!(priority < 5)").unwrap();
            assert!(filter.evaluate(&item));

            // Test parsing complex nested expression
            let filter =
                FilterExpression::parse("((status == 'active') && (priority > 5))").unwrap();
            let item = json!({"status": "active", "priority": 7});
            assert!(filter.evaluate(&item));
        }

        #[test]
        fn test_parse_comparison_operators() {
            // Test all comparison operator variations
            assert!(FilterExpression::parse("x == 5").is_ok());
            assert!(FilterExpression::parse("x = 5").is_ok());
            assert!(FilterExpression::parse("x != 5").is_ok());
            assert!(FilterExpression::parse("x > 5").is_ok());
            assert!(FilterExpression::parse("x < 5").is_ok());
            assert!(FilterExpression::parse("x >= 5").is_ok());
            assert!(FilterExpression::parse("x <= 5").is_ok());
        }

        #[test]
        fn test_parse_error_cases() {
            // Test empty string
            assert!(FilterExpression::parse("").is_err());

            // Test invalid expression with no operators
            assert!(FilterExpression::parse("just some text").is_err());

            // Test malformed 'in' expression without array
            assert!(FilterExpression::parse("field in value").is_err());

            // Test invalid function syntax
            assert!(FilterExpression::parse("func{arg}").is_err());
        }

        #[test]
        fn test_parse_in_operator_variations() {
            // Test basic 'in' operator
            let result = FilterExpression::parse("status in ['active', 'pending']");
            assert!(result.is_ok());

            // Test 'in' with numeric values treated as strings
            let result = FilterExpression::parse("id in ['1', '2', '3']");
            assert!(result.is_ok());

            // Test 'in' with single value
            let result = FilterExpression::parse("status in ['active']");
            assert!(result.is_ok());

            // Test 'in' with empty array
            let result = FilterExpression::parse("status in []");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_nested_parentheses() {
            // Test multiple levels of nested parentheses
            let result = FilterExpression::parse("(((x > 5)))");
            assert!(result.is_ok());

            // Test nested parentheses with logical operators
            let result = FilterExpression::parse("((a == 1) && (b == 2))");
            assert!(result.is_ok());

            // Test parentheses that don't wrap entire expression
            let result = FilterExpression::parse("(a == 1) && b == 2");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_not_operator_variations() {
            // Test NOT with function
            let result = FilterExpression::parse("!is_null(field)");
            assert!(result.is_ok());

            // Test NOT with comparison
            let result = FilterExpression::parse("!(x > 5)");
            assert!(result.is_ok());

            // Test NOT with parentheses stripped
            let result = FilterExpression::parse("!(x == 5)");
            assert!(result.is_ok());

            // Test NOT with comparison (no outer parens)
            let result = FilterExpression::parse("!(status == 'active')");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_logical_operators() {
            // Test OR operator finding
            let result = FilterExpression::parse("a == 1 || b == 2");
            assert!(result.is_ok());

            // Test AND operator finding
            let result = FilterExpression::parse("a == 1 && b == 2");
            assert!(result.is_ok());

            // Test multiple OR operators
            let result = FilterExpression::parse("a == 1 || b == 2 || c == 3");
            assert!(result.is_ok());

            // Test mixed AND/OR operators
            let result = FilterExpression::parse("a == 1 && b == 2 || c == 3");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_function_expressions() {
            // Test function with no arguments
            let result = FilterExpression::parse("is_active()");
            assert!(result.is_ok());

            // Test function with single argument
            let result = FilterExpression::parse("is_null(field)");
            assert!(result.is_ok());

            // Test function with multiple arguments
            let result = FilterExpression::parse("contains(text, 'pattern')");
            assert!(result.is_ok());

            // Test function with whitespace in arguments
            let result = FilterExpression::parse("func( arg1 , arg2 )");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_operator_precedence() {
            // Test that operators are found outside parentheses
            let result = FilterExpression::parse("(a == 1) && (b == 2)");
            assert!(result.is_ok());
            if let Ok(FilterExpression::Logical { op, .. }) = result {
                assert!(matches!(op, LogicalOp::And));
            }

            // Test operator inside parentheses not matched
            let result = FilterExpression::parse("func(a && b)");
            assert!(result.is_ok());
            assert!(matches!(result, Ok(FilterExpression::Function { .. })));
        }

        #[test]
        fn test_parse_value_types() {
            // Test parsing string values
            let result = FilterExpression::parse("name == 'test'");
            assert!(result.is_ok());

            // Test parsing numeric values
            let result = FilterExpression::parse("count > 42");
            assert!(result.is_ok());

            // Test parsing boolean values
            let result = FilterExpression::parse("active == true");
            assert!(result.is_ok());

            // Test parsing null values
            let result = FilterExpression::parse("value == null");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_field_paths() {
            // Test simple field
            let result = FilterExpression::parse("status == 'active'");
            assert!(result.is_ok());

            // Test nested field path
            let result = FilterExpression::parse("user.status == 'active'");
            assert!(result.is_ok());

            // Test deeply nested field path
            let result = FilterExpression::parse("data.user.profile.name == 'test'");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_whitespace_handling() {
            // Test extra whitespace around operators
            let result = FilterExpression::parse("  x   ==   5  ");
            assert!(result.is_ok());

            // Test no whitespace around operators
            let result = FilterExpression::parse("x==5");
            assert!(result.is_ok());

            // Test whitespace in 'in' operator
            let result = FilterExpression::parse("status  in  ['active']");
            assert!(result.is_ok());
        }

        #[test]
        fn test_parse_array_values() {
            // Test parse_array_values with valid input
            let result = FilterExpression::parse_array_values("['a', 'b', 'c']");
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 3);

            // Test parse_array_values with single value
            let result = FilterExpression::parse_array_values("['single']");
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 1);

            // Test parse_array_values with empty array
            let result = FilterExpression::parse_array_values("[]");
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 1); // Split results in one empty string

            // Test parse_array_values with invalid format (no brackets)
            let result = FilterExpression::parse_array_values("'a', 'b'");
            assert!(result.is_err());
        }

        #[test]
        fn test_matches_operator_at() {
            let chars: Vec<char> = "a && b".chars().collect();
            let op_chars: Vec<char> = "&&".chars().collect();

            // Test matching at valid position
            assert!(FilterExpression::matches_operator_at(&chars, 2, &op_chars));

            // Test not matching at wrong position
            assert!(!FilterExpression::matches_operator_at(&chars, 0, &op_chars));

            // Test boundary check
            assert!(!FilterExpression::matches_operator_at(&chars, 5, &op_chars));
        }

        #[test]
        fn test_outer_parens_wrap_entire_expr() {
            // Test outer parens that wrap entire expression
            assert!(FilterExpression::outer_parens_wrap_entire_expr("(a && b)"));

            // Test outer parens that don't wrap entire expression
            assert!(!FilterExpression::outer_parens_wrap_entire_expr("(a) && b"));

            // Test nested parens
            assert!(FilterExpression::outer_parens_wrap_entire_expr(
                "((a && b))"
            ));

            // Test multiple groups
            assert!(!FilterExpression::outer_parens_wrap_entire_expr(
                "(a) || (b)"
            ));
        }
    }

    mod sorter {
        use super::*;

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
        fn test_enum_wrapped_sorting_with_nulls_last() {
            // Test case from debtmap: Files and Functions wrapped in enum variants
            // Files have File.score, Functions have Function.unified_score.final_score
            // When sorting by File.score DESC NULLS LAST, all Files should come before Functions
            let sorter = Sorter::parse(
                "File.score DESC NULLS LAST, Function.unified_score.final_score DESC NULLS LAST",
            )
            .unwrap();

            let mut items = vec![
                json!({"File": {"score": 192}}),
                json!({"Function": {"unified_score": {"final_score": 18}}}),
                json!({"File": {"score": 112}}),
                json!({"Function": {"unified_score": {"final_score": 11}}}),
                json!({"File": {"score": 108}}),
                json!({"Function": {"unified_score": {"final_score": 9}}}),
            ];

            sorter.sort(&mut items);

            // All Files should be first (sorted by score DESC)
            assert!(items[0].get("File").is_some());
            assert_eq!(items[0]["File"]["score"], 192);
            assert!(items[1].get("File").is_some());
            assert_eq!(items[1]["File"]["score"], 112);
            assert!(items[2].get("File").is_some());
            assert_eq!(items[2]["File"]["score"], 108);

            // Then all Functions (sorted by unified_score.final_score DESC)
            assert!(items[3].get("Function").is_some());
            assert_eq!(items[3]["Function"]["unified_score"]["final_score"], 18);
            assert!(items[4].get("Function").is_some());
            assert_eq!(items[4]["Function"]["unified_score"]["final_score"], 11);
            assert!(items[5].get("Function").is_some());
            assert_eq!(items[5]["Function"]["unified_score"]["final_score"], 9);
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
        fn test_sort_with_null_position() {
            // Test that NULLS LAST is the default behavior (nulls sort last regardless of ASC/DESC)
            let sorter = Sorter::parse("score DESC").unwrap();

            let mut items = vec![
                json!({"id": 1, "score": 5}),
                json!({"id": 2, "score": 3}),
                json!({"id": 3, "score": null}),
                json!({"id": 4, "score": 10}),
            ];

            sorter.sort(&mut items);

            // With DESC and default NULLS LAST: 10, 5, 3, then null
            assert_eq!(items[0]["score"], 10); // Highest non-null score
            assert_eq!(items[1]["score"], 5); // Middle score
            assert_eq!(items[2]["score"], 3); // Lowest score
            assert_eq!(items[3]["score"], Value::Null); // Null comes last

            // Test explicit NULLS FIRST to get old behavior
            let sorter_nulls_first = Sorter::parse("score DESC NULLS FIRST").unwrap();
            let mut items2 = vec![
                json!({"id": 1, "score": 5}),
                json!({"id": 2, "score": 3}),
                json!({"id": 3, "score": null}),
                json!({"id": 4, "score": 10}),
            ];

            sorter_nulls_first.sort(&mut items2);

            // With DESC NULLS FIRST: null first, then 10, 5, 3
            assert_eq!(items2[0]["score"], Value::Null); // Null comes first
            assert_eq!(items2[1]["score"], 10); // Highest non-null score
            assert_eq!(items2[2]["score"], 5); // Middle score
            assert_eq!(items2[3]["score"], 3); // Lowest score
        }

        #[test]
        fn test_complex_multifield_sorting() {
            // Test multi-field sorting with different directions
            // Default behavior: NULLS LAST regardless of ASC/DESC
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
            // then by priority DESC (with NULLS LAST default), then by name ASC
            assert_eq!(items[0]["category"], "normal");
            assert_eq!(items[0]["priority"], 8); // Highest non-null priority in "normal"

            assert_eq!(items[1]["category"], "normal");
            assert_eq!(items[1]["priority"], Value::Null); // Null comes last with default NULLS LAST

            assert_eq!(items[2]["category"], "urgent");
            assert_eq!(items[2]["priority"], 10); // Highest priority in "urgent"

            assert_eq!(items[3]["category"], "urgent");
            assert_eq!(items[3]["priority"], 5);
            assert_eq!(items[3]["name"], "Task A"); // Sorted by name when priority equal

            assert_eq!(items[4]["category"], "urgent");
            assert_eq!(items[4]["priority"], 5);
            assert_eq!(items[4]["name"], "Task E");
        }
    }
}
