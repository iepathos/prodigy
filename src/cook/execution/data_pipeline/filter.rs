//! Filter expression parsing and evaluation for data pipeline
//!
//! Provides a filter expression language for selecting items from data.
//! Supports comparison operators, logical operators (AND/OR/NOT), IN expressions,
//! and functions like is_null, is_empty, matches_regex, etc.

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde_json::Value;
use tracing::warn;

/// Path component for field access with array support
#[derive(Debug, Clone, PartialEq)]
pub enum PathPart {
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
    pub(crate) fn try_strip_outer_parens(expr: &str) -> Option<Result<Self>> {
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
    pub(crate) fn has_outer_parens(expr: &str) -> bool {
        expr.starts_with('(') && expr.ends_with(')')
    }

    /// Check if outer parentheses wrap the entire expression
    pub(crate) fn outer_parens_wrap_entire_expr(expr: &str) -> bool {
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
    pub(crate) fn try_parse_not_operator(expr: &str) -> Option<Result<Self>> {
        expr.strip_prefix("!")
            .map(|stripped| Self::parse_not_expression(stripped.trim()))
    }

    /// Parse the inner expression of a NOT operator
    pub(crate) fn parse_not_expression(inner_expr: &str) -> Result<Self> {
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
    pub(crate) fn try_parse_or_operator(expr: &str) -> Option<Result<Self>> {
        Self::find_logical_operator(expr, "||")
            .map(|pos| Self::parse_binary_logical(expr, pos, 2, LogicalOp::Or))
            .or_else(|| {
                Self::find_word_logical_operator(expr, "OR")
                    .map(|pos| Self::parse_binary_logical(expr, pos, 2, LogicalOp::Or))
            })
    }

    /// Try to parse an AND logical operator (supports both && and AND)
    pub(crate) fn try_parse_and_operator(expr: &str) -> Option<Result<Self>> {
        Self::find_logical_operator(expr, "&&")
            .map(|pos| Self::parse_binary_logical(expr, pos, 2, LogicalOp::And))
            .or_else(|| {
                Self::find_word_logical_operator(expr, "AND")
                    .map(|pos| Self::parse_binary_logical(expr, pos, 3, LogicalOp::And))
            })
    }

    /// Find the position of a logical operator outside of parentheses
    pub(crate) fn find_logical_operator(expr: &str, op: &str) -> Option<usize> {
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
    pub(crate) fn find_word_logical_operator(expr: &str, op: &str) -> Option<usize> {
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
    pub(crate) fn matches_operator_at(chars: &[char], pos: usize, op_chars: &[char]) -> bool {
        if pos + op_chars.len() > chars.len() {
            return false;
        }

        chars[pos..pos + op_chars.len()]
            .iter()
            .zip(op_chars.iter())
            .all(|(a, b)| a == b)
    }

    /// Parse a binary logical expression (AND/OR)
    pub(crate) fn parse_binary_logical(
        expr: &str,
        pos: usize,
        op_len: usize,
        op: LogicalOp,
    ) -> Result<Self> {
        let left = Self::parse(&expr[..pos])?;
        let right = Self::parse(&expr[pos + op_len..])?;

        Ok(FilterExpression::Logical {
            op,
            operands: vec![left, right],
        })
    }

    /// Try to parse an 'in' operator expression
    pub(crate) fn try_parse_in_operator(expr: &str) -> Option<Result<Self>> {
        expr.find(" in ")
            .map(|pos| Self::parse_in_expression(expr, pos))
    }

    /// Parse an 'in' expression for checking if a value is in a list
    pub(crate) fn parse_in_expression(expr: &str, in_pos: usize) -> Result<Self> {
        let field = expr[..in_pos].trim().to_string();
        let values_str = expr[in_pos + 4..].trim();

        let values = Self::parse_array_values(values_str)?;

        Ok(FilterExpression::In { field, values })
    }

    /// Parse an array of values from a string like "['value1', 'value2']"
    pub(crate) fn parse_array_values(values_str: &str) -> Result<Vec<Value>> {
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
    pub(crate) fn parse_quoted_string(s: &str) -> Value {
        let unquoted = s.trim_matches('\'').trim_matches('"');
        Value::String(unquoted.to_string())
    }

    /// Try to parse a function call expression
    pub(crate) fn try_parse_function(expr: &str) -> Option<Result<Self>> {
        if !expr.contains('(') || !expr.contains(')') {
            return None;
        }

        Some(Self::parse_function_expression(expr))
    }

    /// Parse a function call expression
    pub(crate) fn parse_function_expression(expr: &str) -> Result<Self> {
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
    pub(crate) fn parse_function_args(args_str: &str) -> Vec<String> {
        if args_str.is_empty() {
            Vec::new()
        } else {
            args_str.split(',').map(|s| s.trim().to_string()).collect()
        }
    }

    /// Try to parse a comparison expression
    pub(crate) fn try_parse_comparison(expr: &str) -> Option<Result<Self>> {
        Self::find_comparison_operator(expr)
            .map(|(op_str, pos)| Self::parse_comparison_expression(expr, op_str, pos))
    }

    /// Find a comparison operator in the expression
    pub(crate) fn find_comparison_operator(expr: &str) -> Option<(&'static str, usize)> {
        let operators = ["==", "!=", ">=", "<=", ">", "<", "="];

        operators
            .iter()
            .find_map(|&op| expr.find(op).map(|pos| (op, pos)))
    }

    /// Parse a comparison expression
    pub(crate) fn parse_comparison_expression(
        expr: &str,
        op_str: &str,
        op_pos: usize,
    ) -> Result<Self> {
        let field = expr[..op_pos].trim().to_string();
        let value_str = expr[op_pos + op_str.len()..].trim();
        let value = Self::parse_value(value_str)?;
        let op = Self::string_to_comparison_op(op_str)?;

        Ok(FilterExpression::Comparison { field, op, value })
    }

    /// Convert a string operator to a ComparisonOp
    pub(crate) fn string_to_comparison_op(op_str: &str) -> Result<ComparisonOp> {
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
    pub(crate) fn parse_value(value_str: &str) -> Result<Value> {
        let trimmed = value_str.trim();

        let value = Self::try_parse_quoted_string(trimmed)
            .or_else(|| Self::try_parse_boolean(trimmed))
            .or_else(|| Self::try_parse_null(trimmed))
            .or_else(|| Self::try_parse_number(trimmed))
            .unwrap_or_else(|| Value::String(trimmed.to_string()));

        Ok(value)
    }

    /// Pure function: Try to parse a quoted string
    pub(crate) fn try_parse_quoted_string(s: &str) -> Option<Value> {
        Self::is_quoted(s).then(|| Value::String(Self::unquote(s)))
    }

    /// Pure function: Check if string is quoted
    pub(crate) fn is_quoted(s: &str) -> bool {
        (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\''))
    }

    /// Pure function: Remove quotes from string
    pub(crate) fn unquote(s: &str) -> String {
        s[1..s.len() - 1].to_string()
    }

    /// Pure function: Try to parse a boolean value
    pub(crate) fn try_parse_boolean(s: &str) -> Option<Value> {
        match s {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        }
    }

    /// Pure function: Try to parse a null value
    pub(crate) fn try_parse_null(s: &str) -> Option<Value> {
        (s == "null").then_some(Value::Null)
    }

    /// Pure function: Try to parse a numeric value
    pub(crate) fn try_parse_number(s: &str) -> Option<Value> {
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
    pub(crate) fn get_nested_field(item: &Value, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = item.clone();

        for part in parts {
            current = current.get(part)?.clone();
        }

        Some(current)
    }

    /// Get a nested field value with array index support
    pub(crate) fn get_nested_field_with_array(item: &Value, path: &str) -> Option<Value> {
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
    pub(crate) fn parse_path_with_array(path: &str) -> Vec<PathPart> {
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
    pub(crate) fn parse_next_path_part(
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Option<PathPart> {
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
    pub(crate) fn parse_field_name(
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Option<PathPart> {
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
    pub(crate) fn parse_array_index(
        chars: &mut std::iter::Peekable<std::str::Chars>,
    ) -> Option<PathPart> {
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
    pub(crate) fn compare(actual: Option<&Value>, op: &ComparisonOp, expected: &Value) -> bool {
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
    pub(crate) fn compare_equal(actual: Option<&Value>, expected: &Value) -> bool {
        match (actual, expected) {
            (None, Value::Null) => true,              // Missing field equals null
            (Some(Value::Null), Value::Null) => true, // Explicit null equals null
            _ => actual == Some(expected),
        }
    }

    /// Pure function: Compare for inequality with null handling
    pub(crate) fn compare_not_equal(actual: Option<&Value>, expected: &Value) -> bool {
        !Self::compare_equal(actual, expected)
    }

    /// Pure function: Compare for greater than
    pub(crate) fn compare_greater(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a > e, |a, e| a > e)
    }

    /// Pure function: Compare for less than
    pub(crate) fn compare_less(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a < e, |a, e| a < e)
    }

    /// Pure function: Compare for greater than or equal
    pub(crate) fn compare_greater_equal(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a >= e, |a, e| a >= e)
    }

    /// Pure function: Compare for less than or equal
    pub(crate) fn compare_less_equal(actual: Option<&Value>, expected: &Value) -> bool {
        Self::compare_numeric_or_string(actual, expected, |a, e| a <= e, |a, e| a <= e)
    }

    /// Pure function: Compare using numeric or string comparison
    pub(crate) fn compare_numeric_or_string<FNum, FStr>(
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
    pub(crate) fn compare_string_op<F>(actual: Option<&Value>, expected: &Value, op: F) -> bool
    where
        F: Fn(&str, &str) -> bool,
    {
        match (actual, expected) {
            (Some(Value::String(a)), Value::String(e)) => op(a.as_str(), e.as_str()),
            _ => false,
        }
    }

    /// Pure function: Compare string against regex pattern
    pub(crate) fn compare_regex(actual: Option<&Value>, expected: &Value) -> bool {
        match (actual, expected) {
            (Some(Value::String(a)), Value::String(pattern)) => Regex::new(pattern)
                .map(|re| re.is_match(a))
                .unwrap_or_else(|e| {
                    warn!("Invalid regex pattern '{}': {}", pattern, e);
                    false
                }),
            _ => false,
        }
    }

    /// Evaluate a function expression
    pub(crate) fn evaluate_function(item: &Value, name: &str, args: &[String]) -> bool {
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
    pub(crate) fn eval_string_binary_fn<F>(item: &Value, args: &[String], op: F) -> bool
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
    pub(crate) fn eval_is_null(item: &Value, args: &[String]) -> bool {
        args.len() == 1 && Self::get_nested_field_with_array(item, &args[0]) == Some(Value::Null)
    }

    /// Pure function: Evaluate is_not_null function
    pub(crate) fn eval_is_not_null(item: &Value, args: &[String]) -> bool {
        args.len() == 1
            && Self::get_nested_field_with_array(item, &args[0]).is_some_and(|v| v != Value::Null)
    }

    /// Pure function: Evaluate type checking function
    pub(crate) fn eval_type_check<F>(item: &Value, args: &[String], predicate: F) -> bool
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
    pub(crate) fn eval_length(item: &Value, args: &[String]) -> bool {
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
    pub(crate) fn get_value_length(value: &Value) -> Option<f64> {
        match value {
            Value::String(s) => Some(s.len() as f64),
            Value::Array(arr) => Some(arr.len() as f64),
            Value::Object(obj) => Some(obj.len() as f64),
            _ => None,
        }
    }

    /// Pure function: Evaluate regex matches function
    pub(crate) fn eval_matches(item: &Value, args: &[String]) -> bool {
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
    pub(crate) fn regex_matches(text: &str, pattern: &str) -> bool {
        Regex::new(pattern)
            .map(|re| re.is_match(text))
            .unwrap_or_else(|e| {
                warn!("Invalid regex pattern '{}': {}", pattern, e);
                false
            })
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
